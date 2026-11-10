#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::{
    CaptureCoordinator, Direct, DirectCapture, REPAINT_MARGIN, TRAY_MENU_CLOSE, Timer,
};
use es_app::dispatcher::RouteDispatcher;
use es_app::errors::Result;
use es_app::events::Origin;
use es_app::jobs::{JobFinished, Jobs};
use es_app::platform_ui::{SelectMode, Selection, SelectionFlow};
use es_app::windows::{WindowHandle, WindowKind, Windows};
use es_geom::Rgba;
use es_protocol::{JobStatus, Route, SettingsPage, TimedMode};
use gpui::{App, Task, TestAppContext};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn color() -> Selection {
    Selection::Color(Rgba::new(0.0, 0.0, 0.0, 1.0))
}

struct Instant;

impl SelectionFlow for Instant {
    fn run(&self, _: SelectMode, _: &mut App) -> Task<Result<Selection>> {
        Task::ready(Ok(color()))
    }
}

impl DirectCapture for Instant {
    fn run(&self, _: Direct, _: &mut App) -> Task<Result<Selection>> {
        Task::ready(Ok(color()))
    }
}

struct Open;

impl WindowHandle for Open {
    fn is_open(&self, _: &App) -> bool {
        true
    }

    fn focus(&self, _: &mut App) {}

    fn close(&self, _: &mut App) {}
}

struct Rig {
    dispatcher: RouteDispatcher,
    jobs: Jobs,
    done: async_channel::Receiver<JobFinished>,
    log: Log,
}

fn rig(wired: bool) -> Rig {
    let (jobs, done) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    let windows = Windows::default();
    let dispatcher = RouteDispatcher::new(jobs.clone(), coordinator.clone(), windows.clone());
    let log = Log::default();
    if wired {
        coordinator.set_selection_flow(Rc::new(Instant));
        coordinator.set_direct_capture(Rc::new(Instant));
        let sink = log.clone();
        coordinator.set_pipeline(Rc::new(move |request, _, _| {
            let timer = match request.timer {
                Timer::Off => String::new(),
                t => format!(" {t:?}"),
            };
            sink.borrow_mut()
                .push(format!("{:?} {:?}{timer}", request.kind, request.variant));
            Task::ready(Ok(()))
        }));
        for kind in [WindowKind::Home, WindowKind::Editor, WindowKind::CaptureBar] {
            let sink = log.clone();
            windows.register(
                kind,
                Rc::new(move |key, _| {
                    sink.borrow_mut().push(format!("{kind:?} {key:?}"));
                    Ok(Rc::new(Open))
                }),
            );
        }
        let sink = log.clone();
        dispatcher.set_action_handler(Rc::new(move |action, _| {
            sink.borrow_mut().push(format!("{action:?}"));
            Ok(())
        }));
    }
    Rig {
        dispatcher,
        jobs,
        done,
        log,
    }
}

fn every_route() -> Vec<(Route, Vec<&'static str>)> {
    let path = PathBuf::from("/nonexistent/shot.png");
    let mut routes = vec![
        (Route::CaptureRegion, vec!["Region Normal"]),
        (Route::CaptureFullscreen, vec!["Fullscreen Normal"]),
        (Route::CaptureWindow, vec!["Window Normal"]),
        (Route::CapturePrevious, vec!["Previous Normal"]),
        (Route::CaptureTimed, vec!["Region Normal Setting"]),
        (
            Route::timed(TimedMode::Window, Some(5)),
            vec!["Window Normal Seconds(5)"],
        ),
        (
            Route::timed(TimedMode::Fullscreen, None),
            vec!["Fullscreen Normal Setting"],
        ),
        (Route::CaptureRegionCopy, vec!["Region RegionCopy"]),
        (Route::CaptureRegionSave, vec!["Region RegionSave"]),
        (Route::CaptureRegionAnnotate, vec!["Region RegionAnnotate"]),
        (Route::CaptureRegionPin, vec!["Region RegionPin"]),
        (Route::CaptureScroll, vec!["Scroll Normal"]),
        (Route::CaptureScrollWindow, vec!["ScrollWindow Normal"]),
        (Route::Ocr, vec!["Text Normal"]),
        (Route::OcrSingleLine, vec!["TextLine Normal"]),
        (Route::ColorPicker, vec!["Color Normal"]),
        (Route::Record, vec!["Recording Normal"]),
        (Route::RecordArea, vec!["RecordingArea Normal"]),
        (Route::RecordStop, vec!["RecordStop"]),
        (Route::RecordPause, vec!["RecordPause"]),
        (Route::RecordRestart, vec!["RecordRestart"]),
        (Route::RecordDiscard, vec!["RecordDiscard"]),
        (Route::CaptureBar, vec!["CaptureBar Single"]),
        (Route::Home, vec!["Home Single"]),
        (Route::Gallery, vec!["ShowHome(Gallery)"]),
        (Route::RestoreLast, vec!["RestoreLast"]),
        (Route::PinLast, vec!["PinLast"]),
        (Route::UnpinAll, vec!["UnpinAll"]),
        (Route::EditClipboard, vec!["EditClipboard"]),
        (Route::DeckToggle, vec!["DeckToggle"]),
        (Route::DeckSaveAll, vec!["DeckSaveAll"]),
        (Route::DeckCloseAll, vec!["DeckCloseAll"]),
        (Route::Quit, vec!["Quit"]),
        (Route::AgentRevoke, vec!["RevokeAgent"]),
        (
            Route::Open(path.clone()),
            vec!["Editor Path(\"/nonexistent/shot.png\")"],
        ),
        (Route::Settings(None), vec!["ShowHome(Settings(None))"]),
    ];
    for page in SettingsPage::ALL {
        routes.push((
            Route::Settings(Some(page)),
            vec![Box::leak(
                format!("ShowHome(Settings(Some({page:?})))").into(),
            )],
        ));
    }
    routes
}

