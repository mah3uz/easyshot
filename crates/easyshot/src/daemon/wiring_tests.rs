use super::bar::{BarSources, NoAudio, devices_from, new_bar};
use super::pickers::{Declining, declining_folder_picker};
use super::reopen_recovered;
use super::windows::{Wiring, onboarding_input, open_practice};
use es_app::capture_coordinator::CaptureCoordinator;
use es_app::deck::{Deck, DeckPolicy};
use es_app::dispatcher::{Action, RouteDispatcher};
use es_app::errors::{Error, Result};
use es_app::events::Origin;
use es_app::history::{History, HistoryEvent};
use es_app::platform_ui::ClipboardSink;
use es_app::platform_ui::{LayerShellSurfaces, OutputSource};
use es_app::settings_model::SettingsModel;
use es_app::toast::{MemorySink, install_toasts};
use es_app::windows::{WindowKey, WindowKind, Windows};
use es_backend::{BindHelper, BindSyntax, Caps, Trigger};
use es_desktop::{Notification, NotificationId, Notifier};
use es_editor::EditorWindow;
use es_editor::session::journal::{self, Entry};
use es_geom::Point;
use es_model::AnnotationDocument;
use es_model::capture::{CaptureVariant, Step, after_capture_plan};
use es_model::platform::{ClipItem, OutputId, OutputInfo};
use es_overlays::deck::view::{DeckSettings, DeckSurface};
use es_overlays::record::bar::BarEvent;
use es_protocol::{Route, SettingsPage};
use es_shell::gallery::{FileResult, GalleryFiles};
use es_shell::home::{HomeWindow, Tab};
use es_shell::onboarding::{Onboarding, OnboardingEvent};
use es_shell::settings::Page;
use es_store::{ConfigFile, Paths};
use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Entity, Task, TestAppContext};
use gpui_component::Root;
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
    settings: Entity<SettingsModel>,
    deck: Entity<Deck>,
    history: Entity<History>,
    deck_surface: Rc<DeckSurface>,
    _watch: Option<Task<()>>,
    _live: gpui::Subscription,
}

struct NoOutputs;

impl OutputSource for NoOutputs {
    fn outputs(&self) -> Vec<OutputInfo> {
        Vec::new()
    }
    fn pointer_output(&self) -> Option<OutputId> {
        None
    }
    fn focused_output(&self) -> Option<OutputId> {
        None
    }
    fn pointer_global(&self) -> Option<Point> {
        None
    }
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
    fn file_name(&self) -> &'static str {
        "binds.conf"
    }
    fn file_text(&self, binds: &[(Trigger, Route)]) -> String {
        self.snippet(binds)
    }
    fn include_line(&self, dir: &str) -> String {
        format!("include {dir}/binds.conf")
    }
    fn is_included(&self, _: &std::path::Path) -> es_backend::Result<bool> {
        Ok(false)
    }
}

fn rig_with(cx: &mut TestAppContext, binds: Option<Arc<dyn BindHelper>>) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    cx.update(|cx| {
        let config = ConfigFile::load(&paths.config_file()).unwrap();
        let settings = cx.new(|_| SettingsModel::new(config));
        std::mem::forget(super::look::install(&settings, None, cx));
        let watch = super::config_watch::install(&settings, cx);
        let history = cx.new(|_| History::open(&paths, 10).unwrap());
        let deck = cx.new(|_| Deck::new(DeckPolicy::default()));
        let surfaces = Rc::new(LayerShellSurfaces::new(Arc::new(NoOutputs), None));
        let deck_surface = DeckSurface::attach(surfaces, &deck, DeckSettings::default(), cx);
        let live = super::live_settings::install(&settings, &deck, &deck_surface, &history, cx);
        let windows = Windows::default();
        let wiring = Wiring::new(
            paths.clone(),
            settings.clone(),
            history.clone(),
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
        let w = wiring.clone();
        dispatcher.set_action_handler(Rc::new(move |action, cx| match action {
            Action::ShowHome(tab) => {
                w.show_home(tab, cx);
                Ok(())
            }
            _ => Err(Error::Unsupported),
        }));
        Rig {
            wiring,
            dispatcher,
            paths,
            dir,
            settings,
            deck,
            history,
            deck_surface,
            _watch: watch,
            _live: live,
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

/// How many open windows hold a `T` as their content; the registry only records what was asked for.
fn showing<T: 'static>(cx: &mut TestAppContext) -> usize {
    cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|w| w.downcast::<Root>())
            .filter(|w| {
                w.read(cx)
                    .is_ok_and(|root| root.view().clone().downcast::<T>().is_ok())
            })
            .count()
    })
}

