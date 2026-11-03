use crate::errors::{Error, Result};
use crate::jobs::run_cpu;
use crate::pipeline::{self, PipelineDeps, Work};
use es_model::CaptureKind as StoredKind;
use es_model::capture::{CaptureVariant, Step, after_capture_plan};
use es_model::platform::Frame;
use es_render::encode::{PngProfile, encode_png};
use es_store::atomic::write_atomic;
use gpui::{App, Task};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub out: Option<PathBuf>,
    /// Show the deck card and run the after-capture steps, as for a capture made by hand.
    pub ui: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub output: Option<String>,
}

/// With `out` and no UI the file is all there is: no library entry, no deck card.
pub fn save(
    deps: &Rc<PipelineDeps>,
    options: Options,
    kind: StoredKind,
    frame: Arc<Frame>,
    cx: &mut App,
) -> Task<Result<Saved>> {
    let (scale, output) = (frame.scale, frame.output.as_ref().map(|o| o.0.clone()));
    if let (Some(out), false) = (&options.out, options.ui) {
        let out = out.clone();
        let encoded = run_cpu(cx, move || {
            let image = pipeline::frame_rgba(&frame)?;
            let png = encode_png(&pipeline::pixmap(&image)?, PngProfile::Fast)
                .map_err(|e| Error::Internal(e.to_string()))?;
            write_atomic(&out, &png)?;
            Ok::<_, Error>((out, png_size(&png)))
        });
        return cx.spawn(async move |_| {
            let (path, (width, height)) = encoded.await??;
            Ok(Saved {
                path,
                width,
                height,
                scale,
                output,
            })
        });
    }
    let mut config = deps.settings.read(cx).config().clone();
    let plan = after_capture_plan(CaptureVariant::Normal, &config);
    if !options.ui {
        Arc::make_mut(&mut config).capture.keep_in_deck_until_saved = false;
    }
    let work = Work {
        paths: deps.paths.clone(),
        file_stem: pipeline::next_stem(&deps.paths, &config),
        kind,
        save: plan.contains(&Step::Save),
        show: options.ui && plan.contains(&Step::ShowPreview),
        config,
    };
    let prepared = run_cpu(cx, move || pipeline::prepare(&work, &frame));
    let deps = deps.clone();
    cx.spawn(async move |cx| {
        let prepared = prepared.await??;
        let png = prepared.png.clone();
        let kept = prepared
            .library
            .as_ref()
            .and_then(|l| l.as_ref().ok())
            .map(|entry| entry.file.clone())
            .or_else(|| {
                prepared
                    .saved
                    .as_ref()
                    .and_then(|s| s.as_ref().ok())
                    .cloned()
            })
            .or_else(|| prepared.card.as_ref().map(|c| c.file.clone()))
            .or_else(|| prepared.transient.clone())
            .ok_or_else(|| Error::Internal("the capture was kept nowhere".into()))?;
        let path = match &options.out {
            Some(out) => {
                write_atomic(out, &png)?;
                out.clone()
            }
            None => kept,
        };
        if options.ui {
            pipeline::finish(&deps, &plan, prepared, cx).await?;
        } else {
            match (prepared.saved, prepared.library) {
                (Some(Err(e)), _) | (_, Some(Err(e))) => return Err(e),
                (_, Some(Ok(entry))) => {
                    let added = cx.update(|cx| deps.history.update(cx, |h, cx| h.add(entry, cx)));
                    if let Err(e) = added {
                        tracing::warn!(error = %e, "history");
                    }
                }
                _ => {}
            }
        }
        let (width, height) = png_size(&png);
        Ok(Saved {
            path,
            width,
            height,
            scale,
            output,
        })
    })
}

fn png_size(png: &[u8]) -> (u32, u32) {
    let be = |at: usize| {
        png.get(at..at + 4)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map_or(0, u32::from_be_bytes)
    };
    (be(16), be(20))
}
