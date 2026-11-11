mod deck;
mod failure;
pub mod flow;
mod job;
mod prepare;
mod target;
mod window_host;

pub use deck::{DeckMarks, DeckOrigin};
pub use failure::ShareFailure;
pub use flow::{Anchor, PopoverHost, ShareFlow, ShareHooks, ShareOrigin, copy_link};
pub use job::run_route;
pub use prepare::{Caps, Job as PrepareJob, PrepareError, Prepared, Tools, prepare, prepare_with};
pub use target::{FREE_BASE_URL, KeyPlace, Keyring, LiveTargets, SystemKeyring, Target, Targets};
pub use window_host::{FileOrigin, WindowPopover, popover_from};

use crate::history::History;
use crate::platform_ui::ClipboardSink;
use crate::settings_model::SettingsModel;
use es_model::config::SHARE_DAYS;
use es_model::platform::ClipItem;
use es_model::{
    BucketKeys, FormatKind, ShareCfg, ShareMode, ShareRecord, ShareSource, ShareTarget,
};
use es_record::clip::export::ExportJob;
use es_record::clip::options::ExportOptions;
use es_share::{Code, Event, ShareError};
use es_store::shares::SharesFile;
use es_store::{JsonFile, Paths};
use gpui::{App, Context, Entity, EventEmitter, Task};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

pub const TERMS_URL: &str = "https://easyshot.site/terms";

pub const SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);

pub type ShareStart = es_share::ShareRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShareJobId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareJobKind {
    /// The file is being made ready to send, in thousandths.
    Preparing {
        permille: u16,
    },
    Reserved {
        page_url: String,
    },
    Progress {
        sent: u64,
        total: u64,
    },
    Done(Box<ShareRecord>),
    Failed(ShareFailure),
    Cancelled,
}

/// After `Cancelled` a job emits nothing more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareJobEvent {
    pub id: ShareJobId,
    pub kind: ShareJobKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharesChanged;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareDeleted {
    pub record: ShareRecord,
    pub result: Result<(), ShareFailure>,
}

/// A local file to share: an image, a video or a recording folder. `days` of `None` is the setting;
/// `source` of `None` records the link against `path` itself.
#[derive(Debug, Clone, PartialEq)]
pub struct FileShare {
    pub path: PathBuf,
    pub days: Option<u8>,
    pub title: Option<String>,
    pub source: Option<ShareSource>,
    /// A video trimmed and toned as this job would export it; its output is ignored.
    pub export: Option<ExportJob>,
}

enum Msg {
    Preparing(u16),
    Event(Event),
    Finished(Result<ShareRecord, ShareFailure>),
}

struct Job {
    cancel: Arc<AtomicBool>,
    cancelled: bool,
}

/// Runs share uploads and deletes, and owns the record of every link made.
pub struct Shares {
    paths: Paths,
    file: SharesFile,
    writable: bool,
    settings: Entity<SettingsModel>,
    history: Entity<History>,
    clipboard: Rc<dyn ClipboardSink>,
    targets: Arc<dyn Targets>,
    jobs: HashMap<ShareJobId, Job>,
    next: u64,
    swept: OffsetDateTime,
    _sweep: Task<()>,
}

impl EventEmitter<ShareJobEvent> for Shares {}
impl EventEmitter<SharesChanged> for Shares {}
impl EventEmitter<ShareDeleted> for Shares {}

