#![allow(clippy::unwrap_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use es_geom::{Rect, Size};
use es_ocr::{Error, OcrOptions, decode_codes, probe, recognize, redact};
use image::{Rgba, RgbaImage};
use rxing::{BarcodeFormat, Writer, qrcode::QRCodeWriter};

fn fixture(name: &str) -> RgbaImage {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    image::open(path).unwrap().to_rgba8()
}

fn qr(payload: &str, side: i32) -> RgbaImage {
    let m = QRCodeWriter
        .encode(payload, &BarcodeFormat::QR_CODE, side, side)
        .unwrap();
    RgbaImage::from_fn(m.getWidth(), m.getHeight(), |x, y| {
        if m.get(x, y) {
            Rgba([0, 0, 0, 255])
        } else {
            Rgba([255, 255, 255, 255])
        }
    })
}

fn have_tesseract() -> bool {
    let ok = probe(None).available;
    if !ok {
        eprintln!("skipped: tesseract is not installed");
    }
    ok
}

#[test]
fn text_lines_keep_their_order() {
    if !have_tesseract() {
        return;
    }
    let got = recognize(&fixture("text.png"), &OcrOptions::default()).unwrap();
    assert_eq!(got.text, "OCR PROBE 4711 Hello\nSecond line here");
    assert_eq!(got.lines.len(), 2);
    assert!(got.words.iter().all(|w| w.confidence > 0.0));
}

#[test]
fn code_payload_comes_before_text() {
    if !have_tesseract() {
        return;
    }
    let text = fixture("text.png");
    let mut canvas =
        RgbaImage::from_pixel(text.width() + 260, text.height().max(260), Rgba([255; 4]));
    image::imageops::overlay(&mut canvas, &qr("https://example.test/qr", 220), 20, 20);
    image::imageops::overlay(&mut canvas, &text, 260, 0);
    let got = recognize(&canvas, &OcrOptions::default()).unwrap();
    assert_eq!(got.codes, ["https://example.test/qr"]);
    assert_eq!(
        got.text,
        "https://example.test/qr\nOCR PROBE 4711 Hello\nSecond line here"
    );
}

#[test]
fn blank_image_yields_no_text() {
    if !have_tesseract() {
        return;
    }
    let blank = RgbaImage::from_pixel(400, 200, Rgba([255; 4]));
    let got = recognize(&blank, &OcrOptions::default()).unwrap();
    assert_eq!(got.text, "");
    assert!(got.words.is_empty());
}

#[test]
fn several_codes_are_decoded_top_to_bottom() {
    let mut canvas = RgbaImage::from_pixel(260, 520, Rgba([255; 4]));
    image::imageops::overlay(&mut canvas, &qr("second", 220), 20, 280);
    image::imageops::overlay(&mut canvas, &qr("first", 220), 20, 20);
    assert_eq!(decode_codes(&canvas), ["first", "second"]);
}

#[test]
fn probe_lists_installed_languages() {
    if !have_tesseract() {
        return;
    }
    let p = probe(None);
    assert!(p.version.is_some());
    assert!(p.languages.iter().any(|l| l == "eng"));
    assert_eq!(
        p.missing_languages(&["eng".to_owned(), "zzz".to_owned()]),
        ["zzz"]
    );
}

#[test]
fn smart_redaction_boxes_the_sensitive_words_only() {
    if !have_tesseract() {
        return;
    }
    let img = fixture("secrets.png");
    let got = recognize(&img, &OcrOptions::default()).unwrap();
    let size = Size::new(f64::from(img.width()), f64::from(img.height()));
    let rects = redact::find(&got.words, size);
    let rows: Vec<usize> = rects
        .iter()
        .map(|r| (r.center().y / 60.0) as usize)
        .collect();
    assert_eq!(
        rows,
        [0, 1, 3],
        "email, card and address rows; the non-Luhn order row stays: {rects:?}"
    );
    let email = rects[0];
    assert!(email.x0 > 100.0 && email.x1 < 600.0, "{email:?}");
    let _: Rect = email;
}

fn fake_tesseract(dir: &tempfile::TempDir, body: &str) -> PathBuf {
    let path = dir.path().join("tesseract");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn small() -> RgbaImage {
    RgbaImage::from_pixel(20, 20, Rgba([255; 4]))
}

#[test]
fn a_hung_tesseract_times_out_and_is_killed() {
    let dir = tempfile::tempdir().unwrap();
    let opts = OcrOptions {
        tesseract_path: Some(fake_tesseract(&dir, "exec sleep 30")),
        timeout: Duration::from_millis(200),
        ..OcrOptions::default()
    };
    let start = Instant::now();
    assert!(matches!(recognize(&small(), &opts), Err(Error::Timeout)));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn cancelling_stops_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let opts = OcrOptions {
        tesseract_path: Some(fake_tesseract(&dir, "exec sleep 30")),
        cancel: Some(cancel.clone()),
        ..OcrOptions::default()
    };
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    assert!(matches!(recognize(&small(), &opts), Err(Error::Cancelled)));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_failing_tesseract_reports_its_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let opts = OcrOptions {
        tesseract_path: Some(fake_tesseract(&dir, "echo no such language >&2; exit 1")),
        ..OcrOptions::default()
    };
    match recognize(&small(), &opts) {
        Err(Error::Failed { stderr, .. }) => assert_eq!(stderr, "no such language"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_binary_is_not_installed() {
    let opts = OcrOptions {
        tesseract_path: Some(PathBuf::from("/nonexistent/tesseract")),
        ..OcrOptions::default()
    };
    assert!(matches!(
        recognize(&small(), &opts),
        Err(Error::NotInstalled)
    ));
}
