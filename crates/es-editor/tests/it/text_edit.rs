#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;
use crate::interaction_support;
use crate::text_edit_support;

use canvas_support::*;
use es_anno::document::page_transform;
use es_anno::draw::draw_items;
use es_anno::{BuildContext, build_draw_list};
use es_editor::canvas::CARET_BLINK;
use es_geom::{Affine, PVec};
use es_model::{ShapeKind, TextAlign, Tool};
use es_render::paint::{ClipShape, paint_draw_list};
use es_ui::tokens::ThemeMode;
use interaction_support::*;
use std::f64::consts::FRAC_PI_6;
use std::time::Duration;
use text_edit_support::*;
use tiny_skia::Pixmap;

fn page(shapes: Vec<es_model::AnnoShape>) -> (App, es_editor::EditorSession) {
    open(900.0, 560.0, opened(1200, 640, shapes, Default::default()))
}

fn click(app: &mut App, session: &es_editor::EditorSession, x: f64, y: f64) {
    let at = at_page(app, session, x, y);
    down(app, at);
    up(app, at);
}

#[test]
fn typing_edits_the_shape_and_the_whole_edit_undoes_in_one_step() {
    let (mut app, session) = page(Vec::new());
    set_tool(&mut app, &session, Tool::Text);
    click(&mut app, &session, 300.0, 300.0);
    let id = editing(&app, &session).expect("a click with the text tool starts a text");
    type_text(&mut app, "Hello");
    type_text(&mut app, " world");
    assert_eq!(text_of(&app, &session, id).as_deref(), Some("Hello world"));
    assert_eq!(
        state(&app).selection,
        11..11,
        "the caret follows the typing"
    );
    key(&mut app, "backspace");
    key(&mut app, "enter");
    type_text(&mut app, "x");
    assert_eq!(
        text_of(&app, &session, id).as_deref(),
        Some("Hello worl\nx")
    );
    key(&mut app, "escape");
    assert_eq!(editing(&app, &session), None, "Escape commits");
    assert!(canvas(&app).read_with(&app.cx, |c, cx| !c.is_editing_text(cx)));
    app.cx
        .update(|cx| session.doc.update(cx, |d, cx| d.apply(cx, |e| e.undo())));
    app.settle();
    assert!(
        ids(&app, &session).is_empty(),
        "creating and typing a text is one undo step, so undo removes it"
    );
    app.cx
        .update(|cx| session.doc.update(cx, |d, cx| d.apply(cx, |e| e.redo())));
    app.settle();
    assert_eq!(
        text_of(&app, &session, id).as_deref(),
        Some("Hello worl\nx")
    );
}

#[test]
fn ime_composition_is_shown_marked_and_then_committed() {
    let (mut app, session) = page(Vec::new());
    set_tool(&mut app, &session, Tool::Text);
    click(&mut app, &session, 300.0, 300.0);
    let id = editing(&app, &session).unwrap();
    type_text(&mut app, "a");
    compose(&mut app, "ni");
    assert_eq!(text_of(&app, &session, id).as_deref(), Some("ani"));
    assert_eq!(marked(&mut app), Some(1..3));
    assert_eq!(
        state(&app).marked,
        Some(1..3),
        "the composing text is underlined"
    );
    compose(&mut app, "nih");
    assert_eq!(
        text_of(&app, &session, id).as_deref(),
        Some("anih"),
        "composing replaces itself"
    );
    type_text(&mut app, "你好");
    assert_eq!(text_of(&app, &session, id).as_deref(), Some("a你好"));
    assert_eq!(marked(&mut app), None, "committing ends the composition");
    let sel = selection_utf16(&mut app).unwrap();
    assert_eq!(
        sel.range,
        3..3,
        "the caret sits after the committed text, in UTF-16"
    );
}

#[test]
fn the_candidate_window_follows_a_rotated_text() {
    let mut rotated =
        text_edit_support::text_shape(300.0, 200.0, "Hello world", TextAlign::Start, FRAC_PI_6);
    rotated.mirrored = false;
    let id = rotated.id;
    let (mut app, session) = page(vec![rotated]);
    begin(&mut app, &session, id);
    let got = ime_bounds(&mut app, 5..5);
    let (shape, layout) = session.doc.read_with(&app.cx, |d, _| {
        let s = d.editor().doc().shape(id).cloned().unwrap();
        let ShapeKind::Text(p) = &s.kind else {
            panic!()
        };
        (s.clone(), d.text().layout(p))
    });
    let (line, x) = layout.caret_x(5);
    let lb = &layout.lines[line];
    let local = PVec::new(x, lb.top + lb.height / 2.0);
    let p = page_transform(&shape).apply_to_point(local);
    let want = at_page(&app, &session, p.x, p.y);
    let centre = got.center();
    assert!(
        (centre.x - want.x).abs() < gpui::px(1.0) && (centre.y - want.y).abs() < gpui::px(1.0),
        "the caret's box is centred on the rotated caret: got {centre:?}, want {want:?}"
    );
    assert!(
        got.size.width > gpui::px(10.0),
        "a rotated caret's box is wider than the caret itself"
    );
}

