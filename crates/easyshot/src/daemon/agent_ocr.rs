use super::capture::{Deps, Fail, acquire, check_caps, fail_of};
use super::gate::scope::Scope;
use super::window_entry;
use es_app::pipeline::frame_rgba;
use es_geom::Rect;
use es_ocr::{OcrOptions, Recognized};
use es_protocol::agent::{
    AgentReply, ErrorCode, OcrCode, OcrInfo, OcrInput, OcrLine, OcrSource, OcrSpec, ReplyKind,
};
use gpui::{App, AsyncApp, Task};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

pub(super) const INSTALL_HINT: &str = "text recognition needs Tesseract: install the tesseract \
     package and the language data you need, for example tesseract-data-eng";

pub type OcrRunner = Rc<dyn Fn(OcrSpec, Scope, &mut App) -> Task<AgentReply>>;
pub type Recognizer =
    Arc<dyn Fn(&RgbaImage, &OcrOptions) -> Result<Recognized, es_ocr::Error> + Send + Sync>;

pub fn recognizer() -> Recognizer {
    #[cfg(feature = "test-hooks")]
    if let Some(text) = fixed_text() {
        return Arc::new(move |image, _| {
            let rect = Rect::new(
                0.0,
                0.0,
                f64::from(image.width()),
                f64::from(image.height()),
            );
            let line = es_ocr::Line {
                text: text.clone(),
                rect,
                confidence: 90.0,
            };
            Ok(Recognized {
                text: text.clone(),
                lines: vec![line],
                ..Recognized::default()
            })
        });
    }
    Arc::new(es_ocr::recognize)
}

#[cfg(feature = "test-hooks")]
fn fixed_text() -> Option<String> {
    (std::env::var("EASYSHOT_TEST").as_deref() == Ok("1"))
        .then(|| std::env::var("EASYSHOT_TEST_OCR_TEXT").ok())
        .flatten()
}

pub fn runner(deps: Rc<Deps>, recognizer: Recognizer) -> OcrRunner {
    Rc::new(move |spec, scope, cx| {
        let (deps, recognizer) = (deps.clone(), recognizer.clone());
        cx.spawn(async move |cx| {
            run(&deps, &recognizer, spec, &scope, cx)
                .await
                .unwrap_or_else(|Fail(code, message)| {
                    AgentReply::failed(ReplyKind::Ocr, code, message)
                })
        })
    })
}

async fn run(
    deps: &Deps,
    recognizer: &Recognizer,
    spec: OcrSpec,
    scope: &Scope,
    cx: &mut AsyncApp,
) -> Result<AgentReply, Fail> {
    let languages = if spec.languages.is_empty() {
        cx.update(|cx| {
            deps.pipeline
                .settings
                .read(cx)
                .config()
                .ocr
                .languages
                .clone()
        })
    } else {
        spec.languages
    };
    let (image, source) = match spec.input {
        OcrInput::File(path) => {
            let shown = path.display().to_string();
            let image = cx
                .background_executor()
                .spawn(async move { load_image(&path) })
                .await?;
            let source = OcrSource {
                file: Some(shown),
                ..OcrSource::default()
            };
            (image, source)
        }
        OcrInput::Target(target) => {
            check_caps(deps.backend.caps, &target)?;
            let shot = acquire(deps, &target, scope, cx).await?;
            let output = shot
                .frame
                .output
                .as_ref()
                .or_else(|| shot.window.as_ref().and_then(|w| w.output.as_ref()))
                .map(|o| o.0.clone());
            let source = OcrSource {
                output,
                window: shot.window.as_ref().map(window_entry),
                file: None,
            };
            let frame = shot.frame;
            let image = cx
                .background_executor()
                .spawn(async move { frame_rgba(&frame) })
                .await
                .map_err(fail_of)?;
            (image, source)
        }
    };
    let recognizer = recognizer.clone();
    let info = cx
        .background_executor()
        .spawn(async move { read(&image, &languages, &recognizer) })
        .await?;
    Ok(AgentReply {
        ocr: Some(Box::new(OcrInfo { source, ..info })),
        ..AgentReply::ok(ReplyKind::Ocr)
    })
}

fn load_image(path: &Path) -> Result<RgbaImage, Fail> {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() => {}
        Ok(_) => {
            return Err(Fail::new(
                ErrorCode::InvalidArguments,
                format!("{} is not a file", path.display()),
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Fail::new(
                ErrorCode::NotFound,
                format!("no file {}", path.display()),
            ));
        }
        Err(e) => {
            return Err(Fail::new(
                ErrorCode::Failed,
                format!("{}: {e}", path.display()),
            ));
        }
    }
    let failed = |e: &dyn std::fmt::Display| {
        Fail::new(
            ErrorCode::Failed,
            format!("could not read {}: {e}", path.display()),
        )
    };
    let reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(|e| failed(&e))?;
    if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
        return Err(Fail::new(
            ErrorCode::InvalidArguments,
            format!("{} is not a PNG or JPEG image", path.display()),
        ));
    }
    Ok(reader.decode().map_err(|e| failed(&e))?.to_rgba8())
}

fn read(image: &RgbaImage, languages: &[String], recognizer: &Recognizer) -> Result<OcrInfo, Fail> {
    let options = OcrOptions {
        languages: languages.to_vec(),
        ..OcrOptions::default()
    };
    let found = recognizer(image, &options).map_err(|e| match e {
        es_ocr::Error::NotInstalled => Fail::new(ErrorCode::Unsupported, INSTALL_HINT),
        es_ocr::Error::Cancelled => Fail::new(ErrorCode::Cancelled, "cancelled"),
        other => Fail::new(ErrorCode::Failed, other.to_string()),
    })?;
    let lines = found
        .lines
        .iter()
        .map(|l| {
            let (x, y, width, height) = bounds(l.rect);
            OcrLine {
                text: l.text.clone(),
                x,
                y,
                width,
                height,
                confidence: (f64::from(l.confidence) * 100.0).round() / 100.0,
            }
        })
        .collect();
    let qr = found
        .codes
        .iter()
        .zip(&found.code_rects)
        .map(|(text, rect)| {
            let (x, y, width, height) = bounds(*rect);
            OcrCode {
                text: text.clone(),
                x,
                y,
                width,
                height,
            }
        })
        .collect();
    Ok(OcrInfo {
        text: found.text,
        lines,
        qr,
        source: OcrSource::default(),
    })
}

fn bounds(r: Rect) -> (i32, i32, u32, u32) {
    (
        r.x0.round() as i32,
        r.y0.round() as i32,
        r.width().round().max(0.0) as u32,
        r.height().round().max(0.0) as u32,
    )
}

#[cfg(test)]
#[path = "agent_ocr_tests.rs"]
mod tests;
