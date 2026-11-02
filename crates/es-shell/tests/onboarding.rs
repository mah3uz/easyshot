use std::cell::RefCell;
use std::rc::Rc;

use es_backend::Caps;
use es_model::State;
use es_protocol::Trigger;
use es_shell::onboarding::{
    self, CURRENT_VERSION, Check, CheckRow, HEIGHT, Onboarding, OnboardingEvent, OnboardingInput,
    ShortcutSetup, Step, WIDTH,
};
use es_store::paths::Paths;
use es_testkit::Headless;
use es_ui::{
    assets::Assets,
    components::gallery,
    theme,
    tokens::{Appearance, ThemeMode},
};
use gpui::{AppContext, Entity};
use gpui_component::Root;

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
fn shortcuts_show_the_snippet_unless_the_portal_asks_for_them() {
    let snippet = Some("bind = SUPER SHIFT, 4, exec, easyshot region".to_owned());
    assert_eq!(
        ShortcutSetup::for_caps(Caps::empty(), snippet.clone()),
        Some(ShortcutSetup::Snippet(snippet.clone().expect("snippet")))
    );
    assert_eq!(
        ShortcutSetup::for_caps(Caps::PORTAL_SHORTCUTS, snippet),
        Some(ShortcutSetup::Portal)
    );
    assert_eq!(ShortcutSetup::for_caps(Caps::empty(), None), None);
}

fn input() -> OnboardingInput {
    OnboardingInput {
        caps: Caps::CAPTURE_OUTPUT | Caps::LAYER_SHELL | Caps::TRAY | Caps::CLIPBOARD_NO_FOCUS,
        snippet: Some(
            "bind = SUPER SHIFT, 4, exec, easyshot region\nbind = SUPER SHIFT, 3, exec, easyshot screen\nbind = SUPER SHIFT, 5, exec, easyshot record"
                .into(),
        ),
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
    let app = Headless::open(
        Assets,
        |cx| {
            gallery::init(cx);
            theme::set_mode(mode, cx);
        },
        WIDTH,
        height,
        |window, cx| {
            let view = cx.new(|cx| Onboarding::new(paths, input(), window, cx));
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
    for expected in [Step::Welcome, Step::Desktop, Step::Saving, Step::Practice] {
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
