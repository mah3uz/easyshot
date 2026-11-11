#![allow(clippy::unwrap_used)]
use super::share::{Fake, Rig, consent, hold, kinds, made, rig, wait_until};
use es_app::deck::{Deck, DeckItem, DeckPolicy, ItemStatus};
use es_app::errors::Result;
use es_app::platform_ui::ClipboardSink;
use es_app::share::flow::BuildPopover;
use es_app::share::{Anchor, DeckShare, PopoverHost, ShareHooks, ShareJobKind};
use es_app::toast::{CloseReason, MemorySink, ToastRequest, ToastStyle, install_toasts};
use es_desktop::RecordingNotifier;
use es_model::platform::ClipItem;
use es_model::{CaptureKind, ShareMode};
use es_share::{Code, ShareError};
use es_ui::components::{ShareEvent, SharePopover, ShareTarget, ShareView};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, IntoElement, Render, Task,
    TestAppContext, Window, WindowOptions,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

struct Holder(Entity<SharePopover>);

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

type Shown = Option<(AnyWindowHandle, Rc<dyn Fn(&mut App)>)>;

/// Opens the popover in a test window instead of on a layer surface.
#[derive(Default)]
struct Host {
    shown: RefCell<Shown>,
    opened: Cell<usize>,
    settings: Rc<Cell<usize>>,
}

impl PopoverHost for Host {
    fn open(
        &self,
        _: Option<&Anchor>,
        build: BuildPopover,
        closed: Rc<dyn Fn(&mut App)>,
        cx: &mut App,
    ) -> Result<()> {
        self.close(cx);
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                let popover = build(window, cx);
                cx.new(|_| Holder(popover))
            })
            .unwrap();
        self.opened.set(self.opened.get() + 1);
        *self.shown.borrow_mut() = Some((window.into(), closed));
        Ok(())
    }

    fn close(&self, cx: &mut App) {
        let taken = self.shown.borrow_mut().take();
        if let Some((window, closed)) = taken {
            let _ = window.update(cx, |_, window, _| window.remove_window());
            closed(cx);
        }
    }
}

#[derive(Default)]
struct Clip(RefCell<Vec<ClipItem>>);

impl ClipboardSink for Clip {
    fn copy(&self, items: Vec<ClipItem>, _: &mut App) -> Task<Result<()>> {
        self.0.borrow_mut().extend(items);
        Task::ready(Ok(()))
    }
}

struct Flow {
    rig: Rig,
    deck: Entity<Deck>,
    flow: Entity<DeckShare>,
    host: Rc<Host>,
    sink: Arc<MemorySink>,
    item: DeckItem,
}

fn flow(cx: &mut TestAppContext, fake: &Arc<Fake>) -> Flow {
    let rig = rig(cx, fake, |_| {});
    let file = rig._dir.path().join("shot.png");
    image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255]))
        .save(&file)
        .unwrap();
    let item = DeckItem {
        id: Uuid::new_v4(),
        name: "shot".into(),
        file: file.clone(),
        thumb: file,
        kind: CaptureKind::Region,
        video: false,
        staged: false,
        status: ItemStatus::Idle,
        auto_dismiss: true,
        saved: None,
    };
    let host = Rc::new(Host::default());
    let sink = MemorySink::new();
    let (deck, flow) = cx.update(|cx| {
        es_ui::init(cx);
        install_toasts(sink.clone(), Arc::new(RecordingNotifier::default()), cx);
        let deck = cx.new(|_| Deck::new(DeckPolicy::default()));
        deck.update(cx, |d, cx| d.show(item.clone(), cx));
        let settings = host.settings.clone();
        let hooks = ShareHooks {
            open_settings: Rc::new(move |_| settings.set(settings.get() + 1)),
            open_url: Rc::new(|_, _| {}),
        };
        let popovers: Rc<dyn PopoverHost> = host.clone();
        let (shares, d) = (rig.shares.clone(), deck.clone());
        let flow =
            cx.new(|cx| DeckShare::new(shares, d, popovers, hooks, Rc::new(Clip::default()), cx));
        (deck, flow)
    });
    Flow {
        rig,
        deck,
        flow,
        host,
        sink,
        item,
    }
}

impl Flow {
    fn open(&self, cx: &mut TestAppContext) -> Entity<SharePopover> {
        let item = self.item.clone();
        self.flow.update(cx, |f, cx| f.open(item, None, cx));
        cx.run_until_parked();
        self.flow.read_with(cx, |f, _| f.popover()).unwrap()
    }

    fn press(&self, popover: &Entity<SharePopover>, event: ShareEvent, cx: &mut TestAppContext) {
        popover.update(cx, |_, cx| cx.emit(event));
        cx.run_until_parked();
    }

    fn toast(&self) -> ToastRequest {
        self.sink.shown().last().cloned().expect("a toast")
    }

    /// Presses a toast's button as its surface does: the action, then the toast closes.
    fn act(&self, label: &str, cx: &mut TestAppContext) {
        let toast = self.toast();
        let (_, act) = toast
            .actions
            .iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("no {label} on {:?}", toast.title))
            .clone();
        cx.update(|cx| {
            act.run(cx);
            toast.close(CloseReason::Acted, cx);
        });
        cx.run_until_parked();
    }
}

fn view(popover: &Entity<SharePopover>, cx: &mut TestAppContext) -> ShareView {
    popover.read_with(cx, |p, _| p.view().clone())
}

