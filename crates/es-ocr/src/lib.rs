mod qr;
pub mod redact;
mod tesseract;
mod tsv;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use es_geom::Rect;
use image::RgbaImage;

pub use qr::decode_codes;
pub use tesseract::{Probe, probe};
pub use tsv::parse_tsv;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("tesseract is not installed")]
    NotInstalled,
    #[error("could not run tesseract: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("could not encode the image: {0}")]
    Encode(#[source] image::ImageError),
    #[error("tesseract exited with {status}: {stderr}")]
    Failed { status: String, stderr: String },
    #[error("text recognition timed out")]
    Timeout,
    #[error("text recognition was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct OcrOptions {
    pub languages: Vec<String>,
    pub tesseract_path: Option<PathBuf>,
    pub timeout: Duration,
    pub psm: Option<u8>,
    pub cancel: Option<Arc<AtomicBool>>,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            languages: vec!["eng".to_owned()],
            tesseract_path: None,
            timeout: Duration::from_secs(60),
            psm: None,
            cancel: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub rect: Rect,
    pub confidence: f32,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub rect: Rect,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Recognized {
    pub text: String,
    pub codes: Vec<String>,
    pub lines: Vec<Line>,
    pub words: Vec<Word>,
}

pub fn recognize(image: &RgbaImage, options: &OcrOptions) -> Result<Recognized, Error> {
    let codes = decode_codes(image);
    let words = tesseract::run(image, options)?;
    let lines = tsv::lines(&words);
    let text = join_text(&codes, &lines);
    tracing::debug!(
        codes = codes.len(),
        lines = lines.len(),
        chars = text.chars().count(),
        "recognized text"
    );
    Ok(Recognized {
        text,
        codes,
        lines,
        words,
    })
}

pub fn join_text(codes: &[String], lines: &[Line]) -> String {
    codes
        .iter()
        .map(String::as_str)
        .chain(lines.iter().map(|l| l.text.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(t: &str) -> Line {
        Line {
            text: t.to_owned(),
            rect: Rect::ZERO,
        }
    }

    #[test]
    fn codes_come_before_text_and_empties_drop() {
        let codes = vec!["https://x.test/qr".to_owned(), "  ".to_owned()];
        let lines = [line("First"), line(""), line(" Second ")];
        assert_eq!(
            join_text(&codes, &lines),
            "https://x.test/qr\nFirst\nSecond"
        );
    }

    #[test]
    fn nothing_found_joins_to_empty() {
        assert_eq!(join_text(&[], &[]), "");
    }
}
