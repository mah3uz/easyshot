use crate::error::{Code, ShareError};
use crate::http::Http;
use crate::pow;
use crate::sending::Sending;
use es_model::share::{ShareDelete, ShareKind, ShareRecord, ShareSource, ShareState, ShareTarget};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::json;
use std::fs::{self, File};
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const RETRIES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    pub kind: ShareKind,
    pub mime: String,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration_ms: Option<u64>,
    /// A JPEG still; sent for a video only.
    pub poster: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest {
    pub media: Media,
    pub title: Option<String>,
    pub days: u8,
    pub source: ShareSource,
}

pub trait InstallTokens {
    fn load(&self) -> Option<String>;
    fn save(&self, token: &str);
    fn clear(&self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The links are known; a bucket page answers only once its upload is done.
    Reserved {
        page_url: String,
        media_url: String,
        expires_at: Option<OffsetDateTime>,
    },
    Progress {
        sent: u64,
        total: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Limits {
    pub image_max_bytes: u64,
    pub gif_max_bytes: u64,
    pub video_max_bytes: u64,
    pub poster_max_bytes: u64,
    pub days: Vec<u8>,
    pub default_days: u8,
    pub part_bytes: u64,
    pub daily_shares: u32,
    pub daily_byte_days: u64,
    #[serde(default)]
    pub shares_left: Option<u32>,
    #[serde(default)]
    pub byte_days_left: Option<u64>,
    pub accepting: bool,
}

/// The hosted share service. Every call blocks; run it on a job thread.
pub struct Free {
    http: Http,
    tokens: Box<dyn InstallTokens + Send + Sync>,
    backoff: Duration,
}

#[derive(Deserialize)]
struct Created {
    id: String,
    page_url: String,
    media_url: String,
    poster_url: Option<String>,
    expires_at: String,
    upload_token: String,
    part_bytes: u64,
    parts: u64,
    delete_token: String,
}

#[derive(Deserialize)]
struct Completed {
    page_url: String,
    media_url: String,
    poster_url: Option<String>,
    expires_at: String,
}

struct Piece<'a> {
    path: String,
    token: &'a str,
    content_type: Option<&'static str>,
    offset: u64,
    len: u64,
    sent_before: u64,
    total: u64,
}

impl Free {
    pub fn new(
        base_url: &str,
        app_version: &str,
        tokens: impl InstallTokens + Send + Sync + 'static,
    ) -> Self {
        Self {
            http: Http::new(base_url, app_version, true),
            tokens: Box::new(tokens),
            backoff: Duration::from_secs(1),
        }
    }

    #[cfg(test)]
    pub(crate) fn plain_http(
        base_url: &str,
        app_version: &str,
        tokens: impl InstallTokens + Send + Sync + 'static,
    ) -> Self {
        Self {
            http: Http::new(base_url, app_version, false),
            tokens: Box::new(tokens),
            backoff: Duration::from_millis(1),
        }
    }

    pub fn limits(&self) -> Result<Limits, ShareError> {
        let token = self.tokens.load();
        match self.http.get("/v1/limits", token.as_deref())?.expect(200) {
            Err(ShareError::Service {
                code: Code::Unauthorized,
                ..
            }) if token.is_some() => {
                self.tokens.clear();
                self.http.get("/v1/limits", None)?.expect(200)
            }
            other => other,
        }
    }

    /// Cancel or any failure after the share exists deletes it before returning.
    pub fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<ShareRecord, ShareError> {
        let media = &req.media;
        let video = media.kind == ShareKind::Video;
        let size = fs::metadata(&media.path)?.len();
        let poster = match (&media.poster, video) {
            (Some(path), true) => Some((path.as_path(), fs::metadata(path)?.len())),
            _ => None,
        };
        let body = json!({
            "kind": media.kind,
            "mime": media.mime,
            "size": size,
            "days": req.days,
            "title": req.title,
            "width": media.width,
            "height": media.height,
            "duration_ms": if video { media.duration_ms } else { None },
            "poster_size": poster.map(|(_, len)| len),
        });
        stop_if(cancel)?;
        let created = self.create(&body, cancel)?;
        if !valid_id(&created.id) {
            return Err(ShareError::Protocol("share id".into()));
        }
        match self.upload(req, &created, size, poster, on_event, cancel) {
            Ok(done) => Ok(ShareRecord {
                id: created.id,
                target: ShareTarget::Free,
                kind: media.kind,
                page_url: done.page_url,
                media_url: done.media_url,
                poster_url: done.poster_url,
                title: req.title.clone(),
                days: Some(req.days),
                created_at: OffsetDateTime::now_utc(),
                expires_at: Some(parse_time(&done.expires_at)?),
                source: req.source.clone(),
                delete: ShareDelete::FreeToken(created.delete_token),
                state: ShareState::Ready,
            }),
            Err(err) => {
                if let Err(e) = self.delete_share(&created.id, &created.delete_token) {
                    tracing::warn!("share {}: delete after a failed upload: {e}", created.id);
                }
                Err(err)
            }
        }
    }

    /// A share already gone counts as deleted.
    pub fn delete(&self, record: &ShareRecord) -> Result<(), ShareError> {
        let ShareDelete::FreeToken(token) = &record.delete else {
            return Err(ShareError::Protocol("not a Free share".into()));
        };
        if !valid_id(&record.id) {
            return Err(ShareError::Protocol("share id".into()));
        }
        self.delete_share(&record.id, token)
    }

    fn delete_share(&self, id: &str, token: &str) -> Result<(), ShareError> {
        let reply = self.http.delete(&format!("/v1/shares/{id}"), token)?;
        if reply.status == 204 {
            return Ok(());
        }
        match reply.error() {
            ShareError::Service {
                code: Code::NotFound,
                ..
            } => Ok(()),
            err => Err(err),
        }
    }

    fn create(&self, body: &serde_json::Value, cancel: &AtomicBool) -> Result<Created, ShareError> {
        let mut token = match self.tokens.load() {
            Some(token) => token,
            None => self.install(cancel)?,
        };
        let mut reinstalled = false;
        loop {
            let reply = self.http.post("/v1/shares", Some(&token), Some(body))?;
            match reply.expect(201) {
                Err(ShareError::Service {
                    code: Code::Unauthorized,
                    ..
                }) if !reinstalled => {
                    self.tokens.clear();
                    token = self.install(cancel)?;
                    reinstalled = true;
                }
                other => return other,
            }
        }
    }

    fn install(&self, cancel: &AtomicBool) -> Result<String, ShareError> {
        #[derive(Deserialize)]
        struct Challenge {
            challenge: String,
            difficulty: u32,
        }
        #[derive(Deserialize)]
        struct Installed {
            install_token: String,
        }
        let got: Challenge = self.http.get("/v1/challenge", None)?.expect(200)?;
        if got.difficulty > pow::MAX_DIFFICULTY {
            return Err(ShareError::Protocol("challenge difficulty".into()));
        }
        let nonce =
            pow::solve(&got.challenge, got.difficulty, cancel).ok_or(ShareError::Cancelled)?;
        let body = json!({ "challenge": got.challenge, "nonce": nonce.to_string() });
        let installed: Installed = self
            .http
            .post("/v1/install", None, Some(&body))?
            .expect(200)?;
        self.tokens.save(&installed.install_token);
        Ok(installed.install_token)
    }

    fn upload(
        &self,
        req: &ShareRequest,
        created: &Created,
        size: u64,
        poster: Option<(&Path, u64)>,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<Completed, ShareError> {
        https(&created.page_url)?;
        https(&created.media_url)?;
        created.poster_url.as_deref().map(https).transpose()?;
        if created.part_bytes == 0 || created.parts != size.div_ceil(created.part_bytes) {
            return Err(ShareError::Protocol("part count".into()));
        }
        on_event(Event::Reserved {
            page_url: created.page_url.clone(),
            media_url: created.media_url.clone(),
            expires_at: Some(parse_time(&created.expires_at)?),
        });
        let total = size + poster.map_or(0, |(_, len)| len);
        let mut file = File::open(&req.media.path)?;
        for n in 1..=created.parts {
            let offset = (n - 1) * created.part_bytes;
            let piece = Piece {
                path: format!("/v1/shares/{}/parts/{n}", created.id),
                token: &created.upload_token,
                content_type: None,
                offset,
                len: created.part_bytes.min(size - offset),
                sent_before: offset,
                total,
            };
            self.put_piece(&piece, &mut file, on_event, cancel)?;
        }
        if let Some((path, len)) = poster {
            let piece = Piece {
                path: format!("/v1/shares/{}/poster", created.id),
                token: &created.upload_token,
                content_type: Some("image/jpeg"),
                offset: 0,
                len,
                sent_before: size,
                total,
            };
            self.put_piece(&piece, &mut File::open(path)?, on_event, cancel)?;
        }
        stop_if(cancel)?;
        let path = format!("/v1/shares/{}/complete", created.id);
        let done: Completed = self
            .http
            .post(&path, Some(&created.upload_token), None)?
            .expect(200)?;
        https(&done.page_url)?;
        https(&done.media_url)?;
        done.poster_url.as_deref().map(https).transpose()?;
        Ok(done)
    }

    fn put_piece(
        &self,
        piece: &Piece<'_>,
        file: &mut File,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<(), ShareError> {
        let mut tries = 0;
        loop {
            file.seek(SeekFrom::Start(piece.offset))?;
            let total = piece.total;
            let mut on_sent = |sent| on_event(Event::Progress { sent, total });
            let mut body = Sending::new(
                &mut *file,
                piece.len,
                piece.sent_before,
                &mut on_sent,
                cancel,
            );
            let result = self.http.put(
                &piece.path,
                piece.token,
                piece.content_type,
                piece.len,
                &mut body,
            );
            let failed = body.failed.take();
            stop_if(cancel)?;
            if let Some(err) = failed {
                return Err(err.into());
            }
            let retry = match &result {
                Err(ShareError::Network(_)) => true,
                Ok(reply) => reply.status >= 500 || reply.status == 429,
                Err(_) => false,
            };
            if !retry || tries == RETRIES {
                return result?.expect::<IgnoredAny>(200).map(|_| ());
            }
            tries += 1;
            tracing::debug!("{}: attempt {tries} failed, retrying", piece.path);
            self.pause(self.backoff * 2u32.pow(tries - 1), cancel)?;
        }
    }

    fn pause(&self, length: Duration, cancel: &AtomicBool) -> Result<(), ShareError> {
        let until = Instant::now() + length;
        loop {
            stop_if(cancel)?;
            let now = Instant::now();
            if now >= until {
                return Ok(());
            }
            thread::sleep((until - now).min(Duration::from_millis(50)));
        }
    }
}

pub(crate) fn stop_if(cancel: &AtomicBool) -> Result<(), ShareError> {
    match cancel.load(Ordering::Relaxed) {
        true => Err(ShareError::Cancelled),
        false => Ok(()),
    }
}

// The id goes into request paths, so anything outside the protocol's shape is refused.
fn valid_id(id: &str) -> bool {
    id.len() == 22
        && id.starts_with(['1', '3', '7'])
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn https(url: &str) -> Result<(), ShareError> {
    match url.starts_with("https://") {
        true => Ok(()),
        false => Err(ShareError::Protocol("a link that is not https".into())),
    }
}

fn parse_time(text: &str) -> Result<OffsetDateTime, ShareError> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|e| ShareError::Protocol(e.to_string()))
}

#[cfg(test)]
pub(crate) mod fake;
#[cfg(test)]
mod tests;
