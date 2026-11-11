#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use super::canvas_support::{TEXT, base, rect};
use es_anno::GestureId;
use es_app::errors::{Error as AppError, Result as AppResult};
use es_app::platform_ui::ClipboardSink;
use es_editor::session::io::{ChooseRequest, ExportChooser, IoConfig, IoSettings};
use es_editor::{EditorSession, EditorWindow, EditorWindowEvent, Job, Opened};
use es_model::AnnotationDocument;
use es_model::platform::ClipItem;
use es_store::{ExportFormat, Paths};
use es_testkit::Headless;
use es_ui::tokens::ThemeMode;
use gpui::{App, AppContext, Entity, KeyDownEvent, KeyUpEvent, Keystroke, PlatformInput, Task};
use gpui_component::Root;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tempfile::TempDir;

pub type Shell = Headless<Root>;

#[derive(Default)]
pub struct FakeSink {
    pub items: RefCell<Vec<Vec<ClipItem>>>,
    pub fail: RefCell<bool>,
}

impl ClipboardSink for FakeSink {
    fn copy(&self, items: Vec<ClipItem>, _: &mut App) -> Task<AppResult<()>> {
        if *self.fail.borrow() {
            return Task::ready(Err(AppError::Protocol("clipboard unavailable".into())));
        }
        self.items.borrow_mut().push(items);
        Task::ready(Ok(()))
    }
}

#[derive(Default)]
pub struct FakeChooser {
    pub answer: RefCell<Option<PathBuf>>,
    pub asked: RefCell<Vec<(String, PathBuf, String)>>,
}

impl ExportChooser for FakeChooser {
    fn choose(&self, request: ChooseRequest, _: &mut App) -> Task<Result<Option<PathBuf>, String>> {
        self.asked
            .borrow_mut()
            .push((request.title, request.folder, request.suggested_name));
        Task::ready(Ok(self.answer.borrow().clone()))
    }
}

pub struct Fixture {
    pub app: Shell,
    pub editor: Entity<EditorWindow>,
    pub dir: TempDir,
    pub paths: Paths,
    pub shots: PathBuf,
    pub sink: Rc<FakeSink>,
    pub chooser: Rc<FakeChooser>,
    pub events: Rc<RefCell<Vec<EditorWindowEvent>>>,
}

pub fn launch() -> Fixture {
    launch_with(BTreeMap::new())
}

pub fn launch_with(overrides: BTreeMap<String, String>) -> Fixture {
    launch_seeded(overrides, |_| {})
}

pub fn launch_seeded(overrides: BTreeMap<String, String>, seed: impl FnOnce(&Paths)) -> Fixture {
    launch_image(overrides, seed, base(1200, 640))
}

pub fn launch_image(
    overrides: BTreeMap<String, String>,
    seed: impl FnOnce(&Paths),
    image: image::RgbaImage,
) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(&dir.path().join("xdg"));
    paths.ensure_all().unwrap();
    seed(&paths);
    let style_paths = paths.clone();
    let shots = dir.path().join("shots");
    let display = dir.path().join("capture.png");
    image.save(&display).unwrap();
    let opened = Opened {
        display,
        base: image,
        document: AnnotationDocument {
            shapes: vec![
                rect(100.0, 100.0, 300.0, 200.0),
                rect(500.0, 120.0, 200.0, 150.0),
            ],
            ..AnnotationDocument::default()
        },
        read_only: false,
    };
    let sink = Rc::new(FakeSink::default());
    let chooser = Rc::new(FakeChooser::default());
    let config = IoConfig {
        paths: Some(paths.clone()),
        settings: Some(IoSettings {
            save_dir: shots.clone(),
            template: "shot_{counter}".into(),
            format: ExportFormat::Png,
            quality: 0.9,
        }),
        chooser: Some(chooser.clone()),
        clipboard: Some(sink.clone()),
    };
    let slot: Rc<RefCell<Option<Entity<EditorWindow>>>> = Rc::default();
    let out = slot.clone();
    let mut app = Headless::open(
        es_ui::assets::Assets,
        move |cx| {
            es_ui::init(cx);
            es_ui::theme::set_mode(ThemeMode::Dark, cx);
            es_editor::keyboard::bind(cx, &overrides, &Default::default());
        },
        1080.0,
        720.0,
        move |window, cx| {
            let editor = cx.new(|cx| {
                let session =
                    EditorSession::with_style_paths(opened, TEXT.clone(), Some(style_paths), cx);
                session.io.update(cx, |io, _| io.configure(config));
                EditorWindow::new(session, cx)
            });
            *out.borrow_mut() = Some(editor.clone());
            cx.new(|cx| Root::new(editor, window, cx))
        },
    );
    let editor = slot.borrow_mut().take().unwrap();
    let events: Rc<RefCell<Vec<EditorWindowEvent>>> = Rc::default();
    let seen = events.clone();
    app.cx.update(|cx| {
        cx.subscribe(&editor, move |_, e: &EditorWindowEvent, _| {
            seen.borrow_mut().push(*e)
        })
        .detach()
    });
    app.settle();
    app.cx.advance_clock(Duration::from_millis(50));
    app.settle();
    Fixture {
        app,
        editor,
        dir,
        paths,
        shots,
        sink,
        chooser,
        events,
    }
}