fn home_tab(rig: &Rig, cx: &mut TestAppContext) -> (Tab, Page) {
    cx.update(|cx| {
        let home = rig.wiring.home().expect("the Home window");
        let home = home.read(cx);
        (home.tab(), home.settings().read(cx).page())
    })
}

#[gpui::test]
fn home_gallery_and_settings_share_one_window_and_switch_its_tab(cx: &mut TestAppContext) {
    let rig = rig(cx);
    open(&rig, Route::Home, cx);
    assert!(is_open(&rig, WindowKind::Home, &WindowKey::Single, cx));
    assert_eq!(showing::<HomeWindow>(cx), 1);
    assert_eq!(home_tab(&rig, cx), (Tab::Gallery, Page::Home));

    open(&rig, Route::Settings(Some(SettingsPage::Shortcuts)), cx);
    assert_eq!(home_tab(&rig, cx), (Tab::Settings, Page::Shortcuts));
    open(&rig, Route::Home, cx);
    assert_eq!(
        home_tab(&rig, cx),
        (Tab::Settings, Page::Shortcuts),
        "a bare open keeps the tab"
    );
    open(&rig, Route::Gallery, cx);
    assert_eq!(home_tab(&rig, cx).0, Tab::Gallery);
    open(&rig, Route::Settings(None), cx);
    assert_eq!(
        home_tab(&rig, cx),
        (Tab::Settings, Page::Shortcuts),
        "settings with no page keeps the page"
    );
    assert_eq!(
        cx.update(|cx| cx.windows().len()),
        1,
        "focused, not duplicated"
    );
}

#[gpui::test]
fn settings_with_a_page_opens_home_on_that_page(cx: &mut TestAppContext) {
    let rig = rig(cx);
    open(&rig, Route::Settings(Some(SettingsPage::Recording)), cx);
    assert_eq!(showing::<HomeWindow>(cx), 1);
    assert_eq!(home_tab(&rig, cx), (Tab::Settings, Page::Recording));
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
    assert_eq!(showing::<EditorWindow>(cx), 1);
    assert!(is_open(&rig, WindowKind::Editor, &WindowKey::Path(a), cx));
    open(&rig, Route::Open(b), cx);
    assert_eq!(rig.wiring.editor_count(), 2);
    assert_eq!(cx.update(|cx| cx.windows().len()), 2);
}

#[gpui::test]
fn keys_reach_an_editor_the_daemon_opens(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.dir.path().join("a.png");
    png(&file);
    open(&rig, Route::Open(file), cx);
    let (window, editor) = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|w| w.downcast::<Root>())
            .find_map(|w| {
                let view = w.read(cx).ok()?.view().clone();
                Some((w, view.downcast::<EditorWindow>().ok()?))
            })
            .unwrap()
    });
    let tool = |cx: &mut TestAppContext| {
        editor.read_with(cx, |w, cx| {
            w.session().unwrap().doc.read(cx).editor().tool()
        })
    };
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(50));
    cx.run_until_parked();
    cx.simulate_keystrokes(window.into(), "o");
    assert_eq!(tool(cx), es_model::Tool::Ellipse);
    cx.simulate_keystrokes(window.into(), "a");
    assert_eq!(tool(cx), es_model::Tool::Arrow);
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
    assert_eq!(showing::<Onboarding>(cx), 1);
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

struct FakeBar(Rc<std::cell::Cell<bool>>);

impl es_app::windows::WindowHandle for FakeBar {
    fn is_open(&self, _: &App) -> bool {
        self.0.get()
    }
    fn focus(&self, _: &mut App) {}
    fn close(&self, _: &mut App) {
        self.0.set(false);
    }
}

