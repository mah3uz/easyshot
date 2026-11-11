mod sigv4;

use crate::error::{BucketFault, Code, ShareError};
use crate::free::{Event, ShareRequest, stop_if};
use crate::http::{Http, Reply};
use crate::sending::Sending;
use crate::viewer;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use es_model::share::{BucketKeys, ShareDelete, ShareKind, ShareRecord, ShareState, ShareTarget};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use time::OffsetDateTime;
use ureq::http::Uri;

/// The largest object S3 accepts in one PUT.
const MAX_PUT: u64 = 5 * 1024 * 1024 * 1024;
const HOME_URL: &str = "https://easyshot.site";
const DEFAULT_TITLE: &str = "Shared with easyshot";
const CACHE: &str = "public, max-age=3600";
const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const TYPES: [(&str, &str); 5] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
    ("video/mp4", "mp4"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BucketConfig {
    pub endpoint: String,
    pub bucket: String,
    pub region: String,
    pub public_base_url: String,
    pub prefix: String,
    pub keys: BucketKeys,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("the endpoint is not an https URL of a host")]
    Endpoint,
    #[error("the public base URL is not an https URL")]
    PublicBaseUrl,
    #[error("the region is empty or not a region name")]
    Region,
    #[error("the bucket name is empty or not a bucket name")]
    BucketName,
    #[error("an access key is missing")]
    Keys,
}

/// The user's own S3-compatible bucket, path-style. Every call blocks; run it on a job thread.
pub struct Bucket {
    http: Http,
    endpoint: String,
    host: String,
    bucket: String,
    region: String,
    public_base_url: String,
    prefix: String,
    keys: BucketKeys,
}

struct Plan {
    id: String,
    ext: &'static str,
    media_key: String,
    poster_key: String,
    index_key: String,
    page_url: String,
    media_url: String,
    poster_url: Option<String>,
}

impl Bucket {
    pub fn new(cfg: &BucketConfig, app_version: &str) -> Result<Self, ConfigError> {
        Self::build(cfg, app_version, true)
    }

    #[cfg(test)]
    pub(crate) fn plain_http(cfg: &BucketConfig, app_version: &str) -> Result<Self, ConfigError> {
        Self::build(cfg, app_version, false)
    }

