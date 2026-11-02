use super::bar::{BarSources, NoAudio, devices_from, new_bar};
use super::pickers::{Declining, declining_folder_picker};
use super::reopen_recovered;
use super::windows::{Wiring, onboarding_input, open_practice};
use es_app::capture_coordinator::CaptureCoordinator;
use es_app::dispatcher::RouteDispatcher;
use es_app::errors::Result;
use es_app::events::Origin;
use es_app::history::History;
use es_app::platform_ui::ClipboardSink;
use es_app::settings_model::SettingsModel;
use es_app::toast::{MemorySink, install_toasts};
use es_app::windows::{WindowKey, WindowKind, Windows};
use es_backend::{BindHelper, BindSyntax, Caps, Trigger};
use es_desktop::{Notification, NotificationId, Notifier};
use es_editor::session::journal::{self, Entry};
use es_model::AnnotationDocument;
use es_model::platform::ClipItem;
use es_overlays::record::bar::BarEvent;
use es_protocol::Route;
use es_shell::gallery::{FileResult, GalleryFiles};
use es_store::{ConfigFile, Paths};
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Task, TestAppContext};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

struct NoClipboard;

impl ClipboardSink for NoClipboard {
    fn copy(&self, _: Vec<ClipItem>, _: &mut App) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }
}

struct NoFiles;

impl GalleryFiles for NoFiles {
    fn show_in_folder(&self, _: PathBuf) -> BoxFuture<'static, FileResult> {
        Box::pin(async { Ok(()) })
    }
    fn trash(&self, _: &Path) -> FileResult {
        Ok(())
    }
}

struct Silent;

impl Notifier for Silent {
    fn notify(&self, _: Notification) -> es_desktop::Result<NotificationId> {
        Ok(NotificationId(1))
    }
    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        async_channel::unbounded().1
    }
    fn supports_actions(&self) -> bool {
        false
    }
}

struct Rig {
    wiring: Rc<Wiring>,
    dispatcher: RouteDispatcher,
    paths: Paths,
    dir: tempfile::TempDir,
}

fn png(path: &Path) {
    image::RgbaImage::from_pixel(16, 12, image::Rgba([10, 120, 200, 255]))
        .save(path)
        .unwrap();
}

fn rig(cx: &mut TestAppContext) -> Rig {
    rig_with(cx, None)
}

struct Lines;

impl BindHelper for Lines {
    fn syntax(&self) -> BindSyntax {
        BindSyntax::Sway
    }
    fn snippet(&self, binds: &[(Trigger, Route)]) -> String {
        binds
            .iter()
            .map(|(t, r)| format!("bindsym {} exec easyshot {}", t.0, r.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn existing_binds(&self) -> es_backend::Result<Vec<(Trigger, String)>> {
        Ok(Vec::new())
    }
}

fn rig_with(cx: &mut TestAppContext, binds: Option<Arc<dyn BindHelper>>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    cx.update(|cx| {
        es_ui::hello::init(cx);
        let config = ConfigFile::load(&paths.config_file()).unwrap();
        let settings = cx.new(|_| SettingsModel::new(config));
        let history = cx.new(|_| History::open(&paths, 10).unwrap());
        let windows = Windows::default();
        let wiring = Wiring::new(
            paths.clone(),
            settings,
            history,
            Rc::new(NoClipboard),
            Rc::new(Declining),
            Arc::new(NoFiles),
            declining_folder_picker(),
            Caps::empty(),
            binds,
            Rc::new(Vec::new),
            None,
            windows.clone(),
        );
        wiring.register();
        let (jobs, _finished) = es_app::jobs::Jobs::new();
        let dispatcher = RouteDispatcher::new(jobs.clone(), CaptureCoordinator::new(jobs), windows);
        wiring.set_dispatcher(dispatcher.clone());
        Rig {
            wiring,
            dispatcher,
            paths,
            dir,
        }
    })
}

fn open(rig: &Rig, route: Route, cx: &mut TestAppContext) {
    cx.update(|cx| {
        rig.dispatcher.dispatch(route, Origin::Internal, cx);
    });
    cx.run_until_parked();
}

fn is_open(rig: &Rig, kind: WindowKind, key: &WindowKey, cx: &mut TestAppContext) -> bool {
    cx.update(|cx| rig.wiring.windows.is_open(kind, key, cx))
}

#[gpui::test]
fn settings_and_gallery_routes_open_their_windows_once(cx: &mut TestAppContext) {
    let rig = rig(cx);
    open(&rig, Route::Settings(None), cx);
    assert!(is_open(&rig, WindowKind::Settings, &WindowKey::Single, cx));
    assert!(!is_open(&rig, WindowKind::Gallery, &WindowKey::Single, cx));
    open(&rig, Route::Gallery, cx);
    assert!(is_open(&rig, WindowKind::Gallery, &WindowKey::Single, cx));
    assert_eq!(cx.update(|cx| cx.windows().len()), 2);
    open(&rig, Route::Settings(None), cx);
    open(&rig, Route::Gallery, cx);
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        2,
        "focused, not duplicated"
    );
}

#[gpui::test]
fn open_route_gives_one_editor_per_file(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let (a, b) = (rig.dir.path().join("a.png"), rig.dir.path().join("b.png"));
    png(&a);
    png(&b);
    open(&rig, Route::Open(a.clone()), cx);
    open(&rig, Route::Open(a.clone()), cx);
    assert_eq!(rig.wiring.editor_count(), 1);
    assert!(is_open(&rig, WindowKind::Editor, &WindowKey::Path(a), cx));
    open(&rig, Route::Open(b), cx);
    assert_eq!(rig.wiring.editor_count(), 2);
    assert_eq!(cx.update(|cx| cx.windows().len()), 2);
}

#[gpui::test]
fn onboarding_opens_through_the_registry(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.wiring
            .windows
            .open(WindowKind::Onboarding, WindowKey::Single, cx)
            .unwrap();
    });
    assert!(is_open(
        &rig,
        WindowKind::Onboarding,
        &WindowKey::Single,
        cx
    ));
}

