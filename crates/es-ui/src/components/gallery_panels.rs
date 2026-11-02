use es_model::background::Alignment;
use gpui::{
    AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, point, px,
};

use crate::{
    components::{
        AlignmentPad, ButtonKind, DisclosureSection, EditorButton, FocusPad, Interaction, Popover,
        PresetBar, Segment, Segmented, Toast, ToastStyle, gallery::Section,
    },
    fl,
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};

pub const SECTION: Section = Section {
    name: "panels",
    height: 1250.,
    render: panels,
};

const INSPECTOR_WIDTH: f32 = 300.;

fn caption(t: &Tokens, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .whitespace_nowrap()
        .child(text.into())
}

fn sample(t: &Tokens, name: &'static str, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(body)
        .child(caption(t, name))
        .into_any_element()
}

fn row(t: &Tokens, label: &'static str, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .typeset(t.type_scale.label)
                .text_color(t.text)
                .child(label),
        )
        .child(control)
}

fn presets() -> Vec<SharedString> {
    vec!["Soft Studio".into(), "Launch Week Wallpaper".into()]
}

fn inspector(t: &Tokens) -> impl IntoElement {
    let pad =
        |id: &'static str| AlignmentPad::new(id, fl!("panels-align-label")).on_select(|_, _, _| {});
    div()
        .w(px(INSPECTOR_WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .rounded(t.metrics.card_radius)
        .bg(t.inspector)
        .child(PresetBar::new("presets", presets()).on_add(|_, _| {}))
        .child(
            div()
                .mx(t.metrics.inspector_padding)
                .h(t.metrics.hairline)
                .bg(t.separator.opacity(0.45)),
        )
        .child(
            DisclosureSection::new("background", "Background")
                .expanded(true)
                .reset("Remove background", |_, _| {})
                .on_toggle(|_, _, _| {})
                .child(row(t, "Alignment", pad("align"))),
        )
        .child(
            DisclosureSection::new("camera", "Camera")
                .reset("Reset camera", |_, _| {})
                .on_toggle(|_, _, _| {}),
        )
        .child(
            DisclosureSection::new("blur", "Progressive Blur")
                .expanded(true)
                .toggle(true, |_, _, _| {})
                .reset("Reset blur", |_, _| {})
                .on_toggle(|_, _, _| {})
                .child(
                    FocusPad::new("focus", fl!("panels-focus-label"))
                        .position(point(0.62, 0.4))
                        .on_change(|_, _, _| {}),
                ),
        )
        .child(
            DisclosureSection::new("border", "Border, header hover")
                .toggle(false, |_, _, _| {})
                .preview(Interaction::Hover)
                .on_toggle(|_, _, _| {}),
        )
        .child(
            DisclosureSection::new("watermark", "Watermark, keyboard focus")
                .preview(Interaction::Focused)
                .on_toggle(|_, _, _| {}),
        )
}

fn pads(t: &Tokens) -> impl IntoElement {
    let pad =
        |id: &'static str| AlignmentPad::new(id, fl!("panels-align-label")).on_select(|_, _, _| {});
    div()
        .flex()
        .flex_wrap()
        .gap_x(px(16.))
        .gap_y(px(12.))
        .child(sample(t, "centre", pad("a-rest")))
        .child(sample(
            t,
            "top left",
            pad("a-top").selected(Alignment::TopLeading),
        ))
        .child(sample(
            t,
            "hover right",
            pad("a-hover").preview(Alignment::Trailing, Interaction::Hover),
        ))
        .child(sample(
            t,
            "focused",
            pad("a-focus")
                .selected(Alignment::Bottom)
                .preview(Alignment::Bottom, Interaction::Focused),
        ))
        .child(sample(
            t,
            "disabled",
            pad("a-off")
                .selected(Alignment::TopTrailing)
                .disabled(true)
                .help("Reset Camera to use alignment"),
        ))
}

fn focus_pads(t: &Tokens) -> impl IntoElement {
    let pad =
        |id: &'static str| FocusPad::new(id, fl!("panels-focus-label")).on_change(|_, _, _| {});
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .child(div().w(px(116.)).child(sample(
            t,
            "hover",
            pad("f-hover").preview(Interaction::Hover),
        )))
        .child(
            div().w(px(116.)).child(sample(
                t,
                "dragging",
                pad("f-drag")
                    .position(point(0.25, 0.7))
                    .preview(Interaction::Pressed),
            )),
        )
        .child(
            div().w(px(116.)).child(sample(
                t,
                "keyboard focus",
                pad("f-focus")
                    .position(point(0.82, 0.22))
                    .preview(Interaction::Focused),
            )),
        )
}

fn popover(t: &Tokens) -> impl IntoElement {
    let segments = || ["30 fps", "60 fps"].map(Segment::text);
    let content = div().flex().flex_col().items_start().child(
        Popover::new("export-popover", "Export Options")
            .width(px(240.))
            .default_open(true)
            .settled()
            .trigger(
                EditorButton::new("export")
                    .label("Export")
                    .icon(IconName::Download)
                    .kind(ButtonKind::Plain),
            )
            .content(move |_, cx| {
                let t = tokens(cx).clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().typeset(t.type_scale.title).child("Export Options"))
                    .child(caption(&t, "Frame Rate"))
                    .child(
                        Segmented::new("fps", "Frame Rate", segments())
                            .selected(1)
                            .height(px(28.)),
                    )
                    .child(
                        div()
                            .typeset(t.type_scale.label)
                            .text_color(t.text_secondary)
                            .child("60 fps keeps cursor movement and zooms smoother."),
                    )
            }),
    );
    sample(
        t,
        "popover open, radius 12",
        div().h(px(190.)).child(content),
    )
}

fn toasts(t: &Tokens) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(sample(
            t,
            "toast",
            div().flex().child(
                Toast::new("toast", "Saved")
                    .message("Screenshot saved!")
                    .icon(IconName::CircleCheck)
                    .settled(),
            ),
        ))
        .child(sample(
            t,
            "error toast, two-line message",
            div().flex().child(
                Toast::new("toast-error", "Couldn’t save capture")
                    .message(
                        "The capture is still in the deck. Check the save folder in General \
                     settings and try Save again.",
                    )
                    .icon(IconName::TriangleAlert)
                    .style(ToastStyle::Error)
                    .settled(),
            ),
        ))
}

fn panels(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let applied = div()
        .w_full()
        .rounded(t.metrics.card_radius)
        .bg(t.inspector)
        .child(
            PresetBar::new("presets-applied", presets())
                .applied(Some(1))
                .preview(Interaction::Focused)
                .on_add(|_, _| {})
                .on_delete(|_, _, _| {}),
        );
    let right = div()
        .flex_1()
        .w(px(0.))
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(sample(&t, "applied, long name, keyboard focus", applied))
        .child(pads(&t))
        .child(focus_pads(&t));
    div()
        .flex()
        .flex_col()
        .gap(px(20.))
        .child(div().flex().gap(px(20.)).child(inspector(&t)).child(right))
        .child(popover(&t))
        .child(toasts(&t))
        .into_any_element()
}
