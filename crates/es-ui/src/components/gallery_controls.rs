use es_protocol::{Modifiers, Trigger};
use gpui::{
    AnyElement, App, ElementId, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    div, px,
};

use crate::{
    components::{
        ButtonKind, Capsule, EditorButton, IconButton, InspectorSlider, Interaction, Segment,
        Segmented, ShortcutField, ToolButton, ValueFormat, ZoomCapsule,
    },
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};

const STATES: [(&str, Interaction); 4] = [
    ("rest", Interaction::Rest),
    ("hover", Interaction::Hover),
    ("pressed", Interaction::Pressed),
    ("focused", Interaction::Focused),
];

fn caption(t: &Tokens, text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(text.into())
}

fn cell(t: &Tokens, name: impl Into<SharedString>, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .child(div().h(px(36.)).flex().items_center().child(body))
        .child(caption(t, name))
        .into_any_element()
}

fn row(t: &Tokens, name: &str, cells: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_start()
        .gap(px(14.))
        .child(
            div()
                .w(px(76.))
                .flex_none()
                .pt(px(10.))
                .child(caption(t, name.to_owned())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_wrap()
                .gap_x(px(14.))
                .gap_y(px(10.))
                .children(cells),
        )
        .into_any_element()
}

fn panel(t: &Tokens, body: impl IntoElement) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(t.metrics.row_spacing)
        .p(t.metrics.inspector_padding)
        .rounded(t.metrics.card_radius)
        .bg(t.inspector)
        .child(body)
        .into_any_element()
}

fn key(name: &str, cx: &App) -> ElementId {
    let appearance = tokens(cx).appearance;
    ElementId::Name(format!("{name}-{appearance:?}").into())
}

pub fn buttons(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let kinds = [
        ("plain", ButtonKind::Plain, Some(IconName::Crop), "Crop"),
        (
            "bordered",
            ButtonKind::Bordered,
            Some(IconName::Copy),
            "Copy",
        ),
        (
            "prominent",
            ButtonKind::Prominent,
            Some(IconName::Download),
            "Export",
        ),
    ];
    let rows = kinds.map(|(name, kind, icon, label)| {
        let button = |id: String| {
            let mut b = EditorButton::new(ElementId::Name(id.into()))
                .kind(kind)
                .label(label);
            if let Some(icon) = icon {
                b = b.icon(icon);
            }
            b
        };
        let mut cells: Vec<AnyElement> = STATES
            .iter()
            .map(|(state_name, state)| {
                cell(
                    &t,
                    *state_name,
                    button(format!("{name}-{state_name}")).preview(*state),
                )
            })
            .collect();
        cells.push(cell(
            &t,
            "selected",
            button(format!("{name}-selected")).selected(true),
        ));
        cells.push(cell(
            &t,
            "selected hover",
            button(format!("{name}-selected-hover"))
                .selected(true)
                .preview(Interaction::Hover),
        ));
        cells.push(cell(
            &t,
            "disabled",
            button(format!("{name}-disabled")).disabled(true),
        ));
        row(&t, name, cells)
    });
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .children(rows)
        .into_any_element()
}