#[gpui::test]
fn a_capture_closes_an_open_capture_bar_first(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let open_flag = Rc::new(std::cell::Cell::new(false));
    let flag = open_flag.clone();
    rig.wiring.windows.register(
        WindowKind::CaptureBar,
        Rc::new(move |_, _| {
            flag.set(true);
            Ok(Rc::new(FakeBar(flag.clone())))
        }),
    );
    open(&rig, Route::CaptureBar, cx);
    assert!(open_flag.get());
    let hook = super::bar::close_before_capture(&rig.wiring.windows);
    cx.update(|cx| hook(cx));
    assert!(!open_flag.get());
    assert!(!is_open(
        &rig,
        WindowKind::CaptureBar,
        &WindowKey::Single,
        cx
    ));
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
fn onboarding_offers_the_bind_helper_with_no_keys_chosen(cx: &mut TestAppContext) {
    let rig = rig_with(cx, Some(Arc::new(Lines)));
    let input = cx.update(|cx| onboarding_input(&rig.wiring, cx));
    assert!(input.binds.is_some());
    assert!(input.global_shortcuts.is_empty());
}

fn set_autosave_in_onboarding(on: bool, cx: &mut TestAppContext) {
    cx.update(|cx| {
        let view = cx
            .windows()
            .into_iter()
            .filter_map(|w| w.downcast::<Root>())
            .find_map(|w| {
                w.read(cx)
                    .ok()
                    .and_then(|root| root.view().clone().downcast::<Onboarding>().ok())
            })
            .unwrap();
        view.update(cx, |_, cx| cx.emit(OnboardingEvent::SetAutosave(on)));
    });
}

fn normal_capture_saves(rig: &Rig, cx: &mut TestAppContext) -> bool {
    let config = rig.settings.read_with(cx, |s, _| s.config().clone());
    after_capture_plan(CaptureVariant::Normal, &config).contains(&Step::Save)
}

#[gpui::test]
fn onboarding_autosave_is_the_setting_settings_shows_and_what_a_capture_obeys(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.wiring
            .windows
            .open(WindowKind::Onboarding, WindowKey::Single, cx)
            .unwrap();
    });
    assert!(normal_capture_saves(&rig, cx));
    set_autosave_in_onboarding(false, cx);
    cx.run_until_parked();
    let shown = rig
        .settings
        .read_with(cx, |s, _| s.config().general.auto_save);
    assert!(!shown, "Settings reads general.auto_save");
    assert!(!normal_capture_saves(&rig, cx));
    set_autosave_in_onboarding(true, cx);
    cx.run_until_parked();
    assert!(
        rig.settings
            .read_with(cx, |s, _| s.config().general.auto_save)
    );
    assert!(normal_capture_saves(&rig, cx));
}

#[gpui::test]
fn onboarding_names_the_saved_area_capture_key(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings.update(cx, |s, cx| {
        s.update(
            |c| {
                c.global_shortcuts
                    .insert("capture-region".into(), "Super+Shift+4".into());
            },
            cx,
        )
    });
    let input = cx.update(|cx| onboarding_input(&rig.wiring, cx));
    assert_eq!(
        input.capture_shortcut.map(|t| t.label()),
        Some("Super+Shift+4".into())
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
        recorder_missing: false,
    });
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    assert_eq!((windows.get(), devices.get()), (1, 1), "listed at open");
    bar.update(cx, |_, cx| cx.emit(BarEvent::RefreshWindows));
    assert_eq!(windows.get(), 2, "Refresh Windows lists the windows again");
    assert_eq!(devices.get(), 1);
}

#[gpui::test]
fn the_capture_bar_is_told_when_no_recorder_is_installed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sources = BarSources {
        recorder_missing: true,
        ..BarSources::default()
    };
    rig.wiring.set_bar_sources(sources);
    let bar = cx.update(|cx| new_bar(&rig.wiring, cx));
    let hint = cx.update(|cx| bar.read(cx).recording_hint());
    assert!(hint.contains("No screen recorder is installed"), "{hint}");
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

#[test]
fn editors_open_fullscreen_only_when_the_setting_is_on() {
    let bounds = gpui::Bounds::new(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(1.), gpui::px(1.)),
    );
    assert!(matches!(
        super::windows::editor_bounds(true, bounds),
        gpui::WindowBounds::Fullscreen(_)
    ));
    assert!(matches!(
        super::windows::editor_bounds(false, bounds),
        gpui::WindowBounds::Windowed(_)
    ));
}

