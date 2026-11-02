#![allow(clippy::unwrap_used)]

use es_anno::draw::{DrawItem, DrawList};
use es_geom::{BezPath, FillRule, Rect, Rgba};
use es_model::{BackgroundColor, BackgroundSettings, BackgroundStyle, ShadowStyle};
use es_render::beautify::compose;
use es_render::beautify::shadow::{paint_shadow, shadow_layer};
use es_render::paint::ClipShape;
use tiny_skia::{Color, Pixmap};

fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let c = p.pixel(x, y).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn solid(w: u32, h: u32, c: Color) -> Pixmap {
    let mut p = Pixmap::new(w, h).unwrap();
    p.fill(c);
    p
}

fn blue_stage() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Solid(BackgroundColor::custom(0.0, 0.0, 1.0)),
        padding: 0.2,
        shadow: 0.0,
        ..BackgroundSettings::default()
    }
}

fn green_square(x0: f64, y0: f64, x1: f64, y1: f64) -> DrawList {
    let mut path = BezPath::new();
    path.move_to((x0, y0));
    path.line_to((x1, y0));
    path.line_to((x1, y1));
    path.line_to((x0, y1));
    path.close_path();
    DrawList {
        items: vec![DrawItem::Fill {
            path,
            color: Rgba::new(0.0, 1.0, 0.0, 1.0),
            rule: FillRule::NonZero,
        }],
    }
}

#[test]
fn shadow_falls_outside_the_card_and_never_inside_it() {
    let mut canvas = Pixmap::new(400, 400).unwrap();
    let card = ClipShape {
        rect: Rect::new(100.0, 100.0, 300.0, 300.0),
        radii: [12.0; 4],
    };
    let layer = shadow_layer(ShadowStyle::Soft, 0.6, 200.0).unwrap();
    paint_shadow(&mut canvas, &card, &layer);

    assert!(px(&canvas, 200, 310)[3] > 0, "shadow below the card");
    assert_eq!(px(&canvas, 200, 200)[3], 0, "card interior stays clear");
    assert_eq!(px(&canvas, 150, 150)[3], 0);
    assert_eq!(px(&canvas, 5, 5)[3], 0, "far corner untouched");
    assert!(
        px(&canvas, 200, 310)[3] > px(&canvas, 200, 92)[3],
        "drop is offset downward"
    );
}

#[test]
fn shadow_strength_grows_the_drop_but_caps_the_opacity() {
    let weak = shadow_layer(ShadowStyle::Soft, 0.1, 500.0).unwrap();
    let strong = shadow_layer(ShadowStyle::Soft, 1.0, 500.0).unwrap();
    assert!(strong.radius > weak.radius * 5.0);
    assert!(strong.alpha <= 0.5 && strong.alpha >= weak.alpha);
    assert!(shadow_layer(ShadowStyle::Soft, 0.0, 500.0).is_none());
    assert_eq!(
        shadow_layer(ShadowStyle::Glow, 0.5, 500.0)
            .unwrap()
            .y_offset,
        0.0
    );
    let long = shadow_layer(ShadowStyle::Long, 0.5, 500.0).unwrap();
    let crisp = shadow_layer(ShadowStyle::Crisp, 0.5, 500.0).unwrap();
    assert!(long.y_offset > crisp.y_offset);
}

#[test]
fn the_image_is_clipped_to_rounded_corners_over_the_background() {
    let image = solid(200, 100, Color::from_rgba8(255, 0, 0, 255));
    let settings = BackgroundSettings {
        corner_radius: 0.3,
        ..blue_stage()
    };
    let out = compose(&image, &DrawList::default(), &settings).unwrap();
    assert_eq!((out.width(), out.height()), (240, 140));
    assert_eq!(px(&out, 120, 70), [255, 0, 0, 255], "image centre");
    assert_eq!(
        px(&out, 22, 22),
        [0, 0, 255, 255],
        "clipped corner shows the background"
    );
    assert_eq!(px(&out, 2, 2), [0, 0, 255, 255]);
}

#[test]
fn square_corners_keep_the_whole_image() {
    let image = solid(200, 100, Color::from_rgba8(255, 0, 0, 255));
    let settings = BackgroundSettings {
        corner_radius: 0.0,
        ..blue_stage()
    };
    let out = compose(&image, &DrawList::default(), &settings).unwrap();
    assert_eq!(px(&out, 20, 20), [255, 0, 0, 255]);
    assert_eq!(px(&out, 19, 20), [0, 0, 255, 255]);
}

