#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::{
    CaptureCoordinator, CaptureKind, CaptureRequest, Direct, DirectCapture,
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

struct FakeDirect(Log);

impl DirectCapture for FakeDirect {
    fn run(&self, source: Direct, _cx: &mut App) -> Task<Result<Selection>> {
        self.0.borrow_mut().push(format!("direct {source:?}"));
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
        let job = self.jobs.allocate();
        let request = CaptureRequest::new(kind, CaptureVariant::Normal);
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
fn the_picker_hides_before_selection_and_a_timed_capture_counts_down_first(
    cx: &mut TestAppContext,
) {
    let rig = rig();
    let hide = rig.log.clone();
    rig.coordinator.set_before_capture(Rc::new(move |_| {
        hide.borrow_mut().push("hide picker".into())
    }));
    let count = rig.log.clone();
    rig.coordinator.set_countdown(Rc::new(move |_| {
        count.borrow_mut().push("countdown".into());
        Task::ready(Ok(()))
    }));
    rig.submit(cx, CaptureKind::Timed);
    cx.run_until_parked();
    assert_eq!(rig.log(), ["hide picker", "countdown", "select Region"]);
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