fn wait_until(
    cx: &mut TestAppContext,
    what: &str,
    mut done: impl FnMut(&mut TestAppContext) -> bool,
) {
    cx.executor().allow_parking();
    for _ in 0..100 {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn history_limit(rig: &Rig, cx: &mut TestAppContext) -> u32 {
    cx.update(|cx| rig.settings.read(cx).config().general.history_limit)
}

fn settle(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    std::thread::sleep(std::time::Duration::from_millis(600));
    cx.run_until_parked();
}

#[gpui::test]
fn a_hand_edit_applies_live_and_survives_the_next_settings_save(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.paths.config_file();
    std::fs::write(&file, "[general]\nhistory_limit = 7\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 7);

    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.play_sound = false, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("history_limit = 7"), "{text}");
    assert!(text.contains("play_sound = false"), "{text}");
}

#[gpui::test]
fn an_edit_made_while_a_save_is_pending_wins(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 3, cx))
    });
    std::fs::write(rig.paths.config_file(), "[general]\nhistory_limit = 9\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 9);
    settle(cx);
    assert_eq!(history_limit(&rig, cx), 9);
    let text = std::fs::read_to_string(rig.paths.config_file()).unwrap();
    assert!(text.contains("history_limit = 9"), "{text}");
}

#[gpui::test]
fn a_broken_file_keeps_the_last_good_settings_and_is_not_overwritten(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.paths.config_file();
    std::fs::write(&file, "[general]\nhistory_limit = 7\n").unwrap();
    wait_until(cx, "the edit to apply", |cx| history_limit(&rig, cx) == 7);

    let broken = "[general\nhistory_limit = ";
    std::fs::write(&file, broken).unwrap();
    wait_until(cx, "the file to be seen as broken", |cx| {
        cx.update(|cx| rig.settings.read(cx).load_error().is_some())
    });
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 11, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    settle(cx);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), broken);
    assert_eq!(history_limit(&rig, cx), 11);
}

#[gpui::test]
fn the_daemons_own_save_does_not_come_back_as_an_edit(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let seen = Rc::new(std::cell::Cell::new(0));
    let counter = seen.clone();
    let _sub =
        cx.update(|cx| cx.subscribe(&rig.settings, move |_, _, _| counter.set(counter.get() + 1)));
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 5, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    settle(cx);
    assert_eq!(seen.get(), 1);
    assert_eq!(history_limit(&rig, cx), 5);
}

#[gpui::test]
fn overlay_and_keep_previews_changes_reach_the_live_deck(cx: &mut TestAppContext) {
    let rig = rig(cx);
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(
                |c| {
                    c.overlay.card_size = "large".into();
                    c.overlay.position = "top-left".into();
                    c.overlay.dismiss_delay = 9;
                    c.capture.keep_in_deck_until_saved = !c.capture.keep_in_deck_until_saved;
                },
                cx,
            )
        });
    });
    cx.update(|cx| {
        let cfg = rig.settings.read(cx).config().clone();
        let policy = rig.deck.read(cx).policy();
        assert_eq!(
            policy,
            DeckPolicy::from_settings(&cfg.overlay, &cfg.capture)
        );
        assert_eq!(policy.delay, Some(std::time::Duration::from_secs(9)));
        assert_eq!(
            rig.deck_surface.settings(),
            DeckSettings::from_settings(&cfg.overlay)
        );
        assert_ne!(rig.deck_surface.settings(), DeckSettings::default());
    });
}

#[gpui::test]
fn a_new_history_limit_reaches_the_live_history(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let limits = Rc::new(std::cell::Cell::new(0));
    let counter = limits.clone();
    let _sub = cx.update(|cx| {
        cx.subscribe(&rig.history, move |_, e: &HistoryEvent, _| {
            if *e == HistoryEvent::LimitChanged {
                counter.set(counter.get() + 1);
            }
        })
    });
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.history_limit = 3, cx))
    });
    assert_eq!(limits.get(), 1);
}

#[gpui::test]
fn a_broken_file_raises_a_toast_with_settings_closed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    std::fs::write(rig.paths.config_file(), "[general\nhistory_limit = ").unwrap();
    wait_until(cx, "the toast", |_| !sink.shown().is_empty());
    let shown = sink.shown();
    assert_eq!(shown[0].title, "Settings file has an error");
    assert!(
        shown[0]
            .message
            .as_deref()
            .is_some_and(|m| m.contains("Line 1"))
    );
}

