mod live;

use super::{EditorCanvas, underlay};
use crate::session::viewport::ViewMath;
use es_anno::{BuildContext, DrawList, build_draw_list};
use es_model::{AnnoShapeId, BackgroundSettings};
use es_render::beautify::compose::compose_scaled;
use es_render::beautify::layout;
use gpui::{Bounds, Context, Pixels, RenderImage, Task, point, px, size};
use image::RgbaImage;
use image::imageops::FilterType;
use live::LiveStage;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::Pixmap;

/// Quiet time after the last change before the exact composite is rendered.
pub const SETTLE_DELAY: Duration = Duration::from_millis(120);

/// The camera and the scene blur act on the whole composed card, so the canvas shows them as one image.
pub fn active(settings: &BackgroundSettings) -> bool {
    settings.camera.has_effect() || settings.progressive_blur.is_active()
}

#[derive(Debug, Clone, PartialEq)]
struct SceneKey {
    settings: BackgroundSettings,
    base: u64,
    shapes: Vec<(AnnoShapeId, u64)>,
    /// Composite pixels per page pixel.
    scale: f64,
}

impl SceneKey {
    fn same_content(&self, other: &Self) -> bool {
        self.base == other.base && self.shapes == other.shapes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    /// The exact export composite.
    Settled,
    /// The half-resolution approximation drawn while a setting is dragged.
    Live,
}

/// What the effects layer did, for tests and benches.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectsState {
    /// What the last frame painted instead of the flat canvas.
    pub showing: Option<Showing>,
    pub pending: bool,
    pub composites_rendered: usize,
    pub live_frames: usize,
    /// Images on hand: zero or one.
    pub held: usize,
    pub released: usize,
}

struct Shown {
    key: SceneKey,
    image: Placed,
    settled: bool,
}

/// A rendered scene and where its screenshot sits in it, so it lines up with the flat canvas pixel for pixel.
struct Placed {
    image: Arc<RenderImage>,
    image_origin: (f64, f64),
    /// Image pixels per page pixel on each axis.
    scale: (f64, f64),
    pixels: (u32, u32),
}

impl Placed {
    fn new(pixmap: &Pixmap, image_origin: (f64, f64), scale: (f64, f64)) -> Option<Self> {
        Some(Self {
            image: underlay::render_image(pixmap)?,
            image_origin,
            scale,
            pixels: (pixmap.width(), pixmap.height()),
        })
    }

    fn bounds(&self, element: Bounds<Pixels>, vp: &es_anno::Viewport) -> Bounds<Pixels> {
        let (kx, ky) = (vp.scale / self.scale.0, vp.scale / self.scale.1);
        let x = vp.origin.x - self.image_origin.0 * kx;
        let y = vp.origin.y - self.image_origin.1 * ky;
        Bounds::new(
            element.origin + point(px(x as f32), px(y as f32)),
            size(
                px((f64::from(self.pixels.0) * kx) as f32),
                px((f64::from(self.pixels.1) * ky) as f32),
            ),
        )
    }
}

#[derive(Default)]
pub struct Effects {
    shown: Option<Shown>,
    pending: Option<SceneKey>,
    task: Option<Task<()>>,
    live: bool,
    stage: Option<LiveStage>,
    showing: Option<Showing>,
    composites: usize,
    live_frames: usize,
    released: usize,
}

impl Effects {
    fn retire(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        if let Some(old) = self.shown.take() {
            retired.push(old.image.image);
            self.released += 1;
        }
    }

    fn show(&mut self, shown: Shown, retired: &mut Vec<Arc<RenderImage>>) {
        self.retire(retired);
        self.shown = Some(shown);
    }

    fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        self.retire(retired);
        self.pending = None;
        self.task = None;
        self.stage = None;
    }
}

/// The stepped display scale the underlay uses, so both re-render at the same zoom steps.
fn composite_scale(math: &ViewMath, scale_factor: f64) -> f64 {
    let edge = math.layout.canvas_width.max(math.layout.canvas_height);
    underlay::stepped_scale(math.scale() * scale_factor, edge)
}

/// The screenshot resized by `scale`, premultiplied, with the exact scale on each axis.
fn scaled_base(base: &RgbaImage, scale: f64) -> Option<(Pixmap, (f64, f64))> {
    let (w, h) = (base.width(), base.height());
    if scale >= 1.0 {
        return Some((super::redaction::premultiplied(base)?, (1.0, 1.0)));
    }
    let sw = ((f64::from(w) * scale).round() as u32).max(1);
    let sh = ((f64::from(h) * scale).round() as u32).max(1);
    let small = image::imageops::resize(base, sw, sh, FilterType::Triangle);
    let scale = (f64::from(sw) / f64::from(w), f64::from(sh) / f64::from(h));
    Some((super::redaction::premultiplied(&small)?, scale))
}

