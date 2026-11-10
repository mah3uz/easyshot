use super::{BASE_HEIGHT, GAP, Menu, RecordingOptions, same_mic};
use crate::capture::strings::fl;
use crate::record::groups::window_groups;
use es_model::platform::OutputId;
use es_ui::components::menu::{
    MENU_INSET, MENU_ROW, MENU_SECTION, MENU_SEPARATOR, menu, menu_separator,
};
use es_ui::components::{MenuItem, MenuLead, MenuNote, MenuSection};
use es_ui::icons::IconName;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, point, px,
};
use std::path::Path;
use std::sync::Arc;

const SCREEN_SHARE: f64 = 0.6;
const MIN_MENU_ROWS: f64 = 4.0;
const BORDER: f64 = 2.0;

#[derive(Clone)]
pub(super) enum Pick {
    Display(OutputId),
    Window(usize),
    Refresh,
    Mic(Option<String>),
}

pub(super) struct Item {
    label: String,
    detail: Option<String>,
    a11y: String,
    lead: MenuLead,
    pick: Pick,
}

pub(super) enum Row {
    Section(String),
    Note(String),
    Separator,
    Item(Item),
}

impl Row {
    fn height(&self) -> f64 {
        f64::from(match self {
            Row::Section(_) => MENU_SECTION,
            Row::Note(_) | Row::Item(_) => MENU_ROW,
            Row::Separator => MENU_SEPARATOR,
        })
    }

    fn label(&self) -> Option<&str> {
        match self {
            Row::Section(text) | Row::Note(text) => Some(text),
            Row::Item(item) => Some(&item.a11y),
            Row::Separator => None,
        }
    }
}

fn item(label: String, lead: MenuLead, pick: Pick) -> Row {
    Row::Item(Item {
        a11y: label.clone(),
        label,
        detail: None,
        lead,
        pick,
    })
}

/// The title, then the app's name unless the title is only that name.
pub fn window_text(title: &str, app: &str) -> (String, Option<String>) {
    let (title, app) = (title.trim(), app.trim());
    match (title.is_empty(), app.is_empty()) {
        (true, true) => (fl!("rec-window-untitled"), None),
        (true, false) => (app.to_owned(), None),
        (false, true) => (title.to_owned(), None),
        (false, false) if title.eq_ignore_ascii_case(app) => (title.to_owned(), None),
        (false, false) => (title.to_owned(), Some(app.to_owned())),
    }
}

impl RecordingOptions {
    pub(super) fn rows(&self, menu: Menu) -> Vec<Row> {
        match menu {
            Menu::Display => self
                .outputs
                .iter()
                .enumerate()
                .map(|(n, o)| {
                    let number = n + 1;
                    let name = if o.description.is_empty() {
                        fl!("rec-display-fallback", n = number)
                    } else {
                        o.description.clone()
                    };
                    let (w, h) = o.physical;
                    Row::Item(Item {
                        a11y: fl!("rec-display-item", name = name.clone(), w = w, h = h),
                        detail: Some(fl!("rec-display-size", w = w, h = h)),
                        label: name,
                        lead: MenuLead::Glyph(IconName::Monitor),
                        pick: Pick::Display(o.id.clone()),
                    })
                })
                .collect(),
            Menu::Window => {
                let mut rows = Vec::new();
                for group in window_groups(&self.windows) {
                    if let Some(name) = group.workspace {
                        rows.push(Row::Section(if group.shown {
                            fl!("rec-windows-workspace-shown", name = name)
                        } else {
                            fl!("rec-windows-workspace", name = name)
                        }));
                    }
                    for i in group.windows {
                        let w = &self.windows[i];
                        let app = self.apps.get(&w.app_id);
                        let name = app.map_or("", |a| a.name.as_str());
                        let (label, detail) = window_text(&w.title, name);
                        let a11y = match &detail {
                            Some(app) => {
                                fl!("rec-window-a11y", title = label.clone(), app = app.clone())
                            }
                            None => label.clone(),
                        };
                        let icon = app.and_then(|a| a.icon.as_deref()).map(Arc::<Path>::from);
                        rows.push(Row::Item(Item {
                            label,
                            detail,
                            a11y,
                            lead: MenuLead::App(icon),
                            pick: Pick::Window(i),
                        }));
                    }
                }
                if rows.is_empty() {
                    rows.push(Row::Note(fl!("rec-windows-empty")));
                }
                rows.push(Row::Separator);
                rows.push(item(
                    fl!("rec-windows-refresh"),
                    MenuLead::Glyph(IconName::RefreshCw),
                    Pick::Refresh,
                ));
                rows
            }
            Menu::Mic => std::iter::once(item(
                fl!("rec-mic-off"),
                MenuLead::Check(self.mic.is_none()),
                Pick::Mic(None),
            ))
            .chain(self.mics.iter().map(|d| {
                let on = self.mic.as_deref().is_some_and(|m| same_mic(&d.name, m));
                item(
                    d.description.clone(),
                    MenuLead::Check(on),
                    Pick::Mic(Some(d.name.clone())),
                )
            }))
            .collect(),
        }
    }