#[test]
fn escape_and_a_click_outside_commit_and_an_empty_text_is_removed() {
    let (mut app, session) = page(Vec::new());
    set_tool(&mut app, &session, Tool::Text);
    click(&mut app, &session, 200.0, 200.0);
    let first = editing(&app, &session).unwrap();
    type_text(&mut app, "kept");
    click(&mut app, &session, 700.0, 450.0);
    let second = editing(&app, &session).expect("the click outside starts another text");
    assert_ne!(first, second);
    assert_eq!(
        text_of(&app, &session, first).as_deref(),
        Some("kept"),
        "the click committed it"
    );
    type_text(&mut app, "  \n ");
    key(&mut app, "escape");
    assert_eq!(editing(&app, &session), None);
    assert_eq!(
        ids(&app, &session),
        vec![first],
        "a blank text is deleted on commit"
    );
    assert!(canvas(&app).read_with(&app.cx, |c, _| c.text_edit_state().is_none()));
}

#[test]
fn a_click_inside_the_text_moves_the_caret_and_keys_move_and_select() {
    let shape = text_edit_support::text_shape(200.0, 200.0, "abcdef", TextAlign::Start, 0.0);
    let id = shape.id;
    let (mut app, session) = page(vec![shape]);
    begin(&mut app, &session, id);
    assert_eq!(state(&app).selection, 6..6, "editing starts at the end");
    key(&mut app, "home");
    assert_eq!(state(&app).selection, 0..0);
    key(&mut app, "shift-right");
    key(&mut app, "shift-right");
    assert_eq!(state(&app).selection, 0..2);
    key(&mut app, "ctrl-c");
    key(&mut app, "end");
    key(&mut app, "ctrl-v");
    assert_eq!(
        text_of(&app, &session, id).as_deref(),
        Some("abcdefab"),
        "copy and paste through the clipboard"
    );
    key(&mut app, "ctrl-a");
    key(&mut app, "ctrl-x");
    assert_eq!(text_of(&app, &session, id).as_deref(), Some(""));
    key(&mut app, "ctrl-v");
    key(&mut app, "left");
    key(&mut app, "delete");
    assert_eq!(text_of(&app, &session, id).as_deref(), Some("abcdefa"));
    let left_edge = at_page(&app, &session, 202.0, 220.0);
    down(&mut app, left_edge);
    up(&mut app, left_edge);
    assert_eq!(
        editing(&app, &session),
        Some(id),
        "a click on the text keeps editing it"
    );
    assert_eq!(
        state(&app).selection,
        0..0,
        "and puts the caret where it landed"
    );
}

#[test]
fn centred_text_keeps_its_anchor_while_it_grows() {
    let shape = text_edit_support::text_shape(400.0, 200.0, "ab", TextAlign::Middle, 0.4);
    let id = shape.id;
    let (mut app, session) = page(vec![shape]);
    let anchor = |app: &App| {
        session.doc.read_with(&app.cx, |d, _| {
            let s = d.editor().doc().shape(id).cloned().unwrap();
            let ShapeKind::Text(p) = &s.kind else {
                panic!()
            };
            let w = d.editor().doc().measure_text(p).width;
            page_transform(&s).apply_to_point(PVec::new(w / 2.0, 0.0))
        })
    };
    let before = anchor(&app);
    begin(&mut app, &session, id);
    type_text(&mut app, "cdefgh");
    let after = anchor(&app);
    assert_eq!(text_of(&app, &session, id).as_deref(), Some("abcdefgh"));
    assert!(
        (before.x - after.x).abs() < 0.01 && (before.y - after.y).abs() < 0.01,
        "the centre anchor stays put: {before:?} -> {after:?}"
    );
}

