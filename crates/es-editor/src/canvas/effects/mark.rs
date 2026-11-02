use super::{Placed, composite_scale, scaled_size};
use crate::canvas::EditorCanvas;
use crate::session::viewport::ViewMath;
use es_model::{BackgroundSettings, WatermarkSettings};
use es_render::beautify::compose::paint_watermark;
use es_render::beautify::layout;
use gpui::{Bounds, Context, Pixels, RenderImage, Task};
use std::sync::Arc;
use tiny_skia::Pixmap;

#[derive(Debug, Clone, PartialEq)]
struct MarkKey {
    mark: WatermarkSettings,
    canvas: (u32, u32),
    image_origin: (f64, f64),
    scale: (f64, f64),
}

impl MarkKey {
    fn new(settings: &BackgroundSettings, image: (u32, u32), scale: f64) -> Option<Self> {
        if !settings.watermark.is_visible() {
            return None;
        }
        let (w, h, scale) = scaled_size(image.0, image.1, scale);
        let lay = layout(f64::from(w), f64::from(h), settings).pixel_aligned();
        Some(Self {
            mark: settings.watermark.clone(),
            canvas: lay.canvas_pixels(),
            image_origin: (lay.image.x0, lay.image.y0),
            scale,
        })
    }

    fn render(&self) -> Option<Placed> {
        let mut layer = Pixmap::new(self.canvas.0, self.canvas.1)?;
        paint_watermark(&mut layer, &self.mark, self.scale.0);
        Placed::new(&layer, self.image_origin, self.scale)
    }
}

/// The watermark drawn over the flat canvas, so a look with no camera or blur needs no composite.
#[derive(Default)]
pub struct Mark {
    shown: Option<(MarkKey, Placed)>,
    pending: Option<MarkKey>,
    task: Option<Task<()>>,
}

impl Mark {
    pub fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        if let Some((_, old)) = self.shown.take() {
            retired.push(old.image);
        }
        self.pending = None;
        self.task = None;
    }
}

impl EditorCanvas {
    pub(in crate::canvas) fn watermark_frame(
        &mut self,
        bounds: Bounds<Pixels>,
        math: &ViewMath,
        scale_factor: f64,
        flat: bool,
        retired: &mut Vec<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Option<(Bounds<Pixels>, Arc<RenderImage>)> {
        let size = self.doc.read(cx).base().dimensions();
        let scale = composite_scale(math, scale_factor);
        let Some(key) = MarkKey::new(&self.background, size, scale) else {
            self.effects.mark_painted = false;
            self.effects.mark.clear(retired);
            return None;
        };
        let current = self
            .effects
            .mark
            .shown
            .as_ref()
            .is_some_and(|(k, _)| *k == key);
        if !current && self.effects.mark.pending.as_ref() != Some(&key) {
            self.effects.mark.pending = Some(key.clone());
            let job = key.clone();
            let rendering = cx.background_executor().spawn(async move { job.render() });
            self.effects.mark.task = Some(cx.spawn(async move |this, cx| {
                let placed = rendering.await;
                let _ = this.update(cx, |canvas, cx| {
                    if canvas.effects.mark.pending.as_ref() != Some(&key) {
                        return;
                    }
                    canvas.effects.mark.pending = None;
                    if let Some(placed) = placed
                        && let Some((_, old)) = canvas.effects.mark.shown.replace((key, placed))
                    {
                        canvas.retired.push(old.image);
                    }
                    cx.notify();
                });
            }));
        }
        let painted = self.effects.mark.shown.as_ref().filter(|_| flat);
        self.effects.mark_painted = painted.is_some();
        let (_, placed) = painted?;
        Some((
            placed.bounds(bounds, &math.viewport()),
            placed.image.clone(),
        ))
    }
}
