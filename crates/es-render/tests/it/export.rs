#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList};
use es_geom::{BezPath, FillRule, Rgba};
use es_model::{BackgroundColor, BackgroundSettings, BackgroundStyle, WatermarkSettings};
use es_render::beautify::{GradientPreset, layout};
use es_render::encode::{OutputFormat, PngProfile};
use es_render::export::{ExportCache, Purpose, export};
use image::ColorType;
use std::path::PathBuf;
use std::time::Instant;
use tiny_skia::{Color, Pixmap};

const FAST: OutputFormat = OutputFormat::Png(PngProfile::Fast);

fn solid(w: u32, h: u32, c: Color) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    p.fill(c);
    p
}

fn square(x: f64, y: f64, s: f64, color: Rgba) -> DrawItem {
    let mut path = BezPath::new();
    path.move_to((x, y));
    path.line_to((x + s, y));
    path.line_to((x + s, y + s));
    path.line_to((x, y + s));
    path.close_path();
    DrawItem::Fill {
        path,
        color,
        rule: FillRule::NonZero,
    }
}

fn red_square() -> DrawList {
    DrawList {
        items: vec![square(2.0, 2.0, 6.0, Rgba::new(1.0, 0.0, 0.0, 1.0))],
    }
}

fn blue_stage() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Solid(BackgroundColor::custom(0.0, 0.0, 1.0)),
        shadow: 0.0,
        corner_radius: 0.0,
        ..BackgroundSettings::default()
    }
}

#[test]
fn identical_inputs_compose_once_and_any_changed_input_composes_again() {
    let base = solid(32, 32, Color::WHITE);
    let notes = red_square();
    let look = blue_stage();
    let wall = b"wallpaper-a".as_slice();
    let mut cache = ExportCache::default();

    let first = cache
        .export(&base, &notes, &look, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 0);
    let again = cache
        .export(&base, &notes, &look, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 1);
    assert_eq!(first, again);

    cache
        .export(
            &solid(32, 32, Color::BLACK),
            &notes,
            &look,
            Some(wall),
            FAST,
        )
        .unwrap();
    assert_eq!(cache.hits(), 1, "a different base is a miss");

    cache
        .export(&base, &notes, &look, Some(wall), FAST)
        .unwrap();
    let moved = DrawList {
        items: vec![square(3.0, 2.0, 6.0, Rgba::new(1.0, 0.0, 0.0, 1.0))],
    };
    cache
        .export(&base, &moved, &look, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 1, "different annotations are a miss");

    cache
        .export(&base, &moved, &look, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 2);
    let padded = BackgroundSettings {
        padding: 0.3,
        ..look.clone()
    };
    cache
        .export(&base, &moved, &padded, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 2, "different background settings are a miss");

    cache
        .export(&base, &moved, &padded, Some(wall), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 3);
    cache
        .export(&base, &moved, &padded, Some(b"wallpaper-b"), FAST)
        .unwrap();
    assert_eq!(cache.hits(), 3, "different wallpaper bytes are a miss");
}

#[test]
fn a_cache_hit_can_be_encoded_in_another_format() {
    let base = solid(32, 32, Color::WHITE);
    let mut cache = ExportCache::default();
    let look = blue_stage();
    let png = cache
        .export(&base, &red_square(), &look, None, FAST)
        .unwrap();
    let jpg = cache
        .export(
            &base,
            &red_square(),
            &look,
            None,
            OutputFormat::Jpeg {
                quality_percent: 90,
            },
        )
        .unwrap();
    assert_eq!(cache.hits(), 1);
    assert!(png.starts_with(b"\x89PNG"));
    assert!(jpg.starts_with(&[0xFF, 0xD8]));
}

#[test]
fn export_carries_the_background_annotations_and_padding_into_the_file() {
    let base = solid(40, 40, Color::WHITE);
    let look = blue_stage();
    let bytes = export(&base, &red_square(), &look, None, FAST).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().into_rgba8();
    let lay = layout::layout(40.0, 40.0, &look).pixel_aligned();
    assert_eq!(img.dimensions(), lay.canvas_pixels());
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
    let (ox, oy) = (lay.image.x0 as u32, lay.image.y0 as u32);
    assert_eq!(img.get_pixel(ox + 4, oy + 4).0, [255, 0, 0, 255]);
    assert_eq!(img.get_pixel(ox + 20, oy + 20).0, [255, 255, 255, 255]);
}

#[test]
fn without_a_background_the_export_has_the_screenshots_own_size_and_pixels() {
    let base = solid(37, 23, Color::from_rgba8(10, 200, 30, 255));
    let look = BackgroundSettings {
        padding: 0.4,
        corner_radius: 0.1,
        ..BackgroundSettings::default()
    };
    let bytes = export(&base, &DrawList::default(), &look, None, FAST).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(img.dimensions(), (37, 23));
    assert_eq!(img.get_pixel(0, 0).0, [10, 200, 30, 255]);
}

