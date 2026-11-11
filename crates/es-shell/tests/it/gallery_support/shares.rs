use super::{Connect, Rig, library, rig_connected};
use es_app::history::History;
use es_app::platform_ui::ClipboardSink as ItemClipboard;
use es_app::share::{Caps, KeyPlace, ShareFailure, Shares, Target, Targets};
use es_model::platform::ClipItem;
use es_model::{
    BucketKeys, HistoryEntry, ShareCfg, ShareDelete, ShareKind, ShareMode, ShareRecord,
    ShareSource, ShareState, ShareTarget,
};
use es_share::{Event, ShareError, ShareRequest};
use es_shell::gallery::GalleryEvent;
use es_shell::settings::ClipboardSink;
use es_store::Paths;
use es_ui::tokens::ThemeMode;
use gpui::{App, AppContext, Entity, Task};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use time::{Duration, OffsetDateTime};

/// What the share service was asked to delete; uploads finish at once as a seven-day free link.
#[derive(Default)]
pub struct Service {
    pub deleted: Mutex<Vec<String>>,
}

struct FakeTarget(Arc<Service>);

impl Target for FakeTarget {
    fn share(
        &self,
        req: &ShareRequest,
        _: &mut dyn FnMut(Event),
        _: &AtomicBool,
    ) -> Result<ShareRecord, ShareError> {
        let now = OffsetDateTime::now_utc();
        let mut made = record("fresh", req.source.clone(), Some(now + Duration::days(7)));
        made.created_at = now;
        Ok(made)
    }

    fn delete(&self, record: &ShareRecord) -> Result<(), ShareError> {
        self.0.deleted.lock().unwrap().push(record.id.clone());
        Ok(())
    }
}

struct FakeTargets(Arc<Service>);

impl Targets for FakeTargets {
    fn open(&self, _: ShareMode, _: &ShareCfg) -> Result<Box<dyn Target>, ShareFailure> {
        Ok(Box::new(FakeTarget(self.0.clone())))
    }
    fn caps(&self, _: ShareMode, _: &ShareCfg) -> Result<Caps, ShareFailure> {
        Err(ShareFailure::Unexpected)
    }
    fn test_bucket(&self, _: &ShareCfg) -> Result<(), ShareFailure> {
        Ok(())
    }
    fn saved_keys(&self) -> Option<KeyPlace> {
        None
    }
    fn save_keys(&self, _: &BucketKeys) -> Result<KeyPlace, ShareFailure> {
        Err(ShareFailure::Keyring)
    }
    fn remove_keys(&self) -> Result<(), ShareFailure> {
        Ok(())
    }
}

#[derive(Default)]
pub struct Copied(pub RefCell<Vec<String>>);

impl ClipboardSink for Copied {
    fn copy(&self, text: String, _: &mut App) {
        self.0.borrow_mut().push(text);
    }
}

struct NoClipboard;

impl ItemClipboard for NoClipboard {
    fn copy(&self, _: Vec<ClipItem>, _: &mut App) -> Task<es_app::errors::Result<()>> {
        Task::ready(Ok(()))
    }
}

pub fn record(id: &str, source: ShareSource, expires_at: Option<OffsetDateTime>) -> ShareRecord {
    ShareRecord {
        id: id.into(),
        target: ShareTarget::Free,
        kind: ShareKind::Image,
        page_url: format!("https://share.example/s/{id}"),
        media_url: format!("https://share.example/m/{id}.png"),
        poster_url: None,
        title: None,
        days: Some(3),
        created_at: OffsetDateTime::now_utc(),
        expires_at,
        source,
        delete: ShareDelete::FreeToken("token".into()),
        state: ShareState::Ready,
    }
}

pub struct SharedRig {
    pub rig: Rig,
    pub shares: Entity<Shares>,
    pub copied: Rc<Copied>,
    pub service: Arc<Service>,
    pub events: Rc<RefCell<Vec<GalleryEvent>>>,
}

/// A library of `screenshots` with the links `records` makes for its history, written before the window opens.
pub fn shared_rig(
    screenshots: usize,
    mode: ThemeMode,
    records: impl FnOnce(&Paths, &[HistoryEntry]) -> Vec<ShareRecord>,
) -> SharedRig {
    let dir = tempfile::tempdir().unwrap();
    let paths = library(dir.path(), screenshots);
    let entries = History::open(&paths, 0).unwrap().entries().to_vec();
    let mut file = es_store::shares::open(&paths).unwrap();
    file.value.entries = records(&paths, &entries);
    file.save().unwrap();
    let service = Arc::new(Service::default());
    let copied = Rc::new(Copied::default());
    let made: Rc<RefCell<Option<Entity<Shares>>>> = Rc::default();
    let (sink, share_paths, targets, clip) =
        (made.clone(), paths.clone(), service.clone(), copied.clone());
    let connect: Connect = Box::new(move |history, settings, cx| {
        let shares = cx.new(|cx| {
            Shares::new(
                &share_paths,
                settings,
                history,
                Rc::new(NoClipboard),
                Arc::new(FakeTargets(targets)),
                cx,
            )
        });
        *sink.borrow_mut() = Some(shares.clone());
        (Some(shares), clip)
    });
    let mut rig = rig_connected(dir, paths, mode, 1080., 740., Some(connect));
    let shares = made.borrow_mut().take().unwrap();
    let events: Rc<RefCell<Vec<GalleryEvent>>> = Rc::default();
    let (gallery, seen) = (rig.gallery.clone(), events.clone());
    rig.app.cx.update(|cx| {
        cx.subscribe(&gallery, move |_, e: &GalleryEvent, _| {
            seen.borrow_mut().push(e.clone())
        })
        .detach()
    });
    SharedRig {
        rig,
        shares,
        copied,
        service,
        events,
    }
}
