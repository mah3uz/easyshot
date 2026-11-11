use super::{
    FileShare, ShareFailure, ShareJobEvent, ShareJobId, ShareJobKind, Shares, SharesChanged,
};
use crate::deck::{Deck, DeckItem};
use crate::errors::Result;
use crate::platform_ui::ClipboardSink;
use crate::services::Strings;
use crate::toast::{IconName, Toast, dismiss_toast, show_toast, update_toast};
use es_geom::Rect;
use es_model::platform::{ClipItem, OutputId};
use es_model::{ShareMode, ShareRecord, ShareTarget};
use es_ui::components::{Destination, ShareEvent, SharePopover, ShareTarget as Choice, ShareView};
use gpui::{App, AppContext as _, Context, Entity, Subscription, WeakEntity, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use uuid::Uuid;

/// The control a popover points at, in logical pixels from its output's top-left.
#[derive(Debug, Clone, PartialEq)]
pub struct Anchor {
    pub output: OutputId,
    pub rect: Rect,
}

pub type BuildPopover = Box<dyn FnOnce(&mut Window, &mut App) -> Entity<SharePopover>>;

/// Shows the share popover; `build` runs inside the window that shows it.
pub trait PopoverHost {
    /// `closed` runs once when the popover goes, whoever closes it.
    fn open(
        &self,
        anchor: Option<&Anchor>,
        build: BuildPopover,
        closed: Rc<dyn Fn(&mut App)>,
        cx: &mut App,
    ) -> Result<()>;
    fn close(&self, cx: &mut App);
}

pub type OpenUrl = Rc<dyn Fn(&str, &mut App)>;

pub struct ShareHooks {
    pub open_settings: Rc<dyn Fn(&mut App)>,
    pub open_url: OpenUrl,
}

type PopoverSlot = Rc<RefCell<Option<WeakEntity<SharePopover>>>>;

struct Shown {
    ask: Ask,
    generation: u64,
    popover: PopoverSlot,
}

struct Ask {
    item: DeckItem,
    anchor: Option<Anchor>,
    title: Option<String>,
}

/// Share on a deck card: the popover, one toast per upload, and the card's shared mark.
pub struct DeckShare {
    shares: Entity<Shares>,
    deck: Entity<Deck>,
    host: Rc<dyn PopoverHost>,
    hooks: ShareHooks,
    clipboard: Rc<dyn ClipboardSink>,
    open: Option<Shown>,
    generation: u64,
    jobs: HashMap<ShareJobId, Ask>,
    marked: HashSet<Uuid>,
    weak: WeakEntity<Self>,
    _subscriptions: [Subscription; 3],
}

impl DeckShare {
    pub fn new(
        shares: Entity<Shares>,
        deck: Entity<Deck>,
        host: Rc<dyn PopoverHost>,
        hooks: ShareHooks,
        clipboard: Rc<dyn ClipboardSink>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = [
            cx.subscribe(&shares, |this, _, e: &ShareJobEvent, cx| this.on_job(e, cx)),
            cx.subscribe(&shares, |this, _, _: &SharesChanged, cx| {
                this.mark(true, cx)
            }),
            cx.observe(&deck, |this, _, cx| this.mark(false, cx)),
        ];
        Self {
            shares,
            deck,
            host,
            hooks,
            clipboard,
            open: None,
            generation: 0,
            jobs: HashMap::new(),
            marked: HashSet::new(),
            weak: cx.entity().downgrade(),
            _subscriptions: subscriptions,
        }
    }

    pub fn open(&mut self, item: DeckItem, anchor: Option<Anchor>, cx: &mut Context<Self>) {
        let consented = self.shares.read(cx).config(cx).consented;
        let view = if consented {
            ShareView::Ready
        } else {
            ShareView::FirstShare
        };
        self.show(
            Ask {
                item,
                anchor,
                title: None,
            },
            view,
            cx,
        );
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn popover(&self) -> Option<Entity<SharePopover>> {
        self.open.as_ref()?.popover.borrow().as_ref()?.upgrade()
    }

    fn show(&mut self, ask: Ask, view: ShareView, cx: &mut Context<Self>) {
        self.host.close(cx);
        let id = ask.item.id;
        let cfg = self.shares.read(cx).config(cx);
        let days = self.shares.read(cx).days(cx);
        let destination = match cfg.target {
            ShareMode::Free => Destination::Free,
            ShareMode::Bucket => Destination::Bucket(cfg.bucket_name.clone().into()),
        };
        self.generation += 1;
        let generation = self.generation;
        let this = cx.entity().downgrade();
        let built = this.clone();
        let slot = PopoverSlot::default();
        let made = slot.clone();
        let build: BuildPopover = Box::new(move |window, cx| {
            let popover = cx.new(|cx| {
                let mut p = SharePopover::new(window, cx);
                p.set_view(view, cx);
                p.set_days(days, cx);
                p.set_destination(destination, cx);
                p
            });
            let weak = built.clone();
            cx.subscribe(&popover, move |popover, e: &ShareEvent, cx| {
                let e = e.clone();
                let _ = weak.update(cx, |s, cx| s.on_popover(&popover, e, cx));
            })
            .detach();
            *made.borrow_mut() = Some(popover.downgrade());
            popover
        });
        // The host may close from inside this entity's own update, so the answer waits a turn.
        let closed: Rc<dyn Fn(&mut App)> = Rc::new(move |cx| {
            let this = this.clone();
            cx.defer(move |cx| {
                let _ = this.update(cx, |s, cx| s.closed(generation, cx));
            });
        });
        let anchor = ask.anchor.clone();
        self.open = Some(Shown {
            ask,
            generation,
            popover: slot,
        });
        self.deck.update(cx, |d, cx| d.hold(Some(id), cx));
        if let Err(e) = self.host.open(anchor.as_ref(), build, closed, cx) {
            tracing::warn!(error = %e, "share popover not shown");
            self.closed(generation, cx);
        }
    }

    /// A popover that has since been replaced closing late changes nothing.
    fn closed(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self
            .open
            .as_ref()
            .is_some_and(|o| o.generation == generation)
        {
            self.open = None;
            self.deck.update(cx, |d, cx| d.hold(None, cx));
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        self.host.close(cx);
        if self.open.take().is_some() {
            self.deck.update(cx, |d, cx| d.hold(None, cx));
        }
    }

    fn on_popover(
        &mut self,
        popover: &Entity<SharePopover>,
        e: ShareEvent,
        cx: &mut Context<Self>,
    ) {
        match e {
            ShareEvent::Share { title } => self.start(title, cx),
            ShareEvent::Retry => {
                let title = self.open.as_ref().and_then(|o| o.ask.title.clone());
                self.start(title, cx);
            }
            ShareEvent::Days(days) => {
                if self.shares.update(cx, |s, cx| s.set_days(days, cx)) {
                    popover.update(cx, |p, cx| p.set_days(days, cx));
                }
            }
            ShareEvent::Choose(Choice::Free) => {
                self.shares
                    .update(cx, |s, cx| s.consent(ShareMode::Free, cx));
                popover.update(cx, |p, cx| p.set_view(ShareView::Ready, cx));
            }
            ShareEvent::Choose(Choice::OwnBucket) | ShareEvent::ChangeDestination => {
                self.close(cx);
                (self.hooks.open_settings.clone())(cx);
            }
            ShareEvent::Terms => (self.hooks.open_url.clone())(super::TERMS_URL, cx),
            ShareEvent::Close => self.close(cx),
        }
    }

    fn start(&mut self, title: Option<String>, cx: &mut Context<Self>) {
        let Some(Shown { ask, .. }) = self.open.take() else {
            return;
        };
        self.host.close(cx);
        self.deck.update(cx, |d, cx| d.hold(None, cx));
        let file = FileShare {
            path: ask.item.file.clone(),
            days: None,
            title: title.clone(),
        };
        let ask = Ask { title, ..ask };
        let id = self.shares.update(cx, |s, cx| s.start_file(file, cx));
        self.jobs.insert(id, ask);
        self.toast(
            id,
            Toast::new("share-toast-preparing", IconName::Info),
            true,
            cx,
        );
    }

    fn on_job(&mut self, e: &ShareJobEvent, cx: &mut Context<Self>) {
        let id = e.id;
        if !self.jobs.contains_key(&id) {
            return;
        }
        match &e.kind {
            ShareJobKind::Preparing { permille } => {
                let t = Toast::new("share-toast-preparing", IconName::Info)
                    .progress(f32::from(*permille) / 1000.);
                self.toast(id, t, false, cx);
            }
            ShareJobKind::Reserved { .. } => self.toast(id, uploading(0, 1), true, cx),
            ShareJobKind::Progress { sent, total } => {
                self.toast(id, uploading(*sent, *total), false, cx)
            }
            ShareJobKind::Done(record) => {
                self.jobs.remove(&id);
                show_toast(self.shared(id, record), cx);
                self.mark(true, cx);
            }
            ShareJobKind::Failed(failure) => {
                let ask = self.jobs.remove(&id);
                show_toast(self.failed(id, *failure, ask), cx);
            }
            ShareJobKind::Cancelled => {
                self.jobs.remove(&id);
                dismiss_toast(id.0, cx);
            }
        }
    }

    /// A sticky toast with Cancel; `fresh` shows it even after another toast took its place.
    fn toast(&self, id: ShareJobId, t: Toast, fresh: bool, cx: &mut Context<Self>) {
        let shares = self.shares.downgrade();
        let t = t
            .sticky()
            .keyed(id.0)
            .action("share-toast-cancel", move |cx| {
                let _ = shares.update(cx, |s, cx| s.cancel(id, cx));
            });
        if fresh {
            show_toast(t, cx);
        } else {
            update_toast(t, cx);
        }
    }

    fn shared(&self, id: ShareJobId, record: &ShareRecord) -> Toast {
        let t = match &record.target {
            ShareTarget::Bucket { label } => {
                Toast::new("share-toast-shared-bucket", IconName::Check)
                    .arg("bucket", label.clone())
            }
            ShareTarget::Free => Toast::new("share-toast-shared", IconName::Check)
                .arg("days", record.days.unwrap_or_default().to_string()),
        };
        let (open, copy) = (self.hooks.open_url.clone(), self.clipboard.clone());
        let (url, link) = (record.page_url.clone(), record.page_url.clone());
        t.keyed(id.0)
            .action("share-toast-open", move |cx| open(&url, cx))
            .action("share-toast-copy-link", move |cx| {
                let copied = copy.copy(vec![ClipItem::Text(link.clone())], cx);
                cx.spawn(async move |_| {
                    if let Err(e) = copied.await {
                        tracing::warn!(error = %e, "copying the share link");
                    }
                })
                .detach();
            })
    }

    fn failed(&self, id: ShareJobId, failure: ShareFailure, ask: Option<Ask>) -> Toast {
        let message = failure.message(&Strings);
        let t = Toast::error("share-toast-failed")
            .arg("error", message.clone())
            .keyed(id.0);
        let Some(ask) = ask else {
            return t;
        };
        let flow = self.weak.clone();
        t.action("share-toast-retry", move |cx| {
            let view = ShareView::Failed {
                message: message.clone().into(),
                wait: None,
            };
            let ask = Ask {
                item: ask.item.clone(),
                anchor: ask.anchor.clone(),
                title: ask.title.clone(),
            };
            let _ = flow.update(cx, |s, cx| s.retry(ask, view, cx));
        })
    }

    fn retry(&mut self, ask: Ask, view: ShareView, cx: &mut Context<Self>) {
        let anchor = self
            .deck
            .read(cx)
            .get(ask.item.id)
            .is_some()
            .then_some(ask.anchor)
            .flatten();
        self.show(Ask { anchor, ..ask }, view, cx);
    }

    /// Only a change to which cards are here, or to the shares, asks the engine again.
    fn mark(&mut self, shares_changed: bool, cx: &mut Context<Self>) {
        let items: Vec<DeckItem> = self.deck.read(cx).items().cloned().collect();
        let ids: HashSet<Uuid> = items.iter().map(|i| i.id).collect();
        if !shares_changed && ids == self.marked {
            return;
        }
        self.marked = ids;
        let shares = self.shares.read(cx);
        let marks: Vec<(Uuid, bool)> = items
            .iter()
            .map(|i| (i.id, shares.for_file(&i.file, cx).is_some()))
            .collect();
        self.deck.update(cx, |d, cx| {
            for (id, shared) in marks {
                d.set_shared(id, shared, cx);
            }
        });
    }
}

fn uploading(sent: u64, total: u64) -> Toast {
    let done = if total == 0 {
        0.
    } else {
        sent as f64 / total as f64
    };
    Toast::new("share-toast-link-copied", IconName::Info)
        .arg("percent", ((done * 100.).floor() as u32).to_string())
        .progress(done as f32)
}
