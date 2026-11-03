use es_geom::Rect;
use es_model::platform::{Frame, OutputId, Transform};
use es_overlays::capture::{RegionSelector, SelectorEvent};
use es_testkit::Headless;
use gpui::{
    AppContext as _, Entity, KeyDownEvent, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, Subscription, point, px,
};
use image::RgbaImage;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

const W: f32 = 640.;
const H: f32 = 400.;
// Headless windows render at scale 2, so the frozen frame is 2 px per point, as on a 2× output.
const SF: u32 = 2;

fn frame() -> Arc<Frame> {
    let (w, h) = (W as u32 * SF, H as u32 * SF);
    let mut bgra = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let light = ((x / 80) + (y / 80)) % 2 == 0;
            let (b, g, r) = if light { (214, 168, 92) } else { (120, 74, 40) };
            bgra.extend_from_slice(&[b, g, r, 255]);
        }
    }
    Arc::new(Frame {
        output: Some(OutputId("HEADLESS-1".into())),
        size_px: (w, h),
        logical: Rect::new(0., 0., f64::from(W), f64::from(H)),
        scale: f64::from(SF),
        bgra: Arc::from(bgra),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    })
}

struct Selector {
    app: Headless<RegionSelector>,
    view: Entity<RegionSelector>,
    events: Rc<RefCell<Vec<SelectorEvent>>>,
    _sub: Subscription,
}

fn open(preselect: Option<Rect>, capture_on_release: bool) -> Selector {
    let frozen = frame();
    let mut view = None;
    let mut app = Headless::open(es_ui::assets::Assets, es_ui::init, W, H, |_, cx| {
        let v = cx.new(|cx| RegionSelector::new(frozen, preselect, capture_on_release, cx));
        view = Some(v.clone());
        v
    });
    let view = view.expect("the window built its root");
    let events = Rc::new(RefCell::new(Vec::new()));
    let seen = events.clone();
    let sub = app.cx.update(|cx| {
        cx.subscribe(&view, move |_, e: &SelectorEvent, _| {
            seen.borrow_mut().push(*e)
        })
    });
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.activate_window())
        .expect("window is open");
    app.settle();
    Selector {
        app,
        view,
        events,
        _sub: sub,
    }
}

impl Selector {
    fn send(&mut self, event: PlatformInput) {
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .expect("window is open");
        self.app.settle();
    }

    fn down(&mut self, x: f32, y: f32, clicks: usize) {
        self.send(PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Modifiers::default(),
            click_count: clicks,
            first_mouse: false,
        }));
    }

    fn hover(&mut self, x: f32, y: f32, pressed: bool) {
        self.send(PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: pressed.then_some(MouseButton::Left),
            modifiers: Modifiers::default(),
        }));
    }

    fn up(&mut self, x: f32, y: f32, clicks: usize) {
        self.send(PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position: point(px(x), px(y)),
            modifiers: Modifiers::default(),
            click_count: clicks,
        }));
    }

    fn drag(&mut self, from: (f32, f32), to: (f32, f32)) {
        self.hover(from.0, from.1, false);
        self.down(from.0, from.1, 1);
        self.hover((from.0 + to.0) / 2., (from.1 + to.1) / 2., true);
        self.hover(to.0, to.1, true);
        self.up(to.0, to.1, 1);
    }

    fn key(&mut self, key: &'static str) {
        self.send(PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(key).expect("keystroke"),
            is_held: false,
            prefer_character_input: false,
        }));
    }

    fn selection(&mut self) -> Option<Rect> {
        let view = self.view.clone();
        self.app.cx.update(|cx| view.read(cx).selection())
    }

    fn events(&self) -> Vec<SelectorEvent> {
        self.events.borrow().clone()
    }
}

fn rgb(image: &RgbaImage, x: f32, y: f32) -> [u8; 3] {
    let p = image
        .get_pixel((x * SF as f32) as u32, (y * SF as f32) as u32)
        .0;
    [p[0], p[1], p[2]]
}

fn frame_rgb(x: f32, y: f32) -> [u8; 3] {
    let (x, y) = ((x * SF as f32) as u32, (y * SF as f32) as u32);
    if ((x / 80) + (y / 80)) % 2 == 0 {
        [92, 168, 214]
    } else {
        [40, 74, 120]
    }
}

fn near(a: [u8; 3], b: [u8; 3], tolerance: i16) -> bool {
    a.iter()
        .zip(b)
        .all(|(x, y)| (i16::from(*x) - i16::from(y)).abs() <= tolerance)
}

fn dimmed(c: [u8; 3]) -> [u8; 3] {
    c.map(|v| (f32::from(v) * 0.7).round() as u8)
}

