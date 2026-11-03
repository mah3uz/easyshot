#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::{
    CaptureCoordinator, CaptureKind, CaptureRequest, Direct, DirectCapture, Timer,
};
use es_app::errors::{Error, Result};
use es_app::jobs::{JobFinished, JobId, Jobs};
use es_app::platform_ui::{SelectMode, Selection, SelectionFlow};
use es_geom::Rgba;
use es_model::capture::CaptureVariant;
use es_protocol::JobStatus;
use gpui::{App, Task, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<String>>>;

fn color() -> Selection {
    Selection::Color(Rgba::new(0.0, 0.0, 0.0, 1.0))
}

#[derive(Default)]
struct Flow {
    log: Log,
    gates: RefCell<Vec<async_channel::Sender<Selection>>>,
}

impl SelectionFlow for Flow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        self.log.borrow_mut().push(format!("select {mode:?}"));
        let (tx, rx) = async_channel::bounded(1);
        self.gates.borrow_mut().push(tx);
        cx.foreground_executor()
            .spawn(async move { rx.recv().await.map_err(|_| Error::Cancelled) })
    }
}

fn kind(selection: &Selection) -> &'static str {
    match selection {
        Selection::Region { .. } => "region",
        Selection::Window { .. } => "window",
        Selection::Color(_) => "color",
        Selection::Cancelled => "cancelled",
    }
}

struct FakeDirect(Log);

impl DirectCapture for FakeDirect {
    fn run(&self, source: Direct, _cx: &mut App) -> Task<Result<Selection>> {
        let line = match source {
            Direct::Again(picked) => format!("again {}", kind(&picked)),
            other => format!("direct {other:?}"),
        };
        self.0.borrow_mut().push(line);
        Task::ready(Ok(color()))
    }
}

struct Rig {
    jobs: Jobs,
    done: async_channel::Receiver<JobFinished>,
    coordinator: CaptureCoordinator,
    flow: Rc<Flow>,
    log: Log,
}

fn rig() -> Rig {
    let (jobs, done) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    let log = Log::default();
    let flow = Rc::new(Flow {
        log: log.clone(),
        ..Flow::default()
    });
    coordinator.set_selection_flow(flow.clone());
    coordinator.set_direct_capture(Rc::new(FakeDirect(log.clone())));
    let sink = log.clone();
    coordinator.set_pipeline(Rc::new(move |request, _, _| {
        sink.borrow_mut()
            .push(format!("pipeline {:?}", request.kind));
        Task::ready(Ok(()))
    }));
    Rig {
        jobs,
        done,
        coordinator,
        flow,
        log,
    }
}

impl Rig {
    fn submit(&self, cx: &mut TestAppContext, kind: CaptureKind) -> JobId {
        self.submit_timed(cx, kind, Timer::Off)
    }

    fn submit_timed(&self, cx: &mut TestAppContext, kind: CaptureKind, timer: Timer) -> JobId {
        let job = self.jobs.allocate();
        let request = CaptureRequest::new(kind, CaptureVariant::Normal).timed(timer);
        cx.update(|cx| self.coordinator.submit(job, request, cx));
        job
    }

