#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use es_ocr::{OcrOptions, probe, recognize};

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn recognised_text_never_reaches_the_log() {
    if !probe(None).available {
        eprintln!("skipped: tesseract is not installed");
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/secrets.png");
    let image = image::open(path).unwrap().to_rgba8();
    let sink = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(sink.clone())
        .finish();
    let got = tracing::subscriber::with_default(subscriber, || {
        recognize(&image, &OcrOptions::default()).unwrap()
    });
    let log = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    assert!(
        log.contains("recognized text"),
        "the count line is logged: {log}"
    );
    assert!(got.text.contains('@'), "the fixture has an e-mail address");
    for word in got.words.iter().filter(|w| w.text.chars().count() > 3) {
        assert!(
            !log.contains(&word.text),
            "{:?} leaked into the log: {log}",
            word.text
        );
    }
}