#[test]
fn before_any_selection_the_frozen_frame_is_dimmed_and_guides_follow_the_pointer() {
    let mut s = open(None, false);
    s.hover(203., 151., false);
    let shot = s.app.screenshot("region-idle");
    assert!(near(rgb(&shot, 50., 50.), dimmed(frame_rgb(50., 50.)), 3));
    let brighter = |x: u32, y: u32| {
        let p = shot.get_pixel(x, y).0;
        let d = dimmed(frame_rgb(x as f32 / SF as f32, y as f32 / SF as f32));
        (0..3).all(|i| p[i] > d[i] + 20)
    };
    let (gx, gy) = (203 * SF, 151 * SF);
    assert!(
        (gy - 2..=gy + 1).any(|y| brighter(800, y)),
        "a horizontal guide crosses the pointer's row"
    );
    assert!(
        (gx - 2..=gx + 1).any(|x| brighter(x, 600)),
        "a vertical guide crosses the pointer's column"
    );
    assert!(!brighter(800, gy + 12), "the guide is a hairline");
    s.hover(500., 60., false);
    let moved = s.app.screenshot("region-idle-moved");
    assert!(
        !(gy - 2..=gy + 1).any(|y| {
            let p = moved.get_pixel(800, y).0;
            p[0] > dimmed(frame_rgb(400., 151.))[0] + 20
        }),
        "the guide follows the pointer away"
    );
    assert!(s.events().is_empty());
}

#[test]
fn a_drag_selects_and_the_hole_shows_the_frame_undimmed_inside_a_white_border() {
    let mut s = open(None, false);
    s.drag((120., 90.), (420., 250.));
    assert_eq!(s.selection(), Some(Rect::new(120., 90., 420., 250.)));
    assert_eq!(s.events(), vec![SelectorEvent::Began]);
    let shot = s.app.screenshot("region-selection");
    assert!(
        near(rgb(&shot, 200., 170.), frame_rgb(200., 170.), 2),
        "inside is the frame"
    );
    assert!(
        near(rgb(&shot, 60., 170.), dimmed(frame_rgb(60., 170.)), 3),
        "outside is dimmed"
    );
    assert!(
        near(rgb(&shot, 270., 90.), [255, 255, 255], 8),
        "the border is white"
    );
    assert!(
        near(rgb(&shot, 420., 170.), [255, 255, 255], 8),
        "the right handle is white"
    );
}

#[test]
fn a_corner_handle_resizes_and_the_opposite_corner_stays() {
    let mut s = open(None, false);
    s.drag((120., 90.), (420., 250.));
    s.drag((426., 255.), (500., 300.));
    assert_eq!(s.selection(), Some(Rect::new(120., 90., 494., 295.)));
}

#[test]
fn a_drag_inside_a_drawn_selection_moves_it() {
    let mut s = open(None, false);
    s.drag((120., 90.), (420., 250.));
    s.drag((250., 170.), (280., 150.));
    assert_eq!(s.selection(), Some(Rect::new(150., 70., 450., 230.)));
}

#[test]
fn a_double_click_inside_captures_the_selection() {
    let mut s = open(None, false);
    s.drag((120., 90.), (420., 250.));
    s.down(250., 170., 1);
    s.up(250., 170., 1);
    s.down(250., 170., 2);
    let sel = Rect::new(120., 90., 420., 250.);
    assert_eq!(s.events().last(), Some(&SelectorEvent::Capture(sel)));
}

#[test]
fn return_captures_escape_cancels_and_space_asks_for_window_mode() {
    let mut s = open(None, false);
    s.key("enter");
    assert!(s.events().is_empty(), "nothing to capture yet");
    s.drag((120., 90.), (420., 250.));
    s.key("enter");
    s.key("space");
    s.key("escape");
    let sel = Rect::new(120., 90., 420., 250.);
    assert_eq!(
        s.events(),
        vec![
            SelectorEvent::Began,
            SelectorEvent::Capture(sel),
            SelectorEvent::WindowMode,
            SelectorEvent::Cancel
        ]
    );
}

#[test]
fn a_click_with_no_selection_cancels_but_keeps_an_existing_one() {
    let mut s = open(None, false);
    s.drag((120., 90.), (420., 250.));
    s.drag((550., 350.), (551., 351.));
    assert!(!s.events().contains(&SelectorEvent::Cancel));
    assert_eq!(s.selection(), Some(Rect::new(120., 90., 420., 250.)));

    let mut s = open(None, false);
    s.drag((300., 200.), (301., 201.));
    assert_eq!(s.events().last(), Some(&SelectorEvent::Cancel));
}

#[test]
fn the_previous_area_is_preselected_resizable_but_a_drag_inside_draws_anew() {
    let pre = Rect::new(100., 100., 400., 300.);
    let mut s = open(Some(pre), false);
    assert_eq!(s.selection(), Some(pre));
    s.app.screenshot("region-preselected");
    s.drag((400., 300.), (450., 320.));
    assert_eq!(s.selection(), Some(Rect::new(100., 100., 450., 320.)));
    s.drag((200., 150.), (260., 200.));
    assert_eq!(s.selection(), Some(Rect::new(200., 150., 260., 200.)));
}

#[test]
fn with_capture_on_release_letting_go_captures_at_once() {
    let mut s = open(None, true);
    s.drag((120., 90.), (420., 250.));
    let sel = Rect::new(120., 90., 420., 250.);
    assert_eq!(
        s.events(),
        vec![SelectorEvent::Began, SelectorEvent::Capture(sel)]
    );
    assert_eq!(s.selection(), None);
}

#[test]
fn while_drawing_only_the_size_shows_and_another_output_can_clear_the_selection() {
    let mut s = open(None, false);
    s.hover(120., 90., false);
    s.down(120., 90., 1);
    s.hover(320., 200., true);
    s.app.screenshot("region-drawing");
    s.up(320., 200., 1);
    let view = s.view.clone();
    s.app.cx.update(|cx| view.update(cx, |v, cx| v.clear(cx)));
    assert_eq!(s.selection(), None);
}