#[gpui::test]
fn the_capture_bar_needs_a_surface_provider(cx: &mut TestAppContext) {
    let rig = rig(cx);
    open(&rig, Route::CaptureBar, cx);
    assert!(!is_open(
        &rig,
        WindowKind::CaptureBar,
        &WindowKey::Single,
        cx
    ));
    assert_eq!(cx.update(|cx| cx.windows().len()), 0);
}

#[gpui::test]
fn recovered_journals_reopen_in_editors_with_a_toast(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.dir.path().join("lost.png");
    png(&file);
    journal::write(
        &rig.paths,
        "11111111-1111-4111-8111-111111111111",
        Entry {
            display: file.clone(),
            base_image: file.clone(),
            base_revision: 0,
            document: AnnotationDocument::default(),
            staged_base: None,
        },
    )
    .unwrap();
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    cx.update(|cx| reopen_recovered(&rig.wiring, journal::recover(&rig.paths), cx));
    cx.run_until_parked();
    assert!(is_open(
        &rig,
        WindowKind::Editor,
        &WindowKey::Path(file),
        cx
    ));
    let shown = sink.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].title, "Recovered unsaved edits");
}

#[gpui::test]
fn onboarding_shows_the_compositor_bind_lines_from_the_backend(cx: &mut TestAppContext) {
    let rig = rig_with(cx, Some(Arc::new(Lines)));
    let snippet = cx
        .update(|cx| onboarding_input(&rig.wiring, cx))
        .snippet
        .expect("a snippet when the backend can write binds");
    assert!(
        snippet.contains(&format!("exec easyshot {}", Route::CaptureRegion.as_str())),
        "{snippet}"
    );
    let bare = self::rig(cx);
    assert_eq!(
        cx.update(|cx| onboarding_input(&bare.wiring, cx)).snippet,
        None
    );
}

#[gpui::test]
fn the_practice_image_opens_in_the_editor_without_capturing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| open_practice(&rig.wiring, cx));
    cx.run_until_parked();
    let file = rig.paths.unstaged_dir().join("practice.png");
    let (w, h) = image::image_dimensions(&file).expect("the bundled image decodes");
    assert!(w >= 640 && h >= 400, "{w}x{h}");
    assert!(is_open(
        &rig,
        WindowKind::Editor,
        &WindowKey::Path(file),
        cx
    ));
    assert_eq!(rig.wiring.editor_count(), 1);
}

#[gpui::test]
fn the_capture_bar_lists_devices_and_windows_and_refresh_asks_again(cx: &mut TestAppContext) {
    use std::cell::Cell;
    let rig = rig(cx);
    let (windows, devices) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let (w, d) = (windows.clone(), devices.clone());
    rig.wiring.set_bar_sources(BarSources {
        windows: Rc::new(move || {
            w.set(w.get() + 1);
            Vec::new()
        }),
        devices: Rc::new(move || {
            d.set(d.get() + 1);
            Vec::new()
        }),
    });
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    assert_eq!((windows.get(), devices.get()), (1, 1), "listed at open");
    bar.update(cx, |_, cx| cx.emit(BarEvent::RefreshWindows));
    assert_eq!(windows.get(), 2, "Refresh Windows lists the windows again");
    assert_eq!(devices.get(), 1);
}

struct Pactl(&'static str);

impl es_record::audio::Runner for Pactl {
    fn run(&self, _: &str, _: &[&str]) -> std::io::Result<String> {
        Ok(self.0.to_owned())
    }
}

#[test]
fn audio_devices_come_through_the_runner_and_fail_to_none() {
    let listed = devices_from(Rc::new(Pactl("[]")))();
    assert!(
        listed
            .iter()
            .any(|d| d.kind == es_record::AudioKind::Microphone),
        "{listed:?}"
    );
    assert!(devices_from(Rc::new(NoAudio))().is_empty());
}
