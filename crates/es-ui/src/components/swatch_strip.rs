use std::{f32::consts::TAU, rc::Rc, sync::Arc};

use gpui::{
    Anchor, App, AppContext, Bounds, Context, Div, ElementId, Entity, Focusable, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, PathBuilder, Pixels, RenderOnce,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Subscription, Window, canvas, div,
    hsla, point, prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon,
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
};

use crate::{
    a11y::{Accessible, Role},
    components::{
        button::{Interaction, focus_halo, keyed_focus, shows_focus},
        tile::{PICK_PADDING, arrow_target, pick_cell, pick_edge, pick_face, selection_ring},
    },
    fl,
    theme::tokens,
    tokens::Tokens,
};

pub const SWATCH_COLUMNS: usize = 6;
pub const SWATCH_DIAMETER: Pixels = px(24.);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwatchPick {
    Preset(usize),
    Custom(Hsla),
}

type PickHandler = Rc<dyn Fn(SwatchPick, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SwatchStrip {
    id: ElementId,
    label: SharedString,
    presets: Vec<(Hsla, SharedString)>,
    selected: Option<SwatchPick>,
    preview: Option<(usize, Interaction)>,
    on_select: Option<PickHandler>,
}

impl SwatchStrip {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        presets: impl IntoIterator<Item = (Hsla, SharedString)>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            presets: presets.into_iter().collect(),
            selected: None,
            preview: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, pick: SwatchPick) -> Self {
        self.selected = Some(pick);
        self
    }

    /// The custom well's index is the number of presets; `Focused` pins the group's ring.
    pub fn preview(mut self, index: usize, state: Interaction) -> Self {
        self.preview = Some((index, state));
        self
    }

    /// The picker reports every change while it is dragged, so a stroke recolours live.
    pub fn on_select(
        mut self,
        handler: impl Fn(SwatchPick, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

struct Well {
    picker: Entity<ColorPickerState>,
    on_select: Option<PickHandler>,
    _changes: Subscription,
}

impl Well {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| ColorPickerState::new(window, cx));
        let changes = cx.subscribe_in(&picker, window, |well: &mut Self, _, event, window, cx| {
            let ColorPickerEvent::Change(Some(colour)) = event else {
                return;
            };
            if let Some(on_select) = well.on_select.clone() {
                on_select(SwatchPick::Custom(colour.opacity(1.)), window, cx);
            }
        });
        Self {
            picker,
            on_select: None,
            _changes: changes,
        }
    }
}

fn circle(
    t: &Tokens,
    state: Interaction,
    live: bool,
    face: impl FnOnce(Stateful<Div>) -> Stateful<Div>,
) -> Stateful<Div> {
    face(
        pick_face(state, live)
            .size(SWATCH_DIAMETER)
            .rounded_full()
            .overflow_hidden(),
    )
    .child(pick_edge(t, SWATCH_DIAMETER / 2., state, live))
}

fn rainbow() -> impl IntoElement {
    const SECTORS: usize = 72;
    const STOPS: [f32; 7] = [0., 1. / 6., 1. / 3., 0.5, 2. / 3., 0.77, 1.];
    canvas(
        |_, _, _| (),
        |bounds: Bounds<Pixels>, (), window, _| {
            let centre = bounds.center();
            let r = f32::from(bounds.size.width.min(bounds.size.height)) / 2.;
            let at = |turn: f32| {
                let a = turn * TAU;
                point(centre.x + px(r * a.cos()), centre.y + px(r * a.sin()))
            };
            for i in 0..SECTORS {
                let from = i as f32 / SECTORS as f32;
                // Each wedge overlaps the next, so no seam of background shows between them.
                let to = (i as f32 + 1.6) / SECTORS as f32;
                let mid = (i as f32 + 0.5) / SECTORS as f32;
                let segment = (mid * 6.).floor().min(5.) as usize;
                let local = mid * 6. - segment as f32;
                let hue = STOPS[segment] + (STOPS[segment + 1] - STOPS[segment]) * local;
                let mut path = PathBuilder::fill();
                path.move_to(centre);
                path.line_to(at(from));
                path.line_to(at((from + to) / 2.));
                path.line_to(at(to));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, hsla(hue.fract(), 1., 0.5, 1.));
                }
            }
        },
    )
    .size_full()
}