fn render_settled(base: &RgbaImage, list: &DrawList, key: &SceneKey) -> Option<Placed> {
    let (image, scale) = scaled_base(base, key.scale)?;
    let out = compose_scaled(&image, list, &key.settings, None, scale)?;
    let size = (f64::from(image.width()), f64::from(image.height()));
    let lay = layout(size.0, size.1, &key.settings).pixel_aligned();
    Placed::new(&out, (lay.image.x0, lay.image.y0), scale)
}

impl EditorCanvas {
    /// Called when a slider or pad starts dragging: the canvas draws a fast approximation until `end_live_adjust`.
    pub fn begin_live_adjust(&mut self, cx: &mut Context<Self>) {
        if !self.effects.live {
            self.effects.live = true;
            cx.notify();
        }
    }

    /// The exact composite replaces the approximation once the settings stay unchanged for [`SETTLE_DELAY`].
    pub fn end_live_adjust(&mut self, cx: &mut Context<Self>) {
        if self.effects.live {
            self.effects.live = false;
            self.effects.stage = None;
            cx.notify();
        }
    }

    pub fn effects(&self) -> EffectsState {
        let e = &self.effects;
        EffectsState {
            showing: e.showing,
            pending: e.pending.is_some(),
            composites_rendered: e.composites,
            live_frames: e.live_frames,
            held: usize::from(e.shown.is_some()),
            released: e.released,
        }
    }

    /// The image painted in place of the flat canvas, if the camera or the blur is on.
    pub(super) fn effects_frame(
        &mut self,
        bounds: Bounds<Pixels>,
        math: &ViewMath,
        scale_factor: f64,
        retired: &mut Vec<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) -> Option<(Bounds<Pixels>, Arc<RenderImage>)> {
        self.effects.showing = None;
        if !active(&self.background) {
            self.effects.clear(retired);
            return None;
        }
        let doc = self.doc.read(cx);
        let editor = doc.editor();
        let document = editor.doc();
        let key = SceneKey {
            settings: self.background.clone(),
            base: doc.base_revision(),
            shapes: document
                .shapes()
                .iter()
                .filter_map(|s| Some((s.id, document.revision(s.id)?)))
                .collect(),
            scale: composite_scale(math, scale_factor),
        };

        let current = self.effects.shown.as_ref().is_some_and(|s| s.key == key);
        if self.effects.live && !current {
            let stage = match self.effects.stage.take() {
                Some(stage) if stage.fits(&key) => stage,
                _ => {
                    let list = build_draw_list(document, &BuildContext::new(&**doc.text()));
                    LiveStage::new(doc.base(), &list, &key)?
                }
            };
            let stage = self.effects.stage.insert(stage);
            let (origin, scale) = (stage.image_origin(), stage.scale);
            let frame = stage.frame(&key.settings);
            if let Some(image) = frame.and_then(|p| Placed::new(&p, origin, scale)) {
                self.effects.live_frames += 1;
                let shown = Shown {
                    key,
                    image,
                    settled: false,
                };
                self.effects.show(shown, retired);
            }
            self.effects.pending = None;
            self.effects.task = None;
        } else if !self.effects.live {
            let busy = !editor.selected_ids().is_empty()
                || !editor.is_idle()
                || editor.editing_text_id().is_some()
                || editor.live_stroke().is_some();
            // Pointer input and the selection chrome work in flat page space, so edits show unprojected and unblurred.
            if busy {
                return None;
            }
            let fresh = self
                .effects
                .shown
                .as_ref()
                .is_some_and(|s| s.settled && s.key == key);
            if !fresh && self.effects.pending.as_ref() != Some(&key) {
                let list = build_draw_list(document, &BuildContext::new(&**doc.text()));
                let base = doc.base().clone();
                self.schedule(key.clone(), base, list, cx);
            }
            // A composite of other annotations would show shapes that are gone, so the flat canvas stays until it lands.
            if !self
                .effects
                .shown
                .as_ref()
                .is_some_and(|s| s.key.same_content(&key))
            {
                return None;
            }
        }
        let shown = self.effects.shown.as_ref()?;
        self.effects.showing = Some(if shown.settled {
            Showing::Settled
        } else {
            Showing::Live
        });
        let at = shown.image.bounds(bounds, &math.viewport());
        Some((at, shown.image.image.clone()))
    }

    fn schedule(
        &mut self,
        key: SceneKey,
        base: Arc<RgbaImage>,
        list: DrawList,
        cx: &mut Context<Self>,
    ) {
        self.effects.pending = Some(key.clone());
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE_DELAY).await;
            let job = key.clone();
            let image = cx
                .background_executor()
                .spawn(async move { render_settled(&base, &list, &job) })
                .await;
            let _ = this.update(cx, |canvas, cx| {
                let e = &mut canvas.effects;
                if e.pending.as_ref() != Some(&key) {
                    return;
                }
                e.pending = None;
                if let Some(image) = image {
                    e.composites += 1;
                    let mut retired = std::mem::take(&mut canvas.retired);
                    let shown = Shown {
                        key,
                        image,
                        settled: true,
                    };
                    canvas.effects.show(shown, &mut retired);
                    canvas.retired = retired;
                }
                cx.notify();
            });
        });
        self.effects.task = Some(task);
    }
}
