use es_anno::{Editor, StylePatch};
use es_model::{Swatch, Tool};
use es_ui::{
    a11y::{Accessible, Role},
    components::{EditorButton, Popover, StrokeDots, SwatchPick, SwatchStrip},
    icons::IconName,
    theme::tokens,
};
use gpui::{
    AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Rgba,
    SharedString, Styled, WeakEntity, Window, div, prelude::FluentBuilder, px,
};

use super::{Control, Drawn, ToolRow, text_style, tools::shortcut_label, track};
use crate::strings::fl;

const CAPSULE_WIDTH: Pixels = px(34.);
const CAPSULE_HEIGHT: Pixels = px(16.);
const STROKE_WIDTH: Pixels = px(140.);
const STRENGTH_WIDTH: Pixels = px(180.);
pub(super) const POPOVER_WIDTH: Pixels = px(340.);
const GAP: Pixels = px(8.);

/// Which style controls the inspected tool takes: the one selected shape's, else the first selected, else the armed tool's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Facets {
    colour: bool,
    stroke: bool,
    text: bool,
    strength: bool,
    selection: bool,
}

fn facets(editor: &Editor) -> Option<Facets> {
    let selected = editor.selected_ids();
    let shapes = editor.doc().shapes();
    let first = selected
        .iter()
        .find_map(|id| shapes.iter().find(|s| s.id == *id));
    let inspected = first.map_or(editor.tool(), |s| s.tool());
    let lone_text = selected.len() == 1 && inspected == Tool::Text;
    let f = Facets {
        colour: inspected.supports_color(),
        stroke: inspected.supports_stroke_width(),
        text: lone_text || (selected.is_empty() && editor.tool() == Tool::Text),
        strength: inspected.is_redaction(),
        selection: !selected.is_empty(),
    };
    (f.colour || f.stroke || f.text || f.strength || f.selection).then_some(f)
}

fn swatch_title(swatch: &Swatch) -> String {
    match swatch.id.as_str() {
        "black" => fl!("editor-swatch-black"),
        "red" => fl!("editor-swatch-red"),
        "orange" => fl!("editor-swatch-orange"),
        "yellow" => fl!("editor-swatch-yellow"),
        "green" => fl!("editor-swatch-green"),
        "turquoise" => fl!("editor-swatch-turquoise"),
        "blue" => fl!("editor-swatch-blue"),
        "purple" => fl!("editor-swatch-purple"),
        "pink" => fl!("editor-swatch-pink"),
        "white" => fl!("editor-swatch-white"),
        _ => fl!("editor-swatch-custom"),
    }
}

pub(super) fn colour(swatch: &Swatch) -> Hsla {
    Rgba {
        r: swatch.red as f32,
        g: swatch.green as f32,
        b: swatch.blue as f32,
        a: swatch.alpha as f32,
    }
    .into()
}

fn custom(colour: Hsla) -> Swatch {
    let rgba = Rgba::from(colour);
    Swatch {
        id: "custom".into(),
        title: fl!("editor-swatch-custom"),
        red: f64::from(rgba.r),
        green: f64::from(rgba.g),
        blue: f64::from(rgba.b),
        alpha: 1.0,
    }
}

/// The ten presets and the custom well; a pick restyles the selection as one undo step.
pub(super) fn swatches(
    id: &'static str,
    control: Control,
    current: &Swatch,
    row: WeakEntity<ToolRow>,
    drawn: &Drawn,
) -> impl IntoElement {
    let palette = Swatch::palette();
    let pick = match palette.iter().position(|s| s.id == current.id) {
        Some(i) => SwatchPick::Preset(i),
        None => SwatchPick::Custom(colour(current)),
    };
    let presets: Vec<(Hsla, SharedString)> = palette
        .iter()
        .map(|s| (colour(s), swatch_title(s).into()))
        .collect();
    let strip = SwatchStrip::new(id, fl!("editor-color"), presets)
        .selected(pick)
        .on_select(move |pick, _, cx| {
            let swatch = match pick {
                SwatchPick::Preset(i) => Swatch::palette().into_iter().nth(i),
                SwatchPick::Custom(colour) => Some(custom(colour)),
            };
            let Some(swatch) = swatch else { return };
            let _ = row.update(cx, |r, cx| {
                r.apply_style(StylePatch::Swatch(swatch), None, cx)
            });
        });
    track(drawn, control, strip).w_full()
}

