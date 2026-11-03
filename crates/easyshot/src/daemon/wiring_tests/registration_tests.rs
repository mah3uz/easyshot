use super::editor_config_tests::bound_command;
use super::*;
use es_app::capture_coordinator::{Direct, DirectCapture};
use es_app::jobs::{JobFinished, Jobs};
use es_app::platform_ui::{SelectMode, Selection, SelectionFlow};
use es_app::toast::{IconName, Toast, show_toast};
use es_desktop::{RecordingNotifier, RecordingTrayState};
use es_shell::tray::{Recent, TrayInput};

struct Cancelling;

impl SelectionFlow for Cancelling {
    fn run(&self, _: SelectMode, _: &mut App) -> Task<Result<Selection>> {
        Task::ready(Ok(Selection::Cancelled))
    }
}

impl DirectCapture for Cancelling {
    fn run(&self, _: Direct, _: &mut App) -> Task<Result<Selection>> {
        Task::ready(Ok(Selection::Cancelled))
    }
}

/// A dispatcher over the rig's real window registry that answers every action and capture, so any
/// "unsupported" it reports is a route the dispatcher has no way to reach.
fn dispatch_all(
    rig: &Rig,
    routes: Vec<Route>,
    cx: &mut TestAppContext,
) -> Vec<(Route, JobFinished)> {
    let (jobs, finished) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    coordinator.set_selection_flow(Rc::new(Cancelling));
    coordinator.set_direct_capture(Rc::new(Cancelling));
    coordinator.set_pipeline(Rc::new(|_, _, _| Task::ready(Ok(()))));
    let dispatcher = RouteDispatcher::new(jobs.clone(), coordinator, rig.wiring.windows.clone());
    let sources = crate::daemon::agent::Sources {
        caps: Caps::empty(),
        outputs: Arc::new(Vec::new),
        windows: Arc::new(Vec::new),
    };
    let gate = crate::daemon::agent::gate::tests::GateRig::permissive(cx);
    dispatcher.set_agent_handler(crate::daemon::agent::handler(
        sources,
        jobs,
        gate.gate.clone(),
    ));
    let wiring = rig.wiring.clone();
    dispatcher.set_action_handler(Rc::new(move |action, cx| match action {
        Action::ShowHome(tab) => {
            wiring.show_home(tab, cx);
            Ok(())
        }
        Action::DeckToggle
        | Action::DeckSaveAll
        | Action::DeckCloseAll
        | Action::RestoreLast
        | Action::PinLast
        | Action::UnpinAll
        | Action::EditClipboard
        | Action::RecordStop
        | Action::RecordPause
        | Action::RecordRestart
        | Action::RecordDiscard
        | Action::Quit => Ok(()),
    }));
    let mut sent = Vec::new();
    for route in routes {
        let job = cx.update(|cx| dispatcher.dispatch(route.clone(), Origin::Internal, cx));
        cx.run_until_parked();
        sent.push((job, route));
    }
    let mut done = Vec::new();
    while let Ok(f) = finished.try_recv() {
        done.push(f);
    }
    sent.into_iter()
        .map(|(job, route)| {
            let f = done
                .iter()
                .find(|f| f.id == job)
                .unwrap_or_else(|| panic!("{route} never finished"));
            (route, f.clone())
        })
        .collect()
}

fn unsupported(answers: Vec<(Route, JobFinished)>) -> Vec<String> {
    answers
        .into_iter()
        .filter(|(_, f)| f.detail.as_deref() == Some("unsupported on this system"))
        .map(|(route, _)| route.to_string())
        .collect()
}

#[gpui::test]
fn booting_binds_the_editor_keys(cx: &mut TestAppContext) {
    assert_eq!(bound_command("o", cx), None);
    let _rig = rig(cx);
    assert!(
        bound_command("o", cx).is_some(),
        "the editor keymap is bound at boot"
    );
}

#[gpui::test]
fn booting_starts_the_config_file_watch(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert!(rig._watch.is_some(), "config.toml is watched");
}

#[gpui::test]
fn booting_leaves_the_cursor_visible_for_every_window(cx: &mut TestAppContext) {
    let _rig = rig(cx);
    let mode = cx.update(|cx| cx.cursor_hide_mode());
    assert_eq!(mode, gpui::CursorHideMode::Never);
}

#[gpui::test]
fn booting_applies_the_theme_setting_and_follows_it(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tokens = |cx: &mut TestAppContext| cx.update(|cx| es_ui::theme::tokens(cx).clone());
    let theme = |name: &'static str, cx: &mut TestAppContext| {
        cx.update(|cx| {
            rig.settings
                .update(cx, |s, cx| s.update(|c| c.general.theme = name.into(), cx))
        });
        tokens(cx)
    };
    let (dark, light) = (theme("dark", cx), theme("light", cx));
    assert_ne!(dark, light, "the theme setting reaches the tokens");
    assert_eq!(theme("dark", cx), dark);
}