    fn release(&self, cx: &mut TestAppContext, index: usize) {
        let gate = self.flow.gates.borrow()[index].clone();
        gate.try_send(color()).unwrap();
        cx.run_until_parked();
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn finished(&self) -> Vec<(JobId, JobStatus)> {
        std::iter::from_fn(|| self.done.try_recv().ok())
            .map(|f| (f.id, f.status))
            .collect()
    }
}

#[gpui::test]
fn requests_run_one_at_a_time_in_arrival_order(cx: &mut TestAppContext) {
    let rig = rig();
    let a = rig.submit(cx, CaptureKind::Region);
    let b = rig.submit(cx, CaptureKind::Window);
    let c = rig.submit(cx, CaptureKind::Text);
    cx.run_until_parked();
    assert_eq!(rig.log(), ["select Region"]);

    rig.release(cx, 0);
    assert_eq!(
        rig.log(),
        ["select Region", "pipeline Region", "select Window"]
    );
    rig.release(cx, 1);
    rig.release(cx, 2);
    assert_eq!(
        rig.log()[3..],
        ["pipeline Window", "select Text", "pipeline Text"]
    );
    assert_eq!(
        rig.finished(),
        [(a, JobStatus::Ok), (b, JobStatus::Ok), (c, JobStatus::Ok)]
    );
}

#[gpui::test]
fn a_second_scroll_trigger_stops_the_running_scroll(cx: &mut TestAppContext) {
    let rig = rig();
    let stops = Rc::new(RefCell::new(0));
    let counter = stops.clone();
    rig.coordinator
        .set_scroll_stop(Rc::new(move |_| *counter.borrow_mut() += 1));
    let first = rig.submit(cx, CaptureKind::Scroll);
    cx.run_until_parked();
    let second = rig.submit(cx, CaptureKind::Scroll);
    cx.run_until_parked();

    assert_eq!(*stops.borrow(), 1);
    assert_eq!(rig.finished(), [(second, JobStatus::Ok)]);
    assert!(rig.jobs.is_running(first));
    assert_eq!(rig.log(), ["select Scroll"]);

    rig.release(cx, 0);
    assert_eq!(rig.finished(), [(first, JobStatus::Ok)]);
    rig.submit(cx, CaptureKind::Scroll);
    cx.run_until_parked();
    assert_eq!(*stops.borrow(), 1);
}

#[gpui::test]
fn cancelling_a_queued_request_skips_it(cx: &mut TestAppContext) {
    let rig = rig();
    rig.submit(cx, CaptureKind::Region);
    let queued = rig.submit(cx, CaptureKind::Window);
    rig.submit(cx, CaptureKind::Text);
    cx.run_until_parked();

    assert!(rig.jobs.cancel(queued));
    cx.run_until_parked();
    rig.release(cx, 0);
    rig.release(cx, 1);

    assert_eq!(
        rig.log(),
        [
            "select Region",
            "pipeline Region",
            "select Text",
            "pipeline Text"
        ]
    );
    let statuses: Vec<_> = rig.finished().into_iter().map(|f| f.1).collect();
    assert_eq!(
        statuses,
        [JobStatus::Cancelled, JobStatus::Ok, JobStatus::Ok]
    );
}

#[gpui::test]
fn cancelling_the_running_request_starts_the_next(cx: &mut TestAppContext) {
    let rig = rig();
    let running = rig.submit(cx, CaptureKind::Region);
    rig.submit(cx, CaptureKind::Text);
    cx.run_until_parked();

    rig.jobs.cancel(running);
    cx.run_until_parked();
    assert_eq!(rig.log(), ["select Region", "select Text"]);
    rig.release(cx, 1);
    assert_eq!(rig.log().last().unwrap(), "pipeline Text");
}

#[gpui::test]
fn a_cancelled_selection_is_a_silent_cancelled_job(cx: &mut TestAppContext) {
    let rig = rig();
    let job = rig.submit(cx, CaptureKind::Region);
    cx.run_until_parked();
    rig.flow.gates.borrow()[0]
        .try_send(Selection::Cancelled)
        .unwrap();
    cx.run_until_parked();

    assert_eq!(rig.finished(), [(job, JobStatus::Cancelled)]);
    assert_eq!(rig.log(), ["select Region"]);
}

#[gpui::test]
fn a_gated_request_finishes_refused_before_anything_is_selected(cx: &mut TestAppContext) {
    let rig = rig();
    rig.coordinator
        .set_gate(Rc::new(|request, _| match request.kind {
            CaptureKind::RecordingArea => Err(Error::Internal("the recording is busy".into())),
            _ => Ok(()),
        }));
    let refused = rig.submit(cx, CaptureKind::RecordingArea);
    cx.run_until_parked();
    assert!(rig.log().is_empty(), "no picker opens and nothing freezes");
    let finished = rig.finished();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].0, refused);
    assert_ne!(finished[0].1, JobStatus::Ok);
    rig.submit(cx, CaptureKind::Region);
    cx.run_until_parked();
    assert_eq!(rig.log(), ["select Region"], "other captures pass the gate");
}

