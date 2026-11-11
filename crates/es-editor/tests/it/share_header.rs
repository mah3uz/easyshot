#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::shell::{Fixture, launch, launch_with};
use es_app::share::{PopoverHost, WindowPopover};
use es_editor::window::TITLE_BAR_HEIGHT;
use es_editor::{EditorWindowEvent, Job, ShareRequested};
use es_ui::components::{SHARE_POPOVER_WIDTH, SharePopover, ShareView};
use gpui::{AppContext, point, px};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

const WIDTH: f32 = 1080.;
const HEIGHT: f32 = 720.;
const FOOTER_ROW: f32 = HEIGHT - 26.;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Save,
    Share,
    Export,
    Copy,
}

fn asked(f: &mut Fixture) -> Rc<Cell<usize>> {
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let editor = f.editor.clone();
    f.app.cx.update(|cx| {
        cx.subscribe(&editor, move |_, _: &ShareRequested, _| {
            seen.set(seen.get() + 1)
        })
        .detach()
    });
    count
}

fn wire(f: &mut Fixture) -> WindowPopover {
    let host = WindowPopover::default();
    let given = host.clone();
    f.editor
        .update(&mut f.app.cx, |w, cx| w.set_share(given, cx));
    f.app.settle();
    host
}

/// Clicks along a row of the right half, left to right, noting what answered at each x; a started job is ended at once so the next button stays live.
fn sweep(f: &mut Fixture, y: f32) -> Vec<(Hit, f32)> {
    let shared = asked(f);
    let mut hits = Vec::new();
    let mut x = WIDTH / 2.;
    while x < WIDTH {
        let (events, shares) = (f.events.borrow().len(), shared.get());
        f.click_at(point(px(x), px(y)));
        let event = f.events.borrow().get(events).copied();
        if let Some(event) = event {
            let (hit, job) = match event {
                EditorWindowEvent::Save => (Hit::Save, Job::Save),
                EditorWindowEvent::Export => (Hit::Export, Job::Export),
                EditorWindowEvent::Copy => (Hit::Copy, Job::Copy),
            };
            hits.push((hit, x));
            f.editor
                .update(&mut f.app.cx, |w, cx| w.finish(job, Err("stop".into()), cx));
            f.app.settle();
        } else if shared.get() > shares {
            hits.push((Hit::Share, x));
        }
        x += 4.;
    }
    hits
}

fn order(hits: &[(Hit, f32)]) -> Vec<Hit> {
    let mut seen: Vec<Hit> = hits.iter().map(|(h, _)| *h).collect();
    seen.dedup();
    seen
}

fn span(hits: &[(Hit, f32)], which: Hit) -> (f32, f32) {
    let xs: Vec<f32> = hits
        .iter()
        .filter(|(h, _)| *h == which)
        .map(|(_, x)| *x)
        .collect();
    (xs[0], xs[xs.len() - 1])
}

#[test]
fn share_sits_in_the_title_bar_between_save_and_export() {
    let mut f = launch();
    wire(&mut f);
    let hits = sweep(&mut f, f32::from(TITLE_BAR_HEIGHT) / 2.);
    assert_eq!(
        order(&hits),
        [Hit::Save, Hit::Share, Hit::Export],
        "{hits:?}"
    );
}

#[test]
fn without_sharing_connected_the_title_bar_has_save_then_export_only() {
    let mut f = launch();
    let hits = sweep(&mut f, f32::from(TITLE_BAR_HEIGHT) / 2.);
    assert_eq!(order(&hits), [Hit::Save, Hit::Export], "{hits:?}");
}

#[test]
fn the_footer_holds_copy_and_no_share() {
    let mut f = launch();
    wire(&mut f);
    let hits = sweep(&mut f, FOOTER_ROW);
    assert_eq!(order(&hits), [Hit::Copy], "{hits:?}");
}

fn ready_popover(window: &mut gpui::Window, cx: &mut gpui::App) -> gpui::Entity<SharePopover> {
    cx.new(|cx| {
        let mut p = SharePopover::new(window, cx);
        p.set_view(ShareView::Ready, cx);
        p.set_days(7, cx);
        p
    })
}

/// Opens the popover whenever the editor asks, as the daemon's share flow does.
fn open_on_request(f: &mut Fixture, host: &WindowPopover) {
    let (editor, host) = (f.editor.clone(), host.clone());
    f.app.cx.update(|cx| {
        cx.subscribe(&editor, move |_, _: &ShareRequested, cx| {
            host.open(None, Box::new(ready_popover), Rc::new(|_| {}), cx)
                .unwrap()
        })
        .detach()
    });
}

fn draw(f: &mut Fixture) {
    let window = f.app.window.into();
    for _ in 0..3 {
        f.app.cx.advance_clock(es_ui::motion::POPOVER_OPEN.duration);
        std::thread::sleep(es_ui::motion::POPOVER_OPEN.duration);
        f.app
            .cx
            .update_window(window, |_, window, _| window.refresh())
            .unwrap();
        f.app.settle();
    }
}

/// The box, in window points, of what clearly changed below the title bar; the popover's faint shadow is left out.
fn changed(before: &image::RgbaImage, after: &image::RgbaImage) -> (f32, f32, f32, f32) {
    let k = before.width() as f32 / WIDTH;
    let from = (f32::from(TITLE_BAR_HEIGHT) * k) as u32;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0, 0);
    for y in from..before.height() {
        for x in 0..before.width() {
            let (a, b) = (before.get_pixel(x, y), after.get_pixel(x, y));
            let apart: i32 = (0..3)
                .map(|i| (i32::from(a[i]) - i32::from(b[i])).abs())
                .sum();
            if apart > 48 {
                (left, top) = (left.min(x), top.min(y));
                (right, bottom) = (right.max(x), bottom.max(y));
            }
        }
    }
    assert!(right > 0, "nothing was drawn");
    let p = |v: u32| v as f32 / k;
    (p(left), p(top), p(right), p(bottom))
}

#[test]
fn pressing_share_opens_the_popover_below_it_right_aligned_inside_the_window() {
    let mut f = launch();
    let host = wire(&mut f);
    let y = f32::from(TITLE_BAR_HEIGHT) / 2.;
    let hits = sweep(&mut f, y);
    let (from, to) = span(&hits, Hit::Share);
    let (_, export) = span(&hits, Hit::Export);
    open_on_request(&mut f, &host);
    draw(&mut f);
    let before = f.app.screenshot("editor-share-header-closed");

    f.click_at(point(px((from + to) / 2.), px(y)));
    draw(&mut f);
    assert!(host.is_open(), "Share opened its popover");
    let after = f.app.screenshot("editor-share-header-open");
    let (left, top, right, bottom) = changed(&before, &after);
    let width = f32::from(SHARE_POPOVER_WIDTH);
    assert!(
        top <= f32::from(TITLE_BAR_HEIGHT) + 4.,
        "it hangs from the title bar ({top})"
    );
    assert!(bottom < HEIGHT / 2. + 80., "it opens downwards ({bottom})");
    assert!(
        (right - to).abs() <= 6. && right < export,
        "its right edge meets Share's ({right} against Share ending near {to})"
    );
    assert!(
        (right - left - width).abs() <= 16. && left > 0.,
        "the whole popover is inside the window ({left}..{right})"
    );
}

#[test]
fn the_share_image_key_opens_the_popover() {
    let mut f = launch_with(BTreeMap::from([(
        "image-share".into(),
        "Ctrl+Shift+U".into(),
    )]));
    let host = wire(&mut f);
    open_on_request(&mut f, &host);
    f.key("ctrl-shift-u");
    assert!(host.is_open());
}
