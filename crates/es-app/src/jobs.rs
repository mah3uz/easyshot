use crate::errors::{Error, Result, status_of};
use es_protocol::JobStatus;
use gpui::{App, Task};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFinished {
    pub id: JobId,
    pub status: JobStatus,
    pub detail: Option<String>,
}

struct Running {
    _task: Task<()>,
    cancel: CancelToken,
}

struct Inner {
    next: Cell<u64>,
    running: RefCell<HashMap<JobId, Running>>,
    finished: async_channel::Sender<JobFinished>,
}

impl Inner {
    fn finish(&self, id: JobId, status: JobStatus, detail: Option<String>) -> bool {
        let Some(job) = self.running.borrow_mut().remove(&id) else {
            return false;
        };
        job.cancel.0.store(true, Ordering::Relaxed);
        let _ = self.finished.try_send(JobFinished { id, status, detail });
        true
    }
}

#[derive(Clone)]
pub struct Jobs {
    inner: Rc<Inner>,
}

impl Jobs {
    /// The receiver carries every finished job, for the D-Bus layer to forward.
    pub fn new() -> (Self, async_channel::Receiver<JobFinished>) {
        let (finished, rx) = async_channel::unbounded();
        let jobs = Self {
            inner: Rc::new(Inner {
                next: Cell::new(1),
                running: RefCell::default(),
                finished,
            }),
        };
        (jobs, rx)
    }

    pub fn allocate(&self) -> JobId {
        let id = self.inner.next.get();
        self.inner.next.set(id + 1);
        JobId(id)
    }

    pub fn is_running(&self, id: JobId) -> bool {
        self.inner.running.borrow().contains_key(&id)
    }

    pub fn spawn<Fut>(
        &self,
        id: JobId,
        cx: &App,
        f: impl FnOnce(CancelToken) -> Fut + 'static,
    ) -> CancelToken
    where
        Fut: Future<Output = Result<()>> + 'static,
    {
        self.spawn_answer(id, cx, move |cancel| {
            let work = f(cancel);
            async move {
                let result = work.await;
                (status_of(&result), result.err().map(|e| e.to_string()))
            }
        })
    }

    /// For work whose answer is a detail even when it succeeds.
    pub fn spawn_answer<Fut>(
        &self,
        id: JobId,
        cx: &App,
        f: impl FnOnce(CancelToken) -> Fut + 'static,
    ) -> CancelToken
    where
        Fut: Future<Output = (JobStatus, Option<String>)> + 'static,
    {
        let cancel = CancelToken::default();
        let inner = self.inner.clone();
        let work = f(cancel.clone());
        let task = cx.foreground_executor().spawn(async move {
            let (status, detail) = work.await;
            inner.finish(id, status, detail);
        });
        self.inner.running.borrow_mut().insert(
            id,
            Running {
                _task: task,
                cancel: cancel.clone(),
            },
        );
        cancel
    }

    /// Drops the work at its next await point and reports `Cancelled`.
    pub fn cancel(&self, id: JobId) -> bool {
        self.inner.finish(id, JobStatus::Cancelled, None)
    }

    /// For commands handled synchronously, so they still report a result.
    pub fn report(&self, id: JobId, result: &Result<()>) {
        let detail = result.as_ref().err().map(ToString::to_string);
        let _ = self.inner.finished.try_send(JobFinished {
            id,
            status: status_of(result),
            detail,
        });
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_owned())
}

pub fn run_cpu<T: Send + 'static>(
    cx: &App,
    f: impl FnOnce() -> T + Send + 'static,
) -> Task<Result<T>> {
    cx.background_executor().spawn(async move {
        catch_unwind(AssertUnwindSafe(f)).map_err(|p| Error::Internal(panic_message(&*p)))
    })
}
