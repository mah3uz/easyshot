#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;

use std::{cell::RefCell, rc::Rc};

use canvas_support::{TEXT, every_kind, opened, rect};
use es_editor::{
    EditorCanvas, EditorSession, EditorWindow,
    inspector::{Inspector, Section},
    session::look::{LookEvent, LookModel, PresetError},
};
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::GradientPreset;
use es_store::Paths;
use es_testkit::Headless;
use es_ui::{
    components::{InspectorSlider, SliderEvent, ValueFormat},
    tokens::ThemeMode,
};
use gpui::{AppContext, Entity};
use gpui_component::Root;
use tempfile::TempDir;

struct Rig {
    app: canvas_support::App,
    session: EditorSession,
    canvas: Entity<EditorCanvas>,
    look: Entity<LookModel>,
    inspector: Entity<Inspector>,
    paths: Paths,
    _dir: TempDir,
}

fn gradient() -> BackgroundStyle {
    GradientPreset::by_id("soft-lilac")
        .map(|g| BackgroundStyle::Gradient(g.style()))
        .expect("a bundled gradient")
}

fn rig_in(dir: TempDir, look: BackgroundSettings) -> Rig {
    let paths = Paths::rooted(dir.path());
    let (mut app, session) = canvas_support::open(
        800.0,
        600.0,
        opened(400, 300, vec![rect(10.0, 10.0, 50.0, 40.0)], look),
    );
    let canvas = canvas_support::canvas(&app);
    let (look, inspector) = app.cx.update(|cx| {
        let look = cx.new(|cx| LookModel::new(session.doc.clone(), Some(paths.clone()), cx));
        let inspector = cx.new(|cx| Inspector::new(look.clone(), &canvas, cx));
        (look, inspector)
    });
    Rig {
        app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir: dir,
    }
}

fn rig(look: BackgroundSettings) -> Rig {
    rig_in(tempfile::tempdir().unwrap(), look)
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

#[test]
fn a_reset_x_restores_the_default_and_the_canvas_and_document_follow() {
    let mut look = BackgroundSettings {
        style: gradient(),
        ..BackgroundSettings::default()
    };
    look.camera.tilt_x_degrees = 20.0;
    let r = rig(look);
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        assert!(!look.read(cx).is_default(Section::Background));
        assert!(!look.read(cx).is_default(Section::Camera));

        inspector.update(cx, |i, cx| i.reset(Section::Camera, cx));
        let defaults = BackgroundSettings::default();
        assert_eq!(look.read(cx).settings().camera, defaults.camera);
        assert_eq!(canvas.read(cx).background().camera, defaults.camera);
        assert_eq!(
            canvas.read(cx).background().style,
            gradient(),
            "only the camera reset"
        );

        inspector.update(cx, |i, cx| i.reset(Section::Background, cx));
        assert_eq!(canvas.read(cx).background().style, BackgroundStyle::None);
        assert_eq!(
            session.doc.read(cx).background(),
            look.read(cx).settings(),
            "a save keeps the look"
        );
        assert!(look.read(cx).is_default(Section::Background));
        assert!(
            !session.doc.read(cx).editor().can_undo(),
            "beautify changes are not undoable"
        );
    });
}

#[test]
fn the_blur_and_border_switches_turn_effects_on_and_open_their_section() {
    let r = rig(BackgroundSettings::default());
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        assert!(!inspector.read(cx).is_expanded(Section::Border));

        inspector.update(cx, |i, cx| i.set_enabled(Section::Border, true, cx));
        assert!(canvas.read(cx).background().border.is_enabled);
        assert!(inspector.read(cx).is_expanded(Section::Border));
        assert!(
            !look.read(cx).is_default(Section::Border),
            "the reset X shows"
        );

        inspector.update(cx, |i, cx| i.set_enabled(Section::Blur, true, cx));
        assert!(canvas.read(cx).background().progressive_blur.is_enabled);
        inspector.update(cx, |i, cx| i.set_enabled(Section::Blur, false, cx));
        assert!(!canvas.read(cx).background().progressive_blur.is_enabled);
        assert!(
            inspector.read(cx).is_expanded(Section::Blur),
            "switching off keeps it open"
        );
    });
}

#[test]
fn expanded_sections_round_trip_through_state_json() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let r = rig_in(dir, BackgroundSettings::default());
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        assert!(
            inspector.read(cx).is_expanded(Section::Background),
            "background opens by default"
        );
        inspector.update(cx, |i, cx| {
            i.set_expanded(Section::Background, false, cx);
            i.set_expanded(Section::Watermark, true, cx);
        });

        let state = es_store::state::open(&Paths::rooted(&root)).unwrap();
        assert_eq!(
            state.value.inspector_sections.get("background"),
            Some(&false)
        );
        assert_eq!(state.value.inspector_sections.get("watermark"), Some(&true));

        let look = look.clone();
        let canvas = canvas.clone();
        let reopened = cx.new(|cx| Inspector::new(look, &canvas, cx));
        assert!(!reopened.read(cx).is_expanded(Section::Background));
        assert!(reopened.read(cx).is_expanded(Section::Watermark));
        assert!(!reopened.read(cx).is_expanded(Section::Camera));
    });
}

