#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::{cell::RefCell, rc::Rc};

use canvas_support::{App, every_kind, opened};
use es_anno::crop::CropHandle;
use es_editor::{
    EditorCanvas, EditorSession,
    inspector::background::BackgroundBody,
    session::{
        crop::CropTarget,
        look::{LookEvent, LookModel},
    },
};
use es_geom::Point;
use es_model::{
    BackgroundSettings, BackgroundStyle,
    background::{Alignment, AspectRatio},
};
use es_render::beautify::GradientPreset;
use es_store::Paths;
use es_ui::motion::{CROP_COMMIT, PRESET_APPLY, SETTLE, Transition, set_reduce_motion};
use gpui::{AppContext, Entity};
use image::RgbaImage;
use tempfile::TempDir;

struct Rig {
    app: App,
    session: EditorSession,
    canvas: Entity<EditorCanvas>,
    look: Entity<LookModel>,
    body: Entity<BackgroundBody>,
    seen: Rc<RefCell<Vec<LookEvent>>>,
    _dir: TempDir,
}

fn mint(padding: f64) -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-mint").unwrap().style()),
        padding,
        ..BackgroundSettings::default()
    }
}

fn rig(look: BackgroundSettings) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let (mut app, session) = canvas_support::open(
        900.0,
        600.0,
        opened(1200, 800, every_kind(false, false), look),
    );
    let canvas = canvas_support::canvas(&app);
    let seen: Rc<RefCell<Vec<LookEvent>>> = Rc::default();
    let sink = seen.clone();
    let (look, body) = app.cx.update(|cx| {
        let look = cx.new(|cx| LookModel::new(session.doc.clone(), Some(paths), cx));
        LookModel::drive_canvas(&look, &canvas, cx).detach();
        cx.subscribe(&look, move |_, e: &LookEvent, _| sink.borrow_mut().push(*e))
            .detach();
        let body = cx.new(|cx| BackgroundBody::new(look.clone(), cx));
        (look, body)
    });
    Rig {
        app,
        session,
        canvas,
        look,
        body,
        seen,
        _dir: dir,
    }
}

fn frame(r: &mut Rig, name: &str) -> RgbaImage {
    canvas_support::redraw(&mut r.app);
    r.app.screenshot(name)
}

/// Frames at the start, the middle, just before the end and at the end of `t`.
fn frames(r: &mut Rig, t: Transition, name: &str) -> [RgbaImage; 4] {
    let tick = std::time::Duration::from_millis(1);
    let start = frame(r, &format!("{name}-0-start"));
    r.app.cx.advance_clock(t.duration / 2);
    let mid = frame(r, &format!("{name}-1-mid"));
    r.app.cx.advance_clock(t.duration / 2 - tick * 8);
    let late = frame(r, &format!("{name}-2-late"));
    r.app.cx.advance_clock(tick * 8);
    canvas_support::redraw(&mut r.app);
    let end = frame(r, &format!("{name}-3-end"));
    [start, mid, late, end]
}

/// Rounded card corners may antialias a shade apart depending on the frame they were drawn in.
fn same_but_edges(a: &RgbaImage, b: &RgbaImage) -> bool {
    let off = a
        .pixels()
        .zip(b.pixels())
        .filter(|(p, q)| p.0.iter().zip(q.0).any(|(x, y)| x.abs_diff(y) > 2))
        .count();
    a.dimensions() == b.dimensions() && off * 1000 < a.pixels().len()
}

fn assert_moves_over(frames: &[RgbaImage; 4], settled: &RgbaImage, what: &str) {
    let [start, mid, late, end] = frames;
    assert!(start != mid && mid != end, "{what} draws in motion");
    assert!(
        late != end,
        "{what} is still moving just before its duration"
    );
    assert!(
        same_but_edges(end, settled),
        "{what} arrives at the new state when its duration is up"
    );
}

