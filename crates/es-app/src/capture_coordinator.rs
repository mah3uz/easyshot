use crate::errors::{Error, Result};
use crate::jobs::{JobId, Jobs};
use crate::platform_ui::{SelectMode, Selection, SelectionFlow, Switch};
use crate::toast::{IconName, Toast, show_toast};
use es_model::capture::CaptureVariant;
use gpui::{App, AsyncApp, Task};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// A closed layer surface stays on screen until the compositor repaints without it.
pub const UNMAP_SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Region,
    Fullscreen,
    Window,
    Previous,
    Scroll,
    ScrollRegion,
    Text,
    TextLine,
    Color,
    Recording,
    RecordingArea,
    /// An area handed back to the caller that asked for it, as the capture bar's recording area.
    Area,
}

impl CaptureKind {
    fn scrolls(self) -> bool {
        matches!(self, CaptureKind::Scroll | CaptureKind::ScrollRegion)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Timer {
    #[default]
    Off,
    /// The self-timer setting.
    Setting,
    Seconds(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequest {
    pub kind: CaptureKind,
    pub variant: CaptureVariant,
    pub timer: Timer,
}

impl CaptureRequest {
    pub fn new(kind: CaptureKind, variant: CaptureVariant) -> Self {
        Self {
            kind,
            variant,
            timer: Timer::Off,
        }
    }

    pub fn timed(self, timer: Timer) -> Self {
        Self { timer, ..self }
    }
}

#[derive(Debug, Clone)]
pub enum Direct {
    Fullscreen,
    Previous,
    /// A fresh frame of what was picked, taken once the timer has run out.
    Again(Selection),
}

/// For captures that need no interactive selection.
pub trait DirectCapture {
    fn run(&self, source: Direct, cx: &mut App) -> Task<Result<Selection>>;
}

pub type Pipeline = Rc<dyn Fn(CaptureRequest, Selection, &mut App) -> Task<Result<()>>>;
pub type Countdown = Rc<dyn Fn(Timer, &mut App) -> Task<Result<()>>>;
pub type Hook = Rc<dyn Fn(&mut App)>;
/// True when it closed a surface that could still be on screen.
pub type Closing = Rc<dyn Fn(&mut App) -> bool>;
pub type Gate = Rc<dyn Fn(CaptureRequest, &mut App) -> Result<()>>;
pub type Captured = Rc<dyn Fn(CaptureKind, &Selection, &mut App)>;

#[derive(Default, Clone)]
struct Hooks {
    flow: Option<Rc<dyn SelectionFlow>>,
    direct: Option<Rc<dyn DirectCapture>>,
    pipeline: Option<Pipeline>,
    countdown: Option<Countdown>,
    before_capture: Option<Closing>,
    captured: Option<Captured>,
    scroll_stop: Option<Hook>,
    gate: Option<Gate>,
}

#[derive(Debug, Clone, Copy)]
struct Current {
    request: CaptureRequest,
    /// The job whose task does the work.
    runner: JobId,
    /// The job the outcome is reported to: the runner, or the request that took its pick over.
    owner: JobId,
}

#[derive(Debug, Clone, Copy)]
enum Phase {
    Idle,
    Picking(Current),
    Committed(Current),
    /// Taken by a claim, which answers to no request.
    Held,
}

struct Inner {
    jobs: Jobs,
    hooks: RefCell<Hooks>,
    phase: Cell<Phase>,
    /// Where an `Area` pick goes; dropping it answers the caller that the pick ended.
    area: RefCell<Option<async_channel::Sender<Result<Selection>>>>,
}

pub fn busy() -> Error {
    Error::Internal("a capture is in progress".into())
}

fn busy_toast(cx: &mut App) {
    show_toast(Toast::new("toast-capture-busy", IconName::Info), cx);
}

/// The capture task's hold on the phase; dropping it while it holds ends the capture.
struct Turn {
    inner: Rc<Inner>,
    runner: JobId,
    held: bool,
}

impl Turn {
    fn current(&self) -> Option<Current> {
        match self.inner.phase.get() {
            Phase::Picking(c) | Phase::Committed(c) if self.held && c.runner == self.runner => {
                Some(c)
            }
            _ => None,
        }
    }

    fn commit(&self, fallback: Current) -> Current {
        let Some(c) = self.current() else {
            return fallback;
        };
        self.inner.phase.set(Phase::Committed(c));
        c
    }

    fn release(&mut self) {
        if self.current().is_some() {
            self.inner.phase.set(Phase::Idle);
        }
        self.held = false;
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let Some(c) = self.current() else {
            return;
        };
        if c.owner != c.runner {
            self.inner.jobs.report(c.owner, &Err(Error::Cancelled));
        }
        self.inner.area.borrow_mut().take();
        self.inner.phase.set(Phase::Idle);
    }
}

/// Keeps every other capture out until dropped.
pub struct Claim(Rc<Inner>);

impl Drop for Claim {
    fn drop(&mut self) {
        if matches!(self.0.phase.get(), Phase::Held) {
            self.0.phase.set(Phase::Idle);
        }
    }
}

/// The single owner of what is being captured: one capture at a time, never queued.
#[derive(Clone)]
pub struct CaptureCoordinator {
    inner: Rc<Inner>,
}

impl CaptureCoordinator {
    pub fn new(jobs: Jobs) -> Self {
        Self {
            inner: Rc::new(Inner {
                jobs,
                hooks: RefCell::default(),
                phase: Cell::new(Phase::Idle),
                area: RefCell::default(),
            }),
        }
    }

    pub fn set_selection_flow(&self, flow: Rc<dyn SelectionFlow>) {
        self.inner.hooks.borrow_mut().flow = Some(flow);
    }

    pub fn set_direct_capture(&self, direct: Rc<dyn DirectCapture>) {
        self.inner.hooks.borrow_mut().direct = Some(direct);
    }

    pub fn set_pipeline(&self, pipeline: Pipeline) {
        self.inner.hooks.borrow_mut().pipeline = Some(pipeline);
    }

    pub fn set_countdown(&self, countdown: Countdown) {
        self.inner.hooks.borrow_mut().countdown = Some(countdown);
    }

    /// Runs as a capture is claimed, so the capture bar is gone from the frame.
    pub fn set_before_capture(&self, hook: Closing) {
        self.inner.hooks.borrow_mut().before_capture = Some(hook);
    }

    /// Refuses a request before it selects or captures anything.
    pub fn set_gate(&self, gate: Gate) {
        self.inner.hooks.borrow_mut().gate = Some(gate);
    }

    /// Sees what each capture took before the pipeline does, so it can be taken again.
    pub fn set_captured(&self, hook: Captured) {
        self.inner.hooks.borrow_mut().captured = Some(hook);
    }

    /// Finishes the running scroll capture.
    pub fn set_scroll_stop(&self, hook: Hook) {
        self.inner.hooks.borrow_mut().scroll_stop = Some(hook);
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.inner.phase.get(), Phase::Idle)
    }

    /// Starts `request`, folds it into the pick already running, or refuses it.
    pub fn submit(&self, job: JobId, request: CaptureRequest, cx: &mut App) {
        let gate = self.inner.hooks.borrow().gate.clone();
        if let Some(gate) = gate
            && let Err(e) = gate(request, cx)
        {
            self.inner.jobs.report(job, &Err(e));
            return;
        }
        let running = match self.inner.phase.get() {
            Phase::Idle => return self.start(job, request, cx),
            Phase::Picking(current) if self.replace(job, current, request, cx) => return,
            Phase::Picking(current) | Phase::Committed(current) => Some(current),
            Phase::Held => None,
        };
        let scrolling = running.is_some_and(|c| c.request.kind.scrolls());
        if request.kind.scrolls() && scrolling {
            let stop = self.inner.hooks.borrow().scroll_stop.clone();
            if let Some(stop) = stop {
                stop(cx);
            }
            self.inner.jobs.report(job, &Ok(()));
            return;
        }
        busy_toast(cx);
        self.inner.jobs.report(job, &Err(busy()));
    }

    /// The mode already shown closes the pick; another one takes it over on the same frames.
    fn replace(&self, job: JobId, current: Current, request: CaptureRequest, cx: &mut App) -> bool {
        let flow = self.inner.hooks.borrow().flow.clone();
        let Some(flow) = flow else {
            return false;
        };
        let wanted = switch_for(request.kind);
        let same = matches!(wanted, Switch::To(mode) if flow.showing() == Some(mode));
        let to = if same { Switch::Close } else { wanted };
        if !flow.switch(to, cx) {
            return false;
        }
        if same {
            self.inner.jobs.report(job, &Ok(()));
            return true;
        }
        self.answer_replaced(current);
        let next = Current {
            request,
            owner: job,
            ..current
        };
        self.inner.phase.set(match to {
            Switch::To(_) => Phase::Picking(next),
            Switch::Whole | Switch::Close => Phase::Committed(next),
        });
        true
    }

    fn answer_replaced(&self, current: Current) {
        let cancelled = Err(Error::Cancelled);
        if current.owner == current.runner {
            self.inner.area.borrow_mut().take();
            self.inner.jobs.answer(current.runner, &cancelled);
        } else {
            self.inner.jobs.report(current.owner, &cancelled);
        }
    }

    fn start(&self, job: JobId, request: CaptureRequest, cx: &mut App) {
        let inner = self.inner.clone();
        let first = Current {
            request,
            runner: job,
            owner: job,
        };
        inner.phase.set(match source(request.kind) {
            Source::Pick(_) => Phase::Picking(first),
            Source::Direct(_) => Phase::Committed(first),
        });
        let mut turn = Turn {
            inner: inner.clone(),
            runner: job,
            held: true,
        };
        let hooks = inner.hooks.borrow().clone();
        let closed = hooks.before_capture.as_ref().is_some_and(|close| close(cx));
        let async_cx = cx.to_async();
        let jobs = inner.jobs.clone();
        jobs.spawn(job, cx, move |_| async move {
            if closed {
                settle(&async_cx, UNMAP_SETTLE).await;
            }
            let picked = match async_cx.update(|cx| select(request.kind, &hooks, cx)) {
                Ok(task) => task.await,
                Err(e) => Err(e),
            };
            let current = turn.commit(first);
            if current.owner == job
                && let Some(area) = inner.area.borrow_mut().take()
            {
                turn.release();
                let _ = area.try_send(picked);
                return Ok(());
            }
            let result = finish(current.request, request.kind, picked, &hooks, &async_cx).await;
            turn.release();
            if current.owner == job {
                return result;
            }
            inner.jobs.report(current.owner, &result);
            Err(Error::Cancelled)
        });
    }

    /// For a capture that picks nothing, such as an agent's: refused while another one runs.
    pub fn claim(&self) -> Result<Claim> {
        match self.inner.phase.get() {
            Phase::Idle => {
                self.inner.phase.set(Phase::Held);
                Ok(Claim(self.inner.clone()))
            }
            _ => Err(busy()),
        }
    }

    /// An area picked for the capture bar's recording, which another request can take over.
    pub fn select_area(&self, cx: &mut App) -> Task<Result<Selection>> {
        if !self.is_idle() {
            busy_toast(cx);
            return Task::ready(Err(Error::Cancelled));
        }
        if self.inner.hooks.borrow().flow.is_none() {
            return Task::ready(Err(Error::Unsupported));
        }
        let (tx, rx) = async_channel::bounded(1);
        *self.inner.area.borrow_mut() = Some(tx);
        let request = CaptureRequest::new(CaptureKind::Area, CaptureVariant::Normal);
        self.start(self.inner.jobs.allocate(), request, cx);
        cx.spawn(async move |_| match rx.recv().await {
            Ok(Ok(Selection::Cancelled)) => Err(Error::Cancelled),
            Ok(Ok(picked)) => Ok(picked),
            // The flow has shown why a pick failed; its caller only learns that it ended.
            Ok(Err(_)) | Err(_) => Err(Error::Cancelled),
        })
    }

    /// A request that took a pick over is cancelled through the job that runs it; the request it
    /// replaced was answered then, and its cancel no longer reaches the work.
    pub fn cancel(&self, job: JobId) -> bool {
        match self.inner.phase.get() {
            Phase::Picking(c) | Phase::Committed(c) if c.runner == job && c.owner != job => true,
            Phase::Picking(c) | Phase::Committed(c) if c.owner == job && c.runner != job => {
                self.inner.jobs.cancel(c.runner)
            }
            _ => false,
        }
    }
}

/// Waits after a surface of ours closes, before the screen is grabbed.
pub async fn settle(cx: &AsyncApp, wait: Duration) {
    cx.background_executor().timer(wait).await;
}

async fn finish(
    request: CaptureRequest,
    first: CaptureKind,
    picked: Result<Selection>,
    hooks: &Hooks,
    cx: &AsyncApp,
) -> Result<()> {
    let picked = match picked? {
        Selection::Cancelled if request.kind == CaptureKind::Previous && first != request.kind => {
            settle(cx, UNMAP_SETTLE).await;
            cx.update(|cx| direct(hooks, Direct::Previous, cx))?.await?
        }
        Selection::Cancelled => return Err(Error::Cancelled),
        picked => picked,
    };
    let selection = if request.timer == Timer::Off {
        picked
    } else {
        if let Some(countdown) = &hooks.countdown {
            cx.update(|cx| countdown(request.timer, cx)).await?;
        }
        cx.update(|cx| direct(hooks, Direct::Again(picked), cx))?
            .await?
    };
    if let Some(captured) = &hooks.captured {
        cx.update(|cx| captured(request.kind, &selection, cx));
    }
    let pipeline = hooks.pipeline.clone().ok_or(Error::Unsupported)?;
    cx.update(|cx| pipeline(request, selection, cx)).await
}

enum Source {
    Pick(SelectMode),
    Direct(Direct),
}

fn source(kind: CaptureKind) -> Source {
    match kind {
        CaptureKind::Fullscreen | CaptureKind::Recording => Source::Direct(Direct::Fullscreen),
        CaptureKind::Previous => Source::Direct(Direct::Previous),
        CaptureKind::Region | CaptureKind::RecordingArea | CaptureKind::Area => {
            Source::Pick(SelectMode::Region)
        }
        CaptureKind::Window => Source::Pick(SelectMode::Window),
        CaptureKind::Scroll => Source::Pick(SelectMode::Scroll),
        CaptureKind::ScrollRegion => Source::Pick(SelectMode::ScrollRegion),
        CaptureKind::Text | CaptureKind::TextLine => Source::Pick(SelectMode::Text),
        CaptureKind::Color => Source::Pick(SelectMode::Color),
    }
}

fn switch_for(kind: CaptureKind) -> Switch {
    match source(kind) {
        Source::Pick(mode) => Switch::To(mode),
        Source::Direct(Direct::Previous) => Switch::Close,
        Source::Direct(_) => Switch::Whole,
    }
}

fn select(kind: CaptureKind, hooks: &Hooks, cx: &mut App) -> Result<Task<Result<Selection>>> {
    match source(kind) {
        Source::Pick(mode) => hooks
            .flow
            .as_ref()
            .map(|flow| flow.run(mode, cx))
            .ok_or(Error::Unsupported),
        Source::Direct(from) => direct(hooks, from, cx),
    }
}

fn direct(hooks: &Hooks, source: Direct, cx: &mut App) -> Result<Task<Result<Selection>>> {
    hooks
        .direct
        .as_ref()
        .map(|direct| direct.run(source, cx))
        .ok_or(Error::Unsupported)
}
