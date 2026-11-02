use es_model::{BackgroundColor, Swatch};
use gpui::{
    AnyElement, App, Hsla, IntoElement, ParentElement, SharedString, Styled, Window, div,
    linear_color_stop, linear_gradient, px,
};

use crate::{
    components::{
        CheckerBackground, DotGridBackground, Interaction, StrokeDots, SwatchPick, SwatchStrip,
        Tile, TileGrid, gallery::Section,
    },
    theme::tokens,
    tokens::{Tokens, Typeset},
};

pub const SECTION: Section = Section {
    name: "picks",
    height: 640.,
    render: picks,
};

const INSPECTOR_CONTENT: f32 = 276.;

fn caption(t: &Tokens, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .whitespace_nowrap()
        .child(text.into())
}

fn sample(t: &Tokens, name: &'static str, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(body)
        .child(caption(t, name))
        .into_any_element()
}

fn colour(r: f64, g: f64, b: f64) -> Hsla {
    gpui::Rgba {
        r: r as f32,
        g: g as f32,
        b: b as f32,
        a: 1.,
    }
    .into()
}

fn presets() -> Vec<(Hsla, SharedString)> {
    Swatch::palette()
        .into_iter()
        .map(|s| (colour(s.red, s.green, s.blue), s.title.into()))
        .collect()
}

fn colour_tiles(prefix: &str) -> Vec<Tile> {
    BackgroundColor::plain_presets()
        .into_iter()
        .map(|c| {
            Tile::new(SharedString::from(format!("{prefix}-{}", c.id)), c.title)
                .fill(colour(c.red, c.green, c.blue))
        })
        .collect()
}

fn gradient_tiles(prefix: &str) -> Vec<Tile> {
    let plain = BackgroundColor::plain_presets();
    let (light, rest) = plain.split_at(plain.len() / 2);
    rest.iter()
        .zip(light.iter().rev())
        .enumerate()
        .map(|(i, (from, to))| {
            Tile::new(
                SharedString::from(format!("{prefix}-{i}")),
                from.title.clone(),
            )
            .fill(linear_gradient(
                180.,
                linear_color_stop(colour(from.red, from.green, from.blue), 0.),
                linear_color_stop(colour(to.red, to.green, to.blue), 1.),
            ))
        })
        .collect()
}

fn panel(t: &Tokens, children: Vec<AnyElement>) -> impl IntoElement {
    div()
        .w(px(INSPECTOR_CONTENT) + t.metrics.inspector_padding * 2.)
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(t.metrics.inspector_padding)
        .rounded(t.metrics.card_radius)
        .bg(t.inspector)
        .children(children)
}

fn picks(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let strip = |id: &'static str| SwatchStrip::new(id, "Color", presets()).on_select(|_, _, _| {});
    let left = panel(
        &t,
        vec![
            sample(
                &t,
                "colour tiles, 8 columns, spacing 6",
                TileGrid::new("tiles", "Fill color", colour_tiles("colour")).selected(Some(0)),
            ),
            sample(
                &t,
                "gradient tiles, keyboard focus",
                TileGrid::new("gradients", "Gradient", gradient_tiles("gradient"))
                    .selected(Some(1))
                    .preview(Interaction::Focused),
            ),
            sample(
                &t,
                "swatches, red selected",
                strip("swatches").selected(SwatchPick::Preset(1)),
            ),
            sample(
                &t,
                "custom colour selected, keyboard focus",
                strip("swatches-custom")
                    .selected(SwatchPick::Custom(colour(0.36, 0.62, 0.55)))
                    .preview(10, Interaction::Focused),
            ),
            sample(
                &t,
                "stroke width 4",
                StrokeDots::new("stroke", "Stroke width")
                    .selected(4.)
                    .on_select(|_, _, _| {}),
            ),
            sample(
                &t,
                "keyboard focus",
                StrokeDots::new("stroke-focus", "Stroke width")
                    .selected(12.)
                    .preview(0, Interaction::Focused)
                    .on_select(|_, _, _| {}),
            ),
        ],
    );

    let states = [
        ("rest", Interaction::Rest, false),
        ("hover", Interaction::Hover, false),
        ("pressed", Interaction::Pressed, false),
        ("selected", Interaction::Rest, true),
        ("focused", Interaction::Focused, true),
    ];
    let wallpaper = linear_gradient(
        135.,
        linear_color_stop(t.accent, 0.),
        linear_color_stop(t.success, 1.),
    );
    let tile_states = div()
        .flex()
        .gap(px(12.))
        .children(states.map(|(name, state, selected)| {
            div().w(px(40.)).child(sample(
                &t,
                name,
                Tile::new(name, "Wallpaper")
                    .aspect(1.35)
                    .fill(wallpaper)
                    .selected(selected)
                    .preview(state)
                    .on_click(|_, _, _| {}),
            ))
        }));
    let swatch_states = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(sample(
            &t,
            "hover on yellow",
            strip("swatch-hover")
                .selected(SwatchPick::Preset(0))
                .preview(3, Interaction::Hover),
        ))
        .child(sample(
            &t,
            "pressed on blue, none selected",
            strip("swatch-pressed").preview(6, Interaction::Pressed),
        ));
    let backdrop = |body: AnyElement| {
        div()
            .w(px(64.))
            .h(px(40.))
            .rounded(t.metrics.tile_radius)
            .child(body)
    };
    let backgrounds = div()
        .flex()
        .gap(px(12.))
        .child(sample(
            &t,
            "dot grid, 16 px",
            DotGridBackground::new()
                .w(px(120.))
                .h(px(96.))
                .rounded(t.metrics.card_radius)
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop(div().size_full().bg(wallpaper).into_any_element())),
        ))
        .child(sample(
            &t,
            "checker, 10 px",
            CheckerBackground::new()
                .w(px(120.))
                .h(px(96.))
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop(
                    div()
                        .size_full()
                        .rounded(t.metrics.tile_radius)
                        .bg(t.accent.opacity(0.5))
                        .into_any_element(),
                )),
        ));
    let right = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(sample(&t, "tile states, aspect 1.35", tile_states))
        .child(swatch_states)
        .child(backgrounds)
        .child(sample(
            &t,
            "stroke hover on 8",
            div().w_full().child(
                StrokeDots::new("stroke-hover", "Stroke width")
                    .preview(3, Interaction::Hover)
                    .on_select(|_, _, _| {}),
            ),
        ))
        .child(sample(
            &t,
            "stroke disabled",
            div()
                .w_full()
                .child(StrokeDots::new("stroke-disabled", "Stroke width").disabled(true)),
        ));
    div()
        .flex()
        .gap(px(20.))
        .child(left)
        .child(right)
        .into_any_element()
}
