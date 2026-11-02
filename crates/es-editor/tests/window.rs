#![allow(clippy::unwrap_used, clippy::expect_used)]

mod canvas_support;

use std::{cell::RefCell, rc::Rc, time::Duration};

use canvas_support::{TEXT, every_kind, opened, rect};
use es_editor::{
    EditorSession, EditorWindow, EditorWindowEvent, Error, Job, Opened, Progress,
    window::{FLASH, INSPECTOR_WIDTH},
};
use es_model::{BackgroundSettings, BackgroundStyle};
use es_render::beautify::GradientPreset;
use es_testkit::Headless;
use es_ui::{components::ZoomCommand, tokens::ThemeMode};
use gpui::{
    AppContext, Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, PlatformInput, point,
    px,
};
use gpui_component::Root;

type App = Headless<Root>;

const WIDTH: f32 = 1080.0;
const HEIGHT: f32 = 720.0;

fn launch(doc: Option<Opened>, mode: ThemeMode) -> (App, Entity<EditorWindow>) {
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::components::gallery::init(cx);
            es_ui::theme::set_mode(mode, cx);
        },
        WIDTH,
        HEIGHT,
        move |window, cx| {
            let editor = cx.new(|cx| match doc {
                Some(doc) => EditorWindow::new(EditorSession::new(doc, TEXT.clone(), cx), cx),
                None => {
                    let mut w = EditorWindow::loading();
                    w.loaded(Err(Error::NotFound("gone.png".into())), cx);
                    w
                }
            });
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot
        .borrow_mut()
        .take()
        .expect("the window built the editor");
    (app, editor)
}

fn sample() -> Opened {
    opened(
        1200,
        640,
        vec![rect(100.0, 100.0, 300.0, 200.0)],
        Default::default(),
    )
}

fn styled(name: &str) -> Opened {
    let look = BackgroundSettings {
        style: GradientPreset::by_id("soft-lilac")
            .map(|g| BackgroundStyle::Gradient(g.style()))
            .unwrap_or_default(),
        ..BackgroundSettings::default()
    };
    Opened {
        display: name.into(),
        ..opened(1200, 640, every_kind(false, true), look)
    }
}

fn click(app: &mut App, x: f32, y: f32) {
    let position = point(px(x), px(y));
    for event in [
        PlatformInput::MouseDown(MouseDownEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
            first_mouse: false,
        }),
        PlatformInput::MouseUp(MouseUpEvent {
            button: MouseButton::Left,
            position,
            modifiers: Modifiers::default(),
            click_count: 1,
        }),
    ] {
        let handle = app.window.into();
        app.cx
            .update_window(handle, |_, window, cx| {
                window.dispatch_event(event, cx);
            })
            .expect("window is open");
        app.settle();
    }
}

fn events(app: &mut App, editor: &Entity<EditorWindow>) -> Rc<RefCell<Vec<EditorWindowEvent>>> {
    let seen: Rc<RefCell<Vec<EditorWindowEvent>>> = Rc::default();
    let sink = seen.clone();
    app.cx.update(|cx| {
        cx.subscribe(editor, move |_, event: &EditorWindowEvent, _| {
            sink.borrow_mut().push(*event)
        })
        .detach()
    });
    seen
}

fn wait(app: &mut App, duration: Duration) {
    app.cx.advance_clock(duration);
    app.settle();
}

/// Centres of the title-bar undo and redo buttons: 12 px bar inset, then the capsule's border and padding.
const UNDO: (f32, f32) = (29.0, 19.0);
const REDO: (f32, f32) = (59.0, 19.0);