#[gpui::test]
fn the_tray_feed_follows_the_deck_and_history(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let tray = Arc::new(es_desktop::RecordingTray::default());
    let feed = cx.update(|cx| {
        let feed = crate::daemon::tray::TrayFeed::new(
            {
                let tray = tray.clone();
                Rc::new(move |input| es_shell::tray::show(tray.as_ref(), input))
            },
            Rc::new(|| false),
            rig.history.clone(),
            rig.settings.clone(),
            rig.paths.videos_dir(),
            false,
        );
        let follow = feed.follow(&rig.deck, cx);
        (feed, follow)
    });
    let before = tray.menus().len();
    cx.update(|cx| {
        rig.history
            .update(cx, |h, cx| {
                let file = rig.dir.path().join("x.png");
                png(&file);
                h.add(
                    es_model::HistoryEntry {
                        id: uuid::Uuid::new_v4(),
                        created_at: time::OffsetDateTime::UNIX_EPOCH,
                        kind: es_model::CaptureKind::Region,
                        name: "x".into(),
                        file,
                        export_path: None,
                        pixel_size: (16, 12),
                        has_annotations: false,
                        share_url: None,
                    },
                    cx,
                )
            })
            .unwrap()
    });
    assert!(
        tray.menus().len() > before,
        "history changes rebuild the tray menu"
    );
    drop(feed);
}

#[gpui::test]
fn every_route_has_a_handler_that_is_not_unsupported(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let routes: Vec<Route> = Route::all()
        .into_iter()
        .filter(|r| *r != Route::CaptureBar)
        .collect();
    let unanswered = unsupported(dispatch_all(&rig, routes, cx));
    assert!(unanswered.is_empty(), "no handler for: {unanswered:?}");
}

fn tray_routes() -> Vec<Route> {
    let shot = |name: &str| Recent {
        name: name.into(),
        file: PathBuf::from(format!("{name}.png")),
    };
    let state = |paused| {
        Some(RecordingTrayState {
            elapsed: std::time::Duration::from_secs(5),
            paused,
        })
    };
    let base = TrayInput {
        screenshots: vec![shot("a")],
        recordings: vec![shot("b")],
        has_pins: true,
        ..Default::default()
    };
    let inputs = [
        base.clone(),
        TrayInput {
            recording: state(false),
            ..base.clone()
        },
        TrayInput {
            recording: state(true),
            ..base.clone()
        },
        TrayInput {
            recorder_missing: true,
            ..base
        },
    ];
    let mut routes = Vec::new();
    fn walk(items: &[es_desktop::TrayItem], routes: &mut Vec<Route>) {
        for item in items.iter().filter(|i| !i.separator && i.enabled) {
            if item.children.is_empty() {
                let route = item.route.clone().unwrap_or_else(|| {
                    panic!("tray item {} does nothing when clicked", item.label_id)
                });
                routes.push(route);
            }
            walk(&item.children, routes);
        }
    }
    for input in &inputs {
        walk(&es_shell::tray::menu(input).items, &mut routes);
    }
    routes
}

#[gpui::test]
fn every_tray_menu_item_runs_a_route_with_a_handler(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let file = rig.dir.path().join("a.png");
    png(&file);
    let routes: Vec<Route> = tray_routes()
        .into_iter()
        .map(|r| match r {
            Route::Open(_) => Route::Open(file.clone()),
            other => other,
        })
        .collect();
    assert!(routes.len() > 10);
    let unanswered = unsupported(dispatch_all(&rig, routes, cx));
    assert!(unanswered.is_empty(), "no handler for: {unanswered:?}");
}

struct CountingFiles(std::sync::atomic::AtomicU32);

impl GalleryFiles for CountingFiles {
    fn show_in_folder(&self, _: PathBuf) -> BoxFuture<'static, FileResult> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
    fn trash(&self, _: &Path) -> FileResult {
        Ok(())
    }
}

#[gpui::test]
fn every_notification_action_a_notice_offers_has_an_answer(cx: &mut TestAppContext) {
    use crate::daemon::notices::{Answers, on_action};
    let notifier = Arc::new(RecordingNotifier::default());
    let (opened, edited) = (std::cell::Cell::new(0), std::cell::Cell::new(0));
    let files = CountingFiles(Default::default());
    cx.update(|cx| {
        es_app::toast::install_toasts(MemorySink::without_surface(), notifier.clone(), cx);
        let image = Toast::new("toast-saved", IconName::Check).arg("path", "/s/a.png");
        show_toast(
            es_app::pipeline::capture_notice(image, Path::new("/s/a.png")),
            cx,
        );
        let answers = Answers {
            open: &|_| opened.set(opened.get() + 1),
            files: &files,
            edit: &|_, _| edited.set(edited.get() + 1),
        };
        let sent = notifier.sent();
        assert!(sent[0].actions.len() >= 3);
        for (key, _) in &sent[0].actions {
            let before =
                opened.get() + edited.get() + files.0.load(std::sync::atomic::Ordering::SeqCst);
            on_action(NotificationId(1), key, &answers, cx);
            assert_eq!(
                opened.get() + edited.get() + files.0.load(std::sync::atomic::Ordering::SeqCst),
                before + 1,
                "the {key} button of a capture notice does nothing"
            );
        }
    });
}