impl RenderOnce for SwatchStrip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let focus = keyed_focus(&self.id, window, cx);
        let well = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "well".into()),
            cx,
            Well::new,
        );
        let on_select = self.on_select.clone();
        well.update(cx, |well, _| well.on_select = on_select);
        let picker = well.read(cx).picker.clone();
        let picker_focus = picker.read(cx).focus_handle(cx);

        let count = self.presets.len();
        let selected = self.selected;
        let current = match selected {
            Some(SwatchPick::Preset(i)) => Some(i),
            Some(SwatchPick::Custom(_)) => Some(count),
            None => None,
        };
        let state = |i: usize| match self.preview {
            Some((p, s)) if p == i => s,
            _ => Interaction::Rest,
        };
        let whole = self.preview.map_or(Interaction::Rest, |(_, s)| s);
        let ring = shows_focus(&focus, whole, window);
        let ring_at = current.unwrap_or(0);
        let outer = SWATCH_DIAMETER / 2. + PICK_PADDING;
        let interactive = self.on_select.is_some();

        let presets: Vec<_> = self
            .presets
            .into_iter()
            .enumerate()
            .map(|(i, (colour, name))| {
                let on = current == Some(i);
                let live = interactive && state(i) == Interaction::Rest;
                let on_select = self.on_select.clone();
                pick_cell(ElementId::Integer(i as u64))
                    .rounded_full()
                    .accessible(Role::RadioButton, name)
                    .toggled(on)
                    .aria_position_in_set(i + 1)
                    .aria_size_of_set(count + 1)
                    .child(circle(&t, state(i), live, |d| d.bg(colour)))
                    .when(on, |d| d.child(selection_ring(&t, outer)))
                    .when(ring && ring_at == i, |d| d.child(focus_halo(&t, outer)))
                    .when_some(on_select, |d, on_select| {
                        d.on_click(move |_, window, cx| {
                            if !on {
                                on_select(SwatchPick::Preset(i), window, cx);
                            }
                        })
                    })
                    .into_any_element()
            })
            .collect();

        let custom = match selected {
            Some(SwatchPick::Custom(colour)) => Some(colour),
            _ => None,
        };
        let well_ring = ring && ring_at == count
            || picker_focus.is_focused(window) && window.last_input_was_keyboard();
        let custom_label = fl!("picks-swatch-custom");
        let well = pick_cell("custom".into())
            .rounded_full()
            .accessible(Role::RadioButton, custom_label.clone())
            .toggled(custom.is_some())
            .aria_position_in_set(count + 1)
            .aria_size_of_set(count + 1)
            .child(circle(&t, state(count), interactive, |d| match custom {
                Some(colour) => d.bg(colour),
                None => d.child(rainbow()),
            }))
            .when(custom.is_some(), |d| d.child(selection_ring(&t, outer)))
            .when(well_ring, |d| d.child(focus_halo(&t, outer)))
            .when(interactive, |d| {
                d.child(
                    ColorPicker::new(&picker)
                        .icon(Icon::empty().size(SWATCH_DIAMETER + PICK_PADDING * 2.))
                        .accessibility_label(fl!("picks-swatch-custom-choose"))
                        .anchor(Anchor::TopRight)
                        .absolute()
                        .inset_0(),
                )
            });

        let cells = presets
            .into_iter()
            .chain([well.into_any_element()])
            .map(|cell| div().flex().justify_center().child(cell));

        let navigable = count + usize::from(custom.is_some());
        div()
            .id(self.id)
            .w_full()
            .grid()
            .grid_cols(SWATCH_COLUMNS as u16)
            .gap(t.metrics.row_spacing)
            .accessible(Role::RadioGroup, self.label)
            .track_focus(&focus)
            .tab_index(0)
            .when_some(self.on_select, |d, on_select| {
                d.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    let Some(next) = arrow_target(key, current, navigable, SWATCH_COLUMNS) else {
                        return;
                    };
                    cx.stop_propagation();
                    if Some(next) != current && next < count {
                        on_select(SwatchPick::Preset(next), window, cx);
                    }
                })
            })
            .children(cells)
    }
}
