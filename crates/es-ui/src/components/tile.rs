use std::rc::Rc;

use gpui::{
    AnyElement, App, Background, ClickEvent, Div, ElementId, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Pixels, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Window, div, prelude::FluentBuilder, px,
};

use crate::{
    a11y::{Accessible, Role},
    components::button::{ClickHandler, Interaction, focus_halo, keyed_focus, shows_focus},
    theme::tokens,
    tokens::Tokens,
};

pub const PICK_PADDING: Pixels = px(2.5);
pub const PICK_RING: Pixels = px(2.);

const GROUP: &str = "pick";

pub(crate) type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

pub(crate) fn selection_ring(t: &Tokens, radius: Pixels) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .border(PICK_RING)
        .border_color(t.accent)
}

pub(crate) fn pick_edge(t: &Tokens, radius: Pixels, state: Interaction, live: bool) -> Div {
    let hover = t.hover_border;
    div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .border(t.metrics.hairline)
        .border_color(match state {
            Interaction::Hover | Interaction::Pressed => hover,
            _ => t.border,
        })
        .when(live, |d| {
            d.group_hover(GROUP, move |s| s.border_color(hover))
        })
}

pub(crate) fn pick_face(state: Interaction, live: bool) -> gpui::Stateful<Div> {
    div()
        .id("face")
        .relative()
        .when(state == Interaction::Pressed, |d| d.opacity(0.8))
        .when(live, |d| d.group_active(GROUP, |s| s.opacity(0.8)))
}

pub(crate) fn pick_cell(id: ElementId) -> gpui::Stateful<Div> {
    div().id(id).group(GROUP).relative().p(PICK_PADDING)
}

pub(crate) fn arrow_target(
    key: &str,
    current: Option<usize>,
    count: usize,
    columns: usize,
) -> Option<usize> {
    let columns = columns.max(1) as isize;
    let delta = match key {
        "left" => -1,
        "right" => 1,
        "up" => -columns,
        "down" => columns,
        _ => return None,
    };
    let Some(current) = current else {
        return (count > 0).then_some(0);
    };
    Some(
        current
            .checked_add_signed(delta)
            .filter(|&n| n < count)
            .unwrap_or(current),
    )
}

#[derive(IntoElement)]
pub struct Tile {
    id: ElementId,
    label: SharedString,
    aspect: f32,
    fill: Option<Background>,
    content: Option<AnyElement>,
    selected: bool,
    preview: Interaction,
    position: Option<(usize, usize)>,
    focus_ring: bool,
    on_click: Option<ClickHandler>,
}

impl Tile {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            aspect: 1.,
            fill: None,
            content: None,
            selected: false,
            preview: Interaction::Rest,
            position: None,
            focus_ring: false,
            on_click: None,
        }
    }

    pub fn aspect(mut self, aspect: f32) -> Self {
        self.aspect = aspect;
        self
    }

    pub fn fill(mut self, fill: impl Into<Background>) -> Self {
        self.fill = Some(fill.into());
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Tile {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let radius = t.metrics.tile_radius;
        let outer = radius + PICK_PADDING;
        let live = self.preview == Interaction::Rest && self.on_click.is_some();
        let ring = self.focus_ring || self.preview == Interaction::Focused;
        pick_cell(self.id)
            .w_full()
            .accessible(Role::RadioButton, self.label)
            .toggled(self.selected)
            .when_some(self.position, |d, (i, count)| {
                d.aria_position_in_set(i + 1).aria_size_of_set(count)
            })
            .child(
                pick_face(self.preview, live)
                    .w_full()
                    .aspect_ratio(self.aspect)
                    .rounded(radius)
                    .overflow_hidden()
                    .when_some(self.fill, |d, fill| d.bg(fill))
                    .children(self.content)
                    .child(pick_edge(&t, radius, self.preview, live)),
            )
            .when(self.selected, |d| d.child(selection_ring(&t, outer)))
            .when(ring, |d| d.child(focus_halo(&t, outer)))
            .when_some(self.on_click, |d, on_click| {
                d.on_click(move |event, window, cx| on_click(event, window, cx))
            })
    }
}

#[derive(IntoElement)]
pub struct TileGrid {
    id: ElementId,
    label: SharedString,
    tiles: Vec<Tile>,
    columns: u16,
    spacing: Pixels,
    selected: Option<usize>,
    preview: Interaction,
    on_select: Option<SelectHandler>,
}

impl TileGrid {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        tiles: impl IntoIterator<Item = Tile>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tiles: tiles.into_iter().collect(),
            columns: 8,
            spacing: px(6.),
            selected: None,
            preview: Interaction::Rest,
            on_select: None,
        }
    }

    pub fn columns(mut self, columns: u16) -> Self {
        self.columns = columns.max(1);
        self
    }

    pub fn spacing(mut self, spacing: Pixels) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// `Focused` pins the keyboard focus ring.
    pub fn preview(mut self, state: Interaction) -> Self {
        self.preview = state;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for TileGrid {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = keyed_focus(&self.id, window, cx);
        let ring = shows_focus(&focus, self.preview, window);
        let count = self.tiles.len();
        let selected = self.selected;
        let columns = usize::from(self.columns);
        let focused = selected.unwrap_or(0);
        let on_select = self.on_select.clone();
        let tiles = self.tiles.into_iter().enumerate().map(|(i, mut tile)| {
            tile.selected = selected == Some(i);
            tile.position = Some((i, count));
            tile.focus_ring = ring && i == focused;
            match on_select.clone() {
                Some(on_select) => tile.on_click(move |_, window, cx| {
                    if selected != Some(i) {
                        on_select(i, window, cx);
                    }
                }),
                None => tile,
            }
        });
        div()
            .id(self.id)
            .w_full()
            .grid()
            .grid_cols(self.columns)
            .gap(self.spacing)
            .accessible(Role::RadioGroup, self.label)
            .track_focus(&focus)
            .tab_index(0)
            .when_some(self.on_select, |d, on_select| {
                d.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let key = event.keystroke.key.as_str();
                    let Some(next) = arrow_target(key, selected, count, columns) else {
                        return;
                    };
                    cx.stop_propagation();
                    if Some(next) != selected {
                        on_select(next, window, cx);
                    }
                })
            })
            .children(tiles)
    }
}
