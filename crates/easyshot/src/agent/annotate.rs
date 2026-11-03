use super::annotate_ops;
use clap::Args;
use es_anno::{BuildContext, BundledText, Document, TextMeasure, build_draw_list};
use es_geom::Size;
use es_model::{AnnotationDocument, StylePreset, TextProps};
use es_protocol::agent::{AgentReply, ErrorCode, ReplyKind};
use es_render::encode::{OutputFormat, encode};
use es_render::export::{ExportCache, Purpose};
use serde_json::json;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tiny_skia::{IntSize, Pixmap};

const MAX_OPS_BYTES: u64 = 1 << 20;

#[derive(Args, Debug)]
pub struct AnnotateArgs {
    /// The image to draw on
    #[arg(value_name = "IMAGE")]
    image: PathBuf,
    /// A JSON file of operations, or - to read them from standard input
    #[arg(long, value_name = "FILE")]
    ops: PathBuf,
    /// Write the PNG here; its folder must exist
    #[arg(long, value_name = "PATH")]
    out: PathBuf,
    /// Replace the file if it already exists
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotateCall {
    pub image: PathBuf,
    pub ops: PathBuf,
    pub out: PathBuf,
    pub force: bool,
    pub json: bool,
}

impl AnnotateArgs {
    pub fn call(&self, json: bool) -> AnnotateCall {
        AnnotateCall {
            image: self.image.clone(),
            ops: self.ops.clone(),
            out: self.out.clone(),
            force: self.force,
            json,
        }
    }
}

#[derive(Debug)]
pub(super) struct Failure {
    pub code: ErrorCode,
    pub message: String,
}

pub(super) struct Done {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub ops: usize,
}

fn invalid(message: impl Into<String>) -> Failure {
    Failure {
        code: ErrorCode::InvalidArguments,
        message: message.into(),
    }
}

fn failed(message: impl Into<String>) -> Failure {
    Failure {
        code: ErrorCode::Failed,
        message: message.into(),
    }
}

impl Failure {
    pub(super) fn reply(&self) -> AgentReply {
        AgentReply::failed(ReplyKind::Annotate, self.code, self.message.clone())
    }
}

impl Done {
    pub(super) fn to_json(&self) -> serde_json::Value {
        json!({
            "ok": true, "kind": "annotate", "path": self.path,
            "width": self.width, "height": self.height, "ops": self.ops
        })
    }
}

struct Shared(Arc<BundledText>);

impl TextMeasure for Shared {
    fn measure(&self, props: &TextProps) -> Size {
        self.0.measure(props)
    }
}

pub fn run_annotate(call: &AnnotateCall, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let result = read_ops(&call.ops)
        .and_then(|ops| perform(&call.image, &ops, &call.out, call.force, &saved_style()));
    match result {
        Ok(done) if call.json => {
            let _ = writeln!(out, "{}", done.to_json());
            0
        }
        Ok(done) => {
            let _ = writeln!(out, "{}", done.path.display());
            0
        }
        Err(f) => {
            let reply = f.reply();
            if call.json {
                let _ = writeln!(out, "{}", reply.to_json());
            } else {
                let _ = writeln!(err, "easyshot: {}", f.message);
            }
            i32::from(reply.exit_code())
        }
    }
}

pub(super) fn saved_style() -> StylePreset {
    es_store::Paths::from_xdg()
        .ok()
        .and_then(|p| es_store::presets::open_style(&p).ok())
        .map(|f| f.value)
        .unwrap_or_default()
}

fn read_ops(path: &Path) -> Result<String, Failure> {
    let mut text = String::new();
    let read = if path == Path::new("-") {
        std::io::stdin()
            .take(MAX_OPS_BYTES + 1)
            .read_to_string(&mut text)
    } else {
        std::fs::File::open(path).and_then(|f| f.take(MAX_OPS_BYTES + 1).read_to_string(&mut text))
    };
    match read {
        Ok(_) if text.len() as u64 > MAX_OPS_BYTES => Err(invalid("the ops file is over 1 MiB")),
        Ok(_) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Failure {
            code: ErrorCode::NotFound,
            message: format!("{}: no such file", path.display()),
        }),
        Err(e) => Err(invalid(format!("{}: {e}", path.display()))),
    }
}

pub(super) fn perform(
    image: &Path,
    ops: &str,
    out: &Path,
    force: bool,
    style: &StylePreset,
) -> Result<Done, Failure> {
    let out = std::path::absolute(out).map_err(|e| invalid(format!("{}: {e}", out.display())))?;
    check_out(&out, force)?;
    let base = match es_store::decode::open_image(image) {
        Ok(img) => img.to_rgba8(),
        Err(image::ImageError::IoError(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Failure {
                code: ErrorCode::NotFound,
                message: format!("{}: no such file", image.display()),
            });
        }
        Err(e) => return Err(invalid(format!("{}: {e}", image.display()))),
    };
    let (document, count) = annotate_ops::build(ops, base.dimensions(), style).map_err(invalid)?;
    let (png, width, height) = render(&base, document)?;
    es_store::atomic::write_atomic(&out, &png).map_err(|e| failed(e.to_string()))?;
    Ok(Done {
        path: out,
        width,
        height,
        ops: count,
    })
}

fn check_out(out: &Path, force: bool) -> Result<(), Failure> {
    let is_png = out
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if !is_png {
        return Err(invalid(format!(
            "{}: the file must end in .png",
            out.display()
        )));
    }
    if !out.parent().is_some_and(Path::is_dir) {
        return Err(invalid(format!(
            "the folder of {} does not exist",
            out.display()
        )));
    }
    if out.is_dir() {
        return Err(invalid(format!("{} is a folder", out.display())));
    }
    if out.exists() && !force {
        return Err(invalid(format!(
            "{} already exists; add --force to replace it",
            out.display()
        )));
    }
    Ok(())
}

/// The editor's export path: the same draw list, look and compose, then the PNG encoder.
fn render(
    base: &image::RgbaImage,
    model: AnnotationDocument,
) -> Result<(Vec<u8>, u32, u32), Failure> {
    let look = model.background.clone();
    let text = Arc::new(BundledText::bundled());
    let document = Document::new(model, Box::new(Shared(text.clone())));
    let list = build_draw_list(&document, &BuildContext::new(&*text));
    let (w, h) = base.dimensions();
    let mut data = base.as_raw().clone();
    for px in data.as_chunks_mut::<4>().0 {
        let a = u16::from(px[3]);
        for c in &mut px[..3] {
            *c = ((u16::from(*c) * a + 127) / 255) as u8;
        }
    }
    let pixmap = IntSize::from_wh(w, h)
        .and_then(|size| Pixmap::from_vec(data, size))
        .ok_or_else(|| invalid("the image has no pixels"))?;
    let composed = ExportCache::default()
        .composed(&pixmap, &list, &look, None)
        .map_err(|e| failed(e.to_string()))?;
    let png = encode(&composed, OutputFormat::Png(Purpose::Keep.png_profile()))
        .map_err(|e| failed(e.to_string()))?;
    Ok((png, composed.width(), composed.height()))
}

#[cfg(test)]
#[path = "annotate_tests.rs"]
mod tests;
