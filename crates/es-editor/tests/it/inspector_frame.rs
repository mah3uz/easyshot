#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::{cell::RefCell, rc::Rc};

use canvas_support::{TEXT, every_kind, opened};
use es_editor::{
    EditorCanvas, EditorSession, EditorWindow,
    inspector::{
        Inspector, Section,
        border::BorderBody,
        watermark::{WatermarkBody, WatermarkSlider},
    },
    session::look::{LookEvent, LookModel},
};
use es_model::{BackgroundSettings, BackgroundStyle, BorderSettings, Swatch, WatermarkSettings};
use es_render::beautify::GradientPreset;
use es_testkit::Headless;
use es_ui::{
    components::{InspectorSlider, SliderEvent, SwatchPick},
    tokens::ThemeMode,
};
use gpui::{
    AppContext, Entity, EntityInputHandler, Focusable, KeyDownEvent, Keystroke, PlatformInput,
};
use gpui_component::Root;

struct Rig {
    app: Headless<Root>,
    inspector: Entity<Inspector>,
    look: Entity<LookModel>,
    canvas: Entity<EditorCanvas>,
    border: Entity<BorderBody>,
    mark: Entity<WatermarkBody>,
}

fn rig(mode: ThemeMode, look: BackgroundSettings) -> Rig {
    let doc = opened(1200, 640, every_kind(false, true), look);
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(mode, cx);
            es_ui::motion::set_reduce_motion(true, cx);
        },
        1080.0,
        900.0,
        move |window, cx| {
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot.borrow_mut().take().unwrap();
    let (inspector, canvas) = app.cx.update(|cx| {
        let e = editor.read(cx);
        (e.inspector().unwrap(), e.canvas().unwrap().clone())
    });
    let (look, border, mark) = app.cx.update(|cx| {
        inspector.update(cx, |i, cx| {
            i.set_expanded(Section::Background, false, cx);
            i.set_expanded(Section::Border, true, cx);
            i.set_expanded(Section::Watermark, true, cx);
        });
        let i = inspector.read(cx);
        let body = |s| i.body(s).cloned().expect("the section has a body");
        (
            i.look().clone(),
            body(Section::Border).downcast::<BorderBody>().unwrap(),
            body(Section::Watermark)
                .downcast::<WatermarkBody>()
                .unwrap(),
        )
    });
    app.settle();
    Rig {
        app,
        inspector,
        look,
        canvas,
        border,
        mark,
    }
}

fn record(rig: &mut Rig) -> Rc<RefCell<Vec<LookEvent>>> {
    let seen: Rc<RefCell<Vec<LookEvent>>> = Rc::default();
    let sink = seen.clone();
    let look = rig.look.clone();
    rig.app.cx.update(|cx| {
        cx.subscribe(&look, move |_, e: &LookEvent, _| sink.borrow_mut().push(*e))
            .detach();
    });
    seen
}

fn drag(rig: &mut Rig, slider: &Entity<InspectorSlider>, to: f64) {
    rig.app.cx.update(|cx| {
        slider.update(cx, |_, cx| {
            cx.emit(SliderEvent::DragStarted);
            cx.emit(SliderEvent::Changed(to));
            cx.emit(SliderEvent::DragEnded);
        })
    });
    rig.app.settle();
}

/// The slider's own range, read back by asking it for values far outside any sensible one.
fn range_of(rig: &mut Rig, slider: &Entity<InspectorSlider>) -> (f64, f64) {
    let handle = rig.app.window.into();
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            slider.update(cx, |s, cx| {
                let was = s.value();
                s.set_value(-1e9, window, cx);
                let lo = s.value();
                s.set_value(1e9, window, cx);
                let hi = s.value();
                s.set_value(was, window, cx);
                (lo, hi)
            })
        })
        .unwrap()
}

fn press_right_on(rig: &mut Rig, slider: &Entity<InspectorSlider>) {
    let handle = rig.app.window.into();
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            slider.read(cx).focus_handle(cx).focus(window, cx)
        })
        .unwrap();
    rig.app.settle();
    press(rig, "right");
}

fn press(rig: &mut Rig, key: &'static str) {
    let handle = rig.app.window.into();
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            window.dispatch_event(
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }),
                cx,
            );
        })
        .unwrap();
    rig.app.settle();
}

fn type_watermark(rig: &mut Rig, text: &str) {
    let handle = rig.app.window.into();
    let mark = rig.mark.clone();
    rig.app
        .cx
        .update_window(handle, |_, window, cx| {
            mark.update(cx, |m, cx| m.begin_editing(window, cx));
            let field = mark.read(cx).field().cloned().unwrap();
            field.update(cx, |f, cx| {
                let len = f.value().encode_utf16().count();
                f.replace_text_in_range(Some(0..len), text, window, cx);
            });
        })
        .unwrap();
    rig.app.settle();
}