pub fn tool_buttons(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let tools = [
        (IconName::Pointer, "Select", "V"),
        (IconName::Square, "Rectangle", "R"),
        (IconName::Circle, "Ellipse", "O"),
        (IconName::Slash, "Line", "L"),
        (IconName::ArrowUpRight, "Arrow", "A"),
        (IconName::Pencil, "Draw", "P"),
        (IconName::Counter, "Counter", "N"),
        (IconName::ALargeSmall, "Text", "T"),
        (IconName::Highlight, "Highlight", "H"),
        (IconName::Pixelate, "Pixelate", "X"),
        (IconName::Droplet, "Blur", "B"),
        (IconName::EyeOff, "Redact", "E"),
    ];
    let strip = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(tools.iter().enumerate().map(|(i, (icon, title, key))| {
            ToolButton::new(("tool", i), *icon, *title)
                .shortcut(*key)
                .selected(i == 4)
        }));
    let arrow =
        |id: &'static str| ToolButton::new(id, IconName::ArrowUpRight, "Arrow").shortcut("A");
    let cells = vec![
        cell(&t, "rest", arrow("t-rest")),
        cell(&t, "hover", arrow("t-hover").preview(Interaction::Hover)),
        cell(
            &t,
            "pressed",
            arrow("t-pressed").preview(Interaction::Pressed),
        ),
        cell(&t, "selected", arrow("t-selected").selected(true)),
        cell(
            &t,
            "selected hover",
            arrow("t-sel-hover")
                .selected(true)
                .preview(Interaction::Hover),
        ),
        cell(
            &t,
            "focused",
            arrow("t-focused").preview(Interaction::Focused),
        ),
        cell(&t, "disabled", arrow("t-disabled").disabled(true)),
    ];
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(row(&t, "toolbar", vec![strip.into_any_element()]))
        .child(row(&t, "states", cells))
        .into_any_element()
}

pub fn capsules(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let icon_states: Vec<AnyElement> = STATES
        .iter()
        .map(|(name, state)| {
            cell(
                &t,
                *name,
                IconButton::new(
                    ElementId::Name(format!("icon-{name}").into()),
                    IconName::Trash,
                    "Delete preset",
                )
                .preview(*state),
            )
        })
        .chain([cell(
            &t,
            "disabled",
            IconButton::new("icon-disabled", IconName::Trash, "Delete preset").disabled(true),
        )])
        .collect();
    let history = Capsule::new("history", "History")
        .child(IconButton::new("undo", IconName::Undo2, "Undo"))
        .child(IconButton::new("redo", IconName::Redo2, "Redo").disabled(true));
    let zoom: Vec<AnyElement> = STATES
        .iter()
        .map(|(name, state)| {
            cell(
                &t,
                *name,
                ZoomCapsule::new(ElementId::Name(format!("zoom-{name}").into()), 56)
                    .preview(*state),
            )
        })
        .chain([cell(&t, "400%", ZoomCapsule::new("zoom-max", 400))])
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(row(&t, "icon button", icon_states))
        .child(row(
            &t,
            "capsule",
            vec![cell(&t, "undo, redo off", history)],
        ))
        .child(row(&t, "zoom", zoom))
        .into_any_element()
}

