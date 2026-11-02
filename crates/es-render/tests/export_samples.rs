#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList};
use es_geom::{BezPath, Cap, Join, Rgba};
use es_model::{BackgroundSettings, BackgroundStyle, WatermarkSettings};
use es_render::beautify::GradientPreset;
use es_render::encode::{OutputFormat, PngProfile};
use es_render::export::export;
use tiny_skia::Pixmap;

#[test]
#[ignore = "writes sample files to the directory in EXPORT_SAMPLE_DIR"]
fn write_sample_exports() {
    let Ok(dir) = std::env::var("EXPORT_SAMPLE_DIR") else {
        return;
    };
    let (w, h) = (900u32, 560u32);
    let mut base = Pixmap::new(w, h).unwrap();
    for (i, px) in base
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        let (x, y) = (
            (i as u32 % w) as f32 / w as f32,
            (i as u32 / w) as f32 / h as f32,
        );
        px.copy_from_slice(&[(250.0 - 90.0 * x) as u8, (245.0 - 60.0 * y) as u8, 255, 255]);
    }
    let mut arrow = BezPath::new();
    arrow.move_to((120.0, 420.0));
    arrow.line_to((520.0, 180.0));
    let mut ring = BezPath::new();
    ring.move_to((600.0, 300.0));
    ring.curve_to((600.0, 200.0), (780.0, 200.0), (780.0, 300.0));
    ring.curve_to((780.0, 400.0), (600.0, 400.0), (600.0, 300.0));
    let stroke = |path, color| DrawItem::Stroke {
        path,
        color,
        width: 8.0,
        cap: Cap::Round,
        join: Join::Round,
        dash: None,
    };
    let notes = DrawList {
        items: vec![
            stroke(arrow, Rgba::new(0.9, 0.1, 0.1, 1.0)),
            stroke(ring, Rgba::new(0.1, 0.5, 0.2, 1.0)),
        ],
    };
    let look = BackgroundSettings {
        style: BackgroundStyle::Gradient(GradientPreset::by_id("soft-lilac").unwrap().style()),
        watermark: WatermarkSettings {
            text: "easyshot".into(),
            font_size: 40.0,
            ..WatermarkSettings::default()
        },
        ..BackgroundSettings::default()
    };
    let png = export(
        &base,
        &notes,
        &look,
        None,
        OutputFormat::Png(PngProfile::Small),
    )
    .unwrap();
    let jpg = export(
        &base,
        &notes,
        &look,
        None,
        OutputFormat::Jpeg {
            quality_percent: 90,
        },
    )
    .unwrap();
    std::fs::write(format!("{dir}/sample.png"), png).unwrap();
    std::fs::write(format!("{dir}/sample.jpg"), jpg).unwrap();
}
