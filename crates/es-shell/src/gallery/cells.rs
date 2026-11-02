use super::model::{Item, MediaKind, SortOrder};
use super::thumbs::Thumb;
use super::view::Gallery;
use crate::strings::fl;
use es_ui::{
    a11y::{Accessible, Role},
    icons::IconName,
    theme::tokens,
    tokens::Tokens,
};
use gpui::{
    AnyElement, App, ClickEvent, Context, Div, ElementId, InteractiveElement, Interactivity,
    IntoElement, ObjectFit, ParentElement, RenderOnce, SharedString, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, StyledImage, WeakEntity, Window, div,
    hsla, img, prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon, Selectable,
    menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem},
    tooltip::Tooltip,
};
use uuid::Uuid;

const ART: f32 = 64.;
const ART_PAD: f32 = 10.;
const TITLE_HEIGHT: f32 = 36.;
pub const GRID_ROW_HEIGHT: f32 =
    ART + 2. * ART_PAD + 4. + TITLE_HEIGHT + super::model::GRID_SPACING_Y;
const LIST_ROW: f32 = 30.;
const LIST_ART: f32 = 24.;
const DATE_COLUMN: f32 = 140.;
const KIND_COLUMN: f32 = 96.;

/// A borderless icon button that opens a menu; the menu keeps its open state through `Selectable`.
#[derive(IntoElement)]
pub struct MenuButton {
    base: Stateful<Div>,
    icon: IconName,
    open: bool,
}

impl MenuButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName, label: impl Into<SharedString>) -> Self {
        let label: SharedString = label.into();
        let tip = label.clone();
        let base = div()
            .id(id)
            .accessible(Role::Button, label)
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx));
        Self {
            base,
            icon,
            open: false,
        }
    }
}

impl Styled for MenuButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuButton {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl Selectable for MenuButton {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl DropdownMenu for MenuButton {}

impl RenderOnce for MenuButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx);
        let (hover, glyph, hover_glyph) = (t.hover_fill, t.text_secondary, t.text);
        self.base
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(28.))
            .rounded_full()
            .text_color(glyph)
            .aria_expanded(self.open)
            .when(self.open, |d| d.bg(hover).text_color(hover_glyph))
            .hover(move |s| s.bg(hover).text_color(hover_glyph))
            .child(Icon::new(self.icon).size(px(15.)))
    }
}

pub fn kind_title(kind: MediaKind) -> String {
    match kind {
        MediaKind::Screenshot => fl!("gallery-kind-screenshot"),
        MediaKind::Video => fl!("gallery-kind-video"),
    }
}

fn item_menu(
    mut menu: PopupMenu,
    view: &WeakEntity<Gallery>,
    id: Uuid,
    kind: MediaKind,
) -> PopupMenu {
    let act = |label: String,
               icon: IconName,
               f: fn(&mut Gallery, Uuid, &mut Window, &mut Context<Gallery>)| {
        let view = view.clone();
        PopupMenuItem::new(label)
            .icon(Icon::new(icon))
            .on_click(move |_, window, cx| {
                let _ = view.update(cx, |this, cx| f(this, id, window, cx));
            })
    };
    menu = menu
        .item(act(
            fl!("gallery-menu-open"),
            IconName::SquareArrowOutUpRight,
            |g, id, _, cx| g.open(id, cx),
        ))
        .item(
            act(
                fl!("gallery-menu-edit"),
                IconName::Pencil,
                |g, id, _, cx| g.edit(id, cx),
            )
            .disabled(kind != MediaKind::Screenshot),
        )
        .item(act(
            fl!("gallery-menu-reveal"),
            IconName::FolderOpen,
            |g, id, _, cx| g.show_in_folder(id, cx),
        ))
        .separator()
        .item(act(
            fl!("gallery-menu-trash"),
            IconName::Trash,
            |g, id, window, cx| g.confirm_trash(id, window, cx),
        ));
    menu
}

fn click(
    view: &WeakEntity<Gallery>,
    id: Uuid,
    event: &ClickEvent,
    window: &mut Window,
    cx: &mut App,
) {
    let _ = view.update(cx, |this, cx| {
        this.select(Some(id), cx);
        this.focus.focus(window, cx);
        if event.click_count() >= 2 {
            this.open(id, cx);
        }
    });
}

