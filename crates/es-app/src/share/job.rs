use super::{FileShare, ShareJobEvent, ShareJobId, ShareJobKind, Shares};
use crate::jobs::{JobId, Jobs};
use crate::services::Strings;
use es_protocol::JobStatus;
use gpui::{App, Entity};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

const CANCEL_POLL: Duration = Duration::from_millis(200);

/// Shares a file for a command and finishes `job` with the page link, or with the failure in words.
pub fn run_route(shares: &Entity<Shares>, jobs: &Jobs, job: JobId, file: FileShare, cx: &mut App) {
    let (tx, rx) = async_channel::bounded(1);
    let mine: Rc<Cell<Option<ShareJobId>>> = Rc::default();
    let theirs = mine.clone();
    let events = cx.subscribe(shares, move |_, event: &ShareJobEvent, _| {
        if theirs.get() != Some(event.id) {
            return;
        }
        let answer = match &event.kind {
            ShareJobKind::Preparing { .. }
            | ShareJobKind::Reserved { .. }
            | ShareJobKind::Progress { .. } => return,
            ShareJobKind::Done(record) => (JobStatus::Ok, Some(record.page_url.clone())),
            ShareJobKind::Failed(failure) => (JobStatus::Error, Some(failure.message(&Strings))),
            ShareJobKind::Cancelled => (JobStatus::Cancelled, None),
        };
        let _ = tx.try_send(answer);
    });
    let cancel = jobs.spawn_answer(job, cx, move |_| async move {
        let _events = events;
        rx.recv().await.unwrap_or((JobStatus::Error, None))
    });
    let id = shares.update(cx, |s, cx| s.start_file(file, cx));
    mine.set(Some(id));
    // Cancelling a job drops its task, so the upload is stopped from here.
    let shares = shares.downgrade();
    cx.spawn(async move |cx| {
        while !cancel.is_cancelled() {
            cx.background_executor().timer(CANCEL_POLL).await;
        }
        shares.update(cx, |s, cx| s.cancel(id, cx)).ok();
    })
    .detach();
}
