#![allow(clippy::unwrap_used)]

use es_render::beautify::wallpaper::draw_wallpaper;
use es_render::beautify::wallpaper_cache::WallpaperCache;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tiny_skia::Pixmap;

fn ramp(w: u32, h: u32) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    for y in 0..h {
        let v = (y * 255 / (h - 1)) as u8;
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            p.data_mut()[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    p
}

fn file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, name).unwrap();
    path
}

#[test]
fn a_file_is_decoded_once_however_often_it_is_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "a.png");
    let cache = WallpaperCache::default();
    let loads = Cell::new(0);
    let load = || {
        loads.set(loads.get() + 1);
        Some(ramp(400, 400))
    };
    for _ in 0..5 {
        assert!(cache.get(&path, (100, 100), load).is_some());
    }
    assert_eq!(loads.get(), 1);
}

#[test]
fn a_smaller_canvas_gets_a_smaller_copy_that_still_covers_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "a.png");
    let cache = WallpaperCache::default();
    let source = || Some(ramp(1000, 1000));
    let full = cache.get(&path, (1000, 1000), source).unwrap();
    let small = cache.get(&path, (300, 200), source).unwrap();
    assert_eq!(full.width(), 1000);
    assert!(small.width() >= 300 && small.height() >= 200);
    assert!(small.width() < 600, "reduced: {}", small.width());
}

#[test]
fn a_changed_file_is_decoded_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = file(dir.path(), "a.png");
    let cache = WallpaperCache::default();
    let loads = Cell::new(0);
    let load = || {
        loads.set(loads.get() + 1);
        Some(ramp(64, 64))
    };
    cache.get(&path, (64, 64), load).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(60))
        .unwrap();
    cache.get(&path, (64, 64), load).unwrap();
    assert_eq!(loads.get(), 2);
}

#[test]
fn the_least_recently_used_image_goes_first() {
    let dir = tempfile::tempdir().unwrap();
    let cache = WallpaperCache::new(2, usize::MAX);
    let paths = ["a", "b", "c"].map(|n| file(dir.path(), n));
    let load = || Some(ramp(32, 32));
    cache.get(&paths[0], (32, 32), load).unwrap();
    cache.get(&paths[1], (32, 32), load).unwrap();
    cache.get(&paths[0], (32, 32), load).unwrap();
    cache.get(&paths[2], (32, 32), load).unwrap();
    assert_eq!(cache.len(), 2);
    let loads = Cell::new(0);
    let counting = || {
        loads.set(loads.get() + 1);
        Some(ramp(32, 32))
    };
    cache.get(&paths[0], (32, 32), counting).unwrap();
    assert_eq!(loads.get(), 0, "the touched image survived");
    cache.get(&paths[1], (32, 32), counting).unwrap();
    assert_eq!(loads.get(), 1, "the untouched one was evicted");
}

#[test]
fn the_decoded_bytes_stay_under_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let image = 32 * 32 * 4;
    let cache = WallpaperCache::new(4, image * 2);
    for name in ["a", "b", "c"] {
        let path = file(dir.path(), name);
        cache.get(&path, (32, 32), || Some(ramp(32, 32))).unwrap();
    }
    assert!(cache.bytes() <= image * 2);
    assert_eq!(cache.len(), 2);
}

#[test]
fn a_cover_scaled_wallpaper_is_a_smooth_centred_crop() {
    let wall = ramp(800, 800);
    let mut canvas = Pixmap::new(200, 100).unwrap();
    draw_wallpaper(&mut canvas, &wall);
    let rows: Vec<i32> = (0..100)
        .map(|y| i32::from(canvas.pixel(100, y).unwrap().red()))
        .collect();
    assert!(rows.windows(2).all(|w| w[1] >= w[0]), "a monotonic ramp");
    assert!((rows[50] - 128).abs() <= 4, "{}", rows[50]);
}
