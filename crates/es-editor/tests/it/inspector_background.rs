#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support;

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use canvas_support::{TEXT, every_kind, opened};
use es_editor::{
    EditorCanvas, EditorSession, EditorWindow,
    inspector::{
        Section,
        background::{
            BackgroundBody, Library, WallpaperChooser, WallpaperTab,
            wallpaper_cache::{Source, Thumb},
        },
    },
    session::look::{LookEvent, LookModel},
};
use es_model::{
    BackgroundSettings, BackgroundStyle,
    background::{Alignment, AspectRatio, BackgroundColor},
};
use es_render::beautify::GradientPreset;
use es_store::Paths;
use es_testkit::Headless;
use es_ui::{components::SliderEvent, tokens::ThemeMode};
use gpui::{App, AppContext, Entity, Task, hsla};
use gpui_component::Root;
use tempfile::TempDir;

struct FakeChooser(Option<PathBuf>);

impl WallpaperChooser for FakeChooser {
    fn choose(&self, _: &mut App) -> Task<Option<PathBuf>> {
        Task::ready(self.0.clone())
    }
}

struct Rig {
    app: canvas_support::App,
    canvas: Entity<EditorCanvas>,
    look: Entity<LookModel>,
    body: Entity<BackgroundBody>,
    paths: Paths,
    _dir: TempDir,
}

fn rig_in(dir: TempDir, look: BackgroundSettings) -> Rig {
    let paths = Paths::rooted(dir.path());
    let (mut app, session) = canvas_support::open(800.0, 600.0, opened(400, 300, vec![], look));
    let canvas = canvas_support::canvas(&app);
    let (look, body) = app.cx.update(|cx| {
        let look = cx.new(|cx| LookModel::new(session.doc.clone(), Some(paths.clone()), cx));
        LookModel::drive_canvas(&look, &canvas, cx).detach();
        let body = cx.new(|cx| BackgroundBody::new(look.clone(), cx));
        (look, body)
    });
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            body.update(cx, |b, cx| b.ensure_controls(window, cx))
        })
        .unwrap();
    Rig {
        app,
        canvas,
        look,
        body,
        paths,
        _dir: dir,
    }
}

fn rig(look: BackgroundSettings) -> Rig {
    rig_in(tempfile::tempdir().unwrap(), look)
}

fn png(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 120, 200, 255]))
        .save(&path)
        .unwrap();
    path
}

#[test]
fn colour_gradient_and_built_in_tiles_set_the_look_and_the_canvas() {
    let mut r = rig(BackgroundSettings::default());
    let (body, look, canvas) = (r.body.clone(), r.look.clone(), r.canvas.clone());
    let wall_3 = r.paths.wallpapers_dir().join("wall-3.jpg");
    r.app.cx.update(|cx| {
        body.update(cx, |b, cx| b.pick_color(3, cx));
        let red = BackgroundColor::plain_presets()[3].clone();
        assert_eq!(
            look.read(cx).settings().style,
            BackgroundStyle::Solid(red.clone())
        );
        assert_eq!(
            canvas.read(cx).background().style,
            BackgroundStyle::Solid(red)
        );
        assert_eq!(
            body.read(cx).library(),
            Library::Color,
            "the library follows the look"
        );

        body.update(cx, |b, cx| b.pick_gradient(1, cx));
        let peach = BackgroundStyle::Gradient(GradientPreset::by_id("soft-peach").unwrap().style());
        assert_eq!(canvas.read(cx).background().style, peach);
        assert_eq!(body.read(cx).library(), Library::Gradient);

        body.update(cx, |b, cx| b.pick_built_in(2, cx));
        let style = BackgroundStyle::Wallpaper {
            path: wall_3.clone(),
        };
        assert_eq!(canvas.read(cx).background().style, style);
        assert_eq!(body.read(cx).wallpaper_tab(), WallpaperTab::BuiltIn);

        body.update(cx, |b, cx| b.pick_none(cx));
        assert_eq!(canvas.read(cx).background().style, BackgroundStyle::None);
    });
    let bytes = std::fs::read(&wall_3).expect("the built-in is written where the look points");
    assert!(image::load_from_memory(&bytes).is_ok());
}

#[test]
fn the_custom_colour_becomes_a_solid_background() {
    let mut r = rig(BackgroundSettings::default());
    let (body, canvas) = (r.body.clone(), r.canvas.clone());
    r.app.cx.update(|cx| {
        body.update(cx, |b, cx| b.pick_custom(hsla(0., 1., 0.5, 1.), cx));
        let BackgroundStyle::Solid(c) = &canvas.read(cx).background().style else {
            panic!("a solid background");
        };
        assert_eq!(c.id, "custom");
        assert!((c.red - 1.0).abs() < 1e-3 && c.green.abs() < 1e-3 && c.blue.abs() < 1e-3);
    });
}

