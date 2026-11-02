use super::frozen::Tile;
use es_geom::Rect;
use es_ui::theme::UI_FONT;
use es_ui::tokens::{LIGHT, Tokens};
use gpui::{
    App, Bounds, Corners, CursorImage, CursorStyle, CustomCursorId, Font, FontFeatures, FontWeight,
    Global, Hsla, Pixels, SharedString, TextAlign, TextRun, Window, fill, hsla, point, px, quad,
    size,
};
use std::sync::Arc;

pub const PILL_SIZE: f32 = 11.0;
const PILL_PAD_X: f32 = 6.0;
const PILL_PAD_Y: f32 = 2.0;

pub fn b(r: Rect) -> Bounds<Pixels> {
    es_ui::convert::bounds(r)
}

pub fn white(a: f32) -> Hsla {
    hsla(0., 0., 1., a)
}

pub fn black(a: f32) -> Hsla {
    hsla(0., 0., 0., a)
}

pub fn accent(cx: &App) -> Hsla {
    cx.try_global::<Tokens>()
        .map(|t| t.colours.accent)
        .unwrap_or_else(|| es_ui::convert::hsla(LIGHT.accent))
}

pub fn tiles(tiles: &[Tile], window: &mut Window) {
    for tile in tiles {
        let at = b(tile.local);
        if let Err(error) =
            window.paint_image(at, at, Corners::default(), tile.image.clone(), 0, false)
        {
            tracing::warn!(%error, "frozen tile not painted");
        }
    }
}

pub fn dim(all: Rect, alpha: f32, window: &mut Window) {
    window.paint_quad(fill(b(all), black(alpha)));
}

/// `at` is the pill's top-left, or its top-centre when `centred`; it is pulled back inside `within`.
pub fn pill(
    text: SharedString,
    at: (f64, f64),
    centred: bool,
    within: Rect,
    background: Hsla,
    window: &mut Window,
    cx: &mut App,
) -> Rect {
    let font = Font {
        family: UI_FONT.into(),
        features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
        weight: FontWeight::MEDIUM,
        ..gpui::font(UI_FONT)
    };
    let run = TextRun {
        len: text.len(),
        font,
        color: white(1.),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line_height = px((PILL_SIZE * 1.3).round());
    let line = window
        .text_system()
        .shape_line(text, px(PILL_SIZE), &[run], None);
    let w = f64::from(f32::from(line.width) + PILL_PAD_X * 2.)
        .min(within.width())
        .max(0.0);
    let h = f64::from(f32::from(line_height) + PILL_PAD_Y * 2.);
    let left = if centred { at.0 - w / 2. } else { at.0 };
    let x = left.min(within.x1 - w).max(within.x0);
    let y = at.1.min(within.y1 - h).max(within.y0);
    let r = Rect::new(x, y, x + w, y + h);
    window.paint_quad(quad(
        b(r),
        Corners::all(px(4.)),
        background,
        px(0.),
        gpui::transparent_black(),
        Default::default(),
    ));
    let origin = point(
        px((x + f64::from(PILL_PAD_X)) as f32),
        px((y + f64::from(PILL_PAD_Y)) as f32),
    );
    window.with_content_mask(Some(gpui::ContentMask { bounds: b(r) }), |window| {
        if let Err(error) = line.paint(origin, line_height, TextAlign::Left, None, window, cx) {
            tracing::warn!(%error, "pill text not painted");
        }
    });
    r
}

pub fn cursor(id: CustomCursorId) -> CursorStyle {
    CursorStyle::Custom(id)
}

struct Cursors {
    camera: CustomCursorId,
    hidden: CustomCursorId,
}

impl Global for Cursors {}

fn hidden() -> CursorImage {
    CursorImage {
        images: es_ui::cursors::SCALES
            .iter()
            .map(|&s| {
                let side = (s.round() as usize).max(1);
                (s, Arc::from(vec![0u8; side * side * 4]))
            })
            .collect(),
        size: size(1, 1),
        hotspot: point(0, 0),
    }
}

fn registered(cx: &mut App) -> &Cursors {
    if cx.try_global::<Cursors>().is_none() {
        let cursors = Cursors {
            camera: cx.register_cursor_image(es_ui::cursors::window_camera()),
            hidden: cx.register_cursor_image(hidden()),
        };
        cx.set_global(cursors);
    }
    cx.global::<Cursors>()
}

pub fn camera_cursor(cx: &mut App) -> CursorStyle {
    cursor(registered(cx).camera)
}

pub fn hidden_cursor(cx: &mut App) -> CursorStyle {
    cursor(registered(cx).hidden)
}
