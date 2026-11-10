use std::ops::RangeInclusive;

use es_geom::Rgba;
use es_model::{Swatch, WatermarkSettings};
use es_ui::components::Tip;
use es_ui::{
    a11y::{Accessible, Role},
    components::{InspectorSlider, SwatchPick, SwatchStrip, ValueFormat},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    AppContext, Context, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Subscription, Window, div,
    prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon, Sizable,
    input::{Input, InputEvent, InputState},
};

use super::border::{DIMMED, bound_slider, colour_row, sync_slider};
use crate::session::look::LookModel;
use crate::strings::fl;

pub const DENSITY: RangeInclusive<f64> = 2.0..=10.0;
pub const SIZE: RangeInclusive<f64> = 8.0..=160.0;
pub const ANGLE: RangeInclusive<f64> = -90.0..=90.0;
pub const OPACITY: RangeInclusive<f64> = 0.0..=0.75;

const ROW_HEIGHT: f32 = 26.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatermarkSlider {
    Density,
    Size,
    Angle,
    Opacity,
}

fn hsla(c: Rgba) -> Hsla {
    gpui::Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
    .into()
}

fn rgba(c: Hsla) -> Rgba {
    let c = gpui::Rgba::from(c);
    Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a: 1.0,
    }
}

fn presets() -> [(Rgba, String); 2] {
    [
        (
            WatermarkSettings::default().color,
            fl!("editor-swatch-mercury"),
        ),
        (Swatch::white().to_rgba(), fl!("editor-swatch-white")),
    ]
}

fn preset_of(colour: Rgba) -> Option<usize> {
    let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
    presets().iter().position(|(p, _)| {
        near(p.r, colour.r) && near(p.g, colour.g) && near(p.b, colour.b) && near(p.a, colour.a)
    })
}

/// There is no switch: the watermark is on exactly while it has text.
pub struct WatermarkBody {
    look: Entity<LookModel>,
    sliders: Option<[Entity<InspectorSlider>; 4]>,
    field: Option<Entity<InputState>>,
    editing: bool,
    disabled: Option<bool>,
    _subscriptions: Vec<Subscription>,
}