#[test]
fn the_caret_blinks_every_530_ms_and_stays_solid_while_typing() {
    assert_eq!(CARET_BLINK, Duration::from_millis(530));
    let (mut app, session) = page(Vec::new());
    set_tool(&mut app, &session, Tool::Text);
    click(&mut app, &session, 300.0, 300.0);
    assert!(state(&app).caret_visible);
    app.cx.advance_clock(Duration::from_millis(529));
    app.settle();
    assert!(state(&app).caret_visible, "not yet");
    app.cx.advance_clock(Duration::from_millis(1));
    app.settle();
    assert!(!state(&app).caret_visible, "off at 530 ms");
    app.cx.advance_clock(CARET_BLINK);
    app.settle();
    assert!(state(&app).caret_visible, "on again 530 ms later");
    app.cx.advance_clock(CARET_BLINK);
    app.settle();
    type_text(&mut app, "a");
    assert!(state(&app).caret_visible, "typing shows the caret at once");
    app.cx.advance_clock(Duration::from_millis(300));
    app.settle();
    assert!(
        state(&app).caret_visible,
        "and restarts the blink from the keystroke"
    );
}

#[test]
fn the_canvas_shadow_patch_is_the_export_shadow() {
    let mut shape = text_edit_support::text_shape(100.0, 80.0, "Shadow Qg", TextAlign::Start, 0.3);
    if let ShapeKind::Text(p) = &mut shape.kind {
        p.swatch.alpha = 0.0;
    }
    let id = shape.id;
    let (mut app, session) = page(vec![shape]);
    let scale = 1.5;
    let (exported, patch) = session.doc.read_with(&app.cx, |d, _| {
        let document = d.editor().doc();
        let build = BuildContext::new(&**d.text());
        let (w, h) = (1200.0 * scale, 640.0 * scale);
        let mut target = Pixmap::new(w as u32, h as u32).unwrap();
        let snapshot = target.clone();
        paint_draw_list(
            &build_draw_list(document, &build),
            &mut target,
            Affine::scale(scale),
            ClipShape::page(1200.0, 640.0),
            &snapshot,
        );
        let items = draw_items(document, document.shape(id).unwrap(), &build);
        (
            target,
            es_editor::canvas::text_shadow::build_patch(&items, scale).unwrap(),
        )
    });
    let mut placed = Pixmap::new(exported.width(), exported.height()).unwrap();
    patch.draw_onto(&mut placed);
    let worst = exported
        .data()
        .iter()
        .zip(placed.data())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(
        worst <= 1,
        "preview shadow equals export within one level, worst {worst}"
    );
    let alpha: u64 = exported
        .data()
        .iter()
        .skip(3)
        .step_by(4)
        .map(|a| u64::from(*a))
        .sum();
    assert!(alpha > 10_000, "there is a shadow to compare");

    let c = canvas(&app);
    app.until("the shadow patch", |cx| {
        c.read(cx).text_shadows().held() == 1
    });
    let rect = c
        .read_with(&app.cx, |c, _| c.text_shadows().page_rect(id))
        .unwrap();
    let want = es_geom::Rect::new(
        f64::from(patch.x) / scale,
        f64::from(patch.y) / scale,
        f64::from(patch.x) / scale + f64::from(patch.pixmap.width()) / scale,
        f64::from(patch.y) / scale + f64::from(patch.pixmap.height()) / scale,
    );
    assert!(
        (rect.center().x - want.center().x).abs() < 3.0
            && (rect.center().y - want.center().y).abs() < 3.0,
        "the canvas places its patch where export paints the shadow: {rect:?} vs {want:?}"
    );
}

fn screenshots(mode: ThemeMode, name: &str) {
    let plain = text_edit_support::text_shape(160.0, 120.0, "Edit in place", TextAlign::Start, 0.0);
    let turned =
        text_edit_support::text_shape(560.0, 300.0, "Rotated", TextAlign::Middle, FRAC_PI_6);
    let (a, b) = (plain.id, turned.id);
    let (mut app, session) = page(vec![plain, turned]);
    app.cx.update(|cx| es_ui::theme::set_mode(mode, cx));
    let c = canvas(&app);
    app.until("both shadows", |cx| c.read(cx).text_shadows().held() == 2);
    app.screenshot(&format!("text-edit-shadow-{name}"));
    begin(&mut app, &session, a);
    app.screenshot(&format!("text-edit-caret-{name}"));
    for _ in 0..5 {
        key(&mut app, "shift-left");
    }
    app.screenshot(&format!("text-edit-selection-{name}"));
    begin(&mut app, &session, b);
    compose(&mut app, " kanji");
    app.screenshot(&format!("text-edit-ime-rotated-{name}"));
}

#[test]
fn editing_screenshots_in_light_and_dark() {
    screenshots(ThemeMode::Light, "light");
    screenshots(ThemeMode::Dark, "dark");
}
