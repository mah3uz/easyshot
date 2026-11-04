#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{self, opened};
use crate::inspector_background::window;

use es_editor::{
    EditorCanvas,
    inspector::{Section, background::BackgroundBody},
    session::look::LookModel,
};
use es_model::{BackgroundGradient, BackgroundSettings, BackgroundStyle};
use es_render::beautify::gradients::PRESETS;
use es_store::Paths;
use es_ui::{motion::set_reduce_motion, tokens::ThemeMode};
use gpui::{AppContext, Entity, Window, hsla};
use tempfile::TempDir;

struct Rig {
    app: canvas_support::App,
    canvas: Entity<EditorCanvas>,
    body: Entity<BackgroundBody>,
    paths: Paths,
    _dir: TempDir,
}

fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let look = BackgroundSettings::default();
    let (mut app, session) = canvas_support::open(800.0, 600.0, opened(400, 300, vec![], look));
    let canvas = canvas_support::canvas(&app);
    let body = app.cx.update(|cx| {
        let look = cx.new(|cx| LookModel::new(session.doc.clone(), Some(paths.clone()), cx));
        LookModel::drive_canvas(&look, &canvas, cx).detach();
        cx.new(|cx| BackgroundBody::new(look, cx))
    });
    Rig {
        app,
        canvas,
        body,
        paths,
        _dir: dir,
    }
}

impl Rig {
    fn act(
        &mut self,
        f: impl FnOnce(&mut BackgroundBody, &mut Window, &mut gpui::Context<BackgroundBody>),
    ) {
        let body = self.body.clone();
        self.app
            .cx
            .update_window(self.app.window.into(), |_, window, cx| {
                body.update(cx, |b, cx| f(b, window, cx))
            })
            .unwrap();
    }

    fn canvas_gradient(&mut self) -> BackgroundGradient {
        let canvas = self.canvas.clone();
        self.app
            .cx
            .update(|cx| match &canvas.read(cx).background().style {
                BackgroundStyle::Gradient(g) => g.clone(),
                other => panic!("not a gradient: {other:?}"),
            })
    }

    fn mine(&mut self) -> Vec<BackgroundGradient> {
        let body = self.body.clone();
        self.app
            .cx
            .update(|cx| body.read(cx).user_gradients().to_vec())
    }

    fn stored(&self) -> Vec<BackgroundGradient> {
        es_store::presets::open_gradients(&self.paths)
            .unwrap()
            .value
            .gradients
    }
}

#[test]
fn editing_a_built_in_saves_a_copy_and_leaves_the_built_in_alone() {
    let mut r = rig();
    let violet = PRESETS[0].style();
    r.act(|b, window, cx| {
        b.pick_gradient(0, cx);
        b.edit_gradient(window, cx);
        b.set_stop_color(0, hsla(0., 1., 0.5, 1.), cx);
    });
    let preview = r.canvas_gradient();
    assert!(
        (preview.colors[0].red - 1.0).abs() < 1e-3,
        "the canvas shows the edit live"
    );
    assert!(r.mine().is_empty(), "nothing is stored before Save");
    let id = r
        .app
        .cx
        .update(|cx| r.body.update(cx, |b, cx| b.save_gradient(cx)))
        .unwrap();
    assert_ne!(id, "violet");
    let mine = r.mine();
    assert_eq!(mine.len(), 1);
    assert!(mine[0].is_user());
    assert_eq!(mine[0].title, "Violet Copy");
    assert_eq!(PRESETS[0].style(), violet, "the built-in is fixed");
    assert_eq!(
        r.canvas_gradient().id,
        id,
        "the canvas holds the saved copy"
    );
    assert_eq!(r.stored(), mine, "written through es-store");
}

#[test]
fn a_user_gradient_is_added_changed_in_place_renamed_and_deleted() {
    let mut r = rig();
    r.act(|b, window, cx| {
        b.new_gradient(window, cx);
        b.add_stop(window, cx);
        b.add_stop(window, cx);
        b.add_stop(window, cx);
        b.set_gradient_angle(90.0, window, cx);
        b.set_gradient_name("  Harbour  ", window, cx);
    });
    assert_eq!(r.canvas_gradient().colors.len(), 4, "four colours at most");
    r.app
        .cx
        .update(|cx| r.body.update(cx, |b, cx| b.save_gradient(cx)));
    let first = r.mine()[0].clone();
    assert_eq!((first.title.as_str(), first.colors.len()), ("Harbour", 4));
    assert_eq!(first.angle_degrees(), 90.0);

    r.act(|b, window, cx| {
        b.pick_gradient(PRESETS.len(), cx);
        b.edit_gradient(window, cx);
        b.remove_stop(1, window, cx);
        b.remove_stop(1, window, cx);
        b.remove_stop(1, window, cx);
        b.set_gradient_name("Deep", window, cx);
    });
    r.app
        .cx
        .update(|cx| r.body.update(cx, |b, cx| b.save_gradient(cx)));
    let mine = r.mine();
    assert_eq!(mine.len(), 1, "a user gradient is changed in place");
    assert_eq!(mine[0].id, first.id);
    assert_eq!((mine[0].title.as_str(), mine[0].colors.len()), ("Deep", 2));
    assert_eq!(r.stored(), mine);

    let look = r.canvas_gradient();
    r.app
        .cx
        .update(|cx| r.body.update(cx, |b, cx| b.delete_gradient(&first.id, cx)));
    assert!(r.mine().is_empty() && r.stored().is_empty());
    assert_eq!(r.canvas_gradient(), look, "the canvas keeps its colours");
}

