use super::{SceneKey, scaled_base};
use es_anno::DrawList;
use es_geom::Rect;
use es_model::{BackgroundSettings, BlurEdgeMode, CameraSettings, ProgressiveBlurSettings};
use es_render::beautify::camera::{Quality, projection, warp};
use es_render::beautify::compose::{Stage, blur_scope, paint_watermark, stage};
use es_render::beautify::progressive_blur::{blend_preview, focus_mask, preview_levels};
use image::RgbaImage;
use rayon::prelude::*;
use std::borrow::Cow;
use tiny_skia::{BlendMode, IntRect, Pixmap, PixmapPaint, Transform};

/// The approximation is drawn at half the composite's resolution.
const LIVE_SCALE: f64 = 0.5;

/// The settings with the camera and blur values taken out, keeping what they do to the layout.
fn stage_settings(s: &BackgroundSettings) -> (BackgroundSettings, bool, bool) {
    let camera = s.camera.has_effect();
    let bleed =
        s.progressive_blur.is_active() && s.progressive_blur.edge_mode == BlurEdgeMode::Bleed;
    let neutral = BackgroundSettings {
        camera: CameraSettings::default(),
        progressive_blur: ProgressiveBlurSettings::default(),
        ..s.clone()
    };
    (neutral, camera, bleed)
}

#[derive(Debug, Clone, PartialEq)]
struct LevelsKey {
    camera: CameraSettings,
    strength: f64,
    region: IntRect,
}

/// Built once per drag: the background and the flat card, then projected and blurred each frame.
pub struct LiveStage {
    key: SceneKey,
    stage: Stage,
    pub scale: (f64, f64),
    flat: Option<(CameraSettings, Pixmap)>,
    levels: Option<(LevelsKey, [Pixmap; 3])>,
}

impl LiveStage {
    pub fn new(base: &RgbaImage, list: &DrawList, key: &SceneKey) -> Option<Self> {
        let (image, page_scale) = scaled_base(base, key.scale * LIVE_SCALE)?;
        let settings = &key.settings;
        let stage = stage(&image, list, settings, page_scale)?;
        Some(Self {
            key: key.clone(),
            stage,
            scale: page_scale,
            flat: None,
            levels: None,
        })
    }

    pub fn image_origin(&self) -> (f64, f64) {
        (self.stage.layout.image.x0, self.stage.layout.image.y0)
    }

    pub fn fits(&self, key: &SceneKey) -> bool {
        self.key.same_content(key)
            && self.key.scale == key.scale
            && stage_settings(&self.key.settings) == stage_settings(&key.settings)
    }

    fn flat(&mut self, camera: &CameraSettings) -> Option<&Pixmap> {
        if self.flat.as_ref().is_none_or(|(c, _)| c != camera) {
            let lay = &self.stage.layout;
            let bg = &self.stage.background;
            let size = (f64::from(bg.width()), f64::from(bg.height()));
            let warped = camera
                .has_effect()
                .then(|| projection(lay.card, lay.image, size, camera))
                .flatten()
                .and_then(|p| warp(&self.stage.card, &p.forward, Quality::Full));
            let card = warped.as_ref().unwrap_or(&self.stage.card);
            self.flat = Some((camera.clone(), over(card, bg)?));
        }
        self.flat.as_ref().map(|(_, p)| p)
    }

    pub fn frame(&mut self, settings: &BackgroundSettings) -> Option<Cow<'_, Pixmap>> {
        let blur = &settings.progressive_blur;
        let mark = settings.watermark.is_visible();
        let scale = self.scale.0;
        self.flat(&settings.camera)?;
        if !blur.is_active() {
            let flat = &self.flat.as_ref()?.1;
            if !mark {
                return Some(Cow::Borrowed(flat));
            }
            let mut canvas = flat.clone();
            paint_watermark(&mut canvas, settings, scale);
            return Some(Cow::Owned(canvas));
        }
        let region = self.region(settings)?;
        let key = LevelsKey {
            camera: settings.camera.clone(),
            strength: blur.strength,
            region,
        };
        let flat = &self.flat.as_ref()?.1;
        let whole = region.width() == flat.width() && region.height() == flat.height();
        let sharp = if whole {
            Cow::Borrowed(flat)
        } else {
            Cow::Owned(flat.clone_rect(region)?)
        };
        if self.levels.as_ref().is_none_or(|(k, _)| *k != key) {
            self.levels = Some((key, preview_levels(&sharp, blur)?));
        }
        let levels = &self.levels.as_ref()?.1;
        let mask = focus_mask(sharp.width(), sharp.height(), blur);
        let blurred = blend_preview(&sharp, levels, &mask)?;
        let mut canvas = if whole {
            blurred
        } else {
            let mut canvas = flat.clone();
            canvas.draw_pixmap(
                region.x(),
                region.y(),
                blurred.as_ref(),
                &PixmapPaint {
                    blend_mode: BlendMode::Source,
                    ..PixmapPaint::default()
                },
                Transform::identity(),
                None,
            );
            canvas
        };
        paint_watermark(&mut canvas, settings, scale);
        Some(Cow::Owned(canvas))
    }

    fn region(&self, settings: &BackgroundSettings) -> Option<IntRect> {
        let canvas = &self.stage.background;
        let (cw, ch) = (f64::from(canvas.width()), f64::from(canvas.height()));
        let r = if settings.progressive_blur.edge_mode == BlurEdgeMode::Bleed {
            Rect::new(0.0, 0.0, cw, ch)
        } else {
            blur_scope(canvas, &self.stage.layout, settings)
        };
        let x0 = r.x0.floor().clamp(0.0, cw);
        let y0 = r.y0.floor().clamp(0.0, ch);
        let x1 = r.x1.ceil().clamp(0.0, cw);
        let y1 = r.y1.ceil().clamp(0.0, ch);
        IntRect::from_ltrb(x0 as i32, y0 as i32, x1 as i32, y1 as i32)
    }
}

/// Premultiplied source-over, in parallel: the stage is redrawn on every live frame.
fn over(top: &Pixmap, under: &Pixmap) -> Option<Pixmap> {
    if top.width() != under.width() || top.height() != under.height() {
        return None;
    }
    let mut out = under.clone();
    out.data_mut()
        .par_chunks_mut(64 * 1024)
        .zip(top.data().par_chunks(64 * 1024))
        .for_each(|(dst, src)| {
            let (dst, src) = (dst.as_chunks_mut::<4>().0, src.as_chunks::<4>().0);
            for (d, s) in dst.iter_mut().zip(src) {
                let keep = 255 - u16::from(s[3]);
                for c in 0..4 {
                    d[c] = s[c].saturating_add(((u16::from(d[c]) * keep + 127) / 255) as u8);
                }
            }
        });
    Some(out)
}
