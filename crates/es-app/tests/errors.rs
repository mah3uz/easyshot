#![allow(clippy::unwrap_used)]
use es_app::errors::{Context, Error, Op, Presentation, present, status_of, title_id};
use es_app::strings::loader;
use es_app::toast::{MemorySink, Toast, ToastRequest, ToastStyle, install_toasts, show_toast};
use es_desktop::RecordingNotifier;
use es_protocol::JobStatus;
use gpui::TestAppContext;
use std::sync::Arc;

const OPS: [Op; 8] = [
    Op::Capture,
    Op::Save,
    Op::Copy,
    Op::Ocr,
    Op::Recording,
    Op::Scroll,
    Op::Open,
    Op::Other,
];

fn samples() -> Vec<Error> {
    vec![
        Error::Unsupported,
        Error::Timeout,
        Error::NotFound("x".into()),
        Error::Protocol("x".into()),
        Error::Store("x".into()),
        Error::Internal("x".into()),
    ]
}

#[test]
fn cancelled_is_never_shown() {
    for op in OPS {
        assert!(matches!(
            present(&Error::Cancelled, Context::toast(op)),
            Presentation::Silent
        ));
    }
}

#[test]
fn a_config_parse_error_is_a_notification_with_line_and_column() {
    let error = Error::Parse {
        line: 12,
        col: 7,
        message: "bad value".into(),
    };
    let Presentation::Notification(n) = present(&error, Context::toast(Op::Other)) else {
        panic!("expected a notification");
    };
    assert!(n.body.contains("12") && n.body.contains('7') && n.body.contains("bad value"));
}

#[test]
fn the_context_picks_the_title_and_the_surface() {
    let error = Error::Io {
        path: "/x".into(),
        source: std::io::Error::other("disk full"),
    };
    let Presentation::Toast(t) = present(&error, Context::toast(Op::Save)) else {
        panic!("expected a toast");
    };
    assert_eq!(t.title_id, "toast-capture-save-failed");
    assert_eq!(t.style, ToastStyle::Error);

    let Presentation::Notification(n) = present(&error, Context::notification(Op::Copy)) else {
        panic!("expected a notification");
    };
    assert_eq!(n.title, "Copy Failed");

    let Presentation::Inline(text) = present(&error, Context::inline(Op::Ocr)) else {
        panic!("expected inline text");
    };
    assert!(text.contains("recognize"));
}

#[test]
fn every_title_and_message_the_presenter_can_choose_exists_in_the_catalogue() {
    let l = loader();
    for op in OPS {
        for error in samples() {
            let id = title_id(&error, op);
            assert!(l.has(id), "missing {id}");
        }
    }
}

#[test]
fn job_status_follows_the_error_kind() {
    assert_eq!(status_of(&Ok(())), JobStatus::Ok);
    assert_eq!(
        status_of::<()>(&Err(Error::Cancelled)),
        JobStatus::Cancelled
    );
    assert_eq!(status_of::<()>(&Err(Error::Timeout)), JobStatus::Error);
}

#[gpui::test]
fn a_toast_goes_to_the_surface_when_there_is_one(cx: &mut TestAppContext) {
    let sink = MemorySink::new();
    let notifier = Arc::new(RecordingNotifier::default());
    cx.update(|cx| {
        install_toasts(sink.clone(), notifier.clone(), cx);
        show_toast(Toast::error("toast-copy-failed").arg("error", "nope"), cx);
    });
    let shown = sink.shown();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].title, "Copy Failed");
    assert_eq!(shown[0].message.as_deref(), Some("nope"));
    assert!(notifier.sent().is_empty());
}

#[gpui::test]
fn with_no_surface_a_toast_becomes_a_notification(cx: &mut TestAppContext) {
    let sink = MemorySink::without_surface();
    let notifier = Arc::new(RecordingNotifier::default());
    cx.update(|cx| {
        install_toasts(sink.clone(), notifier.clone(), cx);
        show_toast(Toast::error("toast-copy-failed"), cx);
    });
    assert!(sink.shown().is_empty());
    let sent = notifier.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].title, "Copy Failed");
}

#[test]
fn a_toast_message_uses_the_catalogue_text_when_it_has_one() {
    let request =
        ToastRequest::resolve(&Toast::error("toast-capture-save-failed").arg("error", "x"));
    assert!(request.message.unwrap().contains("still in the deck"));
}
