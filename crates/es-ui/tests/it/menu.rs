use crate::support;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use es_ui::components::{MenuItem, MenuLead};
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use image::{Rgba, RgbaImage};
use support::*;

const SCALE: u32 = 2;
const ROW: u32 = 32;

struct Icons {
    leads: Vec<MenuLead>,
}

impl Render for Icons {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows =
            self.leads.iter().enumerate().map(|(i, lead)| {
                MenuItem::new(("row", i), "Notes", lead.clone()).detail(Some("App"))
            });
        div().size_full().flex().flex_col().children(rows)
    }
}

fn file(name: &str, write: impl FnOnce(&Path)) -> Arc<Path> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("menu-icons");
    std::fs::create_dir_all(&dir).expect("icon dir");
    let path = dir.join(name);
    write(&path);
    Arc::from(path)
}

/// The lead slot of row `i`, 18 points square, 8 in from the row's left edge.
fn slot(shot: &RgbaImage, i: u32) -> RgbaImage {
    let top = (i * ROW + 7) * SCALE;
    image::imageops::crop_imm(shot, 8 * SCALE, top, 18 * SCALE, 18 * SCALE).to_image()
}

fn centre(slot: &RgbaImage) -> [u8; 3] {
    let p = slot.get_pixel(slot.width() / 2, slot.height() / 2).0;
    [p[0], p[1], p[2]]
}

#[test]
fn an_app_icon_draws_from_png_or_svg_and_a_missing_one_falls_back_to_the_window_glyph() {
    let png = file("red.png", |p| {
        RgbaImage::from_pixel(48, 48, Rgba([220, 30, 30, 255]))
            .save(p)
            .expect("png");
    });
    let svg = file("green.svg", |p| {
        std::fs::write(
            p,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48"><rect width="48" height="48" fill="rgb(30,200,60)"/></svg>"#,
        )
        .expect("svg");
    });
    let missing = file("gone.png", |p| {
        let _ = std::fs::remove_file(p);
    });
    let leads = vec![
        MenuLead::App(Some(png)),
        MenuLead::App(Some(svg)),
        MenuLead::App(Some(missing)),
        MenuLead::App(None),
    ];
    let (mut app, _) = open(4. * ROW as f32, |_, _| Icons { leads });
    let shot = app.screenshot("menu-item-icons");

    let red = centre(&slot(&shot, 0));
    assert!(red[0] > 180 && red[1] < 80, "the PNG is drawn: {red:?}");
    let green = centre(&slot(&shot, 1));
    assert!(
        green[1] > 160 && green[0] < 80,
        "the SVG is drawn: {green:?}"
    );

    let (failed, none) = (slot(&shot, 2), slot(&shot, 3));
    assert_eq!(
        failed.as_raw(),
        none.as_raw(),
        "an icon that fails to load looks like no icon, never a broken image"
    );
    let ground = shot.get_pixel(SCALE, 3 * ROW * SCALE + SCALE);
    assert!(
        none.pixels().any(|p| p != ground),
        "the generic window glyph stands in"
    );
}