#[test]
fn annotations_paint_above_the_image() {
    let image = solid(200, 100, Color::from_rgba8(255, 0, 0, 255));
    let settings = BackgroundSettings {
        corner_radius: 0.0,
        ..blue_stage()
    };
    let list = green_square(10.0, 10.0, 60.0, 60.0);
    let out = compose(&image, &list, &settings).unwrap();
    assert_eq!(px(&out, 20 + 30, 20 + 30), [0, 255, 0, 255]);
    assert_eq!(px(&out, 20 + 100, 20 + 80), [255, 0, 0, 255]);
}

#[test]
fn without_a_background_the_export_is_the_image_plus_annotations() {
    let image = solid(50, 50, Color::from_rgba8(255, 0, 0, 255));
    let out = compose(
        &image,
        &green_square(0.0, 0.0, 10.0, 10.0),
        &BackgroundSettings::default(),
    )
    .unwrap();
    assert_eq!((out.width(), out.height()), (50, 50));
    assert_eq!(px(&out, 5, 5), [0, 255, 0, 255]);
    assert_eq!(px(&out, 30, 30), [255, 0, 0, 255]);
}

#[test]
fn a_border_ring_sits_between_the_background_and_the_image() {
    let image = solid(200, 100, Color::from_rgba8(255, 0, 0, 255));
    let mut settings = BackgroundSettings {
        corner_radius: 0.0,
        ..blue_stage()
    };
    settings.border.is_enabled = true;
    settings.border.thickness = 0.1;
    let out = compose(&image, &DrawList::default(), &settings).unwrap();
    assert_eq!((out.width(), out.height()), (260, 160));
    let (ring, interior) = (px(&out, 25, 80), px(&out, 130, 80));
    assert_eq!(interior, [255, 0, 0, 255]);
    assert!(ring[0] > 200 && ring[1] > 200 && ring[2] > 200, "{ring:?}");
}

#[test]
fn the_shadow_darkens_the_background_around_the_card_only() {
    let image = solid(200, 100, Color::from_rgba8(255, 255, 255, 255));
    let settings = BackgroundSettings {
        shadow: 1.0,
        padding: 0.4,
        corner_radius: 0.0,
        style: BackgroundStyle::Solid(BackgroundColor::custom(1.0, 1.0, 1.0)),
        ..BackgroundSettings::default()
    };
    let out = compose(&image, &DrawList::default(), &settings).unwrap();
    let below = px(&out, out.width() / 2, out.height() - 35);
    assert!(below[0] < 240, "{below:?}");
    assert_eq!(px(&out, 3, 3)[0], 255);
}

#[test]
fn a_spotlight_dims_the_image_where_it_sits_on_a_padded_stage() {
    let image = solid(200, 100, Color::from_rgba8(255, 255, 255, 255));
    let settings = BackgroundSettings {
        corner_radius: 0.0,
        ..blue_stage()
    };
    let list = DrawList {
        items: vec![DrawItem::Spotlight { holes: Vec::new() }],
    };
    let out = compose(&image, &list, &settings).unwrap();
    let inside = px(&out, 20 + 190, 20 + 90);
    assert!(
        inside[0] < 255,
        "the image's far corner is dimmed: {inside:?}"
    );
    assert_eq!(
        px(&out, 20 + 205, 20 + 50),
        [0, 0, 255, 255],
        "the stage is not"
    );
}

#[test]
fn a_scaled_compose_places_annotations_on_the_scaled_image() {
    let image = solid(100, 50, Color::from_rgba8(255, 0, 0, 255));
    let settings = BackgroundSettings {
        corner_radius: 0.0,
        ..blue_stage()
    };
    let list = green_square(100.0, 40.0, 160.0, 80.0);
    let out =
        es_render::beautify::compose::compose_scaled(&image, &list, &settings, None, (0.5, 0.5))
            .unwrap();
    assert_eq!(px(&out, 10 + 65, 10 + 30), [0, 255, 0, 255]);
    assert_eq!(px(&out, 10 + 45, 10 + 30), [255, 0, 0, 255]);
}
