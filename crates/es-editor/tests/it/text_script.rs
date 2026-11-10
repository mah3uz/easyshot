#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::*;
use crate::interaction_support::*;
use crate::text_edit_support::*;
use es_anno::{BuildContext, BundledText, build_draw_list};
use es_editor::EditorSession;
use es_geom::Affine;
use es_model::{ShapeKind, Tool};
use es_render::paint::{ClipShape, paint_draw_list};
use es_text::{BUNDLED, FontSet, TextEngine};
use std::sync::Arc;
use tiny_skia::Pixmap;

const BENGALI: &[u8] = include_bytes!("../../../../tests/fixtures/fonts/FallbackBengali.ttf");
const WORD: &str = "বাংলা";

fn with_bengali() -> Arc<BundledText> {
    let fonts = BUNDLED.iter().map(|b| b.to_vec()).chain([BENGALI.to_vec()]);
    Arc::new(BundledText::new(TextEngine::new(FontSet::from_data(fonts))))
}

fn click(app: &mut App, session: &EditorSession, x: f64, y: f64) {
    let at = at_page(app, session, x, y);
    down(app, at);
    up(app, at);
}

fn export(app: &App, session: &EditorSession, other: Option<&BundledText>) -> Pixmap {
    session.doc.read_with(&app.cx, |d, _| {
        let text = other.unwrap_or(d.text());
        let mut target = Pixmap::new(600, 200).unwrap();
        let snapshot = target.clone();
        paint_draw_list(
            &build_draw_list(d.editor().doc(), &BuildContext::new(text)),
            &mut target,
            Affine::IDENTITY,
            ClipShape::page(600.0, 200.0),
            &snapshot,
        );
        target
    })
}

/// Each character arrives on its own, as an input method commits it, and one more is composed first.
fn type_bengali(app: &mut App, session: &EditorSession) -> es_model::AnnoShapeId {
    set_tool(app, session, Tool::Text);
    click(app, session, 60.0, 60.0);
    let id = editing(app, session).expect("the text tool starts a text");
    let mut chars: Vec<char> = WORD.chars().collect();
    let last = chars.pop().unwrap().to_string();
    for c in chars {
        type_text(app, &c.to_string());
    }
    compose(app, &last);
    type_text(app, &last);
    id
}

#[test]
fn bengali_typed_into_the_editor_draws_from_an_installed_font_while_typing_and_in_the_export() {
    let (mut app, session) = open_with_text(
        700.0,
        300.0,
        opened(600, 200, vec![], Default::default()),
        with_bengali(),
    );
    let id = type_bengali(&mut app, &session);
    assert_eq!(text_of(&app, &session, id).as_deref(), Some(WORD));

    let props = session.doc.read_with(&app.cx, |d, _| {
        match &d.editor().doc().shape(id).unwrap().kind {
            ShapeKind::Text(p) => p.clone(),
            _ => unreachable!(),
        }
    });
    let layout = session
        .doc
        .read_with(&app.cx, |d, _| d.text().layout(&props));
    assert!(
        !layout.has_missing_glyphs(),
        "the text being edited has no boxes"
    );

    key(&mut app, "escape");
    assert_eq!(editing(&app, &session), None);
    let drawn = export(&app, &session, None);
    let tofu = export(&app, &session, Some(&BundledText::bundled()));
    assert!(
        drawn.data().iter().any(|&b| b != 0),
        "the export draws the text"
    );
    assert_ne!(
        drawn.data(),
        tofu.data(),
        "the export draws Bengali glyphs, not the boxes the bundled faces alone give"
    );
}
