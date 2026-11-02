mod live;
mod mark;
#[cfg(test)]
mod wallpaper_tests;

use super::{EditorCanvas, underlay};
use crate::session::viewport::ViewMath;
use es_anno::{BuildContext, DrawList, build_draw_list};
use es_model::{AnnoShapeId, BackgroundSettings};
use es_render::beautify::compose::{canvas_size, compose_scaled};
use es_render::beautify::layout;
use es_render::beautify::wallpaper_cache::WallpaperCache;
use gpui::{Bounds, Context, Pixels, RenderImage, Task, point, px, size};
use image::{Frame, RgbaImage};
use live::LiveStage;
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::Pixmap;

/// Quiet time after the last change before the exact composite is rendered.
pub const SETTLE_DELAY: Duration = Duration::from_millis(120);

/// The camera and the scene blur act on the whole composed canvas, so the canvas shows them as one image.
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
    /// The last frame drew the watermark over the flat canvas.
    pub watermark: bool,
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
            image: bgra(pixmap)?,
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
    building: Option<SceneKey>,
    build_task: Option<Task<()>>,
    /// The key of the last frame, so a drag can start building its stage before the first change.
    last: Option<SceneKey>,
    mark: mark::Mark,
    mark_painted: bool,
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

    pub(super) fn held_image(&self) -> Option<&Arc<RenderImage>> {
        self.shown.as_ref().map(|s| &s.image.image)
    }

    pub(super) fn drop_shown(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        self.retire(retired);
    }

    fn clear(&mut self, retired: &mut Vec<Arc<RenderImage>>) {
        self.retire(retired);
        self.pending = None;
        self.task = None;
        self.drop_stage();
        self.last = None;
    }

    fn drop_stage(&mut self) {
        self.stage = None;
        self.building = None;
        self.build_task = None;
    }
}

/// The stepped display scale the underlay uses, so both re-render at the same zoom steps.
fn composite_scale(math: &ViewMath, scale_factor: f64) -> f64 {
    let edge = math.layout.canvas_width.max(math.layout.canvas_height);
    underlay::stepped_scale(math.scale() * scale_factor, edge)
}

/// The size of an image resized by `scale`, never above 1, with the exact scale on each axis.
fn scaled_size(w: u32, h: u32, scale: f64) -> (u32, u32, (f64, f64)) {
    if scale >= 1.0 {
        return (w, h, (1.0, 1.0));
    }
    let sw = ((f64::from(w) * scale).round() as u32).max(1);
    let sh = ((f64::from(h) * scale).round() as u32).max(1);
    (
        sw,
        sh,
        (f64::from(sw) / f64::from(w), f64::from(sh) / f64::from(h)),
    )
}

/// The screenshot resized by `scale`, premultiplied, with the exact scale on each axis.
fn scaled_base(base: &RgbaImage, scale: f64) -> Option<(Pixmap, (f64, f64))> {
    let (sw, sh, scale) = scaled_size(base.width(), base.height(), scale);
    if (sw, sh) == base.dimensions() {
        return Some((super::redaction::premultiplied(base)?, scale));
    }
    let small = es_render::image_ops::resize(base, sw, sh)?;
    Some((super::redaction::premultiplied(&small)?, scale))
}

/// Premultiplied RGBA into GPUI's straight-alpha BGRA.
fn bgra(pixmap: &Pixmap) -> Option<Arc<RenderImage>> {
    let mut data = pixmap.data().to_vec();
    data.par_chunks_mut(64 * 1024).for_each(|chunk| {
        for px in chunk.as_chunks_mut::<4>().0 {
            let a = px[3];
            if a == 255 {
                px.swap(0, 2);
                continue;
            }
            let un = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
                }
            };
            let (r, g, b) = (un(px[0]), un(px[1]), un(px[2]));
            *px = [b, g, r, a];
        }
    });
    let image = RgbaImage::from_raw(pixmap.width(), pixmap.height(), data)?;
    Some(Arc::new(RenderImage::new([Frame::new(image)])))
}