#[gpui::test]
fn a_failed_save_raises_a_toast_with_settings_closed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sink = MemorySink::new();
    cx.update(|cx| install_toasts(sink.clone(), Arc::new(Silent), cx));
    std::fs::remove_dir_all(rig.paths.config_file().parent().unwrap()).unwrap();
    std::fs::write(rig.paths.config_file().parent().unwrap(), "not a directory").unwrap();
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.general.play_sound = false, cx));
        rig.settings.update(cx, |s, cx| s.flush(cx));
    });
    wait_until(cx, "the toast", |_| !sink.shown().is_empty());
    assert_eq!(sink.shown()[0].title, "Couldn’t save settings");
}

fn open_editor_with(
    rig: &Rig,
    edit: fn(&mut es_model::Config),
    cx: &mut TestAppContext,
) -> Entity<EditorWindow> {
    cx.update(|cx| rig.settings.update(cx, |s, cx| s.update(edit, cx)));
    let file = rig.dir.path().join("a.png");
    png(&file);
    open(rig, Route::Open(file), cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(50));
    cx.run_until_parked();
    cx.update(|cx| {
        cx.windows()
            .into_iter()
            .filter_map(|w| w.downcast::<Root>())
            .find_map(|w| {
                w.read(cx)
                    .ok()?
                    .view()
                    .clone()
                    .downcast::<EditorWindow>()
                    .ok()
            })
            .unwrap()
    })
}

fn bound_command(
    key: &'static str,
    cx: &mut TestAppContext,
) -> Option<es_editor::keyboard::Command> {
    use es_editor::keyboard::{CONTEXT, Run};
    let keystroke = gpui::Keystroke::parse(key).ok()?;
    cx.update(|cx| {
        let context = [gpui::KeyContext::parse(CONTEXT).unwrap()];
        let (found, _) = cx
            .key_bindings()
            .borrow()
            .bindings_for_input(&[keystroke], &context);
        found
            .first()
            .and_then(|b| b.action().as_any().downcast_ref::<Run>())
            .map(|run| run.0)
    })
}

#[gpui::test]
fn the_editor_nudges_by_the_configured_steps(cx: &mut TestAppContext) {
    use es_editor::keyboard::Command;
    let rig = rig(cx);
    let _editor = open_editor_with(
        &rig,
        |c| {
            c.editor.nudge_step = 3;
            c.editor.nudge_step_shift = 25;
        },
        cx,
    );
    assert_eq!(bound_command("right", cx), Some(Command::Nudge(3.0, 0.0)));
    assert_eq!(
        bound_command("shift-up", cx),
        Some(Command::Nudge(0.0, -25.0))
    );
    cx.update(|cx| {
        rig.settings
            .update(cx, |s, cx| s.update(|c| c.editor.nudge_step = 5, cx))
    });
    cx.run_until_parked();
    assert_eq!(bound_command("left", cx), Some(Command::Nudge(-5.0, 0.0)));
}

#[gpui::test]
fn the_editor_zooms_about_the_configured_anchor(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(&rig, |c| c.editor.zoom_anchor = "centre".into(), cx);
    let at_pointer = |cx: &mut TestAppContext| {
        editor.read_with(cx, |w, cx| {
            w.session().unwrap().viewport.read(cx).zoom_anchor_pointer()
        })
    };
    assert!(!at_pointer(cx), "a centre anchor was not applied on open");
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.editor.zoom_anchor = "pointer".into(), cx)
        })
    });
    cx.run_until_parked();
    assert!(
        at_pointer(cx),
        "a changed anchor did not reach the open editor"
    );
}

#[gpui::test]
fn smart_redaction_uses_the_configured_ocr_languages(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let editor = open_editor_with(
        &rig,
        |c| c.ocr.languages = vec!["deu".into(), "fra".into()],
        cx,
    );
    let languages =
        |cx: &mut TestAppContext| editor.read_with(cx, |w, _| w.ocr_options().languages.clone());
    assert_eq!(languages(cx), ["deu", "fra"]);
    cx.update(|cx| {
        rig.settings.update(cx, |s, cx| {
            s.update(|c| c.ocr.languages = vec!["spa".into()], cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(languages(cx), ["spa"]);
}

mod style_tests;
mod tray_tests;