fn art(t: &Tokens, item: &Item, thumb: Option<&Thumb>, size: f32, badge: f32) -> AnyElement {
    let placeholder = || {
        let icon = match item.kind {
            MediaKind::Screenshot => IconName::Image,
            MediaKind::Video => IconName::Video,
        };
        Icon::new(icon)
            .size(px(size * 0.6))
            .text_color(t.text_tertiary)
            .into_any_element()
    };
    let picture = match thumb {
        Some(Thumb::Ready(image)) => img(image.clone())
            .size_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        _ => placeholder(),
    };
    div()
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .size(px(size))
        .child(picture)
        .when(item.kind == MediaKind::Video, |d| {
            d.child(
                div()
                    .absolute()
                    .right(px(2.))
                    .bottom(px(2.))
                    .rounded_full()
                    .bg(hsla(0., 0., 0., 0.7))
                    .text_color(hsla(0., 0., 1., 1.))
                    .child(Icon::new(IconName::CirclePlay).size(px(badge))),
            )
        })
        .into_any_element()
}

fn cell(gallery: &Gallery, item: &Item, cx: &mut Context<Gallery>) -> AnyElement {
    let t = tokens(cx).clone();
    let id = item.id;
    let on = gallery.selected == Some(id);
    let view = cx.entity().downgrade();
    let menu_view = view.clone();
    let button_view = view.clone();
    let kind = item.kind;
    let help = fl!("gallery-item-help", title = item.name.clone());
    let actions = MenuButton::new(
        SharedString::from(format!("actions-{id}")),
        IconName::CircleEllipsis,
        fl!("gallery-menu-a11y", title = item.name.clone()),
    )
    .size(px(20.))
    .dropdown_menu(move |menu, _, _| item_menu(menu, &button_view, id, kind));
    div()
        .id(SharedString::from(format!("cell-{id}")))
        .accessible(Role::GridCell, item.name.clone())
        .aria_selected(on)
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.))
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .tooltip(move |window, cx| Tooltip::new(help.clone()).build(window, cx))
        .child(
            div()
                .relative()
                .p(px(ART_PAD))
                .rounded(px(8.))
                .when(on, |d| d.bg(t.hover_fill))
                .child(art(&t, item, gallery.thumbs.get(&item.file), ART, 18.))
                .when(on, |d| {
                    d.child(div().absolute().top(px(3.)).right(px(3.)).child(actions))
                }),
        )
        .child(
            div()
                .h(px(TITLE_HEIGHT))
                .w_full()
                .flex()
                .justify_center()
                .child(
                    div()
                        .max_w_full()
                        .px(px(4.))
                        .rounded(px(4.))
                        .text_size(px(13.))
                        .line_height(px(17.))
                        .text_center()
                        .line_clamp(2)
                        .text_ellipsis()
                        .text_color(if on { t.on_accent } else { t.text })
                        .when(on, |d| d.bg(t.accent))
                        .child(item.name.clone()),
                ),
        )
        .on_click(move |event, window, cx| click(&view, id, event, window, cx))
        .context_menu(move |menu, _, _| item_menu(menu, &menu_view, id, kind))
        .into_any_element()
}

pub fn grid_row(
    gallery: &Gallery,
    row: usize,
    _window: &mut Window,
    cx: &mut Context<Gallery>,
) -> AnyElement {
    let columns = gallery.columns;
    let start = row * columns;
    let end = (start + columns).min(gallery.shown.len());
    let cells: Vec<AnyElement> = gallery.shown[start..end]
        .iter()
        .map(|item| cell(gallery, item, cx))
        .collect();
    let spacers = columns - (end - start);
    div()
        .flex()
        .items_start()
        .gap(px(super::model::GRID_SPACING_X))
        .w_full()
        .h(px(GRID_ROW_HEIGHT))
        .children(cells)
        .children((0..spacers).map(|_| div().flex_1().min_w_0()))
        .into_any_element()
}

