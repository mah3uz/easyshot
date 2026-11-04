use std::cell::RefCell;
use std::rc::Rc;

use es_backend::{BindHelper, Caps, Env, Probe, niri};
use es_model::State;
use es_protocol::Trigger;
use es_shell::onboarding::{
    self, CURRENT_VERSION, Check, CheckRow, HEIGHT, Onboarding, OnboardingEvent, OnboardingInput,
    ShortcutSetup, Step, WIDTH,
};
use es_shell::settings::ClipboardSink;
use es_store::paths::Paths;
use es_testkit::Headless;
use es_ui::motion::STEP_CHANGE;
use es_ui::{
    assets::Assets,
    theme,
    tokens::{Appearance, ThemeMode},
};
use gpui::{AppContext, Entity};
use gpui_component::Root;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

fn profile() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = Paths::rooted(dir.path());
    (dir, paths)
}

fn saved_state(paths: &Paths) -> State {
    es_store::state::open(paths).expect("state").value
}

#[test]
fn a_fresh_install_sees_the_introduction_until_it_is_dismissed() {
    let (_dir, paths) = profile();
    assert!(
        onboarding::prepare_for_launch(&paths).expect("prepare"),
        "only a fresh install counts as new"
    );
    assert!(onboarding::should_present(&saved_state(&paths)));

    assert!(
        !onboarding::prepare_for_launch(&paths).expect("prepare"),
        "a second launch with setup pending is not a new install"
    );
    assert!(
        onboarding::should_present(&saved_state(&paths)),
        "pending first-time setup survives a relaunch"
    );

    onboarding::complete(&paths).expect("complete");
    assert_eq!(saved_state(&paths).onboarding_version, CURRENT_VERSION);
    assert!(!onboarding::should_present(&saved_state(&paths)));
    assert!(!onboarding::prepare_for_launch(&paths).expect("prepare"));
    assert!(
        !onboarding::should_present(&saved_state(&paths)),
        "skip, close and completion do not nag"
    );
}

#[test]
fn an_existing_profile_without_a_marker_is_an_upgrade_and_skips_setup() {
    let (_dir, paths) = profile();
    std::fs::create_dir_all(paths.config_dir()).expect("config dir");
    std::fs::write(paths.config_file(), "# kept\n").expect("config");
    assert!(!onboarding::prepare_for_launch(&paths).expect("prepare"));
    assert!(!onboarding::should_present(&saved_state(&paths)));
    assert_eq!(
        std::fs::read_to_string(paths.config_file()).expect("config"),
        "# kept\n",
        "the user's preferences are left alone"
    );
}

#[test]
fn completing_keeps_the_rest_of_the_state_and_never_downgrades_the_marker() {
    let (_dir, paths) = profile();
    let mut file = es_store::state::open(&paths).expect("state");
    file.value.counter = 42;
    file.value.onboarding_version = CURRENT_VERSION + 1;
    file.save().expect("save");

    assert!(!onboarding::prepare_for_launch(&paths).expect("prepare"));
    onboarding::complete(&paths).expect("complete");
    let state = saved_state(&paths);
    assert_eq!(state.counter, 42);
    assert_eq!(
        state.onboarding_version,
        CURRENT_VERSION + 1,
        "running an older build does not repeat the guide"
    );
    assert!(!onboarding::should_present(&state));
}

#[test]
fn the_checklist_follows_the_desktop_caps() {
    let rows = onboarding::checklist(Caps::CAPTURE_OUTPUT | Caps::TRAY | Caps::OCR);
    let row = |check| {
        *rows
            .iter()
            .find(|r: &&CheckRow| r.check == check)
            .expect("row")
    };
    assert_eq!(rows.len(), 6);
    assert!(row(Check::Capture).ready && row(Check::Capture).required);
    assert!(!row(Check::LayerShell).ready && row(Check::LayerShell).required);
    assert!(!row(Check::Clipboard).ready && !row(Check::Clipboard).required);
    assert!(row(Check::Tray).ready);
    assert!(!row(Check::Recorder).ready);
    assert!(row(Check::Ocr).ready);

    assert!(
        onboarding::checklist(Caps::all()).iter().all(|r| r.ready),
        "a complete desktop has nothing to fix"
    );
}

