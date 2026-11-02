mod support;

use es_ui::{
    components::{StrokeDots, SwatchPick, SwatchStrip, Tile, TileGrid},
    theme,
};
use gpui::{
    Context, Hsla, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, hsla,
};
use support::*;

const SCALE: u32 = 2;

fn near(pixel: [u8; 4], c: Hsla) -> bool {
    let c = c.to_rgb();
    [c.r, c.g, c.b]
        .iter()
        .zip(pixel)
        .all(|(&want, got)| ((want * 255.).round() as u8).abs_diff(got) <= 3)
}

struct TileHost {
    selected: Option<usize>,
}

impl Render for TileHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let tiles =
            (0..10).map(|i| Tile::new(i, format!("Tile {i}")).fill(hsla(0.6, 0.5, 0.5, 1.)));
        div().w(gpui::px(WIDTH)).child(
            TileGrid::new("grid", "Fill", tiles)
                .columns(5)
                .spacing(gpui::px(0.))
                .selected(self.selected)
                .on_select(move |i, _, cx| {
                    this.update(cx, |host, cx| {
                        host.selected = Some(i);
                        cx.notify();
                    })
                }),
        )
    }
}

#[test]
fn tile_grid_selects_by_click_and_moves_by_rows_and_columns_with_arrows() {
    let (mut app, host) = open(200., |_, _| TileHost { selected: None });
    let selected = |app: &mut App| app.cx.update(|cx| host.read(cx).selected);
    let tile = WIDTH / 5.;

    click(&mut app, tile * 2.5, tile * 0.5);
    assert_eq!(
        selected(&mut app),
        Some(2),
        "clicking the third tile selects it"
    );

    key(&mut app, "down");
    assert_eq!(selected(&mut app), Some(7), "Down moves one row of five");
    key(&mut app, "down");
    assert_eq!(selected(&mut app), Some(7), "Down stops at the last row");
    key(&mut app, "right");
    key(&mut app, "right");
    key(&mut app, "right");
    assert_eq!(selected(&mut app), Some(9), "Right stops at the last tile");
    key(&mut app, "up");
    key(&mut app, "left");
    assert_eq!(selected(&mut app), Some(3), "Up and Left move back");

    click(&mut app, tile * 0.5, tile * 1.5);
    assert_eq!(
        selected(&mut app),
        Some(5),
        "a tile on the second row is clickable"
    );
}

struct SwatchHost {
    pick: SwatchPick,
}

fn palette() -> Vec<(Hsla, SharedString)> {
    (0..10)
        .map(|i| {
            (
                hsla(i as f32 / 10., 0.8, 0.5, 1.),
                format!("Colour {i}").into(),
            )
        })
        .collect()
}

impl Render for SwatchHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().size_full().child(
            SwatchStrip::new("swatches", "Color", palette())
                .selected(self.pick)
                .on_select(move |pick, _, cx| {
                    this.update(cx, |host, cx| {
                        host.pick = pick;
                        cx.notify();
                    })
                }),
        )
    }
}

fn swatch_centre(index: usize) -> (f32, f32) {
    let gap = 8.;
    let column = (WIDTH - gap * 5.) / 6.;
    let row = 29. + gap;
    let (c, r) = (index % 6, index / 6);
    (
        c as f32 * (column + gap) + column / 2.,
        r as f32 * row + 14.5,
    )
}

fn popover_shown(app: &mut App) -> bool {
    let dropdown = app.cx.update(|cx| theme::tokens(cx).dropdown);
    let image = app.screenshot("picks-swatch-popover");
    // Inside the popover's left padding, below the strip: nothing else is drawn there.
    let (x, y) = (14 * SCALE, 200 * SCALE);
    near(image.get_pixel(x, y).0, dropdown)
}

#[test]
fn swatch_strip_selects_presets_by_click_and_arrows_and_the_well_opens_the_picker() {
    let (mut app, host) = open(460., |_, _| SwatchHost {
        pick: SwatchPick::Preset(0),
    });
    let pick = |app: &mut App| app.cx.update(|cx| host.read(cx).pick);

    let (x, y) = swatch_centre(4);
    click(&mut app, x, y);
    assert_eq!(
        pick(&mut app),
        SwatchPick::Preset(4),
        "clicking a swatch picks it"
    );

    key(&mut app, "down");
    assert_eq!(
        pick(&mut app),
        SwatchPick::Preset(4),
        "no preset below the fifth"
    );
    key(&mut app, "left");
    key(&mut app, "down");
    assert_eq!(
        pick(&mut app),
        SwatchPick::Preset(9),
        "Down moves a row of six"
    );
    key(&mut app, "right");
    assert_eq!(
        pick(&mut app),
        SwatchPick::Preset(9),
        "arrows never land on the custom well, which has no colour to pick yet"
    );
    key(&mut app, "up");
    assert_eq!(pick(&mut app), SwatchPick::Preset(3), "Up moves back a row");

    assert!(!popover_shown(&mut app), "the picker starts closed");
    key(&mut app, "tab");
    key(&mut app, "enter");
    assert!(
        popover_shown(&mut app),
        "Tab reaches the custom well and Enter opens the picker"
    );
    key(&mut app, "escape");
    assert!(!popover_shown(&mut app), "Escape closes the picker");

    let (x, y) = swatch_centre(10);
    click(&mut app, x, y);
    assert!(
        popover_shown(&mut app),
        "clicking the custom well opens the picker"
    );
    assert_eq!(
        pick(&mut app),
        SwatchPick::Preset(3),
        "opening the picker picks nothing yet"
    );
}

struct StrokeHost {
    width: f32,
}

impl Render for StrokeHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().size_full().child(
            StrokeDots::new("stroke", "Stroke width")
                .selected(self.width)
                .on_select(move |width, _, cx| {
                    this.update(cx, |host, cx| {
                        host.width = width;
                        cx.notify();
                    })
                }),
        )
    }
}

#[test]
fn stroke_dots_pick_a_width_by_click_and_by_arrow_keys() {
    let (mut app, host) = open(80., |_, _| StrokeHost { width: 4. });
    let width = |app: &mut App| app.cx.update(|cx| host.read(cx).width);
    let segment = WIDTH / 5.;

    click(&mut app, segment * 4.5, 12.);
    assert_eq!(width(&mut app), 12., "clicking the last dot picks 12 px");
    key(&mut app, "right");
    assert_eq!(width(&mut app), 12., "Right stops at the widest");
    key(&mut app, "left");
    key(&mut app, "up");
    assert_eq!(width(&mut app), 6., "Left and Up step thinner");
    key(&mut app, "down");
    assert_eq!(width(&mut app), 8., "Down steps wider");
}
