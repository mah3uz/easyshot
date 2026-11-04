use super::{SceneKey, scaled_base};
use crate::canvas::underlay::cached_wallpaper;
use es_anno::DrawList;
use es_geom::Rect;
use es_model::{BackgroundSettings, BlurEdgeMode, CameraSettings, ProgressiveBlurSettings};
use es_render::beautify::compose::{
    Stage, blur_scope, canvas_size, lay_card, paint_watermark, stage,
};
use es_render::beautify::progressive_blur::{blend_levels, focus_mask, levels};
use es_render::beautify::wallpaper_cache::WallpaperCache;
use image::RgbaImage;
use std::borrow::Cow;
use tiny_skia::{BlendMode, IntRect, Pixmap, PixmapPaint, Transform};

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

/// A stage built for `built` serves `wanted` when only the camera and blur values differ.
pub fn stage_fits(built: &SceneKey, wanted: &SceneKey) -> bool {
    built.same_content(wanted)
        && built.scale == wanted.scale
        && stage_settings(&built.settings) == stage_settings(&wanted.settings)
}

#[derive(Debug, Clone, PartialEq)]
struct LevelsKey {
    camera: CameraSettings,
    strength: f64,
    region: IntRect,
}

/// Built once per drag at the composite's resolution, then projected and blurred each frame as the export does.
pub struct LiveStage {
    key: SceneKey,
    stage: Stage,
    pub scale: (f64, f64),
    flat: Option<(CameraSettings, Pixmap)>,
    levels: Option<(LevelsKey, Vec<Pixmap>)>,
}

impl LiveStage {
    pub fn new(
        base: &RgbaImage,
        list: &DrawList,
        key: &SceneKey,
        wallpapers: &WallpaperCache,
    ) -> Option<Self> {
        let (image, page_scale) = scaled_base(base, key.scale)?;
        let settings = &key.settings;
        let canvas = canvas_size((image.width(), image.height()), settings);
        let wallpaper = cached_wallpaper(wallpapers, &settings.style, canvas);
        let stage = stage(&image, list, settings, wallpaper.as_deref(), page_scale)?;
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
        stage_fits(&self.key, key)
    }

    fn flat(&mut self, camera: &CameraSettings) -> Option<&Pixmap> {
        if self.flat.as_ref().is_none_or(|(c, _)| c != camera) {
            let mut flat = self.stage.background.clone();
            lay_card(&mut flat, &self.stage.card, &self.stage.layout, camera);
            self.flat = Some((camera.clone(), flat));
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
            paint_watermark(&mut canvas, &settings.watermark, scale);
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
            self.levels = Some((key, levels(&sharp, blur)?));
        }
        let levels = &self.levels.as_ref()?.1;
        let mask = focus_mask(sharp.width(), sharp.height(), blur);
        let blurred = blend_levels(&sharp, levels, &mask)?;
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
        paint_watermark(&mut canvas, &settings.watermark, scale);
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