    fn build(cfg: &BucketConfig, app_version: &str, https_only: bool) -> Result<Self, ConfigError> {
        let (endpoint, host) =
            origin(cfg.endpoint.trim(), https_only).ok_or(ConfigError::Endpoint)?;
        let public = cfg.public_base_url.trim().trim_end_matches('/');
        if public_base(public, https_only).is_none() {
            return Err(ConfigError::PublicBaseUrl);
        }
        let region = cfg.region.trim();
        if region.is_empty()
            || !region
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(ConfigError::Region);
        }
        let bucket = cfg.bucket.trim();
        if bucket.is_empty()
            || !bucket
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        {
            return Err(ConfigError::BucketName);
        }
        let keys = BucketKeys {
            access_key_id: cfg.keys.access_key_id.trim().to_owned(),
            secret_access_key: cfg.keys.secret_access_key.trim().to_owned(),
        };
        if keys.access_key_id.is_empty() || keys.secret_access_key.is_empty() {
            return Err(ConfigError::Keys);
        }
        let mut prefix = cfg.prefix.trim().trim_start_matches('/').to_owned();
        if !prefix.is_empty() && !prefix.ends_with('/') {
            prefix.push('/');
        }
        Ok(Self {
            http: Http::new("", app_version, https_only),
            endpoint,
            host,
            bucket: bucket.to_owned(),
            region: region.to_owned(),
            public_base_url: public.to_owned(),
            prefix,
            keys,
        })
    }

    /// Cancel or any failure deletes every object this call began writing before returning.
    pub fn share(
        &self,
        req: &ShareRequest,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<ShareRecord, ShareError> {
        let media = &req.media;
        let video = media.kind == ShareKind::Video;
        let ext = extension(&media.mime, media.kind)?;
        let size = fs::metadata(&media.path)?.len();
        let poster = match (&media.poster, video) {
            (Some(path), true) => Some((path.as_path(), fs::metadata(path)?.len())),
            (None, true) => return Err(ShareError::Protocol("a video without a poster".into())),
            _ => None,
        };
        if size > MAX_PUT || poster.is_some_and(|(_, len)| len > MAX_PUT) {
            return Err(ShareError::Service {
                code: Code::TooLarge,
                retry_after_s: None,
            });
        }
        let plan = self.plan(random_id()?, ext, video);
        let page = self.page(req, &plan)?;
        stop_if(cancel)?;
        on_event(Event::Reserved {
            page_url: plan.page_url.clone(),
            media_url: plan.media_url.clone(),
            expires_at: None,
        });
        self.upload(req, &plan, (size, poster), &page, on_event, cancel)?;
        Ok(ShareRecord {
            id: plan.id,
            target: ShareTarget::Bucket {
                label: self.bucket.clone(),
            },
            kind: media.kind,
            page_url: plan.page_url,
            media_url: plan.media_url,
            poster_url: plan.poster_url,
            title: req.title.clone(),
            days: None,
            created_at: OffsetDateTime::now_utc(),
            expires_at: None,
            source: req.source.clone(),
            delete: ShareDelete::Bucket,
            state: ShareState::Ready,
        })
    }

    /// Gone objects count as deleted; a share made under other bucket settings is refused.
    pub fn delete(&self, record: &ShareRecord) -> Result<(), ShareError> {
        let ours = matches!(&record.target, ShareTarget::Bucket { label } if *label == self.bucket);
        if record.delete != ShareDelete::Bucket || !ours || !valid_id(&record.id) {
            return Err(ShareError::Protocol("not a share in this bucket".into()));
        }
        let video = record.kind == ShareKind::Video;
        let plan = TYPES
            .iter()
            .map(|(_, ext)| self.plan(record.id.clone(), ext, video))
            .find(|plan| plan.media_url == record.media_url && plan.page_url == record.page_url)
            .ok_or_else(|| ShareError::Bucket {
                fault: BucketFault::Bucket,
                detail: "the share was made with other bucket settings".into(),
            })?;
        let mut keys = vec![plan.index_key, plan.media_key];
        if video {
            keys.push(plan.poster_key);
        }
        let mut first = Ok(());
        for key in &keys {
            if let Err(err) = self.delete_key(key) {
                tracing::warn!("{key}: delete: {err}");
                first = first.and(Err(err));
            }
        }
        first
    }

    /// Write, read back anonymously, delete; a failed step is a `ShareError::Bucket` naming it.
    pub fn test_connection(&self) -> Result<(), ShareError> {
        let token = random_id()?;
        let key = format!("{}easyshot-test-{token}", self.prefix);
        let body = format!("easyshot connection test {token}\n");
        let hash = sigv4::sha256_hex(body.as_bytes());
        let headers = [("content-type", "text/plain; charset=utf-8")];
        let len = body.len() as u64;
        let put = self.call(
            "PUT",
            &key,
            &headers,
            &hash,
            Some((len, &mut body.as_bytes() as &mut dyn Read)),
        );
        match put.map_err(as_step) {
            Ok(reply) if reply.status == 200 => {}
            Ok(reply) => return Err(refused(&reply)),
            Err(err) => return Err(err),
        }
        let public = |detail: String| ShareError::Bucket {
            fault: BucketFault::PublicUrl,
            detail,
        };
        let served = match self.http.send("GET", &self.public_url(&key), &[], None) {
            Ok(reply) if reply.status == 200 && reply.body() == body.as_bytes() => Ok(()),
            Ok(reply) if reply.status == 200 => Err(public("served other content".into())),
            Ok(reply) => Err(public(format!("HTTP {}", reply.status))),
            Err(err) => Err(public(err.to_string())),
        };
        let removed = self.delete_key(&key).map_err(as_step);
        served.and(removed)
    }

    fn plan(&self, id: String, ext: &'static str, video: bool) -> Plan {
        let dir = format!("{}s/{id}/", self.prefix);
        let media_key = format!("{dir}media.{ext}");
        let poster_key = format!("{dir}poster.jpg");
        let index_key = format!("{dir}index.html");
        Plan {
            page_url: self.public_url(&index_key),
            media_url: self.public_url(&media_key),
            poster_url: video.then(|| self.public_url(&poster_key)),
            id,
            ext,
            media_key,
            poster_key,
            index_key,
        }
    }

    fn page(&self, req: &ShareRequest, plan: &Plan) -> Result<String, ShareError> {
        let media = &req.media;
        let download_name = format!("easyshot-{}.{}", plan.id, plan.ext);
        let (width, height) = (media.width.to_string(), media.height.to_string());
        let values = HashMap::from([
            ("title", req.title.as_deref().unwrap_or(DEFAULT_TITLE)),
            ("page_url", plan.page_url.as_str()),
            ("media_url", plan.media_url.as_str()),
            (
                "poster_url",
                plan.poster_url.as_deref().unwrap_or(&plan.media_url),
            ),
            ("mime", media.mime.as_str()),
            ("width", width.as_str()),
            ("height", height.as_str()),
            ("download_name", download_name.as_str()),
            ("home_url", HOME_URL),
        ]);
        let kind = match media.kind {
            ShareKind::Image => "image",
            ShareKind::Video => "video",
        };
        viewer::fill(viewer::PAGE, &values, &["ready", kind])
            .map_err(|e| ShareError::Protocol(e.to_string()))
    }

    fn upload(
        &self,
        req: &ShareRequest,
        plan: &Plan,
        (size, poster): (u64, Option<(&Path, u64)>),
        page: &str,
        on_event: &mut dyn FnMut(Event),
        cancel: &AtomicBool,
    ) -> Result<(), ShareError> {
        let total = size + poster.map_or(0, |(_, len)| len);
        let mut progress = |sent| on_event(Event::Progress { sent, total });
        let mut written = Vec::new();
        let mut write = || -> Result<(), ShareError> {
            let disposition = format!("inline; filename=\"easyshot-{}.{}\"", plan.id, plan.ext);
            let headers = [
                ("cache-control", CACHE),
                ("content-disposition", disposition.as_str()),
                ("content-type", req.media.mime.as_str()),
            ];
            // Recorded before the PUT: a reply lost after the store wrote the object leaves it there.
            written.push(&plan.media_key);
            let media = (req.media.path.as_path(), size, 0);
            self.put_file(&plan.media_key, media, &headers, &mut progress, cancel)?;
            if let Some((path, len)) = poster {
                let headers = [("cache-control", CACHE), ("content-type", "image/jpeg")];
                written.push(&plan.poster_key);
                self.put_file(
                    &plan.poster_key,
                    (path, len, size),
                    &headers,
                    &mut progress,
                    cancel,
                )?;
            }
            stop_if(cancel)?;
            let headers = [
                ("cache-control", CACHE),
                ("content-type", "text/html; charset=utf-8"),
            ];
            let hash = sigv4::sha256_hex(page.as_bytes());
            let body = (page.len() as u64, &mut page.as_bytes() as &mut dyn Read);
            written.push(&plan.index_key);
            let reply = self.call("PUT", &plan.index_key, &headers, &hash, Some(body))?;
            stop_if(cancel)?;
            ok(&reply)
        };
        let result = write();
        if result.is_err() {
            for key in written.iter().rev() {
                if let Err(e) = self.delete_key(key) {
                    tracing::warn!("{key}: delete after a failed share: {e}");
                }
            }
        }
        result
    }

    fn put_file(
        &self,
        key: &str,
        (path, len, sent_before): (&Path, u64, u64),
        headers: &[(&str, &str)],
        on_sent: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), ShareError> {
        stop_if(cancel)?;
        let mut body = Sending::new(File::open(path)?, len, sent_before, on_sent, cancel);
        let result = self.call(
            "PUT",
            key,
            headers,
            sigv4::UNSIGNED_PAYLOAD,
            Some((len, &mut body as &mut dyn Read)),
        );
        let failed = body.failed.take();
        stop_if(cancel)?;
        if let Some(err) = failed {
            return Err(err.into());
        }
        ok(&result?)
    }

    fn delete_key(&self, key: &str) -> Result<(), ShareError> {
        let reply = self.call("DELETE", key, &[], EMPTY_HASH, None)?;
        match reply.status {
            200 | 204 | 404 => Ok(()),
            _ => Err(refused(&reply)),
        }
    }

    fn call(
        &self,
        method: &str,
        key: &str,
        headers: &[(&str, &str)],
        payload_hash: &str,
        body: Option<(u64, &mut dyn Read)>,
    ) -> Result<Reply, ShareError> {
        let path = format!("/{}/{}", self.bucket, sigv4::encode_path(key));
        let mut sent = vec![
            ("host", self.host.as_str()),
            ("x-amz-content-sha256", payload_hash),
        ];
        sent.extend_from_slice(headers);
        let creds = sigv4::Credentials {
            access_key_id: &self.keys.access_key_id,
            secret_access_key: &self.keys.secret_access_key,
            region: &self.region,
            service: "s3",
        };
        let req = sigv4::Request {
            method,
            path: &path,
            query: &[],
            headers: &sent,
            payload_hash,
        };
        let signed = sigv4::sign(&creds, &req, OffsetDateTime::now_utc())?;
        sent.push(("x-amz-date", &signed.amz_date));
        sent.push(("authorization", &signed.authorization));
        let url = format!("{}{path}", self.endpoint);
        self.http.send(method, &url, &sent, body)
    }

    fn public_url(&self, key: &str) -> String {
        format!("{}/{}", self.public_base_url, sigv4::encode_path(key))
    }
}