impl Shares {
    /// An unreadable shares file is never overwritten: links made then are kept in memory only.
    pub fn new(
        paths: &Paths,
        settings: Entity<SettingsModel>,
        history: Entity<History>,
        clipboard: Rc<dyn ClipboardSink>,
        targets: Arc<dyn Targets>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (file, writable) = match es_store::shares::open(paths) {
            Ok(file) => (file, true),
            Err(e) => {
                tracing::warn!(error = %e, "shares file unreadable; not written this session");
                (
                    JsonFile::fresh(&paths.shares_file(), FormatKind::Shares),
                    false,
                )
            }
        };
        let sweep = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SWEEP_EVERY).await;
                if this.update(cx, |s, cx| s.expire(cx)).is_err() {
                    break;
                }
            }
        });
        let mut shares = Self {
            paths: paths.clone(),
            file,
            writable,
            settings,
            history,
            clipboard,
            targets,
            jobs: HashMap::new(),
            next: 1,
            swept: OffsetDateTime::UNIX_EPOCH,
            _sweep: sweep,
        };
        shares.expire(cx);
        shares
    }

    /// Newest first.
    pub fn all(&self) -> &[ShareRecord] {
        &self.file.value.entries
    }

    /// The newest link of `source` that has not expired.
    pub fn for_source(&self, source: &ShareSource) -> Option<&ShareRecord> {
        self.file
            .value
            .for_source(source, OffsetDateTime::now_utc())
    }

    /// The live link of a file, found as [`Shares::start_file`] would record it.
    pub fn for_file(&self, path: &Path, cx: &App) -> Option<&ShareRecord> {
        self.for_source(&self.source_of(path, cx))
    }

    pub fn config(&self, cx: &App) -> ShareCfg {
        self.settings.read(cx).config().share.clone()
    }

    pub fn days(&self, cx: &App) -> u8 {
        self.settings.read(cx).config().share.days()
    }

    /// A lifetime the service does not offer is refused.
    pub fn set_days(&mut self, days: u8, cx: &mut Context<Self>) -> bool {
        if !SHARE_DAYS.contains(&days) {
            return false;
        }
        self.settings
            .update(cx, |s, cx| s.update(|c| c.share.days = days, cx));
        true
    }

    pub fn saved_keys(&self, cx: &App) -> Task<Result<Option<KeyPlace>, ShareFailure>> {
        self.off_thread(|targets| Ok(targets.saved_keys()), cx)
    }

    pub fn save_keys(&self, keys: BucketKeys, cx: &App) -> Task<Result<KeyPlace, ShareFailure>> {
        self.off_thread(move |targets| targets.save_keys(&keys), cx)
    }

    /// Clears the keyring and the key file both.
    pub fn remove_keys(&self, cx: &App) -> Task<Result<(), ShareFailure>> {
        self.off_thread(|targets| targets.remove_keys(), cx)
    }

    /// Tests the saved bucket settings and keys.
    pub fn test_bucket(&self, cx: &App) -> Task<Result<(), ShareFailure>> {
        let cfg = self.settings.read(cx).config().share.clone();
        self.off_thread(move |targets| targets.test_bucket(&cfg), cx)
    }

    fn off_thread<T: Send + 'static>(
        &self,
        work: impl FnOnce(&dyn Targets) -> Result<T, ShareFailure> + Send + 'static,
        cx: &App,
    ) -> Task<Result<T, ShareFailure>> {
        let targets = self.targets.clone();
        let (tx, rx) = async_channel::bounded(1);
        let spawned = std::thread::Builder::new()
            .name("share-setup".into())
            .spawn(move || {
                let _ = tx.send_blocking(work(&*targets));
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "share setup thread");
            return Task::ready(Err(ShareFailure::Unexpected));
        }
        cx.background_executor()
            .spawn(async move { rx.recv().await.unwrap_or(Err(ShareFailure::Unexpected)) })
    }

    pub fn start(&mut self, start: ShareStart, cx: &mut Context<Self>) -> ShareJobId {
        let id = self.next_id();
        let cfg = self.settings.read(cx).config().share.clone();
        self.launch(id, cfg, cx, move |targets, cfg, cancel, send| {
            let target = targets.open(cfg.mode(), cfg)?;
            target
                .share(&start, &mut |e| send(Msg::Event(e)), cancel)
                .map_err(|e| ShareFailure::from(&e))
        })
    }

    /// Makes the file fit the target, then shares it as `start` does; cancel covers both steps.
    pub fn start_file(&mut self, file: FileShare, cx: &mut Context<Self>) -> ShareJobId {
        let id = self.next_id();
        let config = self.settings.read(cx).config();
        let cfg = config.share.clone();
        let video = ExportOptions::from_config(&config.recording);
        let days = file.days.unwrap_or_else(|| cfg.days());
        if !SHARE_DAYS.contains(&days) {
            let refused = ShareFailure::Refused {
                code: Code::BadRequest,
                retry_after_s: None,
            };
            cx.emit(ShareJobEvent {
                id,
                kind: ShareJobKind::Failed(refused),
            });
            return id;
        }
        let source = file
            .source
            .clone()
            .unwrap_or_else(|| self.source_of(&file.path, cx));
        let paths = self.paths.clone();
        self.launch(id, cfg, cx, move |targets, cfg, cancel, send| {
            let target = targets.open(cfg.mode(), cfg)?;
            let caps = targets.caps(cfg.mode(), cfg)?;
            let job = PrepareJob {
                paths: &paths,
                caps,
                video,
                export: file.export.as_ref(),
                cancel,
            };
            let prepared = prepare(&file.path, &job, &mut |f| {
                send(Msg::Preparing((f.clamp(0., 1.) * 1000.) as u16))
            })
            .map_err(ShareFailure::from)?;
            let request = ShareStart {
                media: prepared.media.clone(),
                title: file.title,
                days,
                source,
            };
            target
                .share(&request, &mut |e| send(Msg::Event(e)), cancel)
                .map_err(|e| ShareFailure::from(&e))
        })
    }

    fn next_id(&mut self) -> ShareJobId {
        let id = ShareJobId(self.next);
        self.next += 1;
        id
    }

    /// An image in the history is shared as that capture; any other file by its path.
    pub fn source_of(&self, path: &Path, cx: &App) -> ShareSource {
        if path.is_dir() || es_store::media::is_video(path) {
            return ShareSource::Recording(path.to_owned());
        }
        let known = self
            .history
            .read(cx)
            .entries()
            .iter()
            .find(|e| e.file == path)
            .map(|e| e.id);
        ShareSource::Capture(known.unwrap_or_else(|| {
            Uuid::new_v5(&Uuid::NAMESPACE_URL, path.as_os_str().as_encoded_bytes())
        }))
    }

    fn launch(
        &mut self,
        id: ShareJobId,
        cfg: ShareCfg,
        cx: &mut Context<Self>,
        work: impl FnOnce(
            &dyn Targets,
            &ShareCfg,
            &AtomicBool,
            &dyn Fn(Msg),
        ) -> Result<ShareRecord, ShareFailure>
        + Send
        + 'static,
    ) -> ShareJobId {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = async_channel::unbounded();
        let targets = self.targets.clone();
        let flag = cancel.clone();
        let spawned = std::thread::Builder::new()
            .name("share".into())
            .spawn(move || {
                let send = |msg| {
                    let _ = tx.send_blocking(msg);
                };
                let result = work(&*targets, &cfg, &flag, &send);
                send(Msg::Finished(result));
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "share thread");
            cx.emit(ShareJobEvent {
                id,
                kind: ShareJobKind::Failed(ShareFailure::Unexpected),
            });
            return id;
        }
        self.jobs.insert(
            id,
            Job {
                cancel,
                cancelled: false,
            },
        );
        cx.spawn(async move |this, cx| {
            while let Ok(msg) = rx.recv().await {
                if this.update(cx, |s, cx| s.on_msg(id, msg, cx)).is_err() {
                    return;
                }
            }
            // The thread died without an answer; a job left running would never end in the UI.
            let lost = Msg::Finished(Err(ShareFailure::Unexpected));
            this.update(cx, |s, cx| s.on_msg(id, lost, cx)).ok();
        })
        .detach();
        id
    }

    /// Answers at once; the upload notices the flag and cleans up on its own thread, which a
    /// stalled connection can hold for minutes.
    pub fn cancel(&mut self, id: ShareJobId, cx: &mut Context<Self>) -> bool {
        let Some(job) = self.jobs.get_mut(&id).filter(|job| !job.cancelled) else {
            return false;
        };
        job.cancelled = true;
        job.cancel.store(true, Ordering::Relaxed);
        cx.emit(ShareJobEvent {
            id,
            kind: ShareJobKind::Cancelled,
        });
        true
    }

    /// A share already gone from where it was counts as deleted.
    pub fn delete(&mut self, record: ShareRecord, cx: &mut Context<Self>) {
        let cfg = self.settings.read(cx).config().share.clone();
        let targets = self.targets.clone();
        let (tx, rx) = async_channel::bounded(1);
        let doomed = record.clone();
        let spawned = std::thread::Builder::new()
            .name("share-delete".into())
            .spawn(move || {
                let _ = tx.send_blocking(delete_remote(&*targets, &cfg, &doomed));
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "share delete thread");
            self.deleted(record, Err(ShareFailure::Unexpected), cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = rx.recv().await.unwrap_or(Err(ShareFailure::Unexpected));
            this.update(cx, |s, cx| s.deleted(record, result, cx)).ok();
        })
        .detach();
    }

    fn on_msg(&mut self, id: ShareJobId, msg: Msg, cx: &mut Context<Self>) {
        let Some(cancelled) = self.jobs.get(&id).map(|job| job.cancelled) else {
            return;
        };
        let kind = match msg {
            Msg::Event(_) | Msg::Preparing(_) if cancelled => return,
            Msg::Preparing(permille) => ShareJobKind::Preparing { permille },
            Msg::Event(Event::Reserved { page_url, .. }) => {
                self.copy_link(&page_url, cx);
                ShareJobKind::Reserved { page_url }
            }
            Msg::Event(Event::Progress { sent, total }) => ShareJobKind::Progress { sent, total },
            Msg::Finished(result) => {
                self.jobs.remove(&id);
                match result {
                    Ok(record) if cancelled => return self.discard(record, cx),
                    Err(_) if cancelled => return,
                    Ok(record) => {
                        self.record(record.clone(), cx);
                        ShareJobKind::Done(Box::new(record))
                    }
                    Err(ShareFailure::Cancelled) => ShareJobKind::Cancelled,
                    Err(failure) => ShareJobKind::Failed(failure),
                }
            }
        };
        cx.emit(ShareJobEvent { id, kind });
    }

    fn copy_link(&self, url: &str, cx: &mut Context<Self>) {
        let copy = self
            .clipboard
            .copy(vec![ClipItem::Text(url.to_owned())], cx);
        cx.spawn(async move |_, _| {
            if let Err(e) = copy.await {
                tracing::warn!(error = %e, "copying the share link");
            }
        })
        .detach();
    }

    /// The upload finished after the user cancelled it; the link must not stay live.
    fn discard(&mut self, record: ShareRecord, cx: &mut Context<Self>) {
        let cfg = self.settings.read(cx).config().share.clone();
        let targets = self.targets.clone();
        let spawned = std::thread::Builder::new()
            .name("share-delete".into())
            .spawn(move || {
                if let Err(e) = delete_remote(&*targets, &cfg, &record) {
                    tracing::warn!("share {}: delete after cancel: {e:?}", record.id);
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "share delete thread");
        }
    }

    fn record(&mut self, record: ShareRecord, cx: &mut Context<Self>) {
        let source = record.source.clone();
        self.file.value.upsert(record);
        self.changed(&[source], cx);
    }

    fn deleted(
        &mut self,
        record: ShareRecord,
        result: Result<(), ShareFailure>,
        cx: &mut Context<Self>,
    ) {
        if result.is_ok() && self.file.value.remove(&record.id).is_some() {
            self.changed(std::slice::from_ref(&record.source), cx);
        }
        cx.emit(ShareDeleted { record, result });
    }

    /// Drops a link from the list without asking its target, as for one the service already deleted.
    pub fn forget(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(record) = self.file.value.remove(id) {
            self.changed(&[record.source], cx);
        }
    }

    /// An expired link is kept, so its uploader can see it expired; its source loses the link.
    fn expire(&mut self, cx: &mut Context<Self>) {
        let now = OffsetDateTime::now_utc();
        let since = std::mem::replace(&mut self.swept, now);
        let lapsed: Vec<ShareSource> = self
            .file
            .value
            .expired(now)
            .filter(|r| r.expires_at.is_some_and(|at| at > since))
            .map(|r| r.source.clone())
            .collect();
        if !lapsed.is_empty() {
            self.changed(&lapsed, cx);
        }
    }

    fn changed(&mut self, sources: &[ShareSource], cx: &mut Context<Self>) {
        if self.writable
            && let Err(e) = self.file.save()
        {
            tracing::warn!(error = %e, "saving the shares file");
        }
        let now = OffsetDateTime::now_utc();
        for source in sources {
            let ShareSource::Capture(id) = source else {
                continue;
            };
            let url = self
                .file
                .value
                .for_source(source, now)
                .map(|r| r.page_url.clone());
            let id = *id;
            self.history.update(cx, |h, cx| {
                if let Err(e) = h.set_share_url(id, url, cx) {
                    tracing::warn!(error = %e, "history share link");
                }
            });
        }
        cx.emit(SharesChanged);
        cx.notify();
    }
}

fn delete_remote(
    targets: &dyn Targets,
    cfg: &ShareCfg,
    record: &ShareRecord,
) -> Result<(), ShareFailure> {
    let mode = match record.target {
        ShareTarget::Free => ShareMode::Free,
        ShareTarget::Bucket { .. } => ShareMode::Bucket,
    };
    match targets.open(mode, cfg)?.delete(record) {
        Ok(())
        | Err(ShareError::Service {
            code: Code::NotFound,
            ..
        }) => Ok(()),
        Err(e) => Err(ShareFailure::from(&e)),
    }
}
