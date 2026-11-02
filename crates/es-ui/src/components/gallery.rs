use gpui::{
    AnyElement, App, AppContext, Context, Entity, Hsla, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, WindowOptions, div, px,
};
use gpui_component::{Icon, Root};

use crate::{
    components::gallery_controls as controls,
    fonts,
    icons::IconName,
    theme::{self, UI_FONT, tokens},
    tokens::{Appearance, TextStyle, Tokens, Typeset},
};

pub const WIDTH: f32 = 1240.;
const PADDING: f32 = 24.;
const HEADER: f32 = 44.;

/// Each component adds one of these to [`sections`], and the gallery test pictures it in both appearances.
#[derive(Clone, Copy)]
pub struct Section {
    pub name: &'static str,
    pub height: f32,
    pub render: fn(&mut Window, &mut App) -> AnyElement,
}

impl Section {
    pub fn window_height(&self) -> f32 {
        self.height + HEADER + 2. * PADDING
    }
}

pub fn sections() -> Vec<Section> {
    vec![
        Section {
            name: "colours",
            height: 450.,
            render: colours,
        },
        Section {
            name: "surfaces",
            height: 190.,
            render: surfaces,
        },
        Section {
            name: "type",
            height: 230.,
            render: type_scale,
        },
        Section {
            name: "metrics",
            height: 260.,
            render: metrics,
        },
        Section {
            name: "icons",
            height: 820.,
            render: icons,
        },
        Section {
            name: "buttons",
            height: 440.,
            render: controls::buttons,
        },
        Section {
            name: "tool-buttons",
            height: 200.,
            render: controls::tool_buttons,
        },
        Section {
            name: "capsules",
            height: 280.,
            render: controls::capsules,
        },
        Section {
            name: "segmented",
            height: 300.,
            render: controls::segmented,
        },
        Section {
            name: "slider",
            height: 400.,
            render: controls::sliders,
        },
        Section {
            name: "shortcut-field",
            height: 150.,
            render: controls::shortcut_fields,
        },
        crate::components::gallery_picks::SECTION,
        crate::components::gallery_panels::SECTION,
    ]
}

pub fn init(cx: &mut App) {
    fonts::register(cx);
    theme::install(cx);
    crate::a11y::bind_focus_keys(cx);
}

pub fn root(sections: Vec<Section>, window: &mut Window, cx: &mut App) -> Entity<Root> {
    let view = cx.new(|_| Gallery { sections });
    cx.new(|cx| Root::new(view, window, cx))
}

pub fn open(cx: &mut App) {
    init(cx);
    if let Err(error) = cx.open_window(WindowOptions::default(), |window, cx| {
        root(sections(), window, cx)
    }) {
        tracing::error!(%error, "cannot open the gallery window");
    }
    cx.activate(true);
}

pub struct Gallery {
    sections: Vec<Section>,
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().id("gallery").size_full().overflow_y_scroll().child(
            div()
                .flex()
                .w_full()
                .min_h_full()
                .children([Appearance::Dark, Appearance::Light].map(|appearance| {
                    let sections = self.sections.clone();
                    theme::scoped(
                        appearance,
                        Column {
                            appearance,
                            sections,
                        },
                    )
                })),
        )
    }
}

#[derive(IntoElement)]
struct Column {
    appearance: Appearance,
    sections: Vec<Section>,
}

impl RenderOnce for Column {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let title = match self.appearance {
            Appearance::Dark => "Dark",
            Appearance::Light => "Light",
        };
        let sections: Vec<AnyElement> = self
            .sections
            .iter()
            .map(|section| {
                div()
                    .flex()
                    .flex_col()
                    .gap(t.metrics.header_spacing)
                    .min_h(px(section.height))
                    .child(
                        div()
                            .typeset(t.type_scale.section_header)
                            .text_color(t.text_secondary)
                            .child(section.name),
                    )
                    .child((section.render)(window, cx))
                    .into_any_element()
            })
            .collect();
        div()
            .id(title)
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(28.))
            .p(px(PADDING))
            .bg(t.window)
            .text_color(t.text)
            .font_family(UI_FONT)
            .child(
                div()
                    .h(px(HEADER - 28.))
                    .typeset(t.type_scale.title)
                    .child(title),
            )
            .children(sections)
    }
}

fn hex(c: Hsla) -> SharedString {
    let c = c.to_rgb();
    let byte = |v: f32| (v * 255.).round() as u8;
    let rgb = format!("#{:02X}{:02X}{:02X}", byte(c.r), byte(c.g), byte(c.b));
    if c.a < 1. {
        format!("{rgb} {:.0}%", c.a * 100.).into()
    } else {
        rgb.into()
    }
}

fn caption(t: &Tokens, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(text.into())
}