#[test]
fn undo_and_redo_buttons_follow_the_document_history() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    let read = |app: &mut App| editor.read_with(&app.cx, |w, cx| (w.can_undo(cx), w.can_redo(cx)));
    assert_eq!(read(&mut app), (false, false));
    click(&mut app, UNDO.0, UNDO.1);
    assert_eq!(
        read(&mut app),
        (false, false),
        "a disabled undo did something"
    );

    let session_doc = editor.read_with(&app.cx, |w, _| w.session().unwrap().doc.clone());
    app.cx.update(|cx| {
        session_doc.update(cx, |d, cx| {
            d.apply(cx, |e| e.select_all());
            d.apply(cx, |e| e.delete_selected());
        })
    });
    app.settle();
    assert_eq!(read(&mut app), (true, false));

    click(&mut app, UNDO.0, UNDO.1);
    assert_eq!(
        read(&mut app),
        (false, true),
        "the undo button did not undo"
    );
    let shapes = app
        .cx
        .update(|cx| session_doc.read(cx).editor().doc().shapes().len());
    assert_eq!(shapes, 1, "undo restores the deleted shape");

    click(&mut app, REDO.0, REDO.1);
    assert_eq!(
        read(&mut app),
        (true, false),
        "the redo button did not redo"
    );
}

#[test]
fn zoom_commands_drive_the_viewport_and_the_footer_label() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    let viewport = editor.read_with(&app.cx, |w, _| w.session().unwrap().viewport.clone());
    let fit = viewport.read_with(&app.cx, |v, _| v.zoom_percent().round() as u32);
    assert_eq!(editor.read_with(&app.cx, |w, cx| w.zoom_percent(cx)), fit);

    let zoom = |app: &mut App, command| {
        app.cx
            .update(|cx| editor.update(cx, |w, cx| w.zoom(command, cx)));
        app.settle();
        let model = viewport.read_with(&app.cx, |v, _| v.zoom_percent());
        let label = editor.read_with(&app.cx, |w, cx| w.zoom_percent(cx));
        assert_eq!(label, model.round() as u32, "the label lags the viewport");
        model
    };
    assert_eq!(zoom(&mut app, ZoomCommand::Percent(200)), 200.0);
    assert_eq!(zoom(&mut app, ZoomCommand::In), 250.0);
    assert_eq!(zoom(&mut app, ZoomCommand::Out), 200.0);
    zoom(&mut app, ZoomCommand::Fit);
    assert!(viewport.read_with(&app.cx, |v, _| v.is_fit()));
}

#[test]
fn save_shows_a_spinner_then_saved_for_its_flash_then_returns() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    let seen = events(&mut app, &editor);
    let progress = |app: &mut App| editor.read_with(&app.cx, |w, _| w.progress(Job::Save));

    assert!(
        app.cx
            .update(|cx| editor.update(cx, |w, cx| w.request(Job::Save, cx)))
    );
    app.settle();
    assert_eq!(*seen.borrow(), vec![EditorWindowEvent::Save]);
    assert_eq!(progress(&mut app), Progress::Busy);
    let again = app
        .cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Copy, cx)));
    assert!(!again, "a second job started while saving");

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.finish_save(Ok(()), cx)));
    app.settle();
    assert_eq!(progress(&mut app), Progress::Done);
    wait(&mut app, FLASH - Duration::from_millis(1));
    assert_eq!(progress(&mut app), Progress::Done, "Saved left too early");
    wait(&mut app, Duration::from_millis(1));
    assert_eq!(progress(&mut app), Progress::Idle, "Saved never left");
}

#[test]
fn a_failed_save_returns_the_button_and_shows_the_error() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Save, cx)));
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.finish_save(Err("disk full".into()), cx)));
    app.settle();
    let (progress, error) =
        editor.read_with(&app.cx, |w, _| (w.progress(Job::Save), w.error().cloned()));
    assert_eq!(progress, Progress::Idle);
    assert_eq!(
        error.as_deref(),
        Some("Failed to save annotation: disk full")
    );
    app.screenshot("window-save-error");

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Save, cx)));
    assert!(
        editor.read_with(&app.cx, |w, _| w.error().is_none()),
        "a new attempt clears the old error"
    );
}

