#![allow(clippy::unwrap_used)]
use es_app::capture_coordinator::{
    CaptureCoordinator, CaptureKind, CaptureRequest, Direct, DirectCapture, Timer,
};
use es_app::errors::{Error, Result};
use es_app::jobs::{JobFinished, JobId, Jobs};
use es_app::platform_ui::{SelectMode, Selection, SelectionFlow, Switch};
use es_app::toast::{MemorySink, install_toasts};
use es_desktop::RecordingNotifier;
use es_geom::Rgba;
use es_model::capture::CaptureVariant;
use es_protocol::JobStatus;
use futures::FutureExt as _;
use gpui::{App, Task, TestAppContext};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

type Log = Rc<RefCell<Vec<String>>>;

fn color() -> Selection {
    Selection::Color(Rgba::new(0.0, 0.0, 0.0, 1.0))
}

struct Live {
    frame: usize,
    tx: async_channel::Sender<Selection>,
}

/// Freezes a numbered frame per run and keeps count of the overlays it shows.
#[derive(Default)]
struct Screen {
    log: Log,
    live: RefCell<Option<Live>>,
    gates: RefCell<Vec<async_channel::Sender<Selection>>>,
    freezes: Cell<usize>,
    open: Cell<usize>,
    most_open: Cell<usize>,
}

struct Shown(Rc<Screen>);

impl Drop for Shown {
    fn drop(&mut self) {
        self.0.open.set(self.0.open.get() - 1);
        self.0.live.borrow_mut().take();
    }
}

struct Flow(Rc<Screen>);

impl SelectionFlow for Flow {
    fn run(&self, mode: SelectMode, cx: &mut App) -> Task<Result<Selection>> {
        let s = &self.0;
        s.freezes.set(s.freezes.get() + 1);
        s.open.set(s.open.get() + 1);
        s.most_open.set(s.most_open.get().max(s.open.get()));
        s.log.borrow_mut().push(format!("select {mode:?}"));
        let (tx, rx) = async_channel::bounded(1);
        s.gates.borrow_mut().push(tx.clone());
        *s.live.borrow_mut() = Some(Live {
            frame: s.freezes.get(),
            tx,
        });
        let shown = Shown(s.clone());
        cx.foreground_executor().spawn(async move {
            let _shown = shown;
            rx.recv().await.map_err(|_| Error::Cancelled)
        })
    }

