use std::rc::Rc;

use es_anno::{CurrentStyle, StylePatch};
use es_model::TextAlign;
use es_ui::components::Tip;
use es_ui::{
    a11y::{Accessible, Role},
    components::{Segment, Segmented},
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};
use gpui::{
    App, AppContext, Context, Div, Entity, Focusable, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, SharedString, StatefulInteractiveElement, Styled,
    WeakEntity, Window, div, px,
};
use gpui_component::{
    input::{Escape, Input, InputEvent, InputState},
    select::Select,
};

use super::{
    Control, ToolOptions, font_picker::family_title, section, swatches::swatches, text_box,
};
use crate::keyboard::TEXT_SIZE_RANGE;
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

fn size_text(size: f64) -> SharedString {
    format!("{}", size.round() as i64).into()
}

/// A typed size, clamped to the range; `None` for text that is not a number, so the field reverts.
pub(crate) fn parse_size(text: &str) -> Option<f64> {
    let size: f64 = text.trim().parse().ok()?;
    let (lo, hi) = TEXT_SIZE_RANGE;
    size.is_finite().then(|| size.round().clamp(lo, hi))
}

impl ToolOptions {
    /// Built on first draw, since the field needs a window; shows the model's size unless it is being typed in.
    fn size_field(
        &mut self,
        size: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let field = match &self.font_size {
            Some(field) => field.clone(),
            None => {
                let field = cx.new(|cx| InputState::new(window, cx));
                let focus = field.focus_handle(cx);
                let events = cx.subscribe_in(&field, window, Self::on_size_field);
                let selecting = cx.on_focus(&focus, window, |this, window, cx| {
                    if let Some(field) = &this.font_size {
                        field.update(cx, |f, cx| f.select_all(window, cx));
                    }
                });
                let leaving = cx.on_blur(&focus, window, |this, _, cx| {
                    if let Some(field) = this.font_size.clone() {
                        this.commit_size(&field, cx);
                    }
                });
                self._subscriptions.extend([events, selecting, leaving]);
                self.font_size = Some(field.clone());
                field
            }
        };
        let text = size_text(size);
        if !field.focus_handle(cx).is_focused(window) && field.read(cx).value() != text {
            field.update(cx, |f, cx| f.set_value(text, window, cx));
        }
        field
    }

    fn on_size_field(
        &mut self,
        field: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::PressEnter { .. } = event {
            self.commit_size(field, cx);
            window.blur(cx);
        }
    }

    fn current_size(&self, cx: &App) -> f64 {
        self.doc.read(cx).editor().current_style().font_size
    }

    fn commit_size(&mut self, field: &Entity<InputState>, cx: &mut Context<Self>) {
        let typed = field.read(cx).value().to_string();
        if typed != size_text(self.current_size(cx)).as_ref()
            && let Some(size) = parse_size(&typed)
        {
            self.apply_style(StylePatch::FontSize(size), None, cx);
        }
    }

    fn revert_size(
        &mut self,
        field: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = size_text(self.current_size(cx));
        field.update(cx, |f, cx| f.set_value(text, window, cx));
    }
}

fn stepper(
    size: f64,
    field: &Entity<InputState>,
    editing: bool,
    apply: Apply,
    t: &Tokens,
    cx: &mut Context<ToolOptions>,
) -> impl IntoElement {
    let m = t.metrics;
    let rule = || div().w(m.hairline).h(px(13.)).bg(t.separator);
    let (down, up) = (apply.clone(), apply);
    let (lo, hi) = TEXT_SIZE_RANGE;
    let help: SharedString = fl!("editor-font-size-help", min = lo, max = hi).into();
    div()
        .flex()
        .items_center()
        .h(m.control_height)
        .rounded(m.field_radius)
        .bg(t.track)
        .border(m.hairline)
        .border_color(if editing {
            t.accent.opacity(0.72)
        } else {
            t.border
        })
        .overflow_hidden()
        .child(step(
            "size-down",
            fl!("editor-font-size-decrease"),
            IconName::Minus,
            t,
            move |cx| down(StylePatch::FontSize((size - 1.).clamp(lo, hi)), cx),
        ))
        .child(rule())
        .child(
            div()
                .id("size")
                .w(SIZE_FIELD_WIDTH)
                .h_full()
                .flex()
                .items_center()
                .tip(help)
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if let Some(field) = &this.font_size {
                            field.update(cx, |f, cx| f.select_all(window, cx));
                        }
                    }),
                )
                .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                    cx.stop_propagation();
                    if let Some(field) = this.font_size.clone() {
                        this.revert_size(&field, window, cx);
                    }
                    window.blur(cx);
                }))
                .child(
                    Input::new(field)
                        .appearance(false)
                        .aria_label(fl!("editor-font-size"))
                        .px(px(2.))
                        .py_0()
                        .text_center()
                        .typeset(t.type_scale.value),
                ),
        )
        .child(rule())
        .child(step(
            "size-up",
            fl!("editor-font-size-increase"),
            IconName::Plus,
            t,
            move |cx| up(StylePatch::FontSize((size + 1.).clamp(lo, hi)), cx),
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
    let size_field = page.size_field(style.font_size, window, cx);
    let editing = size_field.focus_handle(cx).is_focused(window);
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
                .child(page.track(
                    Control::FontSize,
                    stepper(style.font_size, &size_field, editing, apply.clone(), &t, cx),
                ))
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
