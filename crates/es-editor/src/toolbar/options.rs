use es_anno::{CurrentStyle, StylePatch};
use es_model::{Arrowhead, DashStyle, Tool};
use es_ui::{
    a11y::{Accessible, Role},
    components::{EditorButton, Segment, Segmented, StrokeDots, popover_shadow},
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    Anchor, AnyElement, App, Context, Div, InteractiveElement, IntoElement, ParentElement, Pixels,
    Stateful, Styled, Window, anchored, canvas, deferred, div, point, prelude::FluentBuilder, px,
    relative,
};

use super::{
    Control, ToolRow, crop_options,
    menus::{choice_menu, menu_trigger},
    reset,
    style_bar::{Facets, swatches},
    text_style,
    tools::{shortcut_label, title},
    track,
};
use crate::strings::fl;

const WIDTH: Pixels = px(296.);
const NOTCH_WIDTH: Pixels = px(18.);
const NOTCH_HEIGHT: Pixels = px(8.);
const GAP_TO_BUTTON: Pixels = px(6.);
const WINDOW_MARGIN: Pixels = px(8.);
const SECTION_GAP: Pixels = px(12.);
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

/// The options of the armed tool (or of the selection it inspects), and the control they hang under.
pub(super) fn render(
    row: &mut ToolRow,
    tool: Tool,
    window: &mut Window,
    cx: &mut Context<ToolRow>,
) -> Option<(Control, AnyElement)> {
    if row.cropping {
        let panel = panel(fl!("editor-crop"), None, crop_options::content(row, cx), cx);
        return Some((
            Control::Crop,
            attach(track(&row.drawn, Control::Options, panel), cx),
        ));
    }
    let (facets, style) = {
        let editor = row.doc.read(cx).editor();
        (Facets::of(editor)?, editor.current_style().clone())
    };
    let body = content(row, facets, &style, window, cx).child(reset::button(row, facets, cx));
    let delete = facets.selection.then(|| delete(row, cx));
    let panel = panel(title(facets.inspected), delete, body, cx);
    Some((
        Control::Tool(tool),
        attach(track(&row.drawn, Control::Options, panel), cx),
    ))
}

fn content(
    row: &mut ToolRow,
    f: Facets,
    style: &CurrentStyle,
    window: &mut Window,
    cx: &mut Context<ToolRow>,
) -> Div {
    let weak = cx.entity().downgrade();
    if f.text {
        return text_style::content(row, style, cx);
    }
    let strength = f.strength.then(|| row.strength(window, cx));
    let colour = f.colour.then(|| {
        swatches(
            "swatches",
            Control::Swatches,
            &style.swatch,
            weak.clone(),
            &row.drawn,
        )
    });
    let stroke = f.stroke.then(|| {
        let weak = weak.clone();
        let dots = StrokeDots::new("stroke", fl!("editor-stroke-width"))
            .selected(style.stroke_width as f32)
            .on_select(move |width, _, cx| {
                let patch = StylePatch::StrokeWidth(f64::from(width));
                let _ = weak.update(cx, |r, cx| r.apply_style(patch, None, cx));
            });
        track(&row.drawn, Control::Stroke, dots).w_full()
    });
    div()
        .flex()
        .flex_col()
        .gap(SECTION_GAP)
        .when_some(colour, |d, c| d.child(c))
        .when_some(stroke, |d, s| d.child(s))
        .when(f.heads, |d| d.child(heads(row, style, cx)))
        .when(f.dash, |d| d.child(dash(row, style.dash, cx)))
        .when(f.mode, |d| d.child(highlighter_mode(row, cx)))
        .when_some(strength, |d, s| {
            d.child(track(&row.drawn, Control::Strength, s).w_full())
        })
}

fn heads(row: &ToolRow, style: &CurrentStyle, cx: &mut Context<ToolRow>) -> Div {
    let t = tokens(cx).clone();
    let menu = |control,
                id: &'static str,
                label: String,
                current: Arrowhead,
                patch: fn(Arrowhead) -> StylePatch,
                cx: &mut Context<ToolRow>| {
        let weak = cx.entity().downgrade();
        let trigger = menu_trigger(id, label, None, Some(head_title(current).into()), false, cx)
            .w_full()
            .bg(t.track);
        let choices = HEADS.iter().map(|&h| (h, head_title(h), None)).collect();
        let pick = std::rc::Rc::new(move |h, _: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |r, cx| r.apply_style(patch(h), None, cx));
        });
        track(
            &row.drawn,
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
    div()
        .flex()
        .flex_1()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_color(t.text_secondary)
                .typeset(t.type_scale.label)
                .child(label),
        )
        .child(control)
}