#[test]
fn presets_apply_add_and_delete_round_trip_through_the_preset_file() {
    let r = rig(BackgroundSettings {
        style: gradient(),
        padding: 0.2,
        ..BackgroundSettings::default()
    });
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        let library = || es_store::presets::open_backgrounds(&paths).unwrap().value;

        let ocean = look
            .update(cx, |l, cx| l.add_preset("  Ocean   blue ", cx))
            .unwrap();
        assert_eq!(library().presets[0].name, "Ocean blue");
        assert_eq!(
            library().active_preset_id,
            Some(ocean),
            "saving makes it the default"
        );
        assert_eq!(look.read(cx).applied(), Some(ocean));
        assert_eq!(
            look.update(cx, |l, cx| l.add_preset("OCEAN BLUE", cx)),
            Err(PresetError::Duplicate)
        );
        assert_eq!(library().presets.len(), 1);

        look.update(cx, |l, cx| l.reset(Section::Background, cx));
        assert!(look.read(cx).modified());
        let plain = look.update(cx, |l, cx| l.add_preset("Plain", cx)).unwrap();

        look.update(cx, |l, cx| l.apply_preset(Some(ocean), cx));
        assert_eq!(canvas.read(cx).background().style, gradient());
        assert_eq!(canvas.read(cx).background().padding, 0.2);
        assert!(!look.read(cx).modified());

        look.update(cx, |l, cx| l.set_default_preset(None, cx));
        assert_eq!(library().active_preset_id, None);

        look.update(cx, |l, cx| l.delete_preset(ocean, cx));
        let names: Vec<_> = library().presets.into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Plain"]);
        assert_eq!(look.read(cx).applied(), None);
        assert_eq!(
            canvas.read(cx).background().style,
            gradient(),
            "the canvas keeps its look"
        );
        assert!(library().get(plain).is_some());
    });
}

#[test]
fn a_missing_wallpaper_cannot_be_saved_as_a_preset() {
    let r = rig(BackgroundSettings {
        style: BackgroundStyle::Wallpaper {
            path: "/nonexistent/wall.png".into(),
        },
        ..BackgroundSettings::default()
    });
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        assert_eq!(
            look.update(cx, |l, cx| l.add_preset("Wall", cx)),
            Err(PresetError::WallpaperMissing)
        );
        assert!(!paths.bg_presets_file().exists());
    });
}

#[test]
fn a_slider_drag_is_bracketed_by_one_live_adjustment() {
    let mut r = rig(BackgroundSettings::default());
    let seen = record(&mut r);
    let handle = r.app.window.into();
    let slider = r
        .app
        .cx
        .update_window(handle, |_, window, cx| {
            cx.new(|cx| {
                InspectorSlider::new(
                    "Padding",
                    0.08,
                    0.0..=0.5,
                    ValueFormat::percent(),
                    window,
                    cx,
                )
            })
        })
        .unwrap();
    let Rig {
        mut app,
        session,
        canvas,
        look,
        inspector,
        paths,
        _dir,
    } = r;
    let _ = (&session, &canvas, &look, &inspector, &paths);
    app.cx.update(|cx| {
        LookModel::bind_slider(&look, &slider, |s, v| s.padding = v, cx).detach();
        for event in [
            SliderEvent::DragStarted,
            SliderEvent::Changed(0.2),
            SliderEvent::Changed(0.3),
            SliderEvent::DragEnded,
        ] {
            slider.update(cx, |_, cx| cx.emit(event));
        }
        assert_eq!(
            *seen.borrow(),
            [
                LookEvent::LiveAdjust(true),
                LookEvent::Changed,
                LookEvent::Changed,
                LookEvent::LiveAdjust(false),
            ]
        );
        assert_eq!(canvas.read(cx).background().padding, 0.3);
    });
}

fn window(mode: ThemeMode) -> (Headless<Root>, Entity<EditorWindow>) {
    let mut look = BackgroundSettings {
        style: gradient(),
        ..BackgroundSettings::default()
    };
    look.camera.tilt_y_degrees = 12.0;
    look.border.is_enabled = true;
    let doc = opened(1200, 640, every_kind(false, true), look);
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::components::gallery::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        1080.0,
        720.0,
        move |window, cx| {
            let editor =
                cx.new(|cx| EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx));
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot.borrow_mut().take().unwrap();
    (app, editor)
}

#[test]
fn screenshots_inspector_dark_and_light() {
    for (mode, name) in [
        (ThemeMode::Dark, "inspector-dark"),
        (ThemeMode::Light, "inspector-light"),
    ] {
        let (mut app, editor) = window(mode);
        let inspector = app
            .cx
            .update(|cx| editor.read(cx).inspector())
            .expect("the window has an inspector");
        app.cx
            .update(|cx| inspector.update(cx, |i, cx| i.set_expanded(Section::Border, true, cx)));
        app.settle();
        app.screenshot(name);
    }
}

#[test]
fn screenshot_new_preset_duplicate_error() {
    let (mut app, editor) = window(ThemeMode::Dark);
    let inspector = app.cx.update(|cx| editor.read(cx).inspector()).unwrap();
    let presets = app.cx.update(|cx| inspector.read(cx).presets().clone());
    let look = app.cx.update(|cx| inspector.read(cx).look().clone());
    app.cx
        .update(|cx| look.update(cx, |l, cx| l.add_preset("Ocean", cx)))
        .unwrap();
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            presets.update(cx, |p, cx| {
                p.open_new_preset(window, cx);
                p.set_name("ocean", window, cx);
                p.save(window, cx);
            });
        })
        .unwrap();
    app.settle();
    let error = app.cx.update(|cx| presets.read(cx).error());
    assert_eq!(error, Some(PresetError::Duplicate));
    assert!(
        app.cx.update(|cx| presets.read(cx).is_adding()),
        "the form stays open"
    );
    app.cx.advance_clock(std::time::Duration::from_millis(400));
    app.settle();
    app.screenshot("inspector-new-preset");
}
