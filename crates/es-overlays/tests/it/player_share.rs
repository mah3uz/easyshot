#![cfg(feature = "player")]
#![allow(clippy::unwrap_used)]
use crate::player::{Rig, rig};
use es_app::share::flow::BuildPopover;
use es_app::share::{PopoverHost, WindowPopover};
use es_overlays::player::{ExportState, PlayerEvent, PlayerWindow, ShareRequested, bind_share};
use es_ui::components::{Destination, SharePopover, ShareView};
use es_ui::tokens::ThemeMode;
use gpui::AppContext as _;
use std::rc::Rc;
use std::time::Duration;

fn wire(r: &mut Rig) -> WindowPopover {
    let host = WindowPopover::default();
    let (view, given) = (r.s.view.clone(), host.clone());
    r.s.app
        .cx
        .update(|cx| view.update(cx, |p, cx| p.set_share(given, cx)));
    r.s.app.settle();
    host
}

/// Clicks across the title bar's right half from the left until Share or Export answers.
fn sweep(r: &mut Rig) -> (Vec<PlayerEvent>, usize) {
    let (events, _a) = r.s.record::<PlayerEvent>();
    let (shared, _b) = r.s.record::<ShareRequested>();
    let mut x = 620.;
    while shared.borrow().is_empty()
        && r.read(|p| *p.export_state() == ExportState::Idle)
        && x < 950.
    {
        r.s.click(x, 19.);
        x += 6.;
    }
    let events = events.borrow().clone();
    (events, shared.borrow().len())
}

#[test]
fn share_sits_between_open_in_studio_and_export_once_sharing_is_wired() {
    let (_d, mut r) = rig();
    wire(&mut r);
    let (events, shared) = sweep(&mut r);
    assert_eq!(shared, 1);
    assert!(
        !events.is_empty() && events.iter().all(|e| *e == PlayerEvent::OpenStudio),
        "{events:?}"
    );
    assert_eq!(r.read(|p| p.export_state().clone()), ExportState::Idle);
}

#[test]
fn without_sharing_wired_export_follows_open_in_studio_and_nothing_asks_to_share() {
    let (_d, mut r) = rig();
    let (events, shared) = sweep(&mut r);
    assert_eq!(shared, 0, "the viewer has no Share");
    assert!(events.contains(&PlayerEvent::OpenStudio));
    assert!(!matches!(
        r.read(|p| p.export_state().clone()),
        ExportState::Idle
    ));
    r.wait("the export", |p| {
        !matches!(p.export_state(), ExportState::Running(_))
    });
}

#[test]
fn the_share_video_key_asks_to_share_once_bound() {
    let (_d, mut r) = rig();
    wire(&mut r);
    let (shared, _sub) = r.s.record::<ShareRequested>();
    r.s.key("ctrl-shift-u");
    assert!(shared.borrow().is_empty(), "Share Video has no default key");

    r.s.app.cx.update(|cx| bind_share(Some("ctrl-shift-u"), cx));
    r.s.key("ctrl-shift-u");
    assert_eq!(shared.borrow().len(), 1);

    r.s.app.cx.update(|cx| bind_share(None, cx));
    r.s.key("ctrl-shift-u");
    assert_eq!(shared.borrow().len(), 1, "a cleared key no longer shares");
}

/// The headless window draws only as an update to it ends.
fn draw(r: &mut Rig) {
    let window = r.s.app.window.into();
    r.s.app.cx.update_window(window, |_, _, _| {}).unwrap();
    r.s.app.settle();
}

fn ready_popover() -> BuildPopover {
    Box::new(|window, cx| {
        cx.new(|cx| {
            let mut p = SharePopover::new(window, cx);
            p.set_view(ShareView::Ready, cx);
            p.set_days(7, cx);
            p.set_destination(Destination::Free, cx);
            p
        })
    })
}

#[test]
fn pictures_of_share_in_the_title_bar_and_its_popover() {
    for (mode, name) in [(ThemeMode::Light, "light"), (ThemeMode::Dark, "dark")] {
        let (_d, mut r) = rig();
        r.s.app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
        let host = wire(&mut r);
        r.wait("the strip", PlayerWindow::has_strip);
        std::thread::sleep(Duration::from_millis(200));
        r.s.app.settle();
        r.s.app.screenshot(&format!("player-share-{name}"));

        let shown = host.clone();
        r.s.app.cx.update(|cx| {
            shown
                .open(None, ready_popover(), Rc::new(|_| {}), cx)
                .unwrap()
        });
        draw(&mut r);
        draw(&mut r);
        assert!(host.is_open());
        r.s.app.screenshot(&format!("player-share-popover-{name}"));
    }
}
