#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use canvas_support::{TEXT, opened};
use es_anno::draw::{BuildContext, build_draw_list};
use es_editor::window::redact::{State, toast_text};
use es_editor::{EditorSession, EditorWindow, Opened};
use es_model::{RedactionKind, ShapeKind};
use es_ocr::{OcrOptions, probe, recognize, redact, redact::Kind};
use es_render::encode::OutputFormat;
use es_render::export::export;
use es_testkit::Headless;
use gpui::{AppContext, Entity};
use gpui_component::Root;
use image::RgbaImage;
use tiny_skia::{IntSize, Pixmap};

type App = Headless<Root>;

fn fixture() -> RgbaImage {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../es-ocr/tests/fixtures/secrets.png");
    image::open(path).unwrap().to_rgba8()
}

fn have_tesseract() -> bool {
    let ok = probe(None).available;
    if !ok {
        eprintln!("skipped: tesseract is not installed");
    }
    ok
}

fn launch(base: RgbaImage) -> (App, Entity<EditorWindow>) {
    let doc = Opened {
        base,
        ..opened(10, 10, vec![], Default::default())
    };
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        es_ui::init,
        1080.0,
        720.0,
        move |window, cx| {
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    app.settle();
    let editor = slot.borrow_mut().take().unwrap();
    (app, editor)
}

fn scan(app: &mut App, editor: &Entity<EditorWindow>, kind: RedactionKind) {
    editor.update(&mut app.cx, |w, cx| w.smart_redact(kind, cx));
    let e = editor.clone();
    app.until("the scan to finish", |cx| {
        matches!(e.read(cx).redact_state(), State::Added(_))
    });
}

fn shapes(app: &App, editor: &Entity<EditorWindow>) -> Vec<es_model::AnnoShape> {
    editor.read_with(&app.cx, |w, cx| {
        w.session()
            .unwrap()
            .doc
            .read(cx)
            .editor()
            .doc()
            .shapes()
            .to_vec()
    })
}

fn exported(app: &App, editor: &Entity<EditorWindow>) -> RgbaImage {
    editor.read_with(&app.cx, |w, cx| {
        let doc = w.session().unwrap().doc.read(cx);
        let draw = build_draw_list(doc.editor().doc(), &BuildContext::new(&**doc.text()));
        let (width, height) = doc.base().dimensions();
        let mut data = doc.base().as_raw().clone();
        for px in data.as_chunks_mut::<4>().0 {
            let a = u16::from(px[3]);
            for c in &mut px[..3] {
                *c = ((u16::from(*c) * a + 127) / 255) as u8;
            }
        }
        let base = Pixmap::from_vec(data, IntSize::from_wh(width, height).unwrap()).unwrap();
        let png = export(
            &base,
            &draw,
            doc.background(),
            None,
            OutputFormat::Png(es_render::export::Purpose::Keep.png_profile()),
        )
        .unwrap();
        image::load_from_memory(&png).unwrap().to_rgba8()
    })
}

fn secrets_found(image: &RgbaImage) -> usize {
    let found = recognize(image, &OcrOptions::default()).unwrap();
    let size = es_geom::Size::new(f64::from(image.width()), f64::from(image.height()));
    redact::find(&found.words, size).len()
}

#[test]
fn smart_redaction_covers_the_email_card_and_address_in_one_undo_step() {
    if !have_tesseract() {
        return;
    }
    let (mut app, editor) = launch(fixture());
    scan(&mut app, &editor, RedactionKind::Pixelate);
    let added = shapes(&app, &editor);
    assert_eq!(added.len(), 3, "email, card and address rows");
    assert!(added.iter().all(|s| matches!(
        &s.kind,
        ShapeKind::Redaction(p) if p.kind == RedactionKind::Pixelate
    )));
    assert_eq!(
        editor.read_with(&app.cx, |w, _| w.redact_state()),
        State::Added(3)
    );

    editor.update(&mut app.cx, |w, cx| w.session().unwrap().undo(cx));
    assert!(
        shapes(&app, &editor).is_empty(),
        "one undo removes all three"
    );
}

#[test]
fn the_toast_names_what_was_hidden_and_its_undo_removes_every_added_redaction() {
    if !have_tesseract() {
        return;
    }
    let (mut app, editor) = launch(fixture());
    scan(&mut app, &editor, RedactionKind::Blur);
    let toast = editor
        .read_with(&app.cx, |w, _| w.redact_toast().cloned())
        .expect("a toast after the scan");
    assert_eq!(toast.added.len(), 3);
    assert!(toast.text.starts_with("Hid 3 items: "), "{}", toast.text);
    assert_eq!(shapes(&app, &editor).len(), 3);

    editor.update(&mut app.cx, |w, cx| w.undo_redaction(cx));
    assert!(shapes(&app, &editor).is_empty(), "all three are gone");
    assert!(editor.read_with(&app.cx, |w, _| w.redact_toast().is_none()));
    editor.update(&mut app.cx, |w, cx| w.session().unwrap().undo(cx));
    assert_eq!(shapes(&app, &editor).len(), 3, "the removal is one step");
}

#[test]
fn toast_text_counts_items_and_lists_each_kind_once() {
    assert_eq!(
        toast_text(3, &[Kind::Key, Kind::Email, Kind::Card, Kind::Email]),
        "Hid 3 items: emails, card numbers, keys"
    );
    assert_eq!(toast_text(1, &[Kind::Email]), "Hid 1 item: emails");
    assert_eq!(toast_text(0, &[]), "No sensitive text found.");
}

#[test]
fn the_exported_pixels_no_longer_read_as_secrets() {
    if !have_tesseract() {
        return;
    }
    assert_eq!(
        secrets_found(&fixture()),
        3,
        "the fixture has three secrets"
    );
    for kind in [RedactionKind::Pixelate, RedactionKind::Blur] {
        let (mut app, editor) = launch(fixture());
        scan(&mut app, &editor, kind);
        let out = exported(&app, &editor);
        assert_eq!(secrets_found(&out), 0, "{kind:?} export still reads");
    }
}

#[test]
fn a_page_without_secrets_adds_nothing() {
    if !have_tesseract() {
        return;
    }
    let blank = RgbaImage::from_pixel(400, 200, image::Rgba([255; 4]));
    let (mut app, editor) = launch(blank);
    scan(&mut app, &editor, RedactionKind::Blur);
    assert!(shapes(&app, &editor).is_empty());
    assert_eq!(
        editor.read_with(&app.cx, |w, _| w.redact_state()),
        State::Added(0)
    );
    let toast = editor.read_with(&app.cx, |w, _| w.redact_toast().cloned());
    let toast = toast.expect("a toast even when nothing was found");
    assert_eq!(toast.text, "No sensitive text found.");
    assert!(toast.added.is_empty(), "nothing to undo");
}

#[test]
fn a_missing_tesseract_adds_nothing_and_says_so() {
    let (mut app, editor) = launch(fixture());
    editor.update(&mut app.cx, |w, cx| {
        w.set_ocr_options(
            OcrOptions {
                tesseract_path: Some("/nonexistent/tesseract".into()),
                ..OcrOptions::default()
            },
            cx,
        );
        w.smart_redact(RedactionKind::Blur, cx);
    });
    let e = editor.clone();
    app.until("the scan to fail", |cx| {
        e.read(cx).redact_state() == State::Idle
    });
    assert!(shapes(&app, &editor).is_empty());
    let error = editor.read_with(&app.cx, |w, _| w.error().cloned());
    assert_eq!(
        error.as_deref(),
        Some("Smart Redaction needs tesseract, which is not installed.")
    );
}
