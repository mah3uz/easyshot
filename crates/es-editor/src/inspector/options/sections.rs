use es_anno::{CurrentStyle, StylePatch};
use es_model::{Arrowhead, DashStyle};
use es_ui::{
    components::{Segment, Segmented, StrokeDots},
    theme::tokens,
    tokens::Tokens,
};
use gpui::{
    App, Context, Div, IntoElement, ParentElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::{
    Control, ToolOptions, section,
    swatches::{Facets, swatches},
    text_style,
};
use crate::strings::fl;
use crate::toolbar::{
    menus::{choice_menu, menu_trigger},
    track,
};

const HEADS: [Arrowhead; 8] = [
    Arrowhead::None,
    Arrowhead::Arrow,
    Arrowhead::Triangle,
    Arrowhead::Square,
    Arrowhead::Dot,
    Arrowhead::Diamond,
    Arrowhead::Inverted,
    Arrowhead::Bar,
];
const DASHES: [DashStyle; 3] = [DashStyle::Solid, DashStyle::Dashed, DashStyle::Dotted];

fn head_title(head: Arrowhead) -> String {
    match head {
        Arrowhead::None => fl!("editor-arrowhead-none"),
        Arrowhead::Arrow => fl!("editor-arrowhead-arrow"),
        Arrowhead::Triangle => fl!("editor-arrowhead-triangle"),
        Arrowhead::Square => fl!("editor-arrowhead-square"),
        Arrowhead::Dot => fl!("editor-arrowhead-dot"),
        Arrowhead::Diamond => fl!("editor-arrowhead-diamond"),
        Arrowhead::Inverted => fl!("editor-arrowhead-inverted"),
        Arrowhead::Bar => fl!("editor-arrowhead-bar"),
    }
}

pub(super) fn content(
    page: &mut ToolOptions,
    f: Facets,
    style: &CurrentStyle,
    window: &mut Window,
    cx: &mut Context<ToolOptions>,
) -> Div {
    if f.text {
        return text_style::content(page, style, window, cx);
    }
    let t = tokens(cx).clone();
    let weak = cx.entity().downgrade();
    let strength = f.strength.then(|| page.strength(window, cx));
    let colour = f.colour.then(|| {
        section(
            fl!("editor-color"),
            swatches(
                "swatches",
                Control::Swatches,
                &style.swatch,
                weak.clone(),
                &page.drawn,
            ),
            &t,
        )
    });
    let stroke = f.stroke.then(|| {
        let weak = weak.clone();
        let dots = StrokeDots::new("stroke", fl!("editor-stroke-width"))
            .selected(style.stroke_width as f32)
            .on_select(move |width, _, cx| {
                let patch = StylePatch::StrokeWidth(f64::from(width));
                let _ = weak.update(cx, |o, cx| o.apply_style(patch, None, cx));
            });
        section(
            fl!("editor-stroke-width"),
            track(&page.drawn, Control::Stroke, dots).w_full(),
            &t,
        )
    });
    div()
        .flex()
        .flex_col()
        .when_some(colour, |d, c| d.child(c))
        .when_some(stroke, |d, s| d.child(s))
        .when(f.heads, |d| d.child(heads(page, style, cx)))
        .when(f.dash, |d| d.child(dash(page, style.dash, cx)))
        .when(f.mode, |d| d.child(highlighter_mode(page, cx)))
        .when_some(strength, |d, s| {
            d.child(track(&page.drawn, Control::Strength, s).w_full())
        })
}

fn heads(page: &ToolOptions, style: &CurrentStyle, cx: &mut Context<ToolOptions>) -> Div {
    let t = tokens(cx).clone();
    let menu = |control,
                id: &'static str,
                label: String,
                current: Arrowhead,
                patch: fn(Arrowhead) -> StylePatch,
                cx: &mut Context<ToolOptions>| {
        let weak = cx.entity().downgrade();
        let trigger = menu_trigger(id, label, None, Some(head_title(current).into()), false, cx)
            .w_full()
            .bg(t.track);
        let choices = HEADS.iter().map(|&h| (h, head_title(h), None)).collect();
        let pick = std::rc::Rc::new(move |h, _: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |o, cx| o.apply_style(patch(h), None, cx));
        });
        track(
            &page.drawn,
            control,
            choice_menu(trigger, choices, Some(current), pick),
        )
        .flex_1()
    };
    let start = menu(
        Control::ArrowStart,
        "arrow-start",
        fl!("editor-arrowhead-start"),
        style.arrowhead_start,
        StylePatch::ArrowheadStart,
        cx,
    );
    let end = menu(
        Control::ArrowEnd,
        "arrow-end",
        fl!("editor-arrowhead-end"),
        style.arrowhead_end,
        StylePatch::ArrowheadEnd,
        cx,
    );
    div()
        .flex()
        .gap(px(8.))
        .child(labelled(fl!("editor-arrowhead-start"), start, &t))
        .child(labelled(fl!("editor-arrowhead-end"), end, &t))
}

fn labelled(label: String, control: impl IntoElement, t: &Tokens) -> Div {
    section(label, control, t).flex_1()
}

fn dash(page: &ToolOptions, current: DashStyle, cx: &mut Context<ToolOptions>) -> Div {
    let weak = cx.entity().downgrade();
    let segmented = Segmented::new(
        "dash",
        fl!("editor-line-style"),
        [
            Segment::text(fl!("editor-dash-solid")),
            Segment::text(fl!("editor-dash-dashed")),
            Segment::text(fl!("editor-dash-dotted")),
        ],
    )
    .height(tokens(cx).metrics.control_height)
    .selected(DASHES.iter().position(|d| *d == current).unwrap_or(0))
    .on_select(move |i, _, cx| {
        if let Some(&d) = DASHES.get(i) {
            let _ = weak.update(cx, |o, cx| o.apply_style(StylePatch::Dash(d), None, cx));
        }
    });
    section(
        fl!("editor-line-style"),
        track(&page.drawn, Control::Dash, segmented).w_full(),
        tokens(cx),
    )
}

fn highlighter_mode(page: &ToolOptions, cx: &mut Context<ToolOptions>) -> Div {
    let weak = cx.entity().downgrade();
    let straight = page.doc.read(cx).editor().highlighter_straight();
    let segmented = Segmented::new(
        "highlighter-mode",
        fl!("editor-highlighter-mode"),
        [
            Segment::text(fl!("editor-highlighter-freehand")),
            Segment::text(fl!("editor-highlighter-straight")),
        ],
    )
    .height(tokens(cx).metrics.control_height)
    .selected(usize::from(straight))
    .on_select(move |i, _, cx| {
        if (i == 1) != straight {
            let _ = weak.update(cx, |o, cx| o.switch_highlighter_mode(cx));
        }
    });
    section(
        fl!("editor-highlighter-mode"),
        track(&page.drawn, Control::HighlighterMode, segmented).w_full(),
        tokens(cx),
    )
}