pub fn segmented(_window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let shadows = || ["Soft", "Long", "Glow", "Crisp"].map(Segment::text);
    let sample = |name: &'static str, s: Segmented| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(s)
            .child(caption(&t, name))
            .into_any_element()
    };
    let left = div()
        .w(px(276.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(sample(
            "rest",
            Segmented::new("s-rest", "Shadow style", shadows()).selected(0),
        ))
        .child(sample(
            "hover on Glow",
            Segmented::new("s-hover", "Shadow style", shadows())
                .selected(0)
                .preview(2, Interaction::Hover),
        ))
        .child(sample(
            "focused",
            Segmented::new("s-focused", "Shadow style", shadows())
                .selected(1)
                .preview(0, Interaction::Focused),
        ))
        .child(sample(
            "disabled",
            Segmented::new("s-disabled", "Shadow style", shadows())
                .selected(0)
                .disabled(true),
        ));
    let align = [
        (IconName::TextAlignStart, "Left"),
        (IconName::TextAlignCenter, "Centre"),
        (IconName::TextAlignEnd, "Right"),
    ]
    .map(|(icon, label)| Segment::icon(icon, label));
    let right = div()
        .w(px(180.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(sample(
            "icons, height 24",
            Segmented::new("s-icons", "Alignment", align)
                .selected(1)
                .height(t.metrics.control_height),
        ))
        .child(sample(
            "long labels truncate",
            Segmented::new(
                "s-long",
                "Apply to",
                ["Everything in the canvas", "Selection"].map(Segment::text),
            )
            .selected(1),
        ));
    panel(&t, div().flex().gap(px(24.)).child(left).child(right))
}

struct SliderSample {
    id: &'static str,
    caption: &'static str,
    label: &'static str,
    value: f64,
    range: std::ops::RangeInclusive<f64>,
    format: ValueFormat,
    state: Interaction,
    disabled: bool,
}

fn slider(window: &mut Window, cx: &mut App, sample: SliderSample) -> Entity<InspectorSlider> {
    let id = key(sample.id, cx);
    let SliderSample {
        label,
        value,
        range,
        format,
        state,
        disabled,
        ..
    } = sample;
    window.use_keyed_state(id, cx, move |window, cx| {
        let mut s = InspectorSlider::new(label, value, range, format, window, cx);
        s.preview(state, cx);
        if disabled {
            s.set_disabled(true, window, cx);
        }
        s
    })
}

pub fn sliders(window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let percent = ValueFormat::percent;
    let sample = |id, caption, label, value, range, format, state| SliderSample {
        id,
        caption,
        label,
        value,
        range,
        format,
        state,
        disabled: false,
    };
    let rows = [
        sample(
            "sl-rest",
            "rest",
            "Padding",
            0.27,
            0.0..=1.0,
            percent(),
            Interaction::Rest,
        ),
        sample(
            "sl-hover",
            "hover: ticks",
            "Padding",
            0.27,
            0.0..=1.0,
            percent(),
            Interaction::Hover,
        ),
        sample(
            "sl-pressed",
            "pressed or focused",
            "Shadow",
            0.48,
            0.0..=1.0,
            percent(),
            Interaction::Pressed,
        ),
        sample(
            "sl-signed",
            "signed, hover",
            "Tilt X",
            12.0,
            -45.0..=45.0,
            ValueFormat::degrees().signed(),
            Interaction::Hover,
        ),
        sample(
            "sl-zoom",
            "value field focused",
            "Zoom",
            1.5,
            1.0..=3.0,
            ValueFormat::magnification(2),
            Interaction::Focused,
        ),
        SliderSample {
            disabled: true,
            ..sample(
                "sl-disabled",
                "disabled",
                "Corners",
                0.01,
                0.0..=1.0,
                percent(),
                Interaction::Rest,
            )
        },
    ];
    let body = div()
        .w(px(296.))
        .flex()
        .flex_col()
        .gap(t.metrics.row_spacing)
        .children(rows.into_iter().map(|row| {
            let caption_text = row.caption;
            let s = slider(window, cx, row);
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(caption(&t, caption_text))
                .child(s)
        }));
    panel(&t, body)
}

type ShortcutSample = (
    &'static str,
    &'static str,
    Option<Trigger>,
    Interaction,
    Option<Modifiers>,
);

pub fn shortcut_fields(window: &mut Window, cx: &mut App) -> AnyElement {
    let t = tokens(cx).clone();
    let super_shift_4 = Trigger::parse("Super+Shift+4").ok();
    let samples: [ShortcutSample; 6] = [
        (
            "sc-rest",
            "assigned",
            super_shift_4.clone(),
            Interaction::Rest,
            None,
        ),
        ("sc-empty", "unassigned", None, Interaction::Rest, None),
        (
            "sc-hover",
            "hover",
            super_shift_4.clone(),
            Interaction::Hover,
            None,
        ),
        (
            "sc-focused",
            "focused",
            super_shift_4.clone(),
            Interaction::Focused,
            None,
        ),
        (
            "sc-recording",
            "recording",
            super_shift_4.clone(),
            Interaction::Rest,
            Some(Modifiers::empty()),
        ),
        (
            "sc-held",
            "modifiers held",
            super_shift_4,
            Interaction::Rest,
            Some(Modifiers::SUPER | Modifiers::SHIFT),
        ),
    ];
    let cells = samples
        .into_iter()
        .map(|(id, name, trigger, state, held)| {
            let field = window.use_keyed_state(key(id, cx), cx, move |window, cx| {
                let mut f = ShortcutField::new("Capture area", window, cx);
                f.set_trigger(trigger, cx);
                f.preview(state, held, cx);
                f
            });
            cell(&t, name, div().w(px(150.)).child(field))
        })
        .collect();
    row(&t, "shortcut", cells)
}
