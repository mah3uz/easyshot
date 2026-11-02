use super::cells::{self, MenuButton};
use super::model::{self, Category, SortOrder, Step, ViewMode};
use super::view::Gallery;
use crate::strings::fl;
use es_ui::{
    a11y::{Accessible, Role},
    components::{Segment, Segmented},
    icons::IconName,
    theme::{UI_FONT, tokens},
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
    uniform_list,
};
use gpui_component::{
    Icon, Sizable,
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};

pub const SIDEBAR_WIDTH: f32 = 210.;
const TOOLBAR_HEIGHT: f32 = 52.;
const SEARCH_WIDTH: f32 = 200.;
const SIDEBAR_ROW: f32 = 28.;

pub fn category_title(category: Category) -> String {
    match category {
        Category::All => fl!("gallery-all-media"),
        Category::Screenshots => fl!("gallery-screenshots"),
        Category::Videos => fl!("gallery-videos"),
    }
}

fn category_icon(category: Category) -> IconName {
    match category {
        Category::All => IconName::Folder,
        Category::Screenshots => IconName::Image,
        Category::Videos => IconName::Film,
    }
}

fn sort_title(sort: SortOrder) -> String {
    match sort {
        SortOrder::Name => fl!("gallery-sort-name"),
        SortOrder::Kind => fl!("gallery-sort-kind"),
        SortOrder::NewestFirst => fl!("gallery-sort-newest"),
        SortOrder::OldestFirst => fl!("gallery-sort-oldest"),
    }
}

fn step_for(key: &str) -> Option<Step> {
    Some(match key {
        "left" => Step::Left,
        "right" => Step::Right,
        "up" => Step::Up,
        "down" => Step::Down,
        "home" => Step::First,
        "end" => Step::Last,
        _ => return None,
    })
}