fn colours(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    div()
        .flex()
        .flex_wrap()
        .gap_y(px(12.))
        .children(t.colours.named().into_iter().map(|(name, colour)| {
            div()
                .w(px(184.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(32.))
                        .flex_none()
                        .rounded(t.metrics.tile_radius)
                        .border(t.metrics.hairline)
                        .border_color(t.border)
                        .bg(colour),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().typeset(t.type_scale.label).child(name))
                        .child(
                            div()
                                .typeset(t.type_scale.value)
                                .text_color(t.text_secondary)
                                .child(hex(colour)),
                        ),
                )
        }))
        .into_any_element()
}

fn surfaces(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let panel = |name: &'static str, bg: Hsla| {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(t.metrics.row_spacing)
            .p(t.metrics.inspector_padding)
            .rounded(t.metrics.card_radius)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .bg(bg)
            .child(div().typeset(t.type_scale.section_header).child(name))
            .child(div().typeset(t.type_scale.label).child("Primary label"))
            .child(
                div()
                    .typeset(t.type_scale.label)
                    .text_color(t.text_secondary)
                    .child("Secondary label"),
            )
            .child(
                div()
                    .typeset(t.type_scale.label)
                    .text_color(t.text_tertiary)
                    .child("Tertiary label"),
            )
            .child(
                div()
                    .typeset(t.type_scale.value)
                    .text_color(t.accent)
                    .child("Accent 56%"),
            )
    };
    div()
        .flex()
        .gap(px(12.))
        .child(panel("window", t.window))
        .child(panel("inspector", t.inspector))
        .child(panel("workspace", t.workspace))
        .child(panel("dropdown", t.dropdown))
        .into_any_element()
}

fn type_scale(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let row = |name: &'static str, style: TextStyle| {
        let weight = match style.weight.0 as u32 {
            400 => "regular",
            500 => "medium",
            600 => "semibold",
            _ => "bold",
        };
        let numbers = if style.tabular { ", tabular" } else { "" };
        div()
            .flex()
            .items_center()
            .h(px(26.))
            .child(div().w(px(140.)).child(caption(&t, name)))
            .child(
                div()
                    .flex_1()
                    .typeset(style)
                    .child("Padding 27% · Sphinx of black quartz 0123456789"),
            )
            .child(caption(
                &t,
                format!("{} {weight}{numbers}", f32::from(style.size)),
            ))
    };
    let s = t.type_scale;
    div()
        .flex()
        .flex_col()
        .child(row("title", s.title))
        .child(row("tool_button", s.tool_button))
        .child(row("footer", s.footer))
        .child(row("section_header", s.section_header))
        .child(row("label", s.label))
        .child(row("value", s.value))
        .child(row("segmented", s.segmented))
        .into_any_element()
}

fn sample(t: &Tokens, name: String, body: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .child(div().h(px(40.)).flex().items_end().child(body))
        .child(caption(t, name))
}

fn metrics(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let m = t.metrics;
    let radii = [
        ("field", m.field_radius),
        ("tile", m.tile_radius),
        ("button", m.button_radius),
        ("slider", m.slider_radius),
        ("card", m.card_radius),
        ("popover", m.popover_radius),
    ];
    let heights = [
        ("control", m.control_height),
        ("slider", m.slider_height),
        ("button", m.button_height),
        ("header", m.header_height),
    ];
    let spacing = [
        ("inset", m.control_inset),
        ("group label", m.group_label_spacing),
        ("row", m.row_spacing),
        ("header", m.header_spacing),
        ("padding", m.inspector_padding),
        ("label column", m.label_column),
        ("value", m.slider_value_width),
    ];
    let label = |name: &str, v: Pixels| format!("{name} {}", f32::from(v));
    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(div().flex().gap(px(20.)).children(radii.map(|(name, r)| {
            sample(
                &t,
                label(name, r),
                div()
                    .w(px(56.))
                    .h(px(36.))
                    .rounded(r)
                    .border(m.hairline)
                    .border_color(t.border)
                    .bg(t.track),
            )
        })))
        .child(div().flex().gap(px(20.)).children(heights.map(|(name, h)| {
            sample(
                &t,
                label(name, h),
                div().w(px(72.)).h(h).rounded(m.button_radius).bg(t.fill),
            )
        })))
        .child(div().flex().gap(px(20.)).children(
            spacing.map(|(name, w)| sample(&t, label(name, w), div().w(w).h(px(12.)).bg(t.accent))),
        ))
        .into_any_element()
}

fn icons(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    div()
        .flex()
        .flex_wrap()
        .children(IconName::ALL.iter().map(|&icon| {
            div()
                .w(px(143.))
                .h(px(32.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(Icon::new(icon).size(px(16.)).flex_none().text_color(t.text))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .typeset(t.type_scale.label)
                        .text_color(t.text_secondary)
                        .child(icon.name()),
                )
        }))
        .into_any_element()
}
