use es_anno::{Editor, StylePatch};
use es_model::{Swatch, Tool};
use es_ui::components::{SwatchPick, SwatchStrip};
use gpui::{Hsla, IntoElement, Rgba, SharedString, Styled, WeakEntity};

use super::{Control, ToolOptions};
use crate::strings::fl;
use crate::toolbar::{Drawn, track};

/// Which options the inspected tool takes: the one selected shape's, else the first selected, else the armed tool's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Facets {
    pub inspected: Tool,
    pub colour: bool,
    pub stroke: bool,
    pub text: bool,
    pub strength: bool,
    pub heads: bool,
    pub dash: bool,
    pub mode: bool,
    pub selection: bool,
}

impl Facets {
    pub fn of(editor: &Editor) -> Option<Self> {
        let selected = editor.selected_ids();
        let shapes = editor.doc().shapes();
        let first = selected
            .iter()
            .find_map(|id| shapes.iter().find(|s| s.id == *id));
        let inspected = first.map_or(editor.tool(), |s| s.tool());
        let lone_text = selected.len() == 1 && inspected == Tool::Text;
        let f = Self {
            inspected,
            colour: inspected.supports_color(),
            stroke: inspected.supports_stroke_width(),
            text: lone_text || (selected.is_empty() && editor.tool() == Tool::Text),
            strength: inspected.is_redaction(),
            heads: inspected == Tool::Arrow,
            dash: inspected.uses_endpoints(),
            mode: inspected == Tool::Highlighter && editor.tool() == Tool::Highlighter,
            selection: !selected.is_empty(),
        };
        (f.colour || f.stroke || f.text || f.strength || f.selection).then_some(f)
    }
}

pub(crate) fn swatch_title(swatch: &Swatch) -> String {
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

pub(crate) fn colour(swatch: &Swatch) -> Hsla {
    Rgba {
        r: swatch.red as f32,
        g: swatch.green as f32,
        b: swatch.blue as f32,
        a: swatch.alpha as f32,
    }
    .into()
}

pub(crate) fn custom(colour: Hsla) -> Swatch {
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
    page: WeakEntity<ToolOptions>,
    drawn: &Drawn<Control>,
) -> impl IntoElement {
    swatch_strip(
        id,
        fl!("editor-color"),
        control,
        Some(current),
        page,
        drawn,
        StylePatch::Swatch,
    )
}

pub(super) fn swatch_strip(
    id: &'static str,
    label: String,
    control: Control,
    current: Option<&Swatch>,
    page: WeakEntity<ToolOptions>,
    drawn: &Drawn<Control>,
    patch: impl Fn(Swatch) -> StylePatch + 'static,
) -> impl IntoElement {
    let palette = Swatch::palette();
    let pick = current.map(
        |current| match palette.iter().position(|s| s.id == current.id) {
            Some(i) => SwatchPick::Preset(i),
            None => SwatchPick::Custom(colour(current)),
        },
    );
    let presets: Vec<(Hsla, SharedString)> = palette
        .iter()
        .map(|s| (colour(s), swatch_title(s).into()))
        .collect();
    let mut strip = SwatchStrip::new(id, label, presets);
    if let Some(pick) = pick {
        strip = strip.selected(pick);
    }
    let strip = strip.on_select(move |pick, _, cx| {
        let swatch = match pick {
            SwatchPick::Preset(i) => Swatch::palette().into_iter().nth(i),
            SwatchPick::Custom(colour) => Some(custom(colour)),
        };
        let Some(swatch) = swatch else { return };
        let _ = page.update(cx, |o, cx| o.apply_style(patch(swatch), None, cx));
    });
    track(drawn, control, strip).w_full()
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
