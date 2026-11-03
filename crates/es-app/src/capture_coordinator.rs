use crate::errors::{Error, Result};
use crate::jobs::{JobId, Jobs};
use crate::platform_ui::{SelectMode, Selection, SelectionFlow};
use es_model::capture::CaptureVariant;
use futures::channel::oneshot;
use gpui::{App, Task};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Region,
    Fullscreen,
    Window,
    Previous,
    Timed,
    Scroll,
    Text,
    TextLine,
    Color,
    Recording,
    RecordingArea,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequest {
    pub kind: CaptureKind,
    pub variant: CaptureVariant,
}

impl CaptureRequest {
    pub fn new(kind: CaptureKind, variant: CaptureVariant) -> Self {
        Self { kind, variant }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direct {
    Fullscreen,
    Previous,
}

/// For captures that need no interactive selection.
pub trait DirectCapture {
    fn run(&self, source: Direct, cx: &mut App) -> Task<Result<Selection>>;
}

pub type Pipeline = Rc<dyn Fn(CaptureRequest, Selection, &mut App) -> Task<Result<()>>>;
pub type Countdown = Rc<dyn Fn(&mut App) -> Task<Result<()>>>;
pub type Hook = Rc<dyn Fn(&mut App)>;
pub type Gate = Rc<dyn Fn(CaptureRequest, &mut App) -> Result<()>>;

#[derive(Default, Clone)]
struct Hooks {
    flow: Option<Rc<dyn SelectionFlow>>,
    direct: Option<Rc<dyn DirectCapture>>,
    pipeline: Option<Pipeline>,
    countdown: Option<Countdown>,
    before_capture: Option<Hook>,
    scroll_stop: Option<Hook>,
    gate: Option<Gate>,
}

struct Inner {
    jobs: Jobs,
    hooks: RefCell<Hooks>,
    waiting: RefCell<VecDeque<oneshot::Sender<()>>>,
    busy: Cell<bool>,
    scrolling: Cell<bool>,
}

impl Inner {
    fn release(&self) {
        let mut waiting = self.waiting.borrow_mut();
        while let Some(next) = waiting.pop_front() {
            if next.send(()).is_ok() {
                return;
            }
        }
        self.busy.set(false);
    }
}

struct Turn(Rc<Inner>);

impl Drop for Turn {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A request waiting its turn; dropping it hands a turn it was already given to the next one.
struct Waiter {
    rx: oneshot::Receiver<()>,
    inner: Rc<Inner>,
    granted: bool,
}

impl Waiter {
    async fn turn(mut self) -> Result<Turn> {
        (&mut self.rx).await.map_err(|_| Error::Cancelled)?;
        self.granted = true;
        Ok(Turn(self.inner.clone()))
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        if self.granted {
            return;
        }
        self.rx.close();
        if matches!(self.rx.try_recv(), Ok(Some(()))) {
            self.inner.release();
        }
    }
}

struct ScrollFlag(Rc<Inner>);

impl Drop for ScrollFlag {
    fn drop(&mut self) {
        self.0.scrolling.set(false);
    }
}

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
                waiting: RefCell::default(),
                busy: Cell::new(false),
                scrolling: Cell::new(false),
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

    /// Runs before every capture starts, so the recording picker is gone from the frame.
    pub fn set_before_capture(&self, hook: Hook) {
        self.inner.hooks.borrow_mut().before_capture = Some(hook);
    }

    /// Refuses a request before it waits, selects or captures anything.
    pub fn set_gate(&self, gate: Gate) {
        self.inner.hooks.borrow_mut().gate = Some(gate);
    }

    /// Finishes the running scroll capture.
    pub fn set_scroll_stop(&self, hook: Hook) {
        self.inner.hooks.borrow_mut().scroll_stop = Some(hook);
    }

    /// Runs `job` once every earlier request has finished.
    pub fn submit(&self, job: JobId, request: CaptureRequest, cx: &mut App) {
        let inner = self.inner.clone();
        let gate = inner.hooks.borrow().gate.clone();
        if let Some(gate) = gate
            && let Err(e) = gate(request, cx)
        {
            inner.jobs.report(job, &Err(e));
            return;
        }
        if request.kind == CaptureKind::Scroll && inner.scrolling.get() {
            let stop = inner.hooks.borrow().scroll_stop.clone();
            if let Some(stop) = stop {
                stop(cx);
            }
            inner.jobs.report(job, &Ok(()));
            return;
        }
        let waiter = if inner.busy.replace(true) {
            let (tx, rx) = oneshot::channel();
            inner.waiting.borrow_mut().push_back(tx);
            Some(Waiter {
                rx,
                inner: inner.clone(),
                granted: false,
            })
        } else {
            None
        };
        let async_cx = cx.to_async();
        let jobs = inner.jobs.clone();
        jobs.spawn(job, cx, move |_| async move {
            let _turn = match waiter {
                Some(waiter) => waiter.turn().await?,
                None => Turn(inner.clone()),
            };
            let _scroll = (request.kind == CaptureKind::Scroll).then(|| {
                inner.scrolling.set(true);
                ScrollFlag(inner.clone())
            });
            let hooks = inner.hooks.borrow().clone();
            if let Some(before) = &hooks.before_capture {
                async_cx.update(|cx| before(cx));
            }
            if request.kind == CaptureKind::Timed
                && let Some(countdown) = &hooks.countdown
            {
                async_cx.update(|cx| countdown(cx)).await?;
            }
            let selection = async_cx
                .update(|cx| select(request.kind, &hooks, cx))?
                .await?;
            if matches!(selection, Selection::Cancelled) {
                return Err(Error::Cancelled);
            }
            let pipeline = hooks.pipeline.ok_or(Error::Unsupported)?;
            async_cx.update(|cx| pipeline(request, selection, cx)).await
        });
    }
}

fn select(kind: CaptureKind, hooks: &Hooks, cx: &mut App) -> Result<Task<Result<Selection>>> {
    let interactive = |mode, cx: &mut App| {
        hooks
            .flow
            .as_ref()
            .map(|flow| flow.run(mode, cx))
            .ok_or(Error::Unsupported)
    };
    let direct = |source, cx: &mut App| {
        hooks
            .direct
            .as_ref()
            .map(|direct| direct.run(source, cx))
            .ok_or(Error::Unsupported)
    };
    match kind {
        CaptureKind::Fullscreen | CaptureKind::Recording => direct(Direct::Fullscreen, cx),
        CaptureKind::Previous => direct(Direct::Previous, cx),
        CaptureKind::Region | CaptureKind::Timed | CaptureKind::RecordingArea => {
            interactive(SelectMode::Region, cx)
        }
        CaptureKind::Window => interactive(SelectMode::Window, cx),
        CaptureKind::Scroll => interactive(SelectMode::Scroll, cx),
        CaptureKind::Text | CaptureKind::TextLine => interactive(SelectMode::Text, cx),
        CaptureKind::Color => interactive(SelectMode::Color, cx),
    }
}
