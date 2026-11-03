#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::canvas_support::{open, opened};

use es_anno::tool_cursor;
use es_editor::cursor::{cursors, style};
use es_model::Tool;
use gpui::{CursorImage, CursorStyle};
use image::{Rgba, RgbaImage};

#[test]
fn each_tool_family_shows_its_own_cursor() {
    let (mut app, _) = open(400., 300., opened(200, 100, vec![], Default::default()));
    let c = app.cx.update(cursors);
    let custom = CursorStyle::Custom;
    let families = [
        (Tool::Rectangle, custom(c.annotation_plus)),
        (Tool::FilledRectangle, custom(c.annotation_plus)),
        (Tool::Ellipse, custom(c.annotation_plus)),
        (Tool::Line, custom(c.annotation_plus)),
        (Tool::Arrow, custom(c.annotation_plus)),
        (Tool::Freehand, custom(c.pen)),
        (Tool::Highlighter, custom(c.pen)),
        (Tool::Text, CursorStyle::IBeam),
        (Tool::NumberedCircle, custom(c.badge)),
        (Tool::Highlight, custom(c.area)),
        (Tool::Pixelate, custom(c.area)),
        (Tool::Blur, custom(c.area)),
    ];
    for (tool, want) in families {
        assert_eq!(style(tool_cursor(tool), &c), want, "{tool:?}");
    }
    let ids = [c.annotation_plus, c.pen, c.badge, c.area];
    for (i, a) in ids.iter().enumerate() {
        assert!(
            ids[i + 1..].iter().all(|b| a != b),
            "every family has its own image"
        );
    }
}

/// Each tool cursor at 2x on a light and a dark ground, with its hotspot in red.
#[test]
fn contact_sheet_of_the_tool_cursors() {
    use es_ui::cursors::{annotation_plus, area, badge, pen};
    let images: [CursorImage; 4] = [annotation_plus(), pen(), badge(), area()];
    let (tile, zoom) = (64u32, 3u32);
    let grounds = [[236u8, 236, 236], [40, 40, 44]];
    let mut sheet = RgbaImage::new(
        tile * images.len() as u32 * zoom,
        tile * grounds.len() as u32 * zoom,
    );
    for (row, ground) in grounds.iter().enumerate() {
        for (col, image) in images.iter().enumerate() {
            let (scale, bgra) = image
                .images
                .iter()
                .find(|(s, _)| *s == 2.0)
                .expect("a 2x image");
            let side = (image.size.width as f32 * scale) as u32;
            let inset = (tile - side) / 2;
            let hot = (inset + image.hotspot.x * 2, inset + image.hotspot.y * 2);
            for y in 0..tile {
                for x in 0..tile {
                    let mut p = *ground;
                    if (inset..inset + side).contains(&x) && (inset..inset + side).contains(&y) {
                        let i = (((y - inset) * side + (x - inset)) * 4) as usize;
                        let (c, a) = (bgra[i], f32::from(bgra[i + 3]) / 255.);
                        p = p.map(|g| (f32::from(c) + f32::from(g) * (1. - a)).round() as u8);
                    }
                    if (x, y) == hot {
                        p = [230, 30, 30];
                    }
                    let (ox, oy) = (
                        (col as u32 * tile + x) * zoom,
                        (row as u32 * tile + y) * zoom,
                    );
                    for dy in 0..zoom {
                        for dx in 0..zoom {
                            sheet.put_pixel(ox + dx, oy + dy, Rgba([p[0], p[1], p[2], 255]));
                        }
                    }
                }
            }
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    sheet.save(dir.join("tool-cursors.png")).unwrap();
}
