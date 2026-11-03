use super::*;
use es_app::toast::{MemorySink, install_toasts};
use es_desktop::RecordingNotifier;
use es_geom::Rect;
use es_model::platform::Transform;
use es_ocr::{Recognized, Word};
use gpui::TestAppContext;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn frame() -> Frame {
    let (w, h) = (200u32, 60u32);
    let bgra: Vec<u8> = (0..w * h)
        .flat_map(|i| {
            let v = if (i % w / 2 + i / w / 2) % 2 == 0 {
                20
            } else {
                235
            };
            [v, v, v, 0]
        })
        .collect();
    Frame {
        output: None,
        size_px: (w, h),
        logical: Rect::new(0., 0., f64::from(w), f64::from(h)),
        scale: 1.,
        bgra: bgra.into(),
        transform: Transform::Normal,
        captured_at: Instant::now(),
    }
}

fn word(text: &str, x: f64, line: u32) -> Word {
    Word {
        text: text.into(),
        rect: Rect::new(x, 10.0, x + 80.0, 22.0),
        confidence: 90.0,
        line,
    }
}

fn reading(calls: Arc<AtomicUsize>) -> Recognizer {
    Arc::new(move |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(Recognized {
            words: vec![word("password:", 10.0, 0), word("hunter2horse", 100.0, 0)],
            ..Recognized::default()
        })
    })
}

fn without_tesseract() -> Recognizer {
    Arc::new(|_, _| Err(es_ocr::Error::NotInstalled))
}

fn languages() -> Vec<String> {
    vec!["eng".into()]
}

#[test]
fn a_secret_on_the_screen_gives_the_agent_a_covered_frame_and_a_count() {
    let calls = Arc::new(AtomicUsize::new(0));
    let original = frame();
    let verdict = inspect(true, &original, &languages(), &reading(calls)).unwrap();
    let Verdict::Covered(copy, regions) = verdict else {
        panic!("not covered");
    };
    assert_eq!(regions, 1);
    assert_eq!(copy.size_px, original.size_px);
    assert_ne!(copy.bgra, original.bgra, "pixels changed");
}

#[gpui::test]
fn with_redaction_off_nothing_is_read_and_the_reply_says_why(cx: &mut TestAppContext) {
    let calls = Arc::new(AtomicUsize::new(0));
    let verdict = inspect(false, &frame(), &languages(), &reading(calls.clone())).unwrap();
    assert!(matches!(verdict, Verdict::Off));
    assert_eq!(calls.load(Ordering::SeqCst), 0, "no recognition ran");
    let notifier = Arc::new(RecordingNotifier::default());
    let redactor = Redactor::new(without_tesseract());
    cx.update(|cx| install_toasts(MemorySink::without_surface(), notifier.clone(), cx));
    let (copy, redaction) = cx.update(|cx| redactor.settle(verdict, cx));
    assert!(copy.is_none());
    assert_eq!(redaction, Redaction::skipped("off"));
    assert!(notifier.sent().is_empty());
}

#[gpui::test]
fn a_screen_without_secrets_is_sent_as_captured_and_says_nothing_was_found(
    cx: &mut TestAppContext,
) {
    let silent: Recognizer = Arc::new(|_, _| Ok(Recognized::default()));
    let verdict = inspect(true, &frame(), &languages(), &silent).unwrap();
    let redactor = Redactor::new(silent);
    let (copy, redaction) = cx.update(|cx| redactor.settle(verdict, cx));
    assert!(copy.is_none());
    assert_eq!(redaction, Redaction::applied(0));
}

#[gpui::test]
fn a_missing_tesseract_lets_the_capture_through_unredacted_and_tells_the_user_once(
    cx: &mut TestAppContext,
) {
    let notifier = Arc::new(RecordingNotifier::default());
    let redactor = Redactor::new(without_tesseract());
    cx.update(|cx| install_toasts(MemorySink::without_surface(), notifier.clone(), cx));
    for _ in 0..2 {
        let verdict = inspect(true, &frame(), &languages(), &redactor.recognizer).unwrap();
        assert!(matches!(verdict, Verdict::TesseractMissing));
        let (copy, redaction) = cx.update(|cx| redactor.settle(verdict, cx));
        assert!(copy.is_none(), "the original goes on");
        assert_eq!(redaction, Redaction::skipped("tesseract_missing"));
    }
    let sent = notifier.sent();
    assert_eq!(sent.len(), 1, "told once per daemon session");
    assert!(sent[0].body.contains("tesseract"), "{:?}", sent[0].body);
}

#[test]
fn a_recognizer_that_fails_for_another_reason_fails_the_capture_rather_than_leaking_it() {
    let broken: Recognizer = Arc::new(|_, _| Err(es_ocr::Error::Timeout));
    let Err(Fail(code, message)) = inspect(true, &frame(), &languages(), &broken) else {
        panic!("went through");
    };
    assert_eq!(code, ErrorCode::Failed);
    assert!(message.contains("secrets"), "{message}");
}
