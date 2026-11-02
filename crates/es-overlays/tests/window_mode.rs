mod window_mode_support;

use es_geom::Rect;
use es_model::platform::{OutputId, WindowId, WindowInfo};
use es_overlays::capture::window::window_frame;
use es_overlays::capture::windows::{self, Candidate};
use es_overlays::capture::{PickEvent, WindowPicker, WindowSelector};
use window_mode_support::{Surface, bluish, frame, near, rgb};

const W: f32 = 640.;
const H: f32 = 400.;

fn grey(x: u32, y: u32) -> [u8; 3] {
    if ((x / 80) + (y / 80)).is_multiple_of(2) {
        [120, 120, 120]
    } else {
        [90, 90, 90]
    }
}

fn info(id: &str, rect: Option<(f64, f64, f64, f64)>, floating: bool, rank: u32) -> WindowInfo {
    WindowInfo {
        id: WindowId(id.into()),
        app_id: format!("app.{id}"),
        title: format!("{id} title"),
        pid: Some(1000 + rank),
        logical: rect.map(|(x0, y0, x1, y1)| Rect::new(x0, y0, x1, y1)),
        output: Some(OutputId("HEADLESS-1".into())),
        workspace: Some("1".into()),
        floating,
        focus_rank: Some(rank),
    }
}

fn snapshot() -> Vec<WindowInfo> {
    let mut own = info("own", Some((60., 200., 180., 280.)), true, 0);
    own.pid = Some(std::process::id());
    let mut elsewhere = info("elsewhere", Some((0., 0., 640., 400.)), true, 0);
    elsewhere.output = Some(OutputId("OTHER-1".into()));
    vec![
        info("back", Some((40., 40., 400., 300.)), false, 1),
        own,
        info("front", Some((200., 100., 560., 360.)), false, 0),
        info("hidden", None, false, 2),
        elsewhere,
        info("floating", Some((300., 60., 460., 200.)), true, 3),
    ]
}

fn candidates() -> Vec<Candidate> {
    windows::on_output(
        &snapshot(),
        &OutputId("HEADLESS-1".into()),
        Rect::new(0., 0., f64::from(W), f64::from(H)),
        std::process::id(),
    )
}

fn hovered(s: &mut Surface<WindowSelector>) -> Option<String> {
    let view = s.view.clone();
    s.app
        .cx
        .update(|cx| view.read(cx).hovered().map(|c| c.id.0.clone()))
}

#[test]
fn hover_picks_the_frontmost_window_floating_first_then_by_focus_and_never_our_own() {
    let frozen = frame(W, H, 2., grey);
    let mut s = Surface::open(W, H, |cx| WindowSelector::new(frozen, candidates(), cx));
    let (events, _sub) = s.record::<PickEvent>();
    for (at, want) in [
        ((250., 150.), Some("front")),
        ((350., 150.), Some("floating")),
        ((100., 240.), Some("back")),
        ((600., 380.), None),
    ] {
        s.hover(at.0, at.1, false);
        assert_eq!(hovered(&mut s).as_deref(), want, "under {at:?}");
    }
    s.hover(350., 150., false);
    let shot = s.app.screenshot("window-hover");
    let inside = rgb(&shot, 420., 180.);
    assert!(
        i32::from(inside[2]) > i32::from(inside[0]) + 15,
        "the hovered window is tinted with the accent: {inside:?}"
    );
    assert!(
        near(rgb(&shot, 250., 330.), grey(500, 660), 1),
        "the rest of the screen is not dimmed"
    );
    assert!(bluish(rgb(&shot, 300.5, 130.)), "an accent outline");
    assert!(
        bluish(rgb(&shot, 310., 75.)),
        "a title pill at the top-left"
    );

    s.click(350., 150.);
    s.key("space");
    s.key("escape");
    assert_eq!(
        *events.borrow(),
        vec![
            PickEvent::Window {
                id: WindowId("floating".into()),
                local: Some(Rect::new(300., 60., 460., 200.)),
            },
            PickEvent::Region,
            PickEvent::Cancel,
        ]
    );
}

#[test]
fn a_click_on_the_bare_desktop_picks_nothing() {
    let frozen = frame(W, H, 2., grey);
    let mut s = Surface::open(W, H, |cx| WindowSelector::new(frozen, candidates(), cx));
    let (events, _sub) = s.record::<PickEvent>();
    s.click(600., 380.);
    assert!(events.borrow().is_empty());
}

#[test]
fn when_the_window_cannot_be_captured_alone_its_rectangle_is_cut_from_the_frozen_frame() {
    let frozen = frame(W, H, 1.5, grey);
    let local = Rect::new(300., 60., 460., 200.);
    let cut = window_frame(Err("no toplevel capture"), &frozen, Some(local)).expect("cropped");
    assert_eq!(cut.size_px, (240, 210));
    assert_eq!(cut.logical.size(), local.size());

    let own = frame(10., 10., 1., grey);
    let kept = window_frame::<&str>(Ok((*own).clone()), &frozen, Some(local)).expect("own");
    assert_eq!(kept.size_px, (10, 10), "the window's own pixels win");

    assert!(
        window_frame(Err("no toplevel capture"), &frozen, None).is_err(),
        "with no geometry there is nothing to crop"
    );
}

#[test]
fn without_geometry_a_list_of_windows_is_chosen_by_keyboard_or_pointer() {
    let mut all = snapshot();
    for w in &mut all {
        w.logical = None;
    }
    all[0].title = "A very long window title that keeps going well past the width of the list so it has to be cut short".into();
    let items = windows::listed(&all, std::process::id());
    let order: Vec<&str> = items.iter().map(|c| c.id.0.as_str()).collect();
    assert_eq!(order, ["elsewhere", "floating", "front", "back", "hidden"]);

    let frozen = frame(W, H, 2., grey);
    let mut s = Surface::open(W, H, |cx| WindowPicker::new(&frozen, Some(items), cx));
    let (events, _sub) = s.record::<PickEvent>();
    s.key("down");
    let shot = s.app.screenshot("window-picker");
    let x = W / 2. - 150.;
    let rows: Vec<f32> = (0..H as u32 * 2)
        .map(|y| y as f32 / 2.)
        .filter(|&y| bluish(rgb(&shot, x, y)))
        .collect();
    let (top, bottom) = (rows.first().copied(), rows.last().copied());
    let (Some(top), Some(bottom)) = (top, bottom) else {
        panic!("the selected row is drawn in the accent");
    };
    assert!(
        (26.0..=34.0).contains(&(bottom - top)),
        "one row is selected"
    );
    s.click(x, (top + bottom) / 2. + 32.);
    s.key("up");
    s.key("enter");
    s.key("space");
    s.key("escape");
    let picked = |id: &str| PickEvent::Window {
        id: WindowId(id.into()),
        local: None,
    };
    assert_eq!(
        *events.borrow(),
        vec![
            picked("front"),
            picked("floating"),
            PickEvent::Region,
            PickEvent::Cancel
        ]
    );
}
