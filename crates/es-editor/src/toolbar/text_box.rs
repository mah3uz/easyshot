use es_anno::{CurrentStyle, StylePatch};
use es_model::{Swatch, TextBorder};
use es_ui::{
    a11y::{Accessible, Role},
    components::StrokeDots,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    Context, Div, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px,
};

use super::{Control, ToolRow, style_bar::swatch_strip, track};
use crate::strings::fl;

const BORDER_WIDTHS: [f32; 3] = [2., 3., 5.];

fn none_pill(
    id: &'static str,
    control: Control,
    on: bool,
    patch: StylePatch,
    row: &ToolRow,
    cx: &mut Context<ToolRow>,
) -> Div {
    let t = tokens(cx).clone();
    let weak = cx.entity().downgrade();
    let hover = t.hover_fill;
    let pill = div()
        .id(id)
        .px(px(10.))
        .h(px(20.))
        .flex()
        .items_center()
        .rounded(t.metrics.slider_radius)
        .bg(if on {
            t.selected_segment
        } else {
            t.segment_track
        })
        .text_color(if on { t.text } else { t.text_secondary })
        .typeset(t.type_scale.label)
        .accessible(Role::Button, fl!("editor-text-box-none"))
        .toggled(on)
        .hover(move |s| s.bg(hover))
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |r, cx| r.apply_style(patch.clone(), None, cx));
        })
        .child(fl!("editor-text-box-none"));
    track(&row.drawn, control, pill)
}

fn heading(label: String, none: Div, cx: &Context<ToolRow>) -> Div {
    let t = tokens(cx);
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_color(t.text_secondary)
                .typeset(t.type_scale.label)
                .child(label),
        )
        .child(none)
}

pub(super) fn fill(row: &ToolRow, style: &CurrentStyle, cx: &mut Context<ToolRow>) -> Div {
    let weak = cx.entity().downgrade();
    let none = none_pill(
        "text-fill-none",
        Control::TextFillNone,
        style.text_fill.is_none(),
        StylePatch::TextFill(None),
        row,
        cx,
    );
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(heading(fl!("editor-text-background"), none, cx))
        .child(swatch_strip(
            "text-fill",
            fl!("editor-text-background"),
            Control::TextFill,
            style.text_fill.as_ref(),
            weak,
            &row.drawn,
            |swatch| StylePatch::TextFill(Some(swatch)),
        ))
}

pub(super) fn border(row: &ToolRow, style: &CurrentStyle, cx: &mut Context<ToolRow>) -> Div {
    let weak = cx.entity().downgrade();
    let none = none_pill(
        "text-border-none",
        Control::TextBorderNone,
        style.text_border.is_none(),
        StylePatch::TextBorder(None),
        row,
        cx,
    );
    let width = style.text_border.as_ref().map(|b| b.width);
    let keep = width.unwrap_or(TextBorder::default().width);
    let pick = weak.clone();
    let strip = swatch_strip(
        "text-border",
        fl!("editor-text-border"),
        Control::TextBorder,
        style.text_border.as_ref().map(|b| &b.swatch),
        weak.clone(),
        &row.drawn,
        move |swatch: Swatch| {
            StylePatch::TextBorder(Some(TextBorder {
                swatch,
                width: keep,
            }))
        },
    );
    let swatch = style.text_border.as_ref().map(|b| b.swatch.clone());
    let widths = swatch.map(|swatch| {
        let dots = StrokeDots::new("text-border-width", fl!("editor-text-border-width"))
            .widths(BORDER_WIDTHS)
            .selected(width.unwrap_or(keep) as f32)
            .on_select(move |w, _, cx| {
                let border = TextBorder {
                    swatch: swatch.clone(),
                    width: f64::from(w),
                };
                let patch = StylePatch::TextBorder(Some(border));
                let _ = pick.update(cx, |r, cx| r.apply_style(patch, None, cx));
            });
        track(&row.drawn, Control::TextBorderWidth, dots).w_full()
    });
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(heading(fl!("editor-text-border"), none, cx))
        .child(strip)
        .children(widths)
}