#[test]
fn a_chosen_wallpaper_joins_recent_persists_and_shows_as_missing_once_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let picked = png(dir.path(), "beach.png");
    let mut r = rig_in(dir, BackgroundSettings::default());
    let (body, canvas) = (r.body.clone(), r.canvas.clone());
    r.app.cx.update(|cx| {
        body.update(cx, |b, cx| {
            b.set_chooser(Rc::new(FakeChooser(Some(picked.clone()))));
            b.choose_wallpaper(cx).detach();
        })
    });
    r.app.settle();
    r.app.cx.update(|cx| {
        let style = BackgroundStyle::Wallpaper {
            path: picked.clone(),
        };
        assert_eq!(canvas.read(cx).background().style, style);
        assert_eq!(body.read(cx).shown_recent(), std::slice::from_ref(&picked));
        assert_eq!(body.read(cx).wallpaper_tab(), WallpaperTab::Recent);
    });
    let state = es_store::state::open(&r.paths).unwrap();
    assert_eq!(state.value.recent_wallpapers, std::slice::from_ref(&picked));

    std::fs::remove_file(&picked).unwrap();
    let look = r.look.clone();
    let again = r
        .app
        .cx
        .update(|cx| cx.new(|cx| BackgroundBody::new(look, cx)));
    let cache = r.app.cx.update(|cx| {
        assert_eq!(
            again.read(cx).shown_recent(),
            std::slice::from_ref(&picked),
            "read back from state.json"
        );
        again.read(cx).cache().clone()
    });
    let source = Source::File(picked.clone());
    r.app
        .cx
        .update(|cx| cache.update(cx, |c, cx| c.request([source.clone()], cx)));
    r.app
        .until("the thumbnail", |cx| cache.read(cx).is_ready(&source));
    r.app.cx.update(|cx| {
        assert!(matches!(cache.read(cx).get(&source), Thumb::Missing));
    });
}

#[test]
fn a_dismissed_chooser_changes_nothing() {
    let mut r = rig(BackgroundSettings::default());
    let body = r.body.clone();
    r.app.cx.update(|cx| {
        body.update(cx, |b, cx| {
            b.set_chooser(Rc::new(FakeChooser(None)));
            b.choose_wallpaper(cx).detach();
        })
    });
    r.app.settle();
    r.app.cx.update(|cx| {
        assert!(body.read(cx).shown_recent().is_empty());
        assert_eq!(r.look.read(cx).settings().style, BackgroundStyle::None);
    });
}

#[test]
fn layout_sliders_change_the_look_and_bracket_each_drag() {
    let mut r = rig(BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-mint").unwrap().style()),
        ..BackgroundSettings::default()
    });
    let seen: Rc<RefCell<Vec<LookEvent>>> = Rc::default();
    let sink = seen.clone();
    let (body, look, canvas) = (r.body.clone(), r.look.clone(), r.canvas.clone());
    r.app.cx.update(|cx| {
        cx.subscribe(&look, move |_, e: &LookEvent, _| sink.borrow_mut().push(*e))
            .detach();
        let [padding, shadow, corners] = body.read(cx).sliders().unwrap();
        for (slider, value) in [(&padding, 0.2), (&shadow, 0.6), (&corners, 0.05)] {
            for event in [
                SliderEvent::DragStarted,
                SliderEvent::Changed(value),
                SliderEvent::DragEnded,
            ] {
                slider.update(cx, |_, cx| cx.emit(event));
            }
        }
        let s = canvas.read(cx).background().clone();
        assert_eq!((s.padding, s.shadow, s.corner_radius), (0.2, 0.6, 0.05));
    });
    let one_drag = [
        LookEvent::LiveAdjust(true),
        LookEvent::Changed,
        LookEvent::LiveAdjust(false),
    ];
    assert_eq!(*seen.borrow(), one_drag.repeat(3));
}

