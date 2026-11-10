use std::rc::Rc;

use crate::components::Tip;
use es_model::background::Alignment;
use gpui::{
    App, ElementId, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px,
};

use crate::{
    a11y::{Accessible, Role},
    components::button::{Interaction, focus_halo, keyed_focus, shows_focus},
    fl,
    theme::tokens,
};

pub const ALIGNMENT_PAD: Pixels = px(44.);
const CELL: Pixels = px(12.);
const MARKER: Pixels = px(6.);
const MARKER_RADIUS: Pixels = px(1.5);
const GAP: Pixels = px(2.);
const DISABLED_OPACITY: f32 = 0.46;

pub const ALIGNMENTS: [Alignment; 9] = [
    Alignment::TopLeading,
    Alignment::Top,
    Alignment::TopTrailing,
    Alignment::Leading,
    Alignment::Center,
    Alignment::Trailing,
    Alignment::BottomLeading,
    Alignment::Bottom,
    Alignment::BottomTrailing,
];

fn index_of(alignment: Alignment) -> usize {
    ALIGNMENTS.iter().position(|&a| a == alignment).unwrap_or(4)
}

pub fn alignment_title(alignment: Alignment) -> String {
    match alignment {
        Alignment::TopLeading => fl!("panels-align-top-left"),
        Alignment::Top => fl!("panels-align-top"),
        Alignment::TopTrailing => fl!("panels-align-top-right"),
        Alignment::Leading => fl!("panels-align-left"),
        Alignment::Center => fl!("panels-align-center"),
        Alignment::Trailing => fl!("panels-align-right"),
        Alignment::BottomLeading => fl!("panels-align-bottom-left"),
        Alignment::Bottom => fl!("panels-align-bottom"),
        Alignment::BottomTrailing => fl!("panels-align-bottom-right"),
    }
}

/// Moves within the 3×3 grid and stops at its edges rather than wrapping to the next row.
pub(crate) fn step(alignment: Alignment, key: &str) -> Option<Alignment> {
    let i = index_of(alignment);
    let (row, column) = (i / 3, i % 3);
    let (row, column) = match key {
        "left" => (row, column.saturating_sub(1)),
        "right" => (row, (column + 1).min(2)),
        "up" => (row.saturating_sub(1), column),
        "down" => ((row + 1).min(2), column),
        _ => return None,
    };
    Some(ALIGNMENTS[row * 3 + column])
}

type SelectHandler = Rc<dyn Fn(Alignment, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AlignmentPad {
    id: ElementId,
    label: SharedString,
    help: Option<SharedString>,
    selected: Alignment,
    disabled: bool,
    preview: Option<(Alignment, Interaction)>,
    on_select: Option<SelectHandler>,
}

impl AlignmentPad {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            help: None,
            selected: Alignment::Center,
            disabled: false,
            preview: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, alignment: Alignment) -> Self {
        self.selected = alignment;
        self
    }

    /// A disabled pad shows the centre, as the alignment it falls back to.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The tooltip, such as why the pad is disabled.
    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Pins one cell's hover, or the whole pad's focus ring with `Focused`.
    pub fn preview(mut self, alignment: Alignment, state: Interaction) -> Self {
        self.preview = Some((alignment, state));
        self
    }

    pub fn on_select(
        mut self,
        handler: impl Fn(Alignment, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for AlignmentPad {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let m = t.metrics;
        let focus = keyed_focus(&self.id, window, cx);
        let whole = self.preview.map_or(Interaction::Rest, |(_, s)| s);
        let ring = !self.disabled && shows_focus(&focus, whole, window);
        let shown = if self.disabled {
            Alignment::Center
        } else {
            self.selected
        };
        let live = !self.disabled && self.on_select.is_some();
        let on_select = self.on_select.filter(|_| live);
        let (selected_fill, hover_fill, idle_fill) = (
            t.text.opacity(0.92),
            t.text.opacity(0.48),
            t.text.opacity(0.22),
        );
        let cells = ALIGNMENTS.iter().enumerate().map(|(i, &alignment)| {
            let on = alignment == shown;
            let hovered = self.preview == Some((alignment, Interaction::Hover));
            let fill = match (on, hovered) {
                (true, _) => selected_fill,
                (false, true) => hover_fill,
                _ => idle_fill,
            };
            let on_select = on_select.clone();
            div()
                .id(i)
                .group("cell")
                .flex()
                .items_center()
                .justify_center()
                .size(CELL)
                .accessible(
                    Role::RadioButton,
                    fl!("panels-align-cell", position = alignment_title(alignment)),
                )
                .toggled(on)
                .aria_position_in_set(i + 1)
                .aria_size_of_set(ALIGNMENTS.len())
                .child(
                    div()
                        .size(MARKER)
                        .rounded(MARKER_RADIUS)
                        .bg(fill)
                        .when(live && !on, |d| {
                            d.group_hover("cell", move |s| s.bg(hover_fill))
                        }),
                )
                .when_some(on_select, |d, on_select| {
                    d.on_click(move |_, window, cx| {
                        if !on {
                            on_select(alignment, window, cx);
                        }
                    })
                })
        });
        let selected = self.selected;
        let help = self.help.unwrap_or_else(|| self.label.clone());
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .size(ALIGNMENT_PAD)
            .p(m.control_inset)
            .rounded(m.slider_radius)
            .bg(t.track)
            .accessible(Role::RadioGroup, self.label)
            .tip(help)
            .when(self.disabled, |d| d.opacity(DISABLED_OPACITY))
            .when(!self.disabled, |d| d.track_focus(&focus).tab_index(0))
            .child(
                div()
                    .size_full()
                    .grid()
                    .grid_cols(3)
                    .gap(GAP)
                    .children(cells),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(m.slider_radius)
                    .border(m.hairline)
                    .border_color(t.border),
            )
            .when(ring, |d| d.child(focus_halo(&t, m.slider_radius)))
            .when_some(on_select.clone(), |d, on_select| {
                d.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let Some(next) = step(selected, event.keystroke.key.as_str()) else {
                        return;
                    };
                    cx.stop_propagation();
                    if next != selected {
                        on_select(next, window, cx);
                    }
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_stop_at_the_edges_instead_of_wrapping_rows() {
        assert_eq!(step(Alignment::Leading, "left"), Some(Alignment::Leading));
        assert_eq!(
            step(Alignment::TopTrailing, "right"),
            Some(Alignment::TopTrailing)
        );
        assert_eq!(step(Alignment::Center, "up"), Some(Alignment::Top));
        assert_eq!(step(Alignment::Bottom, "down"), Some(Alignment::Bottom));
        assert_eq!(step(Alignment::Center, "tab"), None);
    }
}