fn counting(rig: &Rig) {
    let hide = rig.log.clone();
    rig.coordinator.set_before_capture(Rc::new(move |_| {
        hide.borrow_mut().push("hide picker".into())
    }));
    let count = rig.log.clone();
    rig.coordinator.set_countdown(Rc::new(move |timer, _| {
        count.borrow_mut().push(format!("countdown {timer:?}"));
        Task::ready(Ok(()))
    }));
}

// The screen is frozen only while picking, so the countdown must come after the pick and the
// pipeline must get a fresh frame of the picked target, not the frozen one.
#[gpui::test]
fn a_timed_capture_picks_then_counts_down_then_captures_afresh(cx: &mut TestAppContext) {
    let rig = rig();
    counting(&rig);
    rig.submit_timed(cx, CaptureKind::Region, Timer::Setting);
    cx.run_until_parked();
    assert_eq!(rig.log(), ["hide picker", "select Region"]);
    rig.release(cx, 0);
    rig.submit_timed(cx, CaptureKind::Window, Timer::Seconds(5));
    cx.run_until_parked();
    rig.release(cx, 1);
    rig.submit_timed(cx, CaptureKind::Fullscreen, Timer::Seconds(10));
    cx.run_until_parked();
    assert_eq!(
        rig.log(),
        [
            "hide picker",
            "select Region",
            "countdown Setting",
            "again color",
            "pipeline Region",
            "hide picker",
            "select Window",
            "countdown Seconds(5)",
            "again color",
            "pipeline Window",
            "hide picker",
            "direct Fullscreen",
            "countdown Seconds(10)",
            "again color",
            "pipeline Fullscreen",
        ]
    );
}

#[gpui::test]
fn an_untimed_capture_neither_counts_down_nor_captures_twice(cx: &mut TestAppContext) {
    let rig = rig();
    counting(&rig);
    rig.submit(cx, CaptureKind::Window);
    cx.run_until_parked();
    rig.release(cx, 0);
    assert_eq!(
        rig.log(),
        ["hide picker", "select Window", "pipeline Window"]
    );
}

#[gpui::test]
fn a_cancelled_pick_never_starts_the_countdown(cx: &mut TestAppContext) {
    let rig = rig();
    counting(&rig);
    let job = rig.submit_timed(cx, CaptureKind::Region, Timer::Seconds(3));
    cx.run_until_parked();
    rig.flow.gates.borrow()[0]
        .try_send(Selection::Cancelled)
        .unwrap();
    cx.run_until_parked();
    assert_eq!(rig.finished(), [(job, JobStatus::Cancelled)]);
    assert_eq!(rig.log(), ["hide picker", "select Region"]);
}

#[gpui::test]
fn the_captured_hook_sees_the_final_selection_before_the_pipeline(cx: &mut TestAppContext) {
    let rig = rig();
    let seen = rig.log.clone();
    rig.coordinator
        .set_captured(Rc::new(move |k, selection, _| {
            seen.borrow_mut()
                .push(format!("captured {k:?} {}", kind(selection)));
        }));
    rig.submit_timed(cx, CaptureKind::Fullscreen, Timer::Seconds(3));
    cx.run_until_parked();
    assert_eq!(
        rig.log(),
        [
            "direct Fullscreen",
            "again color",
            "captured Fullscreen color",
            "pipeline Fullscreen"
        ]
    );
}

#[gpui::test]
fn fullscreen_and_previous_skip_the_selection_ui(cx: &mut TestAppContext) {
    let rig = rig();
    rig.submit(cx, CaptureKind::Fullscreen);
    rig.submit(cx, CaptureKind::Previous);
    cx.run_until_parked();
    assert_eq!(
        rig.log(),
        [
            "direct Fullscreen",
            "pipeline Fullscreen",
            "direct Previous",
            "pipeline Previous"
        ]
    );
}

#[gpui::test]
fn a_missing_pipeline_is_an_error_not_a_hang(cx: &mut TestAppContext) {
    let (jobs, done) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    let job = jobs.allocate();
    cx.update(|cx| {
        coordinator.submit(
            job,
            CaptureRequest::new(CaptureKind::Fullscreen, CaptureVariant::Normal),
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(done.try_recv().unwrap().status, JobStatus::Error);
}
