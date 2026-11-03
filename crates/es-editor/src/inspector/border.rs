use std::ops::RangeInclusive;

use es_model::{BackgroundSettings, Swatch};
use es_ui::{
    components::{InspectorSlider, SwatchPick, SwatchStrip, ValueFormat},
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    App, AppContext, Context, Div, Entity, Hsla, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, prelude::FluentBuilder,
};

use crate::inspector::options::swatches::{colour, custom, swatch_title};
use crate::session::look::LookModel;
use crate::strings::fl;

pub const THICKNESS: RangeInclusive<f64> = 0.002..=0.08;
pub const OPACITY: RangeInclusive<f64> = 0.0..=1.0;
pub(super) const DIMMED: f32 = 0.48;

/// Built on first draw, since a slider needs a window; every value it reports reaches the look.
pub(super) fn bound_slider(
    look: &Entity<LookModel>,
    label: String,
    range: RangeInclusive<f64>,
    format: ValueFormat,
    apply: impl Fn(&mut BackgroundSettings, f64) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> (Entity<InspectorSlider>, Subscription) {
    let slider =
        cx.new(|cx| InspectorSlider::new(label, *range.start(), range, format, window, cx));
    let binding = LookModel::bind_slider(look, &slider, apply, cx);
    (slider, binding)
}

pub(super) fn sync_slider(
    slider: &Entity<InspectorSlider>,
    value: f64,
    disabled: Option<bool>,
    window: &mut Window,
    cx: &mut App,
) {
    slider.update(cx, |s, cx| {
        if let Some(disabled) = disabled {
            s.set_disabled(disabled, window, cx);
        }
        s.set_value(value, window, cx);
    });
}

/// The section's label with the picked colour's name across from it.
pub(super) fn colour_row(t: &Tokens, label: String, picked: Option<String>) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .typeset(t.type_scale.label)
        .child(div().text_color(t.text_secondary).child(label))
        .children(picked.map(|name| div().text_color(t.text_tertiary).child(name)))
}

/// Toggled on from the section header; until then the controls are shown dimmed and inert.
pub struct BorderBody {
    look: Entity<LookModel>,
    sliders: Option<[Entity<InspectorSlider>; 2]>,
    disabled: Option<bool>,
    _subscriptions: Vec<Subscription>,
}

impl BorderBody {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&look, |_, _, cx| cx.notify());
        Self {
            look,
            sliders: None,
            disabled: None,
            _subscriptions: vec![observe],
        }
    }

    pub fn thickness(&self) -> Option<&Entity<InspectorSlider>> {
        self.sliders.as_ref().map(|[thickness, _]| thickness)
    }

    pub fn opacity(&self) -> Option<&Entity<InspectorSlider>> {
        self.sliders.as_ref().map(|[_, opacity]| opacity)
    }

    pub fn pick_colour(&mut self, pick: SwatchPick, cx: &mut Context<Self>) {
        let swatch = match pick {
            SwatchPick::Preset(i) => Swatch::palette().into_iter().nth(i),
            SwatchPick::Custom(colour) => Some(custom(colour)),
        };
        if let Some(swatch) = swatch {
            self.look
                .update(cx, |l, cx| l.update(|s| s.border.color = swatch, cx));
        }
    }

    fn sliders(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> [Entity<InspectorSlider>; 2] {
        if let Some(sliders) = &self.sliders {
            return sliders.clone();
        }
        let (thickness, a) = bound_slider(
            &self.look,
            fl!("editor-border-thickness"),
            THICKNESS,
            ValueFormat::percent().digits(1),
            |s, v| s.border.thickness = v,
            window,
            cx,
        );
        let (opacity, b) = bound_slider(
            &self.look,
            fl!("editor-border-opacity"),
            OPACITY,
            ValueFormat::percent(),
            |s, v| s.border.opacity = v,
            window,
            cx,
        );
        self._subscriptions.extend([a, b]);
        let sliders = [thickness, opacity];
        self.sliders = Some(sliders.clone());
        sliders
    }
}

impl Render for BorderBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let border = self.look.read(cx).settings().border.clone();
        let [thickness, opacity] = self.sliders(window, cx);
        let off = !border.is_enabled;
        let toggled = (self.disabled != Some(off)).then_some(off);
        self.disabled = Some(off);
        sync_slider(&thickness, border.thickness, toggled, window, cx);
        sync_slider(&opacity, border.opacity, toggled, window, cx);

        let palette = Swatch::palette();
        let picked = palette.iter().position(|s| s.id == border.color.id);
        let presets: Vec<(Hsla, SharedString)> = palette
            .iter()
            .map(|s| (colour(s), swatch_title(s).into()))
            .collect();
        let selected = picked.map_or(
            SwatchPick::Custom(colour(&border.color)),
            SwatchPick::Preset,
        );
        let this = cx.entity().downgrade();
        let strip = SwatchStrip::new("border-color", fl!("editor-border-color"), presets)
            .selected(selected)
            .when(!off, |strip| {
                strip.on_select(move |pick, _, cx| {
                    let _ = this.update(cx, |this, cx| this.pick_colour(pick, cx));
                })
            });

        div()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .when(off, |d| d.opacity(DIMMED))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.metrics.group_label_spacing)
                    .child(colour_row(
                        &t,
                        fl!("editor-border-color"),
                        Some(swatch_title(&border.color)),
                    ))
                    .child(strip),
            )
            .child(thickness)
            .child(opacity)
    }
}