#[test]
fn a_missing_capability_shows_its_fix_and_a_ready_one_its_purpose() {
    use es_shell::onboarding::check_caption as caption;
    let missing = CheckRow {
        check: Check::Recorder,
        ready: false,
        required: false,
    };
    assert!(caption(missing).contains("Install gpu-screen-recorder"));
    let ready = CheckRow {
        ready: true,
        ..missing
    };
    assert!(!caption(ready).contains("Install"));
}

#[test]
fn shortcuts_use_the_binds_file_and_the_portal_only_without_one() {
    assert_eq!(
        ShortcutSetup::for_caps(Caps::empty(), true),
        Some(ShortcutSetup::Binds)
    );
    assert_eq!(
        ShortcutSetup::for_caps(Caps::PORTAL_SHORTCUTS, true),
        Some(ShortcutSetup::Binds)
    );
    assert_eq!(
        ShortcutSetup::for_caps(Caps::PORTAL_SHORTCUTS, false),
        Some(ShortcutSetup::Portal)
    );
    assert_eq!(ShortcutSetup::for_caps(Caps::empty(), false), None);
}

#[derive(Clone, Default)]
struct FakeClipboard(Rc<RefCell<Vec<String>>>);

impl ClipboardSink for FakeClipboard {
    fn copy(&self, text: String, _: &mut gpui::App) {
        self.0.borrow_mut().push(text);
    }
}

fn niri_config(paths: &Paths) -> PathBuf {
    paths.config_dir().with_file_name("niri-config.kdl")
}

fn niri_binds(config: &std::path::Path) -> Arc<dyn BindHelper> {
    let probe = Probe {
        env: Env::from_vars([("NIRI_CONFIG", config.to_str().expect("utf-8"))]),
        ..Probe::default()
    };
    let helper = (niri::BINDS.build)(&probe);
    Arc::from(helper.unwrap_or_else(|_| panic!("niri binds")))
}

fn input(paths: &Paths, clipboard: &FakeClipboard) -> OnboardingInput {
    let config = niri_config(paths);
    if !config.exists() {
        std::fs::write(
            &config,
            "binds {\n    Print { screenshot; }\n    Ctrl+Print { screenshot-screen; }\n}\n",
        )
        .expect("niri config");
    }
    OnboardingInput {
        caps: Caps::CAPTURE_OUTPUT | Caps::LAYER_SHELL | Caps::TRAY | Caps::CLIPBOARD_NO_FOCUS,
        binds: Some(niri_binds(&config)),
        clipboard: Some(Rc::new(clipboard.clone())),
        global_shortcuts: BTreeMap::new(),
        capture_shortcut: Some(Trigger::parse("Super+Shift+4").expect("trigger")),
        save_dir: Some("/home/you/Pictures/Screenshots".into()),
        autosave: true,
    }
}

type App = Headless<Root>;

fn open(
    paths: Paths,
    mode: ThemeMode,
) -> (App, Entity<Onboarding>, Rc<RefCell<Vec<OnboardingEvent>>>) {
    open_sized(paths, mode, HEIGHT)
}

