use std::rc::Rc;

use es_anno::{CurrentStyle, StylePatch};
use es_model::TextAlign;
use es_ui::{
    a11y::{Accessible, Role},
    components::{Segment, Segmented},
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    App, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window, div, px,
};
use gpui_component::select::Select;

use super::{
    Control, ToolOptions, font_picker::family_title, section, swatches::swatches, text_box,
};
use crate::strings::fl;
use crate::toolbar::track;

const ALIGNS: [TextAlign; 3] = [TextAlign::Start, TextAlign::Middle, TextAlign::End];
const STEP_WIDTH: Pixels = px(24.);
const SIZE_FIELD_WIDTH: Pixels = px(30.);
const EMPHASIS_WIDTH: Pixels = px(90.);
const FONT_MENU_HEIGHT: Pixels = px(320.);

type Apply = Rc<dyn Fn(StylePatch, &mut App)>;

fn applier(page: WeakEntity<ToolOptions>) -> Apply {
    Rc::new(move |patch, cx| {
        let _ = page.update(cx, |o, cx| o.apply_style(patch, None, cx));
    })
}

fn step(
    id: &'static str,
    label: String,
    glyph: IconName,
    t: &Tokens,
    on: impl Fn(&mut App) + 'static,
) -> impl IntoElement {
    let hover = t.hover_fill;
    div()
        .id(id)
        .w(STEP_WIDTH)
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .accessible(Role::Button, label)
        .hover(move |s| s.bg(hover))
        .on_click(move |_, _, cx| on(cx))
        .child(gpui_component::Icon::new(glyph).size(px(11.)))
}

fn stepper(size: f64, apply: Apply, t: &Tokens) -> impl IntoElement {
    let m = t.metrics;
    let rule = || div().w(m.hairline).h(px(13.)).bg(t.separator);
    let (down, up) = (apply.clone(), apply);
    div()
        .flex()
        .items_center()
        .h(m.control_height)
        .rounded(m.field_radius)
        .bg(t.track)
        .border(m.hairline)
        .border_color(t.border)
        .overflow_hidden()
        .child(step(
            "size-down",
            fl!("editor-font-size-decrease"),
            IconName::Minus,
            t,
            move |cx| down(StylePatch::FontSize(size - 1.), cx),
        ))
        .child(rule())
        .child(
            div()
                .id("size")
                .w(SIZE_FIELD_WIDTH)
                .flex()
                .justify_center()
                .typeset(t.type_scale.value)
                .accessible(
                    Role::Label,
                    fl!("editor-font-size-a11y", size = (size.round() as i64)),
                )
                .child(SharedString::from(format!("{}", size.round() as i64))),
        )
        .child(rule())
        .child(step(
            "size-up",
            fl!("editor-font-size-increase"),
            IconName::Plus,
            t,
            move |cx| up(StylePatch::FontSize(size + 1.), cx),
        ))
}

fn emphasis(
    bold: bool,
    italic: bool,
    underline: bool,
    apply: &Apply,
    t: &Tokens,
) -> impl IntoElement {
    let cell = |id: &'static str, glyph: String, label: String, on: bool, patch: StylePatch| {
        let apply = apply.clone();
        let fill = if on {
            t.selected_segment
        } else {
            gpui::transparent_black()
        };
        let hover = t.hover_fill;
        let face = div().child(glyph);
        let face = match id {
            "bold" => face.font_weight(FontWeight::BOLD),
            "italic" => face.italic(),
            _ => face.underline(),
        };
        div()
            .id(id)
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .rounded(t.metrics.slider_radius - t.metrics.control_inset)
            .bg(fill)
            .text_color(if on { t.text } else { t.text_secondary })
            .accessible(Role::Button, label)
            .toggled(on)
            .hover(move |s| s.bg(hover))
            .on_click(move |_, _, cx| apply(patch.clone(), cx))
            .child(face)
    };
    div()
        .flex()
        .w(EMPHASIS_WIDTH)
        .h(t.metrics.control_height)
        .p(t.metrics.control_inset)
        .rounded(t.metrics.slider_radius)
        .bg(t.segment_track)
        .typeset(t.type_scale.segmented)
        .child(cell(
            "bold",
            fl!("editor-text-bold-glyph"),
            fl!("editor-text-bold"),
            bold,
            StylePatch::Bold(!bold),
        ))
        .child(cell(
            "italic",
            fl!("editor-text-italic-glyph"),
            fl!("editor-text-italic"),
            italic,
            StylePatch::Italic(!italic),
        ))
        .child(cell(
            "underline",
            fl!("editor-text-underline-glyph"),
            fl!("editor-text-underline"),
            underline,
            StylePatch::Underline(!underline),
        ))
}

pub(super) fn content(
    page: &mut ToolOptions,
    style: &CurrentStyle,
    window: &mut Window,
    cx: &mut Context<ToolOptions>,
) -> Div {
    let weak = cx.entity().downgrade();
    let t = tokens(cx).clone();
    let apply = applier(weak.clone());
    let installed = page.doc.read(cx).text().has_family(&style.font_family);
    let list = page.font_list(&style.font_family, window, cx);
    let family = Select::new(&list)
        .id("font-family")
        .placeholder(family_title(&style.font_family, installed))
        .search_placeholder(fl!("editor-font-search"))
        .accessibility_label(fl!("editor-font-family-help"))
        .menu_max_h(FONT_MENU_HEIGHT)
        .w_full();
    let align_apply = apply.clone();
    let align = Segmented::new(
        "align",
        fl!("editor-text-align"),
        [
            Segment::icon(IconName::TextAlignStart, fl!("editor-text-align-start")),
            Segment::icon(IconName::TextAlignCenter, fl!("editor-text-align-center")),
            Segment::icon(IconName::TextAlignEnd, fl!("editor-text-align-end")),
        ],
    )
    .height(t.metrics.control_height)
    .selected(ALIGNS.iter().position(|a| *a == style.align).unwrap_or(0))
    .on_select(move |i, _, cx| {
        if let Some(&a) = ALIGNS.get(i) {
            align_apply(StylePatch::Align(a), cx);
        }
    });
    div()
        .flex()
        .flex_col()
        .child(section(
            fl!("editor-font"),
            track(&page.drawn, Control::FontFamily, family).w_full(),
            &t,
        ))
        .child(section(
            fl!("editor-color"),
            swatches(
                "swatches",
                Control::Swatches,
                &style.swatch,
                weak,
                &page.drawn,
            ),
            &t,
        ))
        .child(section(
            fl!("editor-text-style"),
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(stepper(style.font_size, apply.clone(), &t))
                .child(emphasis(
                    style.bold,
                    style.italic,
                    style.underline,
                    &apply,
                    &t,
                )),
            &t,
        ))
        .child(section(fl!("editor-text-align"), align, &t))
        .child(text_box::fill(page, style, cx))
        .child(text_box::border(page, style, cx))
}