    /// What the open menu's rows say to a screen reader, top to bottom.
    pub fn menu_labels(&self) -> Vec<String> {
        self.menu.map_or_else(Vec::new, |m| {
            self.rows(m)
                .iter()
                .filter_map(|r| r.label().map(str::to_owned))
                .collect()
        })
    }

    pub(super) fn menu_height(&self, rows: &[Row]) -> f64 {
        let screen = self
            .outputs
            .iter()
            .map(|o| o.logical.height())
            .fold(f64::INFINITY, f64::min);
        let inset = 2.0 * f64::from(MENU_INSET) + BORDER;
        let floor = MIN_MENU_ROWS * f64::from(MENU_ROW) + inset;
        let room = (screen * SCREEN_SHARE - BASE_HEIGHT - GAP).max(floor);
        (rows.iter().map(Row::height).sum::<f64>() + inset).min(room)
    }

    pub fn menu_offset(&self) -> f64 {
        -f64::from(self.scroll.offset().y)
    }

    /// Up and Down move through the open menu's choices, Page Up, Page Down, Home and End jump,
    /// Enter picks; false when the key is none of them or no menu is open.
    pub fn menu_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(menu) = self.menu else {
            return false;
        };
        let rows = self.rows(menu);
        if key == "enter" {
            let Some(Row::Item(item)) = self.active.and_then(|i| rows.get(i)) else {
                return false;
            };
            self.pick(item.pick.clone(), cx);
            return true;
        }
        let items: Vec<usize> = (0..rows.len())
            .filter(|&i| matches!(rows[i], Row::Item(_)))
            .collect();
        let Some(last) = items.len().checked_sub(1) else {
            return false;
        };
        let at = self.active.and_then(|a| items.iter().position(|&i| i == a));
        let page = ((self.menu_height(&rows) / f64::from(MENU_ROW)) as usize).max(2) - 1;
        let to = match key {
            "down" => at.map_or(0, |a| (a + 1).min(last)),
            "up" => at.map_or(last, |a| a.saturating_sub(1)),
            "pagedown" => at.map_or(0, |a| (a + page).min(last)),
            "pageup" => at.map_or(0, |a| a.saturating_sub(page)),
            "home" => 0,
            "end" => last,
            _ => return false,
        };
        let row = items[to];
        self.active = Some(row);
        let from = match row.checked_sub(1).map(|i| &rows[i]) {
            _ if to == 0 => 0,
            Some(Row::Section(_)) => row - 1,
            _ => row,
        };
        self.reveal(&rows, from, row);
        cx.notify();
        true
    }

    /// Scrolls the least that shows rows `from` through `to`.
    fn reveal(&mut self, rows: &[Row], from: usize, to: usize) {
        let top_of = |i: usize| rows[..i].iter().map(Row::height).sum::<f64>();
        let inset = f64::from(MENU_INSET);
        let top = top_of(from);
        let bottom = top_of(to) + rows[to].height() + 2.0 * inset;
        let view = self.menu_height(rows) - BORDER;
        let now = self.menu_offset();
        let wanted = if top < now {
            top
        } else if bottom > now + view {
            bottom - view
        } else {
            now
        };
        let content = top_of(rows.len()) + 2.0 * inset;
        let at = wanted.clamp(0.0, (content - view).max(0.0));
        self.scroll
            .set_offset(point(self.scroll.offset().x, px(-(at as f32))));
    }

    fn hover(&mut self, row: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.active = Some(row);
        } else if self.active == Some(row) {
            self.active = None;
        }
        cx.notify();
    }

    pub(super) fn render_menu(&self, which: Menu, cx: &mut Context<Self>) -> AnyElement {
        let label = match which {
            Menu::Display => fl!("rec-a11y-display-multi"),
            Menu::Window => fl!("rec-a11y-window"),
            Menu::Mic => fl!("rec-a11y-mic-off"),
        };
        let rows = self.rows(which);
        let mut list = menu("rec-menu", label, cx)
            .max_h(px(self.menu_height(&rows) as f32))
            .overflow_y_scroll()
            .track_scroll(&self.scroll);
        for (i, row) in rows.into_iter().enumerate() {
            list = match row {
                Row::Section(text) => list.child(MenuSection::new(("rec-menu-row", i), text)),
                Row::Note(text) => list.child(MenuNote::new(("rec-menu-row", i), text)),
                Row::Separator => list.child(menu_separator(cx)),
                Row::Item(item) => {
                    let pick = item.pick;
                    list.child(
                        MenuItem::new(("rec-menu-row", i), item.label, item.lead)
                            .detail(item.detail)
                            .a11y(item.a11y)
                            .active(self.active == Some(i))
                            .on_hover(
                                cx.listener(move |o, hovered, _, cx| o.hover(i, *hovered, cx)),
                            )
                            .on_click(cx.listener(move |o, _, _, cx| o.pick(pick.clone(), cx))),
                    )
                }
            };
        }
        list.into_any_element()
    }
}
