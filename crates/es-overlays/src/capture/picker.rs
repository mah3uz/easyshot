use super::frozen::{Tile, tiles};
use super::paint::{self, black, white};
use super::strings::fl;
use super::window::{PickEvent, describe};
use super::windows::Candidate;
use es_geom::Rect;
use es_model::platform::Frame;
use es_ui::a11y::{Accessible as _, Role};
use gpui::{
    App, Context, CursorStyle, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollHandle,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, prelude::*, px,
};
use std::sync::Arc;

const DIM: f32 = 0.3;
const PANEL_WIDTH: f32 = 420.;
const ROW_HEIGHT: f32 = 30.;
const VISIBLE_ROWS: f32 = 10.;

/// The list of windows, for compositors that report windows but not where they are.
pub struct WindowPicker {
    tiles: Arc<[Tile]>,
    bounds: Rect,
    /// None on the outputs that only show the frozen screen behind the list.
    items: Option<Arc<[Candidate]>>,
    query: String,
    /// Indices into `items` that match the query, in list order.
    shown: Vec<usize>,
    /// An index into `shown`.
    selected: usize,
    scroll: ScrollHandle,
    focus: FocusHandle,
    focused: bool,
}

impl EventEmitter<PickEvent> for WindowPicker {}

impl Focusable for WindowPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl WindowPicker {
    pub fn new(frame: &Frame, items: Option<Vec<Candidate>>, cx: &mut Context<Self>) -> Self {
        let shown = (0..items.as_ref().map_or(0, Vec::len)).collect();
        WindowPicker {
            tiles: tiles(frame).into(),
            bounds: Rect::from_origin_size((0.0, 0.0), frame.logical.size()),
            items: items.map(Into::into),
            query: String::new(),
            shown,
            selected: 0,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            focused: false,
        }
    }

    pub fn selected(&self) -> Option<&Candidate> {
        self.items.as_ref()?.get(*self.shown.get(self.selected)?)
    }

    fn len(&self) -> usize {
        self.shown.len()
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        let needle = query.to_lowercase();
        self.shown = self.items.as_ref().map_or_else(Vec::new, |items| {
            (0..items.len())
                .filter(|&i| {
                    let c = &items[i];
                    c.app.to_lowercase().contains(&needle)
                        || c.title.to_lowercase().contains(&needle)
                })
                .collect()
        });
        self.query = query;
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        cx.notify();
    }

    fn typed(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let m = &e.keystroke.modifiers;
        if m.control || m.alt || m.platform || m.function {
            return;
        }
        let key = e.keystroke.key.as_str();
        // Some keyboards and synthetic input leave `key_char` empty for plain keys.
        let plain = match key {
            "space" => Some(" "),
            k if k.chars().count() == 1 => Some(k),
            _ => None,
        };
        if let Some(text) = e
            .keystroke
            .key_char
            .as_deref()
            .or(plain)
            .filter(|t| !t.is_empty() && !t.chars().any(char::is_control))
        {
            self.set_query(format!("{}{text}", self.query), cx);
        }
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.len() {
            self.selected = index;
            self.scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.select(index, cx);
        if let Some(c) = self.selected() {
            cx.emit(PickEvent::Window {
                id: c.id.clone(),
                local: None,
            });
        }
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let last = self.len().saturating_sub(1);
        let filtering = !self.query.is_empty();
        match e.keystroke.key.as_str() {
            "escape" if filtering => self.set_query(String::new(), cx),
            "escape" => cx.emit(PickEvent::Cancel),
            "space" if !filtering => cx.emit(PickEvent::Region),
            "backspace" => {
                let mut query = self.query.clone();
                query.pop();
                self.set_query(query, cx);
            }
            "up" => self.select(self.selected.saturating_sub(1), cx),
            "down" => self.select((self.selected + 1).min(last), cx),
            "home" => self.select(0, cx),
            "end" => self.select(last, cx),
            "enter" | "kp_enter" => self.choose(self.selected, cx),
            _ => self.typed(e, cx),
        }
    }

    fn panel(&self, items: &[Candidate], cx: &mut Context<Self>) -> impl IntoElement {
        let accent = paint::accent(cx);
        let rows = self.shown.iter().enumerate().map(|(i, &item)| {
            let c = &items[item];
            let chosen = i == self.selected;
            div()
                .id(("window-row", i))
                .accessible(Role::Button, describe(c))
                .h(px(ROW_HEIGHT))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .cursor(CursorStyle::PointingHand)
                .text_color(white(if chosen { 1. } else { 0.86 }))
                .when(chosen, |d| d.bg(accent))
                .when(!chosen, |d| d.hover(|d| d.bg(white(0.08))))
                .on_click(cx.listener(move |this, _, _, cx| this.choose(i, cx)))
                .child(
                    div()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(describe(c)),
                )
        });
        let list = if self.shown.is_empty() {
            div()
                .py(px(16.))
                .text_center()
                .text_color(white(0.6))
                .child(if items.is_empty() {
                    fl!("window-picker-empty")
                } else {
                    fl!("window-picker-no-match")
                })
                .into_any_element()
        } else {
            div()
                .id("window-rows")
                .flex()
                .flex_col()
                .gap(px(2.))
                .max_h(px(ROW_HEIGHT * VISIBLE_ROWS + 2. * (VISIBLE_ROWS - 1.)))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .children(rows)
                .into_any_element()
        };
        div()
            .w(px(PANEL_WIDTH))
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .rounded(px(12.))
            .bg(black(0.78))
            .border_1()
            .border_color(white(0.12))
            .text_size(px(13.))
            .child(
                div()
                    .px(px(10.))
                    .pt(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white(1.))
                    .child(if self.query.is_empty() {
                        fl!("window-picker-title")
                    } else {
                        fl!("window-picker-filter", query = self.query.as_str())
                    }),
            )
            .child(list)
            .child(
                div()
                    .px(px(10.))
                    .pb(px(2.))
                    .text_size(px(11.))
                    .text_color(white(0.6))
                    .child(fl!("window-picker-hint")),
            )
    }
}

impl Render for WindowPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let (tiles, all) = (self.tiles.clone(), self.bounds);
        let (w, h) = (all.width() as f32, all.height() as f32);
        let items = self.items.clone();
        div()
            .id("window-picker")
            .accessible(Role::Dialog, fl!("window-picker-title"))
            .track_focus(&self.focus)
            .relative()
            .w(px(w))
            .h(px(h))
            .on_key_down(cx.listener(|this, e, _, cx| this.key(e, cx)))
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        paint::tiles(&tiles, window);
                        paint::dim(all, DIM, window);
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(items.map(|items| {
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(self.panel(&items, cx))
            }))
    }
}