fn watermark(rig: &mut Rig) -> WatermarkSettings {
    rig.app
        .cx
        .update(|cx| rig.look.read(cx).settings().watermark.clone())
}

fn border(rig: &mut Rig) -> BorderSettings {
    rig.app
        .cx
        .update(|cx| rig.look.read(cx).settings().border.clone())
}

fn enabled_border() -> BackgroundSettings {
    let mut look = BackgroundSettings::default();
    look.border.is_enabled = true;
    look
}

#[test]
fn border_controls_set_their_own_field_within_range_and_brackets_live_adjust() {
    let mut r = rig(ThemeMode::Dark, enabled_border());
    let (thickness, opacity) = r.app.cx.update(|cx| {
        let b = r.border.read(cx);
        (
            b.thickness().cloned().unwrap(),
            b.opacity().cloned().unwrap(),
        )
    });
    assert_eq!(
        range_of(&mut r, &thickness),
        (0.002, 0.08),
        "0.2% to 8% of the short edge"
    );
    assert_eq!(range_of(&mut r, &opacity), (0.0, 1.0));

    let seen = record(&mut r);
    drag(&mut r, &thickness, 0.05);
    assert_eq!(
        *seen.borrow(),
        [
            LookEvent::LiveAdjust(true),
            LookEvent::Changed,
            LookEvent::LiveAdjust(false)
        ],
        "the canvas draws a fast preview only while the drag lasts"
    );
    drag(&mut r, &opacity, 0.4);
    let b = border(&mut r);
    assert_eq!((b.thickness, b.opacity), (0.05, 0.4));

    let blue = Swatch::palette()
        .iter()
        .position(|s| s.id == "blue")
        .unwrap();
    r.app.cx.update(|cx| {
        r.border
            .update(cx, |b, cx| b.pick_colour(SwatchPick::Preset(blue), cx))
    });
    let b = border(&mut r);
    assert_eq!(b.color.id, "blue");
    let shown = r
        .app
        .cx
        .update(|cx| r.canvas.read(cx).background().border.clone());
    assert_eq!(shown, b, "the canvas draws what the inspector set");

    press_right_on(&mut r, &opacity);
    assert!(
        (border(&mut r).opacity - 0.41).abs() < 1e-9,
        "Right steps by one percent"
    );
}

#[test]
fn border_controls_are_inert_until_the_switch_is_on() {
    let mut r = rig(ThemeMode::Dark, BackgroundSettings::default());
    let opacity = r
        .app
        .cx
        .update(|cx| r.border.read(cx).opacity().cloned().unwrap());
    press_right_on(&mut r, &opacity);
    assert_eq!(
        border(&mut r).opacity,
        1.0,
        "a switched-off border ignores its sliders"
    );
    let thickness = r
        .app
        .cx
        .update(|cx| r.border.read(cx).thickness().cloned().unwrap());
    press_right_on(&mut r, &thickness);
    assert_eq!(border(&mut r).thickness, 0.012);

    r.app.cx.update(|cx| {
        r.inspector
            .update(cx, |i, cx| i.set_enabled(Section::Border, true, cx))
    });
    r.app.settle();
    press_right_on(&mut r, &thickness);
    assert!(
        (border(&mut r).thickness - 0.013).abs() < 1e-9,
        "a tenth of a percent per step"
    );
}

#[test]
fn border_reset_restores_the_defaults_and_the_sliders_follow() {
    let mut r = rig(ThemeMode::Dark, enabled_border());
    let (thickness, opacity) = r.app.cx.update(|cx| {
        let b = r.border.read(cx);
        (
            b.thickness().cloned().unwrap(),
            b.opacity().cloned().unwrap(),
        )
    });
    drag(&mut r, &thickness, 0.07);
    drag(&mut r, &opacity, 0.2);
    r.app.cx.update(|cx| {
        r.border.update(cx, |b, cx| {
            b.pick_colour(SwatchPick::Custom(gpui::hsla(0.3, 0.8, 0.5, 1.0)), cx)
        })
    });
    assert_eq!(border(&mut r).color.id, "custom");

    r.app
        .cx
        .update(|cx| r.inspector.update(cx, |i, cx| i.reset(Section::Border, cx)));
    r.app.settle();
    assert_eq!(border(&mut r), BorderSettings::default());
    let shown = r
        .app
        .cx
        .update(|cx| (thickness.read(cx).value(), opacity.read(cx).value()));
    assert_eq!(shown, (0.012, 1.0), "the sliders show the restored values");
}

