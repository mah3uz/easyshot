#![cfg(feature = "player")]
#![allow(clippy::unwrap_used)]
use crate::player::{Rig, rig};
use es_app::share::flow::BuildPopover;
use es_app::share::{PopoverHost, WindowPopover};
use es_overlays::player::{
    ExportOptions, ExportState, PlayerEvent, PlayerWindow, ShareRequested, bind_share,
};
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
fn share_asks_nothing_until_sharing_is_wired() {
    let (_d, mut r) = rig();
    let (shared, _sub) = r.s.record::<ShareRequested>();
    let view = r.s.view.clone();
    let press = |r: &mut Rig| {
        r.s.app.cx.update(|cx| view.update(cx, |p, cx| p.share(cx)));
        r.s.app.settle();
    };
    press(&mut r);
    assert!(shared.borrow().is_empty(), "nothing would open a popover");
    wire(&mut r);
    press(&mut r);
    assert_eq!(shared.borrow().len(), 1);
}

#[test]
fn share_sends_the_trim_and_choices_export_would_use() {
    let (_d, mut r) = rig();
    let (start, to) = (r.start_handle(), r.at(1.5).0);
    r.drag(start, to);
    let (end, to) = (r.end_handle(), r.at(4.).0);
    r.drag(end, to);
    let view = r.s.view.clone();
    r.s.app.cx.update(|cx| {
        view.update(cx, |p, cx| {
            let options = ExportOptions {
                brightness: 0.3,
                ..p.options()
            };
            p.set_options(options, cx);
            p.set_remove_audio(true, cx);
        })
    });
    let (job, trim) = r.read(|p| (p.share_job(), p.trim()));
    assert!(trim.start() > 1. && trim.end() < 4.5, "{trim:?}");
    assert_eq!(job.trim, trim);
    assert_eq!(job.options.brightness, 0.3);
    assert!(job.remove_audio);
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

/// Lets the popover's opening play out, redrawing the headless window after each step.
fn draw(r: &mut Rig) {
    let window = r.s.app.window.into();
    for _ in 0..3 {
        r.s.app
            .cx
            .advance_clock(es_ui::motion::POPOVER_OPEN.duration);
        std::thread::sleep(es_ui::motion::POPOVER_OPEN.duration);
        r.s.app
            .cx
            .update_window(window, |_, window, _| window.refresh())
            .unwrap();
        r.s.app.settle();
    }
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
        assert!(host.is_open());
        r.s.app.screenshot(&format!("player-share-popover-{name}"));
    }
}