fn render_settled(
    base: &RgbaImage,
    list: &DrawList,
    key: &SceneKey,
    wallpapers: &WallpaperCache,
) -> Option<Placed> {
    let (image, scale) = scaled_base(base, key.scale)?;
    let canvas = canvas_size((image.width(), image.height()), &key.settings);
    let wallpaper = underlay::cached_wallpaper(wallpapers, &key.settings.style, canvas);
    let out = compose_scaled(&image, list, &key.settings, wallpaper.as_deref(), scale)?;
    let size = (f64::from(image.width()), f64::from(image.height()));
    let lay = layout(size.0, size.1, &key.settings).pixel_aligned();
    Placed::new(&out, (lay.image.x0, lay.image.y0), scale)
}

impl EditorCanvas {
    /// Called when a slider or pad starts dragging: the canvas draws a fast approximation until `end_live_adjust`.
    pub fn begin_live_adjust(&mut self, cx: &mut Context<Self>) {
        if !self.effects.live {
            self.effects.live = true;
            if let Some(key) = self.effects.last.clone() {
                self.build_stage(&key, cx);
            }
            cx.notify();
        }
    }

    /// The exact composite replaces the approximation once the settings stay unchanged for [`SETTLE_DELAY`].
    pub fn end_live_adjust(&mut self, cx: &mut Context<Self>) {
        if self.effects.live {
            self.effects.live = false;
            self.effects.drop_stage();
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
            watermark: e.mark_painted,
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

        self.effects.last = Some(key.clone());

        let current = self.effects.shown.as_ref().is_some_and(|s| s.key == key);
        if self.effects.live && !current {
            self.effects.pending = None;
            self.effects.task = None;
            if !self.effects.stage.as_ref().is_some_and(|s| s.fits(&key)) {
                self.effects.stage = None;
                self.build_stage(&key, cx);
            }
            if let Some(stage) = self.effects.stage.as_mut() {
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
            } else if !self
                .effects
                .shown
                .as_ref()
                .is_some_and(|s| s.key.same_content(&key))
            {
                return None;
            }
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
                let wallpapers = self.wallpapers.clone();
                self.schedule(key.clone(), base, list, wallpapers, cx);
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

    /// The stage is built off the main thread; until it lands the canvas keeps what it shows.
    fn build_stage(&mut self, key: &SceneKey, cx: &mut Context<Self>) {
        if self
            .effects
            .building
            .as_ref()
            .is_some_and(|b| live::stage_fits(b, key))
        {
            return;
        }
        let doc = self.doc.read(cx);
        let list = build_draw_list(doc.editor().doc(), &BuildContext::new(&**doc.text()));
        let base = doc.base().clone();
        let job = key.clone();
        let wallpapers = self.wallpapers.clone();
        let building = cx.background_executor().spawn(async move {
            let mut stage = LiveStage::new(&base, &list, &job, &wallpapers)?;
            let _ = stage.frame(&job.settings);
            Some(stage)
        });
        let key = key.clone();
        self.effects.building = Some(key.clone());
        self.effects.build_task = Some(cx.spawn(async move |this, cx| {
            let stage = building.await;
            let _ = this.update(cx, |canvas, cx| {
                let e = &mut canvas.effects;
                if e.building.as_ref() != Some(&key) {
                    return;
                }
                e.building = None;
                if e.live {
                    e.stage = stage;
                    cx.notify();
                }
            });
        }));
    }

    fn schedule(
        &mut self,
        key: SceneKey,
        base: Arc<RgbaImage>,
        list: DrawList,
        wallpapers: Arc<WallpaperCache>,
        cx: &mut Context<Self>,
    ) {
        self.effects.pending = Some(key.clone());
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE_DELAY).await;
            let job = key.clone();
            let image = cx
                .background_executor()
                .spawn(async move { render_settled(&base, &list, &job, &wallpapers) })
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