#[test]
fn watermark_sliders_set_their_own_field_within_range() {
    let mut look = BackgroundSettings::default();
    look.watermark.text = "Draft".into();
    look.watermark.is_enabled = true;
    let mut r = rig(ThemeMode::Dark, look);
    let slider = |r: &mut Rig, w| {
        r.app
            .cx
            .update(|cx| r.mark.read(cx).slider(w).cloned().unwrap())
    };
    let cases = [
        (WatermarkSlider::Density, (2.0, 10.0), 7.0),
        (WatermarkSlider::Size, (8.0, 160.0), 120.0),
        (WatermarkSlider::Angle, (-90.0, 90.0), -30.0),
        (WatermarkSlider::Opacity, (0.0, 0.75), 0.5),
    ];
    for (which, range, to) in cases {
        let s = slider(&mut r, which);
        assert_eq!(range_of(&mut r, &s), range, "{which:?}");
        let seen = record(&mut r);
        drag(&mut r, &s, to);
        assert_eq!(
            seen.borrow().first(),
            Some(&LookEvent::LiveAdjust(true)),
            "{which:?}"
        );
        assert_eq!(
            seen.borrow().last(),
            Some(&LookEvent::LiveAdjust(false)),
            "{which:?}"
        );
    }
    let w = watermark(&mut r);
    assert_eq!(
        (w.density, w.font_size, w.rotation_degrees, w.opacity),
        (7.0, 120.0, -30.0, 0.5)
    );

    let s = slider(&mut r, WatermarkSlider::Density);
    drag(&mut r, &s, 5.4);
    assert_eq!(watermark(&mut r).density, 5.0, "density counts whole rows");

    r.app.cx.update(|cx| {
        r.mark
            .update(cx, |m, cx| m.pick_colour(SwatchPick::Preset(1), cx))
    });
    assert_eq!(watermark(&mut r).color, Swatch::white().to_rgba());
    let shown = r
        .app
        .cx
        .update(|cx| r.canvas.read(cx).background().watermark.clone());
    assert_eq!(
        shown,
        watermark(&mut r),
        "the canvas draws what the inspector set"
    );
}

#[test]
fn watermark_text_switches_the_watermark_and_its_controls() {
    let mut r = rig(ThemeMode::Dark, BackgroundSettings::default());
    let size = r.app.cx.update(|cx| {
        r.mark
            .read(cx)
            .slider(WatermarkSlider::Size)
            .cloned()
            .unwrap()
    });
    press_right_on(&mut r, &size);
    assert_eq!(
        watermark(&mut r).font_size,
        72.0,
        "no text, so the controls are inert"
    );

    type_watermark(&mut r, "Confidential");
    let w = watermark(&mut r);
    assert_eq!(w.text, "Confidential");
    assert!(w.is_enabled && w.is_visible());
    let shown = r
        .app
        .cx
        .update(|cx| r.canvas.read(cx).background().watermark.clone());
    assert_eq!(shown.text, "Confidential", "the canvas follows the typing");
    press_right_on(&mut r, &size);
    assert_eq!(
        watermark(&mut r).font_size,
        73.0,
        "text enables the controls"
    );

    type_watermark(&mut r, "   ");
    let w = watermark(&mut r);
    assert!(
        !w.is_enabled && !w.is_visible(),
        "blank text turns the watermark off"
    );
    assert!(r.app.cx.update(|cx| r.mark.read(cx).is_editing()));
    press(&mut r, "enter");
    assert!(
        !r.app.cx.update(|cx| r.mark.read(cx).is_editing()),
        "Enter finishes editing"
    );
    press_right_on(&mut r, &size);
    assert_eq!(watermark(&mut r).font_size, 73.0);
}

fn screenshot(mode: ThemeMode, name: &str) {
    let mut look = BackgroundSettings {
        style: GradientPreset::by_id("soft-lilac")
            .map(|g| BackgroundStyle::Gradient(g.style()))
            .unwrap(),
        ..BackgroundSettings::default()
    };
    look.border.is_enabled = true;
    look.border.thickness = 0.02;
    look.watermark.text = "easyshot".into();
    look.watermark.is_enabled = true;
    look.watermark.font_size = 48.0;
    look.watermark.opacity = 0.3;
    let mut r = rig(mode, look);
    for _ in 0..5 {
        r.app
            .cx
            .advance_clock(std::time::Duration::from_millis(400));
        r.app.settle();
    }
    r.app.screenshot(name);
}

#[test]
fn screenshot_border_and_watermark_dark() {
    screenshot(ThemeMode::Dark, "inspector-frame-dark");
}

#[test]
fn screenshot_border_and_watermark_light() {
    screenshot(ThemeMode::Light, "inspector-frame-light");
}