#[test]
fn single_pixel_stripes_stay_exactly_black_and_white() {
    let mut base = Pixmap::new(31, 31).unwrap();
    for (i, px) in base
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let v = if (i % 31).is_multiple_of(2) { 0 } else { 255 };
        px.copy_from_slice(&[v, v, v, 255]);
    }
    let look = BackgroundSettings {
        corner_radius: 0.0,
        shadow: 0.0,
        ..blue_stage()
    };
    let bytes = export(&base, &DrawList::default(), &look, None, FAST).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().into_rgba8();
    let lay = layout::layout(31.0, 31.0, &look).pixel_aligned();
    let (ox, oy) = (lay.image.x0 as u32, lay.image.y0 as u32);
    for x in 0..31 {
        let v = img.get_pixel(ox + x, oy + 15).0[0];
        assert!(v == 0 || v == 255, "x {x} blurred to {v}");
    }
}

#[test]
fn jpeg_export_has_no_alpha_and_transparent_areas_are_white() {
    let base = Pixmap::new(20, 20).unwrap();
    let bytes = export(
        &base,
        &DrawList::default(),
        &BackgroundSettings::default(),
        None,
        OutputFormat::Jpeg {
            quality_percent: 90,
        },
    )
    .unwrap();
    let img = image::load_from_memory(&bytes).unwrap();
    assert_eq!(img.color(), ColorType::Rgb8);
    assert!(img.to_rgb8().get_pixel(10, 10).0.iter().all(|&c| c >= 250));
}

#[test]
fn watermark_text_shows_up_in_the_export_only_when_present() {
    let base = solid(200, 200, Color::WHITE);
    let plain = BackgroundSettings::default();
    let marked = BackgroundSettings {
        watermark: WatermarkSettings {
            text: "MARK".into(),
            density: 2.0,
            font_size: 24.0,
            rotation_degrees: 0.0,
            opacity: 0.5,
            color: Rgba::new(0.0, 0.0, 0.0, 1.0),
            ..WatermarkSettings::default()
        },
        ..plain.clone()
    };
    let decode = |s: &BackgroundSettings| {
        let b = export(&base, &DrawList::default(), s, None, FAST).unwrap();
        image::load_from_memory(&b).unwrap().into_rgba8()
    };
    let (a, b) = (decode(&plain), decode(&marked));
    assert!(a.pixels().all(|p| p.0 == [255, 255, 255, 255]));
    assert!(b.pixels().any(|p| p.0[0] < 250));
}

#[test]
fn a_wallpaper_style_uses_the_supplied_bytes() {
    let base = solid(20, 20, Color::WHITE);
    let wall = solid(8, 8, Color::from_rgba8(0, 0, 255, 255));
    let bytes = es_render::encode::encode_png(&wall, PngProfile::Fast).unwrap();
    let look = BackgroundSettings {
        style: BackgroundStyle::Wallpaper {
            path: PathBuf::from("w.png"),
        },
        shadow: 0.0,
        ..BackgroundSettings::default()
    };
    let out = export(&base, &DrawList::default(), &look, Some(&bytes), FAST).unwrap();
    let img = image::load_from_memory(&out).unwrap().into_rgba8();
    assert_eq!(img.get_pixel(0, 0).0, [0, 0, 255, 255]);
}

#[test]
fn purpose_picks_the_png_profile() {
    assert_eq!(Purpose::Transient.png_profile(), PngProfile::Fast);
    assert_eq!(Purpose::Keep.png_profile(), PngProfile::Small);
}

#[test]
#[ignore = "timing"]
#[allow(clippy::print_stdout)]
fn timing_of_a_large_capture_with_fifty_shapes_on_a_gradient() {
    let mut base = Pixmap::new(2560, 1440).unwrap();
    for (i, px) in base
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let (x, y) = (i % 2560, i / 2560);
        px.copy_from_slice(&[(x / 12) as u8, (y / 7) as u8, ((x + y) / 20) as u8, 255]);
    }
    let items = (0..50)
        .map(|i| {
            let (x, y) = (f64::from(i % 10) * 240.0, f64::from(i / 10) * 270.0);
            square(x + 20.0, y + 20.0, 180.0, Rgba::new(1.0, 0.2, 0.1, 0.8))
        })
        .collect();
    let notes = DrawList { items };
    let look = BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("azure").unwrap().style()),
        ..BackgroundSettings::default()
    };
    for (name, format) in [
        ("png fast", OutputFormat::Png(PngProfile::Fast)),
        ("png small", OutputFormat::Png(PngProfile::Small)),
        (
            "jpeg 90",
            OutputFormat::Jpeg {
                quality_percent: 90,
            },
        ),
    ] {
        let t = Instant::now();
        let bytes = export(&base, &notes, &look, None, format).unwrap();
        println!("{name}: {:?}, {} bytes", t.elapsed(), bytes.len());
    }
    let composed = ExportCache::default()
        .composed(&base, &notes, &look, None)
        .unwrap();
    for profile in [PngProfile::Fast, PngProfile::Small] {
        let t = Instant::now();
        let bytes = es_render::encode::encode_png(&composed, profile).unwrap();
        println!(
            "encode only {profile:?}: {:?}, {} bytes",
            t.elapsed(),
            bytes.len()
        );
    }
}
