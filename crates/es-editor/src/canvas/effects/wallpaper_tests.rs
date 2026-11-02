#![allow(clippy::unwrap_used)]

use super::live::LiveStage;
use super::{SceneKey, render_settled};
use es_anno::DrawList;
use es_model::{BackgroundSettings, BackgroundStyle, CameraSettings};
use es_render::beautify::wallpaper_cache::WallpaperCache;
use image::{Rgba, RgbaImage};

const RED: [u8; 4] = [230, 20, 20, 255];

fn red_wallpaper_look(dir: &std::path::Path) -> BackgroundSettings {
    let path = dir.join("red.png");
    RgbaImage::from_pixel(64, 64, Rgba(RED))
        .save(&path)
        .unwrap();
    BackgroundSettings {
        style: BackgroundStyle::Wallpaper { path },
        padding: 0.5,
        shadow: 0.0,
        camera: CameraSettings {
            tilt_x_degrees: 10.0,
            ..CameraSettings::default()
        },
        ..BackgroundSettings::default()
    }
}

fn key(settings: BackgroundSettings) -> SceneKey {
    SceneKey {
        settings,
        base: 0,
        shapes: Vec::new(),
        scale: 1.0,
    }
}

fn corner_bgra(pixels: &[u8], width: u32) -> [u8; 4] {
    let at = (2 * width as usize + 2) * 4;
    [pixels[at + 2], pixels[at + 1], pixels[at], pixels[at + 3]]
}

#[test]
fn the_settled_composite_draws_the_wallpaper_not_the_fallback_gradient() {
    let dir = tempfile::tempdir().unwrap();
    let key = key(red_wallpaper_look(dir.path()));
    let base = RgbaImage::from_pixel(120, 80, Rgba([10, 200, 10, 255]));
    let placed = render_settled(
        &base,
        &DrawList::default(),
        &key,
        &WallpaperCache::default(),
    )
    .unwrap();
    let bytes = placed.image.as_bytes(0).unwrap();
    assert_eq!(corner_bgra(bytes, placed.pixels.0), RED);
}

#[test]
fn the_live_frame_draws_the_wallpaper_not_the_fallback_gradient() {
    let dir = tempfile::tempdir().unwrap();
    let key = key(red_wallpaper_look(dir.path()));
    let base = RgbaImage::from_pixel(120, 80, Rgba([10, 200, 10, 255]));
    let mut stage = LiveStage::new(
        &base,
        &DrawList::default(),
        &key,
        &WallpaperCache::default(),
    )
    .unwrap();
    let frame = stage.frame(&key.settings).unwrap();
    let c = frame.pixel(2, 2).unwrap();
    assert_eq!(
        [c.red(), c.green(), c.blue(), c.alpha()],
        RED,
        "the live approximation shows the same background"
    );
}