fn open_sized(
    paths: Paths,
    mode: ThemeMode,
    height: f32,
) -> (App, Entity<Onboarding>, Rc<RefCell<Vec<OnboardingEvent>>>) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut host = None;
    let seen = events.clone();
    let clipboard = FakeClipboard::default();
    let app = Headless::open(
        Assets,
        |cx| {
            es_ui::init(cx);
            theme::set_mode(mode, cx);
            es_ui::motion::set_reduce_motion(true, cx);
        },
        WIDTH,
        height,
        |window, cx| {
            let input = input(&paths, &clipboard);
            let view = cx.new(|cx| Onboarding::new(paths, input, window, cx));
            cx.subscribe(&view, move |_, e: &OnboardingEvent, _| {
                seen.borrow_mut().push(e.clone());
            })
            .detach();
            host = Some(view.clone());
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    (app, host.expect("the window built its view"), events)
}

#[test]
fn walking_through_to_done_records_the_introduction_as_seen() {
    let (_dir, paths) = profile();
    onboarding::prepare_for_launch(&paths).expect("prepare");
    let (mut app, view, events) = open(paths.clone(), ThemeMode::Dark);
    let step = |app: &mut App| app.cx.update(|cx| view.read(cx).step());
    for expected in Step::ALL {
        assert_eq!(step(&mut app), expected);
        assert!(onboarding::should_present(&saved_state(&paths)));
        app.cx.update(|cx| view.update(cx, |o, cx| o.advance(cx)));
        app.settle();
    }
    assert_eq!(saved_state(&paths).onboarding_version, CURRENT_VERSION);
    assert_eq!(*events.borrow(), [OnboardingEvent::Closed]);
}

#[test]
fn skip_on_the_first_step_also_counts_as_seen() {
    let (_dir, paths) = profile();
    onboarding::prepare_for_launch(&paths).expect("prepare");
    let (mut app, view, events) = open(paths.clone(), ThemeMode::Dark);
    app.cx.update(|cx| view.update(cx, |o, cx| o.advance(cx)));
    app.cx.update(|cx| view.update(cx, |o, cx| o.back(cx)));
    assert!(
        events.borrow().is_empty(),
        "Back returns to the welcome step"
    );
    app.cx.update(|cx| view.update(cx, |o, cx| o.back(cx)));
    app.settle();
    assert_eq!(*events.borrow(), [OnboardingEvent::Closed]);
    assert!(!onboarding::should_present(&saved_state(&paths)));
}

#[test]
fn every_step_draws_in_light_and_dark() {
    for (mode, name, background) in [
        (ThemeMode::Dark, "dark", [0x1B, 0x1B, 0x1B]),
        (ThemeMode::Light, "light", [0xFF, 0xFF, 0xFF]),
    ] {
        let (_dir, paths) = profile();
        let (mut app, view, _) = open(paths, mode);
        assert_eq!(
            app.cx.update(|cx| theme::tokens(cx).appearance),
            if name == "dark" {
                Appearance::Dark
            } else {
                Appearance::Light
            }
        );
        for (i, step) in Step::ALL.into_iter().enumerate() {
            app.cx.update(|cx| view.update(cx, |o, cx| o.go(step, cx)));
            let image = app.screenshot(&format!("onboarding-{}-{name}", i + 1));
            assert_eq!(
                &image.get_pixel(6, 6).0[..3],
                &background,
                "{name} step {}",
                i + 1
            );
            let inked = image.pixels().filter(|p| p.0[..3] != background).count();
            assert!(
                inked > 20_000,
                "{name} step {} is nearly empty ({inked})",
                i + 1
            );
        }
        let (_dir, paths) = profile();
        let (mut tall, view, _) = open_sized(paths, mode, 1040.);
        tall.cx
            .update(|cx| view.update(cx, |o, cx| o.go(Step::Desktop, cx)));
        tall.screenshot(&format!("onboarding-2-whole-{name}"));
    }
}

fn at_shortcuts(paths: &Paths) -> (App, Entity<Onboarding>, Rc<RefCell<Vec<OnboardingEvent>>>) {
    let (mut app, view, events) = open(paths.clone(), ThemeMode::Dark);
    app.cx
        .update(|cx| view.update(cx, |o, cx| o.go(Step::Shortcuts, cx)));
    app.settle();
    (app, view, events)
}

#[test]
fn suggestions_skip_keys_the_compositor_already_binds() {
    let (_dir, paths) = profile();
    paths.ensure_all().expect("dirs");
    let (mut app, view, _) = at_shortcuts(&paths);
    let suggested = app.cx.update(|cx| view.read(cx).suggested());
    let key = |id: &str| {
        suggested
            .iter()
            .find(|(i, _)| *i == id)
            .and_then(|(_, t)| t.as_ref().map(Trigger::label))
    };
    assert_eq!(
        key("capture-region").as_deref(),
        Some("Super+Shift+S"),
        "Print is bound to the compositor's own screenshot"
    );
    assert_eq!(key("capture-fullscreen").as_deref(), Some("Shift+Print"));
    assert_eq!(key("capture-window").as_deref(), Some("Alt+Print"));
    assert_eq!(
        key("capture-bar").as_deref(),
        Some("Super+Shift+A"),
        "Ctrl+Print is taken too"
    );
}

#[test]
fn the_step_creates_an_empty_binds_file_so_the_include_line_is_safe_to_add() {
    let (_dir, paths) = profile();
    paths.ensure_all().expect("dirs");
    let (_app, _view, events) = at_shortcuts(&paths);
    let text = std::fs::read_to_string(paths.binds_file("binds.kdl")).expect("binds file");
    assert!(text.contains("binds {\n}"), "{text}");
    assert!(events.borrow().is_empty(), "nothing is saved until asked");
}

#[test]
fn using_the_suggestions_writes_the_file_and_hands_the_keys_to_the_config() {
    let (_dir, paths) = profile();
    paths.ensure_all().expect("dirs");
    let (mut app, view, events) = at_shortcuts(&paths);
    app.cx
        .update(|cx| view.update(cx, |o, cx| o.use_suggestions(cx)));
    app.settle();

    let text = std::fs::read_to_string(paths.binds_file("binds.kdl")).expect("binds file");
    assert!(
        text.contains("Mod+Shift+S { spawn \"easyshot\" \"capture\" \"region\"; }"),
        "{text}"
    );
    assert!(
        text.contains("Shift+Print { spawn \"easyshot\" \"capture\" \"fullscreen\"; }"),
        "{text}"
    );
    let events = events.borrow();
    let [OnboardingEvent::SetShortcuts(keys)] = events.as_slice() else {
        panic!("one save request expected, got {events:?}");
    };
    assert_eq!(
        keys.get("capture-region").map(String::as_str),
        Some("Super+Shift+S")
    );
    assert_eq!(
        keys.get("capture-window").map(String::as_str),
        Some("Alt+PrintScreen")
    );
    assert!(app.cx.update(|cx| view.read(cx).shortcuts_saved()));
}

#[test]
fn the_step_shows_the_include_line_and_whether_the_config_has_it() {
    let (_dir, paths) = profile();
    paths.ensure_all().expect("dirs");
    let (mut app, view, _) = at_shortcuts(&paths);
    let line = app
        .cx
        .update(|cx| view.read(cx).include_line())
        .expect("a line");
    assert!(line.starts_with("include optional=true \""), "{line}");

    let included = |app: &mut App| {
        app.cx
            .update(|cx| view.update(cx, |o, cx| o.check_include(cx)));
        app.until("the check", |cx| view.read(cx).included().is_some());
        app.cx
            .update(|cx| view.read(cx).included().cloned())
            .expect("checked")
    };
    assert_eq!(included(&mut app), Ok(false));
    std::fs::write(niri_config(&paths), format!("binds {{ }}\n{line}\n")).expect("config");
    assert_eq!(included(&mut app), Ok(true));
}

#[test]
fn copy_puts_the_include_line_on_the_clipboard() {
    let (_dir, paths) = profile();
    paths.ensure_all().expect("dirs");
    let clipboard = FakeClipboard::default();
    let mut host = None;
    let mut app = Headless::open(Assets, es_ui::init, WIDTH, HEIGHT, |window, cx| {
        let input = input(&paths, &clipboard);
        let view = cx.new(|cx| Onboarding::new(paths.clone(), input, window, cx));
        host = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    });
    let view = host.expect("view");
    app.cx
        .update(|cx| view.update(cx, |o, cx| o.go(Step::Shortcuts, cx)));
    app.cx
        .update(|cx| view.update(cx, |o, cx| o.copy_include_line(cx)));
    let line = app
        .cx
        .update(|cx| view.read(cx).include_line())
        .expect("line");
    assert_eq!(*clipboard.0.borrow(), vec![line]);
}

#[test]
fn the_shortcuts_step_draws_before_and_after_choosing_in_light_and_dark() {
    for (mode, name) in [(ThemeMode::Dark, "dark"), (ThemeMode::Light, "light")] {
        let (_dir, paths) = profile();
        paths.ensure_all().expect("dirs");
        let (mut app, view, _) = open(paths.clone(), mode);
        app.cx
            .update(|cx| view.update(cx, |o, cx| o.go(Step::Shortcuts, cx)));
        app.settle();
        app.screenshot(&format!("onboarding-shortcuts-{name}"));
        app.cx
            .update(|cx| view.update(cx, |o, cx| o.use_suggestions(cx)));
        let line = app
            .cx
            .update(|cx| view.read(cx).include_line())
            .expect("line");
        std::fs::write(niri_config(&paths), format!("binds {{ }}\n{line}\n")).expect("config");
        app.cx
            .update(|cx| view.update(cx, |o, cx| o.check_include(cx)));
        app.until("the check", |cx| view.read(cx).included().is_some());
        app.screenshot(&format!("onboarding-shortcuts-saved-{name}"));
    }
}

/// The page above the footer, whose buttons change with the step at once.
fn page(frame: &image::RgbaImage) -> image::RgbaImage {
    image::imageops::crop_imm(frame, 0, 0, frame.width(), frame.height() * 3 / 4).to_image()
}

fn frame(app: &mut App, name: &str) -> image::RgbaImage {
    let handle = app.window.into();
    app.cx
        .update_window(handle, |_, window, _| window.refresh())
        .expect("window is open");
    app.screenshot(name)
}

#[test]
fn a_step_slides_and_fades_in_over_its_token_and_reduce_motion_shows_it_at_once() {
    let (_dir, paths) = profile();
    let (mut app, view, _) = open(paths, ThemeMode::Dark);
    let go = |app: &mut App, step: Step| {
        app.cx.update(|cx| view.update(cx, |o, cx| o.go(step, cx)));
        app.settle();
    };
    app.cx
        .update(|cx| es_ui::motion::set_reduce_motion(false, cx));

    let welcome = page(&frame(&mut app, "onboarding-step-change-before"));
    go(&mut app, Step::Desktop);
    assert_eq!(
        app.cx.update(|cx| view.read(cx).step()),
        Step::Desktop,
        "the step changes at once; only its drawing moves"
    );
    let start = frame(&mut app, "onboarding-step-change-0");
    assert!(
        page(&start) == welcome,
        "the outgoing step is still whole where it sat when the change starts"
    );
    app.cx.advance_clock(STEP_CHANGE.duration / 2);
    let mid = frame(&mut app, "onboarding-step-change-1-mid");
    app.cx.advance_clock(STEP_CHANGE.duration / 2);
    let end = frame(&mut app, "onboarding-step-change-2-end");
    assert!(start != mid && mid != end && start != end);
    let left_half = |f: &image::RgbaImage| {
        image::imageops::crop_imm(f, 0, 0, f.width() / 2, f.height() * 3 / 4).to_image()
    };
    assert!(
        left_half(&mid) != left_half(&end),
        "halfway, the outgoing step is still leaving across the left half"
    );

    go(&mut app, Step::Welcome);
    app.cx.advance_clock(STEP_CHANGE.duration);
    app.cx
        .update(|cx| es_ui::motion::set_reduce_motion(true, cx));
    go(&mut app, Step::Desktop);
    assert!(
        frame(&mut app, "onboarding-step-change-reduced") == end,
        "with Reduce Motion the new step shows whole"
    );
}