fn origin(url: &str, https_only: bool) -> Option<(String, String)> {
    let uri: Uri = url.parse().ok()?;
    let scheme = uri.scheme_str()?;
    let default_port = match scheme {
        "https" => 443,
        "http" if !https_only => 80,
        _ => return None,
    };
    let authority = uri.authority()?;
    if authority.as_str().contains('@') || !matches!(uri.path(), "" | "/") || uri.query().is_some()
    {
        return None;
    }
    let host = match authority.port_u16() {
        Some(port) if port != default_port => authority.as_str().to_owned(),
        _ => authority.host().to_owned(),
    };
    Some((format!("{scheme}://{}", authority.as_str()), host))
}

fn public_base(url: &str, https_only: bool) -> Option<()> {
    let uri: Uri = url.parse().ok()?;
    let secure =
        uri.scheme_str() == Some("https") || (!https_only && uri.scheme_str() == Some("http"));
    let authority = uri.authority()?;
    (secure && !authority.as_str().contains('@') && uri.query().is_none()).then_some(())
}

fn extension(mime: &str, kind: ShareKind) -> Result<&'static str, ShareError> {
    let video = mime.starts_with("video/");
    TYPES
        .iter()
        .find(|(m, _)| *m == mime)
        .filter(|_| video == (kind == ShareKind::Video))
        .map(|(_, ext)| *ext)
        .ok_or(ShareError::Service {
            code: Code::UnsupportedType,
            retry_after_s: None,
        })
}