impl Gallery {
    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        if let Some(step) = step_for(key) {
            self.step(step, cx);
            cx.stop_propagation();
            return;
        }
        let Some(id) = self.selected else { return };
        match key {
            "enter" | "space" => self.open(id, cx),
            "delete" => self.confirm_trash(id, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn sidebar(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let rows = Category::ALL.into_iter().map(|category| {
            let title = category_title(category);
            let on = self.category == category;
            let a11y = fl!("gallery-sidebar-a11y-local", title = title.clone());
            div()
                .id(SharedString::from(format!("category-{category:?}")))
                .accessible(Role::Tab, a11y)
                .aria_selected(on)
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(SIDEBAR_ROW))
                .px(px(8.))
                .rounded(t.metrics.button_radius)
                .text_size(px(13.))
                .text_color(t.text)
                .when(on, |d| d.bg(t.hover_fill))
                .when(!on, |d| d.hover(|s| s.bg(t.bordered_fill)))
                .child(
                    Icon::new(category_icon(category))
                        .size(px(15.))
                        .text_color(t.accent),
                )
                .child(div().flex_1().truncate().child(title))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_secondary)
                        .child(self.count(category).to_string()),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.set_category(category, cx)))
        });
        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(t.inspector)
            .border_r_1()
            .border_color(t.border)
            .px(px(10.))
            .pt(px(16.))
            .gap(px(2.))
            .child(
                div()
                    .px(px(8.))
                    .pb(px(6.))
                    .typeset(t.type_scale.section_header)
                    .text_color(t.text_secondary)
                    .child(fl!("gallery-section-local")),
            )
            .children(rows)
            .into_any_element()
    }

    fn toolbar(&self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity().downgrade();
        let sort_view = view.clone();
        let current = self.sort;
        let switcher = Segmented::new(
            "view-mode",
            fl!("gallery-view-help"),
            [
                Segment::icon(IconName::Grid2x2, fl!("gallery-view-icons")),
                Segment::icon(IconName::List, fl!("gallery-view-list")),
            ],
        )
        .selected(match self.view {
            ViewMode::Icons => 0,
            ViewMode::List => 1,
        })
        .height(px(26.))
        .on_select(move |index, _, cx| {
            let mode = if index == 0 {
                ViewMode::Icons
            } else {
                ViewMode::List
            };
            let _ = view.update(cx, |this, cx| this.set_view(mode, cx));
        });
        let sort = MenuButton::new("sort", IconName::ArrowUpDown, fl!("gallery-sort-help"))
            .dropdown_menu(move |mut menu, _, _| {
                for order in SortOrder::ALL {
                    let view = sort_view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(sort_title(order))
                            .checked(order == current)
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| this.set_sort(order, cx));
                            }),
                    );
                }
                menu
            });
        let refresh = es_ui::components::IconButton::new(
            "refresh",
            IconName::RefreshCw,
            fl!("gallery-refresh-help"),
        )
        .size(px(28.), px(15.))
        .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)));
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(TOOLBAR_HEIGHT))
            .px(px(16.))
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_1()
                    .truncate()
                    .typeset(t.type_scale.title)
                    .text_size(px(15.))
                    .text_color(t.text)
                    .child(category_title(self.category)),
            )
            .child(div().flex_none().w(px(76.)).child(switcher))
            .child(sort)
            .child(refresh)
            .child(
                div()
                    .w(px(SEARCH_WIDTH))
                    .child(Input::new(&self.search).small().cleanable(true)),
            )
            .into_any_element()
    }

    fn status(&self, t: &Tokens) -> impl IntoElement {
        let selected = self
            .selected
            .and_then(|id| self.item(id))
            .map(|i| i.name.clone());
        let crumb = |text: String| div().truncate().child(text);
        let chevron = || {
            Icon::new(IconName::ChevronRight)
                .size(px(10.))
                .text_color(t.text_tertiary)
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .px(px(16.))
            .py(px(9.))
            .border_t_1()
            .border_color(t.border)
            .bg(t.inspector)
            .text_size(px(11.))
            .text_color(t.text_secondary)
            .child(Icon::new(IconName::HardDrive).size(px(12.)))
            .child(crumb(fl!("gallery-section-local")))
            .child(chevron())
            .child(crumb(category_title(self.category)))
            .when_some(selected, |d, name| d.child(chevron()).child(crumb(name)))
            .child(div().flex_1())
            .child(fl!("gallery-status-count", count = self.shown.len()))
    }

    fn banner(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let message = self.error.clone()?;
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.))
                .mx(px(16.))
                .mt(px(12.))
                .px(px(12.))
                .py(px(8.))
                .rounded(t.metrics.tile_radius)
                .bg(t.destructive.opacity(0.12))
                .border_1()
                .border_color(t.destructive.opacity(0.4))
                .text_size(px(12.))
                .text_color(t.text)
                .child(
                    Icon::new(IconName::CircleAlert)
                        .size(px(14.))
                        .text_color(t.destructive),
                )
                .child(div().flex_1().child(message))
                .child(
                    div()
                        .id("dismiss-error")
                        .accessible(Role::Button, fl!("gallery-banner-dismiss"))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(t.metrics.button_radius)
                        .bg(t.bordered_fill)
                        .hover(|s| s.bg(t.hover_fill))
                        .child(fl!("gallery-banner-dismiss"))
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))),
                )
                .into_any_element(),
        )
    }

    fn content(&mut self, t: &Tokens, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.shown.is_empty() {
            return cells::empty_state(t, self.query.trim().is_empty()).into_any_element();
        }
        match self.view {
            ViewMode::Icons => {
                let width = window.viewport_size().width.as_f32() - SIDEBAR_WIDTH;
                self.set_columns(model::columns_for(width));
                let rows = self.shown.len().div_ceil(self.columns);
                uniform_list(
                    "gallery-grid",
                    rows,
                    cx.processor(|this, range: std::ops::Range<usize>, window, cx| {
                        let columns = this.columns;
                        this.request_thumbs(range.start * columns..range.end * columns, cx);
                        range
                            .map(|row| cells::grid_row(this, row, window, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(&self.grid_scroll)
                .size_full()
                .px(px(model::GRID_PADDING))
                .py(px(model::GRID_PADDING))
                .into_any_element()
            }
            ViewMode::List => div()
                .flex()
                .flex_col()
                .size_full()
                .child(cells::list_header(self, t, cx))
                .child(
                    uniform_list(
                        "gallery-list",
                        self.shown.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, window, cx| {
                            this.request_thumbs(range.clone(), cx);
                            range
                                .map(|index| cells::list_row(this, index, window, cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .track_scroll(&self.list_scroll)
                    .flex_1()
                    .px(px(8.)),
                )
                .into_any_element(),
        }
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let banner = self.banner(&t, cx);
        let content = self.content(&t, window, cx);
        let sidebar = self.sidebar(&t, cx);
        let toolbar = self.toolbar(&t, cx);
        div()
            .size_full()
            .flex()
            .font_family(UI_FONT)
            .bg(t.window)
            .text_color(t.text)
            .child(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(toolbar)
                    .children(banner)
                    .child(
                        div()
                            .id("gallery-content")
                            .accessible(Role::Grid, category_title(self.category))
                            .track_focus(&self.focus)
                            .on_key_down(cx.listener(Self::on_key))
                            .flex_1()
                            .min_h_0()
                            .child(content),
                    )
                    .child(self.status(&t)),
            )
    }
}