impl WatermarkBody {
    pub fn new(look: Entity<LookModel>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&look, |_, _, cx| cx.notify());
        Self {
            look,
            sliders: None,
            field: None,
            editing: false,
            disabled: None,
            _subscriptions: vec![observe],
        }
    }

    pub fn slider(&self, which: WatermarkSlider) -> Option<&Entity<InspectorSlider>> {
        let index = match which {
            WatermarkSlider::Density => 0,
            WatermarkSlider::Size => 1,
            WatermarkSlider::Angle => 2,
            WatermarkSlider::Opacity => 3,
        };
        self.sliders.as_ref().map(|s| &s[index])
    }

    pub fn field(&self) -> Option<&Entity<InputState>> {
        self.field.as_ref()
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn begin_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.look.read(cx).settings().watermark.text.clone();
        let field = match &self.field {
            Some(field) => field.clone(),
            None => {
                let placeholder = fl!("editor-watermark-placeholder");
                let field = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
                let events =
                    cx.subscribe(&field, |this, field, event: &InputEvent, cx| match event {
                        InputEvent::Change => {
                            let text = field.read(cx).value().to_string();
                            this.set_text(&text, cx);
                        }
                        InputEvent::PressEnter { .. } | InputEvent::Blur => this.end_editing(cx),
                        _ => {}
                    });
                self._subscriptions.push(events);
                self.field = Some(field.clone());
                field
            }
        };
        field.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        self.editing = true;
        cx.notify();
    }

    pub fn end_editing(&mut self, cx: &mut Context<Self>) {
        self.editing = false;
        cx.notify();
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.to_owned();
        self.look.update(cx, |l, cx| {
            l.update(
                |s| {
                    s.watermark.is_enabled = !text.trim().is_empty();
                    s.watermark.text = text;
                },
                cx,
            )
        });
    }

    pub fn pick_colour(&mut self, pick: SwatchPick, cx: &mut Context<Self>) {
        let colour = match pick {
            SwatchPick::Preset(i) => presets().get(i).map(|(c, _)| *c),
            SwatchPick::Custom(c) => Some(rgba(c)),
        };
        if let Some(colour) = colour {
            self.look
                .update(cx, |l, cx| l.update(|s| s.watermark.color = colour, cx));
        }
    }

    fn sliders(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> [Entity<InspectorSlider>; 4] {
        if let Some(sliders) = &self.sliders {
            return sliders.clone();
        }
        let look = self.look.clone();
        let (density, a) = bound_slider(
            &look,
            fl!("editor-watermark-density"),
            DENSITY,
            ValueFormat::integer(),
            |s, v| s.watermark.density = v.round(),
            window,
            cx,
        );
        let (size, b) = bound_slider(
            &look,
            fl!("editor-watermark-size"),
            SIZE,
            ValueFormat::pixels(),
            |s, v| s.watermark.font_size = v,
            window,
            cx,
        );
        let (angle, c) = bound_slider(
            &look,
            fl!("editor-watermark-angle"),
            ANGLE,
            ValueFormat::degrees().signed(),
            |s, v| s.watermark.rotation_degrees = v,
            window,
            cx,
        );
        let (opacity, d) = bound_slider(
            &look,
            fl!("editor-watermark-opacity"),
            OPACITY,
            ValueFormat::percent(),
            |s, v| s.watermark.opacity = v,
            window,
            cx,
        );
        self._subscriptions.extend([a, b, c, d]);
        let sliders = [density, size, angle, opacity];
        self.sliders = Some(sliders.clone());
        sliders
    }

    fn text_row(&self, text: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let empty = text.trim().is_empty();
        let (lead, trail, trail_size, label, help) = if empty {
            let add = fl!("editor-watermark-add");
            (IconName::Plus, IconName::ChevronRight, 8., add.clone(), add)
        } else {
            (
                IconName::ALargeSmall,
                IconName::Pencil,
                10.,
                text.to_owned(),
                fl!("editor-watermark-edit-help"),
            )
        };
        let this = cx.entity().downgrade();
        let tip = SharedString::from(help.clone());
        div()
            .id("watermark-text")
            .h(px(ROW_HEIGHT))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(t.metrics.field_radius)
            .bg(t.value_box)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .hover(|d| d.bg(t.hover_fill))
            .accessible(Role::Button, help)
            .tip(tip)
            .on_click(move |_, window, cx| {
                let _ = this.update(cx, |this, cx| this.begin_editing(window, cx));
            })
            .child(Icon::new(lead).size(px(10.)).text_color(t.text_secondary))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .typeset(t.type_scale.segmented)
                    .text_color(if empty { t.text_secondary } else { t.text })
                    .child(label),
            )
            .child(
                Icon::new(trail)
                    .size(px(trail_size))
                    .text_color(t.text_tertiary),
            )
    }
}

impl Render for WatermarkBody {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let mark = self.look.read(cx).settings().watermark.clone();
        let sliders = self.sliders(window, cx);
        let off = mark.text.trim().is_empty();
        let toggled = (self.disabled != Some(off)).then_some(off);
        self.disabled = Some(off);
        let values = [
            mark.density,
            mark.font_size,
            mark.rotation_degrees,
            mark.opacity,
        ];
        for (slider, value) in sliders.iter().zip(values) {
            sync_slider(slider, value, toggled, window, cx);
        }

        let picked = preset_of(mark.color);
        let selected = picked.map_or(SwatchPick::Custom(hsla(mark.color)), SwatchPick::Preset);
        let name = match picked {
            Some(i) => presets()[i].1.clone(),
            None => fl!("editor-swatch-custom"),
        };
        let this = cx.entity().downgrade();
        let strip = SwatchStrip::new(
            "watermark-color",
            fl!("editor-watermark-color"),
            presets().map(|(c, n)| (hsla(c), SharedString::from(n))),
        )
        .selected(selected)
        .when(!off, |strip| {
            strip.on_select(move |pick, _, cx| {
                let _ = this.update(cx, |this, cx| this.pick_colour(pick, cx));
            })
        });

        let field = self
            .field
            .clone()
            .filter(|_| self.editing)
            .map(|field| {
                Input::new(&field)
                    .small()
                    .h(px(ROW_HEIGHT))
                    .into_any_element()
            })
            .unwrap_or_else(|| self.text_row(&mark.text, cx).into_any_element());

        div()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .child(field)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(t.metrics.row_spacing)
                    .when(off, |d| d.opacity(DIMMED))
                    .children(sliders)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(t.metrics.group_label_spacing)
                            .child(colour_row(&t, fl!("editor-watermark-color"), Some(name)))
                            .child(strip),
                    ),
            )
    }
}