fn random_id() -> Result<String, ShareError> {
    let mut bytes = [0u8; 17];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    let mut id = URL_SAFE_NO_PAD.encode(bytes);
    id.truncate(22);
    Ok(id)
}

// The id goes into object keys, so anything outside the shape this module makes is refused.
fn valid_id(id: &str) -> bool {
    id.len() == 22
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn ok(reply: &Reply) -> Result<(), ShareError> {
    match reply.status {
        200 => Ok(()),
        _ => Err(refused(reply)),
    }
}

fn refused(reply: &Reply) -> ShareError {
    let fault = match reply.status {
        401 | 403 => BucketFault::Credentials,
        _ => BucketFault::Bucket,
    };
    let detail = match s3_code(reply.body()) {
        Some(code) => format!("HTTP {} {code}", reply.status),
        None => format!("HTTP {}", reply.status),
    };
    ShareError::Bucket { fault, detail }
}

// Only the error code is kept: a SignatureDoesNotMatch body also carries the string that was signed.
fn s3_code(body: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(body).ok()?;
    let start = text.find("<Code>")? + "<Code>".len();
    let code = &text[start..start + text[start..].find("</Code>")?];
    (!code.is_empty()
        && code.len() <= 64
        && code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.'))
    .then_some(code)
}

fn as_step(err: ShareError) -> ShareError {
    match err {
        ShareError::Network(detail) => ShareError::Bucket {
            fault: BucketFault::Network,
            detail,
        },
        other => other,
    }
}

#[cfg(test)]
mod minio;
#[cfg(test)]
mod tests;