#[test]
fn copy_shows_copied_for_its_flash_and_reports_failures() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    let seen = events(&mut app, &editor);
    let progress = |app: &mut App| editor.read_with(&app.cx, |w, _| w.progress(Job::Copy));

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Copy, cx)));
    app.settle();
    assert_eq!(*seen.borrow(), vec![EditorWindowEvent::Copy]);
    assert_eq!(progress(&mut app), Progress::Busy);
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.finish_copy(Ok(()), cx)));
    app.settle();
    assert_eq!(progress(&mut app), Progress::Done);
    wait(&mut app, FLASH - Duration::from_millis(1));
    assert_eq!(progress(&mut app), Progress::Done);
    wait(&mut app, Duration::from_millis(1));
    assert_eq!(progress(&mut app), Progress::Idle);

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Copy, cx)));
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.finish_copy(Err("no clipboard".into()), cx)));
    app.settle();
    let (progress, error) =
        editor.read_with(&app.cx, |w, _| (w.progress(Job::Copy), w.error().cloned()));
    assert_eq!(progress, Progress::Idle);
    assert_eq!(
        error.as_deref(),
        Some("Failed to copy annotation: no clipboard")
    );
}

#[test]
fn the_inspector_takes_its_column_and_the_canvas_gets_the_rest() {
    let (mut app, editor) = launch(Some(sample()), ThemeMode::Dark);
    let canvas_width = |app: &mut App| {
        let canvas = editor.read_with(&app.cx, |w, _| w.canvas().unwrap().clone());
        canvas.read_with(&app.cx, |c, _| c.bounds().size.width)
    };
    assert!(editor.read_with(&app.cx, |w, _| w.inspector_open()));
    assert_eq!(canvas_width(&mut app), px(WIDTH) - INSPECTOR_WIDTH);

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.toggle_inspector(cx)));
    app.settle();
    assert_eq!(canvas_width(&mut app), px(WIDTH));

    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.toggle_inspector(cx)));
    app.settle();
    assert_eq!(canvas_width(&mut app), px(WIDTH) - INSPECTOR_WIDTH);
}

#[test]
fn a_session_that_failed_to_load_shows_the_error_state() {
    let (mut app, editor) = launch(None, ThemeMode::Dark);
    let (failed, runnable, title) = editor.read_with(&app.cx, |w, cx| {
        (
            w.failed(),
            [Job::Save, Job::Export, Job::Copy].map(|job| w.can_run(job, cx)),
            w.title(cx),
        )
    });
    assert!(failed);
    assert_eq!(
        runnable, [false; 3],
        "a failed window offers no image actions"
    );
    assert_eq!(title.as_ref(), "Image Editor");
    let shot = app.screenshot("window-load-failed");
    let scale = shot.width() / WIDTH as u32;
    let ground = *shot.get_pixel(20 * scale, 120 * scale);
    let (cx_, cy_) = (WIDTH as u32 / 2, (38 + 48 + (HEIGHT as u32 - 52)) / 2);
    let drawn = (scale * (cx_ - 200)..scale * (cx_ + 200))
        .flat_map(|x| (scale * (cy_ - 60)..scale * (cy_ + 60)).map(move |y| (x, y)))
        .filter(|&(x, y)| *shot.get_pixel(x, y) != ground)
        .count();
    assert!(
        drawn > 300,
        "nothing drawn where the message belongs ({drawn} px)"
    );
}

#[test]
fn screenshots_dark_light_and_saved() {
    let long = "Screenshot_2026-11-02_14-23-07_easyshot-capture-of-the-settings-window.png";
    let (mut app, editor) = launch(Some(styled(long)), ThemeMode::Dark);
    app.screenshot("window-dark");
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.set_low_res(true, cx)));
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Save, cx)));
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.finish_save(Ok(()), cx)));
    app.cx
        .update(|cx| editor.update(cx, |w, cx| w.request(Job::Copy, cx)));
    app.screenshot("window-dark-saved");
    drop(app);

    let (mut app, _) = launch(Some(styled("website.png")), ThemeMode::Light);
    app.screenshot("window-light");
}