#[test]
fn the_alignment_pad_is_off_while_the_camera_frames_the_card() {
    let mut look = BackgroundSettings::default();
    look.camera.tilt_x_degrees = 15.0;
    let mut r = rig(look);
    let (body, look) = (r.body.clone(), r.look.clone());
    r.app.cx.update(|cx| {
        assert!(!body.read(cx).alignment_enabled(cx));
        body.update(cx, |b, cx| b.set_alignment(Alignment::Top, cx));
        assert_eq!(look.read(cx).settings().alignment, Alignment::Center);

        look.update(cx, |l, cx| l.reset(Section::Camera, cx));
        assert!(body.read(cx).alignment_enabled(cx));
        body.update(cx, |b, cx| b.set_alignment(Alignment::Top, cx));
        assert_eq!(look.read(cx).settings().alignment, Alignment::Top);
    });
}

fn canvas_aspect(s: &BackgroundSettings) -> f64 {
    let l = es_render::beautify::layout::layout(400.0, 300.0, s);
    l.canvas_width / l.canvas_height
}

#[test]
fn a_canvas_shape_on_a_padless_background_turns_padding_on_so_the_shape_shows() {
    let mut r = rig(BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-mint").unwrap().style()),
        padding: 0.0,
        ..BackgroundSettings::default()
    });
    let (body, canvas) = (r.body.clone(), r.canvas.clone());
    r.app.cx.update(|cx| {
        assert_eq!(canvas_aspect(canvas.read(cx).background()), 4.0 / 3.0);

        body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::Square, cx));
        let s = canvas.read(cx).background().clone();
        assert_eq!(s.aspect_ratio, AspectRatio::Square);
        assert_eq!(s.padding, BackgroundSettings::default().padding);
        assert_eq!(canvas_aspect(&s), 1.0);

        body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::SixteenNine, cx));
        assert_eq!(canvas.read(cx).background().padding, 0.08);
        assert_eq!(canvas_aspect(canvas.read(cx).background()), 16.0 / 9.0);
    });
}

#[test]
fn a_canvas_shape_keeps_chosen_padding_and_free_never_adds_any() {
    let mut r = rig(BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-mint").unwrap().style()),
        padding: 0.2,
        ..BackgroundSettings::default()
    });
    let (body, look) = (r.body.clone(), r.look.clone());
    r.app.cx.update(|cx| {
        body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::ThreeTwo, cx));
        assert_eq!(look.read(cx).settings().padding, 0.2);

        look.update(cx, |l, cx| l.update(|s| s.padding = 0.0, cx));
        body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::Auto, cx));
        let s = look.read(cx).settings();
        assert_eq!((s.aspect_ratio, s.padding), (AspectRatio::Auto, 0.0));
    });
}

#[test]
fn without_a_stage_the_canvas_shape_is_off() {
    let mut r = rig(BackgroundSettings::default());
    let (body, look) = (r.body.clone(), r.look.clone());
    r.app.cx.update(|cx| {
        assert!(!body.read(cx).shape_enabled(cx));
        body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::Square, cx));
        assert_eq!(look.read(cx).settings().aspect_ratio, AspectRatio::Auto);
    });
}

fn window(mode: ThemeMode) -> (Headless<Root>, Entity<EditorWindow>) {
    let look = BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-blush").unwrap().style()),
        padding: 0.11,
        shadow: 0.3,
        corner_radius: 0.02,
        ..BackgroundSettings::default()
    };
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
fn screenshots_background_section() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let (mut app, editor) = window(mode);
        let body = app.cx.update(|cx| {
            let inspector = editor.read(cx).inspector().unwrap();
            let view = inspector
                .read(cx)
                .body(Section::Background)
                .unwrap()
                .clone();
            view.downcast::<BackgroundBody>().unwrap()
        });
        app.settle();
        app.screenshot(&format!("background-gradients-{tag}"));

        app.cx
            .update(|cx| body.update(cx, |b, cx| b.set_aspect_ratio(AspectRatio::SixteenNine, cx)));
        app.settle();
        app.screenshot(&format!("background-canvas-shape-{tag}"));

        app.cx.update(|cx| {
            body.update(cx, |b, cx| {
                b.set_library(Library::Wallpaper, cx);
                b.set_wallpaper_tab(WallpaperTab::BuiltIn, cx);
            })
        });
        app.settle();
        let cache = app.cx.update(|cx| body.read(cx).cache().clone());
        app.until("the built-in thumbnails", |cx| {
            (0..7).all(|i| cache.read(cx).is_ready(&Source::BuiltIn(i)))
        });
        app.screenshot(&format!("background-wallpapers-{tag}"));

        app.cx
            .update(|cx| body.update(cx, |b, cx| b.set_library(Library::Color, cx)));
        app.settle();
        app.screenshot(&format!("background-colors-{tag}"));
    }
}