#[test]
fn cancel_puts_the_earlier_background_back_and_stores_nothing() {
    let mut r = rig();
    r.act(|b, window, cx| {
        b.pick_gradient(3, cx);
        b.new_gradient(window, cx);
        b.set_gradient_angle(300.0, window, cx);
    });
    assert!((r.canvas_gradient().angle_degrees() - 300.0).abs() < 1e-6);
    r.app
        .cx
        .update(|cx| r.body.update(cx, |b, cx| b.cancel_gradient(cx)));
    assert_eq!(r.canvas_gradient(), PRESETS[3].style());
    assert!(r.mine().is_empty() && !r.paths.gradients_file().exists());
}

#[test]
fn user_gradients_follow_the_built_ins_in_the_grid() {
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
        app.cx.update(|cx| set_reduce_motion(true, cx));
        let handle = app.window.into();
        let act = |app: &mut es_testkit::Headless<gpui_component::Root>,
                   f: &dyn Fn(
            &mut BackgroundBody,
            &mut Window,
            &mut gpui::Context<BackgroundBody>,
        )| {
            app.cx
                .update_window(handle, |_, window, cx| {
                    body.update(cx, |b, cx| f(b, window, cx))
                })
                .unwrap();
        };
        for (pick, angle, name) in [(4, 90.0, "Ember"), (13, 30.0, "A very long gradient name")] {
            act(&mut app, &|b, window, cx| {
                b.pick_gradient(pick, cx);
                b.edit_gradient(window, cx);
                b.add_stop(window, cx);
                b.set_stop_color(1, hsla(0.45, 0.8, 0.55, 1.), cx);
                b.set_gradient_angle(angle, window, cx);
                b.set_gradient_name(name, window, cx);
                b.save_gradient(cx);
            });
        }
        let mine = app.cx.update(|cx| body.read(cx).user_gradients().to_vec());
        assert_eq!(mine.len(), 2);
        act(&mut app, &|b, _, cx| b.pick_gradient(PRESETS.len() + 1, cx));
        let picked = app
            .cx
            .update(|cx| body.read(cx).selected_gradient(cx))
            .unwrap();
        assert_eq!(
            picked.id, mine[1].id,
            "the second tile after the 24 built-ins is the user's second"
        );
        app.settle();
        app.screenshot(&format!("gradients-mine-{tag}"));

        act(&mut app, &|b, window, cx| b.edit_gradient(window, cx));
        app.settle();
        app.screenshot(&format!("gradients-editor-{tag}"));

        act(&mut app, &|b, window, cx| {
            b.pick_gradient(0, cx);
            b.edit_gradient(window, cx);
        });
        app.settle();
        app.screenshot(&format!("gradients-editor-copy-{tag}"));
    }
}

fn gradient_editor_frames(mode: ThemeMode, tag: &str, reduce: bool) -> Vec<image::RgbaImage> {
    use es_ui::motion::FILL_SWITCH;
    let (mut app, editor) = window(mode);
    app.cx.update(|cx| set_reduce_motion(reduce, cx));
    let body = app.cx.update(|cx| {
        let inspector = editor.read(cx).inspector().unwrap();
        let view = inspector
            .read(cx)
            .body(Section::Background)
            .unwrap()
            .clone();
        view.downcast::<BackgroundBody>().unwrap()
    });
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, cx| {
            body.update(cx, |b, cx| {
                b.pick_gradient(4, cx);
                b.edit_gradient(window, cx);
            })
        })
        .unwrap();
    let quarter = FILL_SWITCH.duration / 4;
    ["0-start", "1-mid", "2-late", "3-end"]
        .iter()
        .enumerate()
        .map(|(i, step)| {
            if i > 0 {
                app.cx
                    .advance_clock(if i == 1 { quarter * 2 } else { quarter });
            }
            app.cx
                .update_window(handle, |_, window, _| window.refresh())
                .unwrap();
            app.screenshot(&format!("gradient-editor-fade-{tag}-{step}"))
        })
        .collect()
}

#[test]
fn the_gradient_editor_fades_in_and_arrives() {
    for (mode, tag) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let f = gradient_editor_frames(mode, tag, false);
        assert!(
            f[0] != f[1] && f[1] != f[2] && f[2] != f[3],
            "the panel is still fading at every quarter"
        );
        let reduced = gradient_editor_frames(mode, &format!("{tag}-reduced"), true);
        assert!(
            reduced[0] == reduced[3],
            "a 160 ms fade is skipped under Reduce Motion"
        );
        assert!(f[3] == reduced[3], "the fade ends on the panel in full");
    }
}