    fn switch(&self, to: Switch, _: &mut App) -> bool {
        let live = self.0.live.borrow();
        let Some(live) = live.as_ref() else {
            return false;
        };
        self.0
            .log
            .borrow_mut()
            .push(format!("{to:?} on frame {}", live.frame));
        match to {
            Switch::To(_) => {}
            Switch::Whole => live.tx.try_send(color()).unwrap(),
            Switch::Close => live.tx.try_send(Selection::Cancelled).unwrap(),
        }
        true
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
    screen: Rc<Screen>,
    log: Log,
    toasts: Arc<MemorySink>,
    /// Pipelines of these kinds wait for `finish_pipeline`.
    slow: Rc<RefCell<Vec<CaptureKind>>>,
    pending: Rc<RefCell<Vec<async_channel::Sender<()>>>>,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let (jobs, done) = Jobs::new();
    let coordinator = CaptureCoordinator::new(jobs.clone());
    let log = Log::default();
    let screen = Rc::new(Screen {
        log: log.clone(),
        ..Screen::default()
    });
    coordinator.set_selection_flow(Rc::new(Flow(screen.clone())));
    coordinator.set_direct_capture(Rc::new(FakeDirect(log.clone())));
    let (sink, slow, pending) = (log.clone(), Rc::<RefCell<Vec<_>>>::default(), Rc::default());
    let (wait_for, waiting): (_, Rc<RefCell<Vec<async_channel::Sender<()>>>>) =
        (slow.clone(), Rc::clone(&pending));
    coordinator.set_pipeline(Rc::new(move |request, _, cx| {
        sink.borrow_mut()
            .push(format!("pipeline {:?}", request.kind));
        if !wait_for.borrow().contains(&request.kind) {
            return Task::ready(Ok(()));
        }
        let (tx, rx) = async_channel::bounded::<()>(1);
        waiting.borrow_mut().push(tx);
        cx.foreground_executor()
            .spawn(async move { rx.recv().await.map_err(|_| Error::Cancelled) })
    }));
    let toasts = MemorySink::new();
    let notifier = Arc::new(RecordingNotifier::default());
    cx.update(|cx| install_toasts(toasts.clone(), notifier, cx));
    Rig {
        jobs,
        done,
        coordinator,
        screen,
        log,
        toasts,
        slow,
        pending,
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
        cx.run_until_parked();
        job
    }

    fn release(&self, cx: &mut TestAppContext, index: usize) {
        let gate = self.screen.gates.borrow()[index].clone();
        gate.try_send(color()).unwrap();
        cx.run_until_parked();
    }

    /// The pick is over but the work goes on, as when a scroll capture starts.
    fn leave_picking(&self) {
        self.screen.live.borrow_mut().take();
    }

    fn finish_pipeline(&self, cx: &mut TestAppContext) {
        let tx = self.pending.borrow_mut().remove(0);
        tx.try_send(()).unwrap();
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

    fn busy_toasts(&self) -> usize {
        self.toasts
            .shown()
            .iter()
            .filter(|t| t.title == "A capture is in progress")
            .count()
    }
}

#[gpui::test]
fn another_mode_while_picking_takes_the_pick_over_on_the_same_frame(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.submit(cx, CaptureKind::Window);
    let second = rig.submit(cx, CaptureKind::Region);
    assert_eq!(rig.screen.freezes.get(), 1, "no second freeze");
    assert_eq!(rig.screen.open.get(), 1);

    rig.release(cx, 0);
    assert_eq!(
        rig.log(),
        ["select Window", "To(Region) on frame 1", "pipeline Region"]
    );
    assert_eq!(
        rig.finished(),
        [(second, JobStatus::Ok), (first, JobStatus::Cancelled)]
    );
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn the_same_mode_again_closes_the_pick(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.submit(cx, CaptureKind::Region);
    let second = rig.submit(cx, CaptureKind::Region);

    assert_eq!(rig.log(), ["select Region", "Close on frame 1"]);
    assert_eq!(
        rig.finished(),
        [(second, JobStatus::Ok), (first, JobStatus::Cancelled)]
    );
    assert_eq!(rig.screen.open.get(), 0);
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn fullscreen_while_picking_takes_the_frozen_screen_and_is_then_committed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.slow.borrow_mut().push(CaptureKind::Fullscreen);
    rig.submit(cx, CaptureKind::Text);
    let whole = rig.submit(cx, CaptureKind::Fullscreen);
    let refused = rig.submit(cx, CaptureKind::Color);

    assert_eq!(
        rig.log(),
        ["select Text", "Whole on frame 1", "pipeline Fullscreen"],
        "no direct grab: the frozen frame is used"
    );
    assert!(rig.finished().contains(&(refused, JobStatus::Error)));
    rig.finish_pipeline(cx);
    assert!(rig.finished().contains(&(whole, JobStatus::Ok)));
}

#[gpui::test]
fn previous_while_picking_closes_the_pick_and_takes_the_previous_target(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.submit(cx, CaptureKind::Region);
    let previous = rig.submit(cx, CaptureKind::Previous);
    assert_eq!(
        rig.log(),
        [
            "select Region",
            "Close on frame 1",
            "direct Previous",
            "pipeline Previous"
        ]
    );
    assert!(rig.finished().contains(&(previous, JobStatus::Ok)));
}

#[gpui::test]
fn a_request_while_committed_is_refused_with_a_toast_and_never_waits(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.slow.borrow_mut().push(CaptureKind::Region);
    let running = rig.submit(cx, CaptureKind::Region);
    rig.release(cx, 0);
    let refused = rig.submit(cx, CaptureKind::Window);

    assert_eq!(rig.finished(), [(refused, JobStatus::Error)]);
    assert_eq!(rig.busy_toasts(), 1);
    assert_eq!(rig.log(), ["select Region", "pipeline Region"]);
    rig.finish_pipeline(cx);
    assert_eq!(rig.finished(), [(running, JobStatus::Ok)]);
    rig.submit(cx, CaptureKind::Window);
    assert_eq!(rig.log().last().unwrap(), "select Window");
}

#[gpui::test]
fn a_second_scroll_request_stops_the_running_scroll_and_others_are_refused(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let stops = Rc::new(Cell::new(0));
    let counter = stops.clone();
    rig.coordinator
        .set_scroll_stop(Rc::new(move |_| counter.set(counter.get() + 1)));
    let scroll = rig.submit(cx, CaptureKind::Scroll);
    rig.leave_picking();
    let stop = rig.submit(cx, CaptureKind::Scroll);
    let refused = rig.submit(cx, CaptureKind::Region);

    assert_eq!(stops.get(), 1);
    assert_eq!(
        rig.finished(),
        [(stop, JobStatus::Ok), (refused, JobStatus::Error)]
    );
    assert!(rig.jobs.is_running(scroll));
    rig.release(cx, 0);
    assert_eq!(rig.finished(), [(scroll, JobStatus::Ok)]);
}

#[gpui::test]
fn a_timed_capture_is_replaceable_while_picking_and_committed_during_the_countdown(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    let (tick, ticking) = async_channel::bounded::<()>(1);
    let count = rig.log.clone();
    rig.coordinator.set_countdown(Rc::new(move |timer, cx| {
        count.borrow_mut().push(format!("countdown {timer:?}"));
        let ticking = ticking.clone();
        cx.foreground_executor()
            .spawn(async move { ticking.recv().await.map_err(|_| Error::Cancelled) })
    }));
    rig.submit_timed(cx, CaptureKind::Region, Timer::Seconds(3));
    let timed_window = rig.submit_timed(cx, CaptureKind::Window, Timer::Seconds(5));
    rig.release(cx, 0);
    let refused = rig.submit(cx, CaptureKind::Color);
    assert!(rig.finished().contains(&(refused, JobStatus::Error)));

    tick.try_send(()).unwrap();
    cx.run_until_parked();
    assert_eq!(
        rig.log(),
        [
            "select Region",
            "To(Window) on frame 1",
            "countdown Seconds(5)",
            "again color",
            "pipeline Window",
        ]
    );
    assert!(rig.finished().contains(&(timed_window, JobStatus::Ok)));
}

#[gpui::test]
fn a_screenshot_runs_while_a_recording_does(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.slow.borrow_mut().push(CaptureKind::Recording);
    let recording = rig.submit(cx, CaptureKind::Recording);
    let shot = rig.submit(cx, CaptureKind::Fullscreen);

    assert_eq!(
        rig.log(),
        [
            "direct Fullscreen",
            "pipeline Recording",
            "direct Fullscreen",
            "pipeline Fullscreen"
        ]
    );
    assert_eq!(rig.finished(), [(shot, JobStatus::Ok)]);
    assert!(rig.jobs.is_running(recording));
}

#[gpui::test]
fn a_burst_of_mixed_requests_leaves_one_overlay_and_nothing_waiting(cx: &mut TestAppContext) {
    use CaptureKind::*;
    let rig = rig(cx);
    let burst = [
        Window, Region, Region, Text, Color, Scroll, Window, TextLine, Color, Region,
    ];
    let jobs: Vec<JobId> = burst.iter().map(|k| rig.submit(cx, *k)).collect();

    assert_eq!(rig.screen.most_open.get(), 1, "never two overlays");
    assert_eq!(
        rig.screen.freezes.get(),
        2,
        "frozen again only after the toggle closed it"
    );
    let running: Vec<_> = jobs.iter().filter(|j| rig.jobs.is_running(**j)).collect();
    assert_eq!(running.len(), 1);
    let finished: Vec<JobId> = rig.finished().into_iter().map(|f| f.0).collect();
    assert_eq!(
        finished.len(),
        jobs.len() - 2,
        "all but the runner and the owner answered"
    );
    rig.release(cx, 1);
    assert_eq!(rig.log().last().unwrap(), "pipeline Region");
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn cancelling_the_request_that_took_over_ends_the_capture(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let first = rig.submit(cx, CaptureKind::Region);
    let second = rig.submit(cx, CaptureKind::Window);
    rig.finished();

    let dispatcher_cancel = rig.coordinator.cancel(second) || rig.jobs.cancel(second);
    cx.run_until_parked();
    assert!(dispatcher_cancel);
    let mut statuses = rig.finished();
    statuses.sort_by_key(|f| f.0.0);
    assert_eq!(
        statuses,
        [
            (first, JobStatus::Cancelled),
            (second, JobStatus::Cancelled)
        ]
    );
    assert_eq!(rig.screen.open.get(), 0);
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn a_claim_keeps_captures_out_and_is_refused_while_one_runs(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let claim = rig.coordinator.claim().unwrap();
    let refused = rig.submit(cx, CaptureKind::Region);
    assert_eq!(rig.finished(), [(refused, JobStatus::Error)]);
    assert!(rig.log().is_empty());
    let area = cx.update(|cx| rig.coordinator.select_area(cx));
    cx.run_until_parked();
    assert!(area.now_or_never().unwrap().is_err());
    drop(claim);

    rig.submit(cx, CaptureKind::Region);
    assert!(rig.coordinator.claim().is_err());
}

#[gpui::test]
fn the_bar_s_recording_area_is_picked_under_the_same_rule(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let area = cx.update(|cx| rig.coordinator.select_area(cx));
    cx.run_until_parked();
    let refused = rig.submit(cx, CaptureKind::Window);
    assert_eq!(rig.finished(), [(refused, JobStatus::Error)]);
    assert_eq!(rig.screen.most_open.get(), 1);
    rig.release(cx, 0);
    assert!(area.now_or_never().unwrap().is_ok());
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn a_cancelled_selection_is_a_silent_cancelled_job(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let job = rig.submit(cx, CaptureKind::Region);
    rig.screen.gates.borrow()[0]
        .try_send(Selection::Cancelled)
        .unwrap();
    cx.run_until_parked();

    assert_eq!(rig.finished(), [(job, JobStatus::Cancelled)]);
    assert_eq!(rig.log(), ["select Region"]);
    assert!(rig.coordinator.is_idle());
}

#[gpui::test]
fn a_gated_request_finishes_refused_before_anything_is_selected(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.coordinator
        .set_gate(Rc::new(|request, _| match request.kind {
            CaptureKind::RecordingArea => Err(Error::Internal("the recording is busy".into())),
            _ => Ok(()),
        }));
    let refused = rig.submit(cx, CaptureKind::RecordingArea);
    assert!(rig.log().is_empty(), "no picker opens and nothing freezes");
    let finished = rig.finished();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].0, refused);
    assert_ne!(finished[0].1, JobStatus::Ok);
    rig.submit(cx, CaptureKind::Region);
    assert_eq!(rig.log(), ["select Region"], "other captures pass the gate");
}

// The screen is frozen only while picking, so the countdown must come after the pick and the
// pipeline must get a fresh frame of the picked target, not the frozen one.
#[gpui::test]
fn a_timed_capture_picks_then_counts_down_then_captures_afresh(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let hide = rig.log.clone();
    rig.coordinator.set_before_capture(Rc::new(move |_| {
        hide.borrow_mut().push("hide picker".into())
    }));
    let count = rig.log.clone();
    rig.coordinator.set_countdown(Rc::new(move |timer, _| {
        count.borrow_mut().push(format!("countdown {timer:?}"));
        Task::ready(Ok(()))
    }));
    rig.submit_timed(cx, CaptureKind::Region, Timer::Setting);
    rig.release(cx, 0);
    rig.submit_timed(cx, CaptureKind::Fullscreen, Timer::Seconds(10));
    assert_eq!(
        rig.log(),
        [
            "hide picker",
            "select Region",
            "countdown Setting",
            "again color",
            "pipeline Region",
            "hide picker",
            "direct Fullscreen",
            "countdown Seconds(10)",
            "again color",
            "pipeline Fullscreen",
        ]
    );
}

#[gpui::test]
fn the_captured_hook_sees_the_final_selection_before_the_pipeline(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let seen = rig.log.clone();
    rig.coordinator
        .set_captured(Rc::new(move |k, selection, _| {
            seen.borrow_mut()
                .push(format!("captured {k:?} {}", kind(selection)));
        }));
    rig.submit_timed(cx, CaptureKind::Fullscreen, Timer::Seconds(3));
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
    assert!(coordinator.is_idle());
}