fn sort_header(
    gallery: &Gallery,
    t: &Tokens,
    id: &'static str,
    label: String,
    next: SortOrder,
    cx: &mut Context<Gallery>,
) -> Stateful<Div> {
    let on = match next {
        SortOrder::NewestFirst | SortOrder::OldestFirst => {
            matches!(
                gallery.sort,
                SortOrder::NewestFirst | SortOrder::OldestFirst
            )
        }
        order => gallery.sort == order,
    };
    let next = if next == SortOrder::NewestFirst && gallery.sort == SortOrder::NewestFirst {
        SortOrder::OldestFirst
    } else {
        next
    };
    div()
        .id(id)
        .accessible(Role::ColumnHeader, label.clone())
        .px(px(6.))
        .truncate()
        .when(on, |d| d.text_color(t.text))
        .hover(|s| s.text_color(t.text))
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| this.set_sort(next, cx)))
}

pub fn list_header(gallery: &Gallery, t: &Tokens, cx: &mut Context<Gallery>) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .mx(px(8.))
        .pl(px(LIST_ART + 14.))
        .border_b_1()
        .border_color(t.border)
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(t.text_secondary)
        .child(
            sort_header(
                gallery,
                t,
                "column-name",
                fl!("gallery-column-name"),
                SortOrder::Name,
                cx,
            )
            .flex_1()
            .min_w(px(160.)),
        )
        .child(
            sort_header(
                gallery,
                t,
                "column-date",
                fl!("gallery-column-date"),
                SortOrder::NewestFirst,
                cx,
            )
            .w(px(DATE_COLUMN)),
        )
        .child(
            sort_header(
                gallery,
                t,
                "column-kind",
                fl!("gallery-column-kind"),
                SortOrder::Kind,
                cx,
            )
            .w(px(KIND_COLUMN)),
        )
}

pub fn list_row(
    gallery: &Gallery,
    index: usize,
    _window: &mut Window,
    cx: &mut Context<Gallery>,
) -> AnyElement {
    let t = tokens(cx).clone();
    let Some(item) = gallery.shown.get(index) else {
        return div().into_any_element();
    };
    let id = item.id;
    let on = gallery.selected == Some(id);
    let view = cx.entity().downgrade();
    let menu_view = view.clone();
    let kind = item.kind;
    let date = es_store::datefmt::format(item.created_at, &fl!("gallery-date-pattern"));
    let full = es_store::datefmt::format(item.created_at, &fl!("gallery-date-full-pattern"));
    let (text, secondary) = if on {
        (t.on_accent, t.on_accent)
    } else {
        (t.text, t.text_secondary)
    };
    div()
        .id(SharedString::from(format!("row-{id}")))
        .accessible(Role::Row, item.name.clone())
        .aria_selected(on)
        .flex()
        .items_center()
        .gap(px(8.))
        .w_full()
        .h(px(LIST_ROW))
        .px(px(6.))
        .rounded(px(5.))
        .text_size(px(13.))
        .when(on, |d| d.bg(t.accent))
        .when(!on && index % 2 == 1, |d| d.bg(t.bordered_fill))
        .child(art(&t, item, gallery.thumbs.get(&item.file), LIST_ART, 12.))
        .child(
            div()
                .flex_1()
                .min_w(px(160.))
                .px(px(6.))
                .truncate()
                .text_color(text)
                .child(item.name.clone()),
        )
        .child(
            div()
                .id(SharedString::from(format!("date-{id}")))
                .w(px(DATE_COLUMN))
                .px(px(6.))
                .truncate()
                .text_color(secondary)
                .tooltip(move |window, cx| Tooltip::new(full.clone()).build(window, cx))
                .child(date),
        )
        .child(
            div()
                .w(px(KIND_COLUMN))
                .px(px(6.))
                .truncate()
                .text_color(secondary)
                .child(kind_title(kind)),
        )
        .on_click(move |event, window, cx| click(&view, id, event, window, cx))
        .context_menu(move |menu, _, _| item_menu(menu, &menu_view, id, kind))
        .into_any_element()
}

pub fn empty_state(t: &Tokens, no_search: bool) -> impl IntoElement {
    let (title, body) = if no_search {
        (
            fl!("gallery-empty-local-title"),
            fl!("gallery-empty-local-body"),
        )
    } else {
        (
            fl!("gallery-empty-search-title"),
            fl!("gallery-empty-search-body"),
        )
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .child(
            Icon::new(IconName::Images)
                .size(px(40.))
                .text_color(t.text_tertiary),
        )
        .child(
            div()
                .pt(px(6.))
                .text_size(px(15.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(t.text_secondary)
                .child(body),
        )
}
