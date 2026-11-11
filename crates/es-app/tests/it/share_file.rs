#![allow(clippy::unwrap_used)]
use super::share::{Fake, Rig, finished, kinds, made, rig, wait_until};
use es_app::share::{FileShare, ShareFailure, ShareJobKind};
use es_model::ShareSource;
use es_share::Code;
use gpui::TestAppContext;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

fn tiny_png(dir: &std::path::Path) -> PathBuf {
    let file = dir.join("shot.png");
    image::RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255]))
        .save(&file)
        .unwrap();
    file
}

#[gpui::test]
fn a_file_is_shared_as_it_is_when_it_fits_with_the_setting_days(cx: &mut TestAppContext) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    let fake = Fake::new(Box::new(move |r| {
        sink.lock().unwrap().push(r.clone());
        Ok(made(r))
    }));
    let rig = rig(cx, &fake, |_| {});
    let png = tiny_png(rig._dir.path());
    let file = FileShare {
        path: png.clone(),
        days: None,
        title: Some("Login bug".into()),
        source: None,
        export: None,
    };
    rig.shares.update(cx, |s, cx| s.start_file(file, cx));
    wait_until(cx, "the share to finish", || finished(&rig));
    let sent = seen.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].media.path, png);
    assert_eq!((sent[0].media.width, sent[0].media.height), (8, 6));
    assert_eq!(sent[0].media.mime, "image/png");
    assert_eq!(sent[0].days, 7);
    assert_eq!(sent[0].title.as_deref(), Some("Login bug"));
    assert!(matches!(sent[0].source, ShareSource::Capture(_)));
}

#[gpui::test]
fn a_lifetime_the_service_does_not_offer_is_refused_before_any_work(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    let file = FileShare {
        path: tiny_png(rig._dir.path()),
        days: Some(2),
        title: None,
        source: None,
        export: None,
    };
    rig.shares.update(cx, |s, cx| s.start_file(file, cx));
    cx.run_until_parked();
    assert!(matches!(
        kinds(&rig).as_slice(),
        [ShareJobKind::Failed(ShareFailure::Refused {
            code: Code::BadRequest,
            ..
        })]
    ));
    assert_eq!(fake.opened.load(Ordering::SeqCst), 0);
}

fn run_for_a_command(
    rig: &Rig,
    cx: &mut TestAppContext,
    path: PathBuf,
) -> es_app::jobs::JobFinished {
    let (jobs, finished) = es_app::jobs::Jobs::new();
    let job = jobs.allocate();
    let file = FileShare {
        path,
        days: None,
        title: None,
        source: None,
        export: None,
    };
    cx.update(|cx| es_app::share::run_route(&rig.shares, &jobs, job, file, cx));
    wait_until(cx, "the command to be answered", || !finished.is_empty());
    finished.try_recv().unwrap()
}

#[gpui::test]
fn a_shared_file_answers_its_command_with_the_page_link(cx: &mut TestAppContext) {
    let fake = Fake::new(Box::new(|r| Ok(made(r))));
    let rig = rig(cx, &fake, |_| {});
    let done = run_for_a_command(&rig, cx, tiny_png(rig._dir.path()));
    assert_eq!(done.status, es_protocol::JobStatus::Ok);
    assert_eq!(
        done.detail.as_deref(),
        Some("https://share.example/s/abc123")
    );
}