impl Fixture {
    pub fn session_doc(&self) -> Entity<es_editor::DocModel> {
        self.editor
            .read_with(&self.app.cx, |w, _| w.session().unwrap().doc.clone())
    }

    pub fn row(&self) -> Entity<es_editor::toolbar::ToolRow> {
        self.editor
            .read_with(&self.app.cx, |w, _| w.toolbar().unwrap())
    }

    pub fn options(&self) -> Entity<es_editor::inspector::options::ToolOptions> {
        self.editor
            .read_with(&self.app.cx, |w, _| w.tool_options().unwrap())
    }

    pub fn click(&mut self, control: es_editor::toolbar::Control) {
        let at = self
            .row()
            .read_with(&self.app.cx, |r, _| r.control_bounds(control))
            .unwrap_or_else(|| panic!("{control:?} is not drawn"))
            .center();
        self.click_at(at);
    }

    pub fn click_option(&mut self, control: es_editor::inspector::options::Control) {
        let at = self
            .options()
            .read_with(&self.app.cx, |o, _| o.control_bounds(control))
            .unwrap_or_else(|| panic!("{control:?} is not drawn"))
            .center();
        self.click_at(at);
    }

    pub fn click_at(&mut self, at: gpui::Point<gpui::Pixels>) {
        for down in [true, false] {
            self.send(if down {
                PlatformInput::MouseDown(gpui::MouseDownEvent {
                    button: gpui::MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                })
            } else {
                PlatformInput::MouseUp(gpui::MouseUpEvent {
                    button: gpui::MouseButton::Left,
                    position: at,
                    modifiers: Default::default(),
                    click_count: 1,
                })
            });
        }
    }

    pub fn io(&self) -> Entity<es_editor::session::io::EditorIo> {
        self.editor
            .read_with(&self.app.cx, |w, _| w.session().unwrap().io.clone())
    }

    pub fn request(&mut self, job: Job) {
        self.editor.update(&mut self.app.cx, |w, cx| {
            assert!(w.request(job, cx), "{job:?} could not start");
        });
        self.app.settle();
    }

    pub fn wait(&mut self, duration: Duration) {
        self.app.cx.advance_clock(duration);
        self.app.settle();
    }

    /// A full key press: down, then up.
    pub fn key(&mut self, keystroke: &'static str) {
        self.key_down(keystroke);
        let event = PlatformInput::KeyUp(KeyUpEvent {
            keystroke: Keystroke::parse(keystroke).unwrap(),
        });
        self.send(event);
    }

    /// A key going down with no release, which is what the second event of a held key is.
    pub fn key_down(&mut self, keystroke: &'static str) {
        let event = PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse(keystroke).unwrap(),
            is_held: false,
            prefer_character_input: false,
        });
        self.send(event);
    }

    fn send(&mut self, event: PlatformInput) {
        let handle = self.app.window.into();
        self.app
            .cx
            .update_window(handle, |_, window, cx| window.dispatch_event(event, cx))
            .unwrap();
        self.app.settle();
    }

    /// One undo entry per call: moves everything by one pixel.
    pub fn edit(&mut self) {
        let doc = self.session_doc();
        let gesture = GestureId(500 + next_id());
        self.app.cx.update(|cx| {
            doc.update(cx, |d, cx| {
                d.apply(cx, |e| e.select_all());
                d.apply(cx, |e| e.nudge_selected(1.0, 0.0, gesture));
            })
        });
        self.app.settle();
    }

    pub fn dirty(&self) -> bool {
        let io = self.io();
        io.read_with(&self.app.cx, |io, cx| io.is_dirty(cx))
    }

    pub fn window_open(&mut self) -> bool {
        let handle = self.app.window.into();
        self.app.cx.update_window(handle, |_, _, _| ()).is_ok()
    }

    pub fn should_close(&mut self) -> bool {
        let handle = self.app.window.into();
        let editor = self.editor.clone();
        let closes = self
            .app
            .cx
            .update_window(handle, |_, window, cx| {
                editor.update(cx, |w, cx| w.should_close(window, cx))
            })
            .unwrap();
        self.app.settle();
        closes
    }

    pub fn journal_files(&self) -> Vec<PathBuf> {
        es_editor::session::journal::recover(&self.paths)
            .into_iter()
            .map(|r| r.path)
            .collect()
    }
}

fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
