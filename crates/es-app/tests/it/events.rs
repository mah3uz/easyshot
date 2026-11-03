#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::CaptureCoordinator;
use es_app::dispatcher::{Action, RouteDispatcher};
use es_app::events::{EventHandlers, Origin, PlatformEvent, spawn_pump};
use es_app::jobs::Jobs;
use es_app::settings_model::SettingsModel;
use es_app::windows::Windows;
use es_desktop::NotificationId;
use es_protocol::{JobStatus, Route};
use es_store::{ConfigFile, Paths};
use gpui::{AppContext, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

struct Rig {
    tx: async_channel::Sender<PlatformEvent>,
    jobs: Jobs,
    done: async_channel::Receiver<es_app::jobs::JobFinished>,
    model: gpui::Entity<SettingsModel>,
    path: std::path::PathBuf,
    actions: Rc<RefCell<Vec<Action>>>,
    notifications: Rc<RefCell<Vec<(NotificationId, String)>>>,
    _dir: tempfile::TempDir,
    _pump: gpui::Task<()>,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    let path = paths.config_file();
    let model = cx.new(|_| SettingsModel::new(ConfigFile::load(&path).unwrap()));
    let (jobs, done) = Jobs::new();
    let dispatcher = RouteDispatcher::new(
        jobs.clone(),
        CaptureCoordinator::new(jobs.clone()),
        Windows::default(),
    );
    let actions = Rc::new(RefCell::new(Vec::new()));
    let sink = actions.clone();
    dispatcher.set_action_handler(Rc::new(move |action, _| {
        sink.borrow_mut().push(action);
        Ok(())
    }));
    let notifications = Rc::new(RefCell::new(Vec::new()));
    let sink = notifications.clone();
    let handlers = EventHandlers {
        notification_action: Some(Rc::new(move |pair, _| sink.borrow_mut().push(pair))),
        ..EventHandlers::default()
    };
    let (tx, rx) = async_channel::unbounded();
    let pump = cx.update(|cx| spawn_pump(rx, dispatcher, model.clone(), handlers, cx));
    Rig {
        tx,
        jobs,
        done,
        model,
        path,
        actions,
        notifications,
        _dir: dir,
        _pump: pump,
    }
}

#[gpui::test]
fn a_config_change_event_reloads_the_settings(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let sound = rig
        .model
        .read_with(cx, |m, _| m.config().general.play_sound);
    std::fs::write(&rig.path, format!("[general]\nplay_sound = {}\n", !sound)).unwrap();
    rig.tx.try_send(PlatformEvent::ConfigFileChanged).unwrap();
    cx.run_until_parked();
    assert_eq!(
        rig.model
            .read_with(cx, |m, _| m.config().general.play_sound),
        !sound
    );
}

#[gpui::test]
fn a_command_reports_under_the_job_id_it_carries(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let job = rig.jobs.allocate();
    rig.tx
        .try_send(PlatformEvent::Command {
            route: Route::DeckToggle,
            origin: Origin::DBus,
            job,
        })
        .unwrap();
    cx.run_until_parked();
    let done = rig.done.try_recv().unwrap();
    assert_eq!((done.id, done.status), (job, JobStatus::Ok));
    assert_eq!(*rig.actions.borrow(), [Action::DeckToggle]);
}

#[gpui::test]
fn tray_and_shortcut_routes_are_dispatched_in_order(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.tx.try_send(PlatformEvent::Tray(Route::Quit)).unwrap();
    rig.tx
        .try_send(PlatformEvent::Shortcut(Route::PinLast))
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*rig.actions.borrow(), [Action::Quit, Action::PinLast]);
}

#[gpui::test]
fn notification_actions_reach_their_handler_and_unhandled_events_are_dropped(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.tx
        .try_send(PlatformEvent::TestInject("x".into()))
        .unwrap();
    rig.tx
        .try_send(PlatformEvent::NotificationAction {
            id: NotificationId(7),
            action: "open".into(),
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(
        *rig.notifications.borrow(),
        [(NotificationId(7), "open".to_owned())]
    );
}