fn dash(row: &ToolRow, current: DashStyle, cx: &mut Context<ToolRow>) -> Div {
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
            let _ = weak.update(cx, |r, cx| r.apply_style(StylePatch::Dash(d), None, cx));
        }
    });
    labelled(
        fl!("editor-line-style"),
        track(&row.drawn, Control::Dash, segmented).w_full(),
        tokens(cx),
    )
}

/// Picking the highlighter again switches its mode, so a segment that differs re-picks it.
fn highlighter_mode(row: &ToolRow, cx: &mut Context<ToolRow>) -> Div {
    let weak = cx.entity().downgrade();
    let straight = row.doc.read(cx).editor().highlighter_straight();
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
            let _ = weak.update(cx, |r, cx| r.pick_tool(Tool::Highlighter, cx));
        }
    });
    labelled(
        fl!("editor-highlighter-mode"),
        track(&row.drawn, Control::HighlighterMode, segmented).w_full(),
        tokens(cx),
    )
}

fn delete(row: &ToolRow, cx: &mut Context<ToolRow>) -> AnyElement {
    let weak = cx.entity().downgrade();
    let shortcut = shortcut_label("image-delete").unwrap_or_default();
    let button = EditorButton::new("delete")
        .icon(IconName::Trash)
        .accessible_label(fl!("editor-delete-a11y"))
        .tooltip(fl!("editor-delete-help", shortcut = shortcut))
        .padding_x(tokens(cx).metrics.tool_button_padding_x)
        .on_click(move |_, _, cx| {
            let _ = weak.update(cx, |r, cx| r.delete_selection(cx));
        });
    track(&row.drawn, Control::Delete, button).into_any_element()
}

fn panel(heading: String, action: Option<AnyElement>, body: Div, cx: &App) -> Stateful<Div> {
    let t = tokens(cx).clone();
    let m = t.metrics;
    div()
        .id("tool-options")
        .occlude()
        .w(WIDTH)
        .flex()
        .flex_col()
        .gap(SECTION_GAP)
        .p(px(12.))
        .bg(t.dropdown)
        .text_color(t.text)
        .rounded(m.popover_radius)
        .border(m.hairline)
        .border_color(t.border)
        .shadow(popover_shadow(1.))
        .accessible(
            Role::Group,
            fl!("editor-tool-options-a11y", tool = heading.clone()),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .h(m.button_height)
                .child(div().typeset(t.type_scale.title).child(heading))
                .children(action),
        )
        .child(body)
}

/// The panel opens below the button's centre and slides to stay inside the window; the notch stays on the button.
fn attach(panel: Div, cx: &App) -> AnyElement {
    let t = tokens(cx).clone();
    let (fill, edge, hairline) = (t.dropdown, t.border, t.metrics.hairline);
    let notch = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let o = bounds.origin;
            let (w, h) = (NOTCH_WIDTH, NOTCH_HEIGHT);
            let left = point(o.x, o.y + h);
            let apex = point(o.x + w / 2., o.y);
            let right = point(o.x + w, o.y + h);
            let mut body = gpui::PathBuilder::fill();
            body.move_to(left);
            body.line_to(apex);
            body.line_to(right);
            body.line_to(point(right.x, right.y + hairline * 2.));
            body.line_to(point(left.x, left.y + hairline * 2.));
            body.close();
            if let Ok(path) = body.build() {
                window.paint_path(path, fill);
            }
            let mut rim = gpui::PathBuilder::stroke(hairline);
            rim.move_to(left);
            rim.line_to(apex);
            rim.line_to(right);
            if let Ok(path) = rim.build() {
                window.paint_path(path, edge);
            }
        },
    )
    .w(NOTCH_WIDTH)
    .h(NOTCH_HEIGHT);
    div()
        .absolute()
        .left(relative(0.5))
        .top(relative(1.))
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopCenter)
                    .offset(point(px(0.), GAP_TO_BUTTON + NOTCH_HEIGHT - hairline))
                    .snap_to_window_with_margin(WINDOW_MARGIN)
                    .child(panel),
            )
            .with_priority(1),
        )
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopCenter)
                    .offset(point(px(0.), GAP_TO_BUTTON))
                    .child(notch),
            )
            .with_priority(2),
        )
        .into_any_element()
}
