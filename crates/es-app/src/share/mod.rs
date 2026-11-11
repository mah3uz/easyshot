mod failure;
mod target;

pub use failure::ShareFailure;
pub use target::{FREE_BASE_URL, Keyring, LiveTargets, SystemKeyring, Target, Targets};

use crate::history::History;
use crate::platform_ui::ClipboardSink;
use crate::settings_model::SettingsModel;
use es_model::config::SHARE_DAYS;
use es_model::platform::ClipItem;
use es_model::{FormatKind, ShareCfg, ShareMode, ShareRecord, ShareSource, ShareTarget};
use es_share::{Code, Event, ShareError};
use es_store::shares::SharesFile;
use es_store::{JsonFile, Paths};
use gpui::{App, Context, Entity, EventEmitter, Task};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use time::OffsetDateTime;

pub const SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);

pub type ShareStart = es_share::ShareRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShareJobId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareJobKind {
    Reserved { page_url: String },
    Progress { sent: u64, total: u64 },
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

enum Msg {
    Event(Event),
    Finished(Result<ShareRecord, ShareFailure>),
}

struct Job {
    cancel: Arc<AtomicBool>,
    cancelled: bool,
}

/// Runs share uploads and deletes, and owns the record of every link made.
pub struct Shares {
    file: SharesFile,
    writable: bool,
    settings: Entity<SettingsModel>,
    history: Entity<History>,
    clipboard: Rc<dyn ClipboardSink>,
    targets: Arc<dyn Targets>,
    jobs: HashMap<ShareJobId, Job>,
    next: u64,
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
                if this.update(cx, |s, cx| s.drop_expired(cx)).is_err() {
                    break;
                }
            }
        });
        let mut shares = Self {
            file,
            writable,
            settings,
            history,
            clipboard,
            targets,
            jobs: HashMap::new(),
            next: 1,
            _sweep: sweep,
        };
        shares.drop_expired(cx);
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

    pub fn days(&self, cx: &App) -> u8 {
        self.settings.read(cx).config().share.days()
    }

    pub fn set_target(&mut self, mode: ShareMode, cx: &mut Context<Self>) {
        self.settings
            .update(cx, |s, cx| s.update(|c| c.share.target = mode, cx));
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

    pub fn consent(&mut self, mode: ShareMode, cx: &mut Context<Self>) {
        self.settings.update(cx, |s, cx| {
            s.update(
                |c| {
                    c.share.target = mode;
                    c.share.consented = true;
                },
                cx,
            )
        });
    }

    /// Nothing is sent until the user has chosen where shares go.
    pub fn start(&mut self, start: ShareStart, cx: &mut Context<Self>) -> ShareJobId {
        let id = ShareJobId(self.next);
        self.next += 1;
        let cfg = self.settings.read(cx).config().share.clone();
        if !cfg.consented {
            cx.emit(ShareJobEvent {
                id,
                kind: ShareJobKind::Failed(ShareFailure::NoConsent),
            });
            return id;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = async_channel::unbounded();
        let targets = self.targets.clone();
        let flag = cancel.clone();
        let spawned = std::thread::Builder::new()
            .name("share".into())
            .spawn(move || {
                let result = targets.open(cfg.target, &cfg).and_then(|target| {
                    let mut on_event = |e| {
                        let _ = tx.send_blocking(Msg::Event(e));
                    };
                    target
                        .share(&start, &mut on_event, &flag)
                        .map_err(|e| ShareFailure::from(&e))
                });
                let _ = tx.send_blocking(Msg::Finished(result));
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
            Msg::Event(_) if cancelled => return,
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

    /// The service deletes a free share when it expires; a bucket's own rules expire its shares.
    fn drop_expired(&mut self, cx: &mut Context<Self>) {
        let now = OffsetDateTime::now_utc();
        let gone: Vec<ShareRecord> = self
            .file
            .value
            .expired(now)
            .filter(|r| r.target == ShareTarget::Free)
            .cloned()
            .collect();
        if gone.is_empty() {
            return;
        }
        for record in &gone {
            self.file.value.remove(&record.id);
        }
        let sources: Vec<ShareSource> = gone.into_iter().map(|r| r.source).collect();
        self.changed(&sources, cx);
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