#[gpui::test]
fn the_first_share_asks_where_and_free_is_the_consent_that_leads_on_to_sharing(
    cx: &mut TestAppContext,
) {
    let f = flow(cx, &Fake::new(Box::new(|r| Ok(made(r)))));
    let popover = f.open(cx);
    assert_eq!(
        view(&popover, cx),
        ShareView::FirstShare,
        "nothing is sent before a choice"
    );
    assert!(
        f.deck.read_with(cx, |d, _| d.held()) == Some(f.item.id),
        "the card waits"
    );

    f.press(&popover, ShareEvent::Choose(ShareTarget::Free), cx);

    let cfg = f.rig.shares.read_with(cx, |s, cx| s.config(cx));
    assert!(
        cfg.consented,
        "choosing Free is the consent, written to the settings"
    );
    assert_eq!(cfg.target, ShareMode::Free);
    assert_eq!(view(&popover, cx), ShareView::Ready);
    assert!(kinds(&f.rig).is_empty(), "choosing is not sharing");
}

#[gpui::test]
fn the_own_bucket_opens_settings_and_closes_the_popover(cx: &mut TestAppContext) {
    let f = flow(cx, &Fake::new(Box::new(|r| Ok(made(r)))));
    let popover = f.open(cx);
    f.press(&popover, ShareEvent::Choose(ShareTarget::OwnBucket), cx);
    assert_eq!(f.host.settings.get(), 1);
    assert!(!f.flow.read_with(cx, |s, _| s.is_open()));
    assert_eq!(
        f.deck.read_with(cx, |d, _| d.held()),
        None,
        "the card is let go"
    );
    assert!(!f.rig.shares.read_with(cx, |s, cx| s.config(cx)).consented);
}

#[gpui::test]
fn share_uploads_the_card_with_its_title_and_the_toast_follows_it_to_shared(
    cx: &mut TestAppContext,
) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sent = seen.clone();
    let fake = Fake::new(Box::new(move |r| {
        sent.lock().unwrap().push(r.clone());
        Ok(made(r))
    }));
    let gate = hold(&fake);
    let f = flow(cx, &fake);
    consent(&f.rig, cx);
    let popover = f.open(cx);
    f.press(&popover, ShareEvent::Days(3), cx);
    assert_eq!(
        f.rig.shares.read_with(cx, |s, cx| s.days(cx)),
        3,
        "the days go to the engine"
    );

    f.press(
        &popover,
        ShareEvent::Share {
            title: Some("Login bug".into()),
        },
        cx,
    );
    assert!(
        !f.flow.read_with(cx, |s, _| s.is_open()),
        "the popover closes as it shares"
    );
    assert_eq!(f.sink.shown()[0].title, "Preparing…");
    assert!(f.sink.shown()[0].sticky);

    wait_until(cx, "the upload's progress", || {
        f.toast().message.as_deref() == Some("Uploading 50 %")
    });
    let uploading = f.toast();
    assert_eq!(uploading.title, "Link copied");
    assert_eq!(uploading.progress, Some(0.5));
    assert_eq!(uploading.actions[0].0, "Cancel");

    drop(gate);
    wait_until(cx, "the share to finish", || f.toast().title == "Shared");
    let shared = f.toast();
    assert_eq!(shared.message.as_deref(), Some("Expires in 7 days"));
    let labels: Vec<&str> = shared.actions.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, ["Open", "Copy Link"]);
    assert!(!shared.sticky, "a finished share runs its countdown");
    let sent = seen.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].media.path, f.item.file);
    assert_eq!(sent[0].title.as_deref(), Some("Login bug"));
    assert_eq!(sent[0].days, 3);
    assert!(
        f.deck.read_with(cx, |d, _| d.is_shared(f.item.id)),
        "the card is marked once its link is live"
    );
}

#[gpui::test]
fn cancel_removes_the_toast_at_once_and_stops_the_upload(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let gate = hold(&fake);
    let f = flow(cx, &fake);
    consent(&f.rig, cx);
    let popover = f.open(cx);
    f.press(&popover, ShareEvent::Share { title: None }, cx);
    wait_until(cx, "the upload's progress", || {
        f.toast().message.as_deref() == Some("Uploading 50 %")
    });

    f.act("Cancel", cx);

    assert!(
        f.toast().closed.get(),
        "the toast goes while the upload still runs"
    );
    assert_eq!(kinds(&f.rig).last(), Some(&ShareJobKind::Cancelled));
    drop(gate);
    cx.run_until_parked();
    assert!(!f.deck.read_with(cx, |d, _| d.is_shared(f.item.id)));
    assert!(
        f.sink.shown().iter().all(|t| t.title != "Shared"),
        "a cancelled share never says it is shared"
    );
}

#[gpui::test]
fn a_failure_says_why_and_retry_reopens_the_popover_on_the_error(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|_| {
        Err(ShareError::Service {
            code: Code::Full,
            retry_after_s: None,
        })
    }));
    let f = flow(cx, &fake);
    consent(&f.rig, cx);
    let popover = f.open(cx);
    f.press(&popover, ShareEvent::Share { title: None }, cx);
    wait_until(cx, "the failure", || f.toast().style == ToastStyle::Error);

    let failed = f.toast();
    let words = es_app::share::ShareFailure::Refused {
        code: Code::Full,
        retry_after_s: None,
    }
    .message(&es_app::services::Strings);
    assert_eq!(failed.message.as_deref(), Some(words.as_str()));
    assert_eq!(failed.actions[0].0, "Retry");

    f.act("Retry", cx);
    let again = f
        .flow
        .read_with(cx, |s, _| s.popover())
        .expect("the popover again");
    assert_eq!(
        view(&again, cx),
        ShareView::Failed {
            message: words.into(),
            wait: None
        }
    );
    assert_eq!(f.host.opened.get(), 2);
}