fn crop_and_apply(r: &mut Rig) {
    let crop = r.session.crop.clone();
    r.app.cx.update(|cx| {
        crop.update(cx, |c, cx| {
            c.begin(cx);
            c.press(CropTarget::Handle(CropHandle::TopLeft), Point::ZERO);
            c.drag_to(Point::new(500.0, 300.0), false, cx);
            c.release(cx);
        })
    });
    r.app.settle();
    frame(r, "crop-commit-before");
    r.app.cx.update(|cx| crop.update(cx, |c, cx| c.apply(cx)));
    r.app.settle();
}

#[test]
fn applying_a_crop_draws_the_kept_region_growing_from_its_rectangle_to_the_fit() {
    let mut r = rig(BackgroundSettings::default());
    crop_and_apply(&mut r);
    let viewport = r.session.viewport.clone();
    assert!(
        r.app.cx.update(|cx| viewport.read(cx).is_fit()),
        "the view is fitted at once; only its drawing moves"
    );
    let moving = frames(&mut r, CROP_COMMIT, "crop-commit");

    let mut still = rig(BackgroundSettings::default());
    still.app.cx.update(|cx| set_reduce_motion(true, cx));
    crop_and_apply(&mut still);
    let settled = frame(&mut still, "crop-commit-reduced");
    assert_moves_over(&moving, &settled, "the crop commit");
}

fn apply_preset(r: &mut Rig) {
    let look = r.look.clone();
    r.app.cx.update(|cx| {
        let mut wide = mint(0.3);
        wide.corner_radius = 0.06;
        wide.shadow = 0.9;
        wide.aspect_ratio = AspectRatio::Square;
        look.update(cx, |l, cx| l.set(wide, cx));
        let id = look.update(cx, |l, cx| l.add_preset("Wide", cx)).unwrap();
        look.update(cx, |l, cx| l.set(mint(0.04), cx));
        look.update(cx, |l, cx| l.apply_preset(Some(id), cx));
    });
    r.app.settle();
}

#[test]
fn a_preset_draws_its_padding_corners_shadow_and_shape_into_place() {
    let mut r = rig(mint(0.04));
    frame(&mut r, "preset-apply-before");
    apply_preset(&mut r);
    assert!(r.seen.borrow().contains(&LookEvent::Moved(PRESET_APPLY)));
    let canvas = r.canvas.clone();
    assert_eq!(
        r.app.cx.update(|cx| canvas.read(cx).background().padding),
        0.3,
        "the canvas takes the preset at once"
    );
    let moving = frames(&mut r, PRESET_APPLY, "preset-apply");

    let mut still = rig(mint(0.04));
    still.app.cx.update(|cx| set_reduce_motion(true, cx));
    apply_preset(&mut still);
    let settled = frame(&mut still, "preset-apply-reduced");
    assert_moves_over(&moving, &settled, "the preset");
}

#[test]
fn picking_a_shape_or_an_alignment_moves_the_card_and_slider_drags_do_not() {
    let mut r = rig(mint(0.12));
    let body = r.body.clone();
    r.app
        .cx
        .update(|cx| body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::SixteenNine, cx)));
    r.app.settle();
    assert_eq!(
        r.seen.borrow().last(),
        Some(&LookEvent::Moved(PRESET_APPLY))
    );
    r.app.cx.advance_clock(PRESET_APPLY.duration);

    r.app
        .cx
        .update(|cx| body.update(cx, |b, cx| b.set_alignment(Alignment::BottomTrailing, cx)));
    r.app.settle();
    assert_eq!(r.seen.borrow().last(), Some(&LookEvent::Moved(SETTLE)));
    let moving = frames(&mut r, SETTLE, "alignment-settle");
    assert!(moving[0] != moving[3], "the card starts from where it was");

    let look = r.look.clone();
    r.app
        .cx
        .update(|cx| look.update(cx, |l, cx| l.update(|s| s.padding = 0.3, cx)));
    let jumped = frame(&mut r, "slider-change");
    r.app.cx.advance_clock(PRESET_APPLY.duration);
    assert!(
        frame(&mut r, "slider-change-later") == jumped,
        "a slider value shows at once"
    );
}
