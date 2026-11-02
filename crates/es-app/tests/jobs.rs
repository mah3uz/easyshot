#![allow(clippy::unwrap_used)]
use es_app::errors::Error;
use es_app::jobs::{JobFinished, Jobs, run_cpu};
use es_protocol::JobStatus;
use gpui::TestAppContext;

fn finished(rx: &async_channel::Receiver<JobFinished>) -> JobFinished {
    rx.try_recv().unwrap()
}

#[gpui::test]
fn ids_are_unique(_cx: &mut TestAppContext) {
    let (jobs, _rx) = Jobs::new();
    assert_ne!(jobs.allocate(), jobs.allocate());
}

#[gpui::test]
fn a_finished_job_reports_ok_once(cx: &mut TestAppContext) {
    let (jobs, rx) = Jobs::new();
    let id = jobs.allocate();
    cx.update(|cx| jobs.spawn(id, cx, |_| async { Ok(()) }));
    cx.run_until_parked();
    let done = finished(&rx);
    assert_eq!(
        (done.id, done.status, done.detail),
        (id, JobStatus::Ok, None)
    );
    assert!(rx.try_recv().is_err());
    assert!(!jobs.is_running(id));
}

#[gpui::test]
fn a_failed_job_reports_error_with_its_text(cx: &mut TestAppContext) {
    let (jobs, rx) = Jobs::new();
    let id = jobs.allocate();
    cx.update(|cx| jobs.spawn(id, cx, |_| async { Err(Error::Timeout) }));
    cx.run_until_parked();
    let done = finished(&rx);
    assert_eq!(done.status, JobStatus::Error);
    assert_eq!(done.detail.as_deref(), Some("timed out"));
}

#[gpui::test]
fn a_job_that_returns_cancelled_reports_cancelled(cx: &mut TestAppContext) {
    let (jobs, rx) = Jobs::new();
    let id = jobs.allocate();
    cx.update(|cx| jobs.spawn(id, cx, |_| async { Err(Error::Cancelled) }));
    cx.run_until_parked();
    assert_eq!(finished(&rx).status, JobStatus::Cancelled);
}

#[gpui::test]
fn cancel_stops_the_work_and_reports_once(cx: &mut TestAppContext) {
    let (jobs, rx) = Jobs::new();
    let id = jobs.allocate();
    let token = cx.update(|cx| {
        jobs.spawn(id, cx, |_| async {
            std::future::pending::<()>().await;
            Ok(())
        })
    });
    cx.run_until_parked();
    assert!(jobs.is_running(id));
    assert!(jobs.cancel(id));
    assert!(token.is_cancelled());
    assert!(!jobs.cancel(id));
    cx.run_until_parked();
    assert_eq!(finished(&rx).status, JobStatus::Cancelled);
    assert!(rx.try_recv().is_err());
}

#[gpui::test]
fn synchronous_commands_report_through_the_same_channel(_cx: &mut TestAppContext) {
    let (jobs, rx) = Jobs::new();
    let id = jobs.allocate();
    jobs.report(id, &Ok(()));
    assert_eq!(finished(&rx).status, JobStatus::Ok);
}

#[gpui::test]
fn a_panic_in_cpu_work_becomes_an_error(cx: &mut TestAppContext) {
    let task = cx.update(|cx| run_cpu(cx, || -> u32 { panic!("boom") }));
    let result = cx.foreground_executor().block_test(task);
    assert!(matches!(result, Err(Error::Internal(m)) if m == "boom"));
}

#[gpui::test]
fn cpu_work_returns_its_value(cx: &mut TestAppContext) {
    let task = cx.update(|cx| run_cpu(cx, || 7u32));
    assert_eq!(cx.foreground_executor().block_test(task).unwrap(), 7);
}