fn colour_group(row: &ToolRow, current: &Swatch, cx: &mut Context<ToolRow>) -> impl IntoElement {
    let t = tokens(cx).clone();
    let weak = cx.entity().downgrade();
    let drawn = row.drawn.clone();
    let doc = row.doc.clone();
    let capsule = div()
        .id("color-capsule")
        .w(CAPSULE_WIDTH)
        .h(CAPSULE_HEIGHT)
        .rounded_full()
        .bg(colour(current))
        .border(t.metrics.hairline)
        .border_color(t.border)
        .accessible(Role::Button, fl!("editor-color-picker-a11y"));
    let trigger = div()
        .flex()
        .items_center()
        .gap(px(2.))
        .child(track(
            &row.drawn,
            Control::Palette,
            EditorButton::new("palette")
                .icon(IconName::Palette)
                .accessible_label(fl!("editor-color"))
                .tooltip(fl!("editor-color"))
                .padding_x(t.metrics.tool_button_padding_x),
        ))
        .child(track(&row.drawn, Control::ColorCapsule, capsule).px(px(4.)));
    Popover::new("palette-popover", fl!("editor-color"))
        .width(POPOVER_WIDTH)
        .trigger(trigger)
        .content(move |_, cx| {
            let current = doc.read(cx).editor().current_style().swatch.clone();
            div().p(px(8.)).child(swatches(
                "swatches",
                Control::Swatches,
                &current,
                weak.clone(),
                &drawn,
            ))
        })
}

fn stroke_picker(row: &ToolRow, width: f64, cx: &mut Context<ToolRow>) -> impl IntoElement {
    let weak = cx.entity().downgrade();
    let dots = StrokeDots::new("stroke", fl!("editor-stroke-width"))
        .selected(width as f32)
        .on_select(move |width, _, cx| {
            let patch = StylePatch::StrokeWidth(f64::from(width));
            let _ = weak.update(cx, |r, cx| r.apply_style(patch, None, cx));
        });
    track(&row.drawn, Control::Stroke, dots).w(STROKE_WIDTH)
}

fn delete(row: &ToolRow, enabled: bool, cx: &mut Context<ToolRow>) -> impl IntoElement {
    let weak = cx.entity().downgrade();
    let shortcut = shortcut_label("image-delete").unwrap_or_default();
    let button = EditorButton::new("delete")
        .icon(IconName::Trash)
        .accessible_label(fl!("editor-delete-a11y"))
        .tooltip(fl!("editor-delete-help", shortcut = shortcut))
        .padding_x(tokens(cx).metrics.tool_button_padding_x)
        .disabled(!enabled)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |r, cx| r.delete_selection(cx));
        });
    track(&row.drawn, Control::Delete, button)
}

pub(super) fn render(
    row: &mut ToolRow,
    window: &mut Window,
    cx: &mut Context<ToolRow>,
) -> Option<AnyElement> {
    let (facets, style) = {
        let editor = row.doc.read(cx).editor();
        (facets(editor)?, editor.current_style().clone())
    };
    let strength = facets.strength.then(|| row.strength(window, cx));
    let bar = div()
        .id("style-bar")
        .accessible(Role::Group, fl!("editor-style-bar-a11y"))
        .flex()
        .flex_none()
        .items_center()
        .gap(GAP)
        .when(facets.colour, |d| {
            d.child(colour_group(row, &style.swatch, cx))
        })
        .when(facets.stroke, |d| {
            d.child(stroke_picker(row, style.stroke_width, cx))
        })
        .when(facets.text, |d| d.child(text_style::popover(row, cx)))
        .when_some(strength, |d, strength| {
            d.child(track(&row.drawn, Control::Strength, strength).w(STRENGTH_WIDTH))
        })
        .child(delete(row, facets.selection, cx));
    Some(bar.into_any_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_custom_colour_survives_the_round_trip_through_the_swatch() {
        let picked = colour(&Swatch::orange());
        let back = custom(picked);
        assert!((back.red - 1.0).abs() < 1e-3 && (back.blue - 0.08).abs() < 1e-3);
        assert_eq!(swatch_title(&back), fl!("editor-swatch-custom"));
    }
}