#[gpui::test]
fn every_route_reaches_its_handler_and_its_job_finishes_ok(cx: &mut TestAppContext) {
    assert_eq!(every_route().len(), 36 + SettingsPage::ALL.len());
    let rig = rig(true);
    for (route, expected) in every_route() {
        rig.log.borrow_mut().clear();
        let name = route.to_string();
        let job = cx.update(|cx| rig.dispatcher.dispatch(route, Origin::Tray, cx));
        cx.executor()
            .advance_clock(TRAY_MENU_CLOSE + REPAINT_MARGIN);
        cx.run_until_parked();
        assert_eq!(*rig.log.borrow(), expected, "{name}");
        let done = rig.done.try_recv().unwrap();
        assert_eq!((done.id, done.status), (job, JobStatus::Ok), "{name}");
        assert!(rig.done.try_recv().is_err(), "{name}");
    }
}

// The host closes its menu after the click reaches us; a frame grabbed before then shows it.
#[gpui::test]
fn a_capture_from_the_tray_menu_grabs_the_screen_only_once_the_menu_is_gone(
    cx: &mut TestAppContext,
) {
    let rig = rig(true);
    let job = cx.update(|cx| {
        rig.dispatcher
            .dispatch(Route::CaptureWindow, Origin::Tray, cx)
    });
    cx.run_until_parked();
    assert!(rig.log.borrow().is_empty(), "the menu is still up");
    cx.executor().advance_clock(TRAY_MENU_CLOSE);
    cx.run_until_parked();
    assert!(
        rig.log.borrow().is_empty(),
        "faded, but not yet repainted without it"
    );
    cx.executor().advance_clock(REPAINT_MARGIN);
    cx.run_until_parked();
    assert_eq!(*rig.log.borrow(), ["Window Normal"]);
    assert_eq!(rig.done.try_recv().unwrap().id, job);
}

#[gpui::test]
fn only_a_capture_from_the_tray_menu_waits(cx: &mut TestAppContext) {
    let rig = rig(true);
    cx.update(|cx| {
        rig.dispatcher
            .dispatch(Route::CaptureWindow, Origin::Shortcut, cx);
        rig.dispatcher
            .dispatch(Route::Settings(None), Origin::Tray, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        *rig.log.borrow(),
        ["Home Single", "ShowHome(Settings(None))", "Window Normal"]
    );
}

#[gpui::test]
fn routes_with_nothing_registered_finish_with_an_error(cx: &mut TestAppContext) {
    let rig = rig(false);
    for (route, _) in every_route() {
        let name = route.to_string();
        cx.update(|cx| rig.dispatcher.dispatch(route, Origin::DBus, cx));
        cx.run_until_parked();
        let done = rig.done.try_recv().unwrap();
        assert_eq!(done.status, JobStatus::Error, "{name}");
        assert!(done.detail.is_some(), "{name}");
    }
}

#[gpui::test]
fn dispatch_as_reports_under_the_callers_job_id(cx: &mut TestAppContext) {
    let rig = rig(true);
    let job = rig.jobs.allocate();
    cx.update(|cx| {
        rig.dispatcher
            .dispatch_as(job, Route::Quit, Origin::DBus, cx)
    });
    assert_eq!(rig.done.try_recv().unwrap().id, job);
}
