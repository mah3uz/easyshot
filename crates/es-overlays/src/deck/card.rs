use super::layout::{Slot, ToolLayout, chosen};
use crate::capture::strings::fl;
use es_app::deck::{DeckAction, DeckItem, ItemStatus};
use es_geom::Size;
use es_model::config::CardActions;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::icons::IconName;
use es_ui::motion::{CROSSFADE, DECK_HOVER, since_change, tween};
use es_ui::theme::UI_FONT;
use gpui::{
    AnyElement, App, AppContext as _, BoxShadow, ElementId, ExternalDragPayload, FileDragPaths,
    FocusHandle, FontWeight, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ObjectFit,
    ParentElement as _, Pixels, Render, RenderImage, RenderOnce, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, hsla, img, point,
    prelude::FluentBuilder as _, px,
};
use gpui_component::Icon;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub const RADIUS: Pixels = px(12.);
const STROKE: Pixels = px(0.5);
const SCRIM: f32 = 0.28;
const CORNER_BUTTON: f32 = 22.;
const CORNER_FILL: f32 = 0.45;
const CORNER_RING: f32 = 0.18;
const INSET: f32 = 6.;
const GLYPH: f32 = 10.;
const CAPSULE_FILL: f32 = 0.85;
const CAPSULE_PAD_X: f32 = 8.;
const CAPSULE_PAD_Y: f32 = 3.;

pub fn white(a: f32) -> Hsla {
    hsla(0., 0., 1., a)
}

pub fn black(a: f32) -> Hsla {
    hsla(0., 0., 0., a)
}

pub fn card_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: black(0.25),
        offset: point(px(0.), px(4.)),
        blur_radius: px(10.),
        spread_radius: px(0.),
        inset: false,
    }]
}

pub fn action_label(action: DeckAction) -> String {
    match action {
        DeckAction::Pin => fl!("deck-tool-pin"),
        DeckAction::Dismiss => fl!("deck-tool-dismiss"),
        DeckAction::Copy => fl!("deck-tool-copy"),
        DeckAction::Save => fl!("deck-tool-save"),
        DeckAction::Edit => fl!("deck-tool-edit"),
        DeckAction::Share => fl!("deck-tool-cloud-share"),
        DeckAction::ShowInFolder => fl!("deck-tool-show-in-folder"),
        DeckAction::Preview => fl!("deck-tool-preview"),
        DeckAction::Delete => fl!("deck-tool-delete"),
    }
}

fn action_icon(action: DeckAction) -> IconName {
    match action {
        DeckAction::Pin => IconName::Pin,
        DeckAction::Dismiss => IconName::X,
        DeckAction::Copy => IconName::Copy,
        DeckAction::Save => IconName::Download,
        DeckAction::Edit => IconName::Pencil,
        DeckAction::Share => IconName::CloudUpload,
        DeckAction::ShowInFolder => IconName::FolderOpen,
        DeckAction::Preview => IconName::Eye,
        DeckAction::Delete => IconName::Trash,
    }
}

/// A slot's button within a card of `size`; a centre slot gives the card's middle.
pub fn button_rect(slot: Option<Slot>, size: Size, scale: f32) -> es_geom::Rect {
    let side = f64::from(CORNER_BUTTON * scale);
    let inset = f64::from(INSET * scale);
    let (w, h) = (size.width, size.height);
    let x = match slot {
        Some(Slot::TopLeft | Slot::BottomLeft) => inset,
        Some(Slot::TopRight | Slot::BottomRight) => w - inset - side,
        _ => (w - side) / 2.,
    };
    let y = match slot {
        Some(Slot::TopLeft | Slot::TopRight | Slot::TopCenter) => inset,
        Some(Slot::BottomLeft | Slot::BottomRight | Slot::BottomCenter) => h - inset - side,
        _ => (h - side) / 2.,
    };
    es_geom::Rect::from_origin_size(es_geom::Point::new(x, y), Size::new(side, side))
}

pub type ActionHandler = Rc<dyn Fn(DeckAction, &mut Window, &mut App)>;

/// What a drag out of the deck carries: the file, named after the capture.
#[derive(Clone)]
pub struct DraggedCapture {
    pub file: PathBuf,
    pub image: Option<Arc<RenderImage>>,
}

pub struct DragPreview {
    image: Option<Arc<RenderImage>>,
}

impl Render for DragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let preview = div().w(px(96.)).h(px(68.)).rounded(px(8.)).bg(black(0.9));
        match self.image.clone() {
            Some(image) => preview.child(img(image).size_full().object_fit(ObjectFit::Contain)),
            None => preview,
        }
    }
}

/// One capture in the deck: its thumbnail, and its actions while hovered or focused.
#[derive(IntoElement)]
pub struct Card {
    item: DeckItem,
    size: Size,
    image: Option<Arc<RenderImage>>,
    layout: ToolLayout,
    scale: f32,
    tools: bool,
    share: bool,
    shared: bool,
    actions: CardActions,
    focus: FocusHandle,
    on_action: ActionHandler,
}

impl Card {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        item: DeckItem,
        size: Size,
        image: Option<Arc<RenderImage>>,
        layout: ToolLayout,
        scale: f32,
        focus: FocusHandle,
        on_action: ActionHandler,
    ) -> Self {
        Card {
            item,
            size,
            image,
            layout,
            scale,
            tools: false,
            share: false,
            shared: false,
            actions: CardActions::default(),
            focus,
            on_action,
        }
    }

    pub fn tools(mut self, shown: bool) -> Self {
        self.tools = shown;
        self
    }

    pub fn share(mut self, available: bool) -> Self {
        self.share = available;
        self
    }

    /// The capture has a live share link.
    pub fn shared(mut self, shared: bool) -> Self {
        self.shared = shared;
        self
    }

    pub fn actions(mut self, actions: CardActions) -> Self {
        self.actions = actions;
        self
    }

    /// A recording has no still to preview, and its files are not the card's to trash.
    fn usable(&self, action: DeckAction) -> bool {
        let file_only = matches!(action, DeckAction::Preview | DeckAction::Delete);
        chosen(&self.actions, action)
            && (self.share || action != DeckAction::Share)
            && !(self.item.video && file_only)
    }

    fn id(&self, part: &str) -> ElementId {
        ElementId::Name(SharedString::from(format!("deck-{part}-{}", self.item.id)))
    }

    fn tool(&self, slot: Slot, action: DeckAction) -> AnyElement {
        let action = self.item.offered(action);
        let s = self.scale;
        let handler = self.on_action.clone();
        let label = action_label(action);
        let base = div()
            .id(self.id(&format!("tool-{}", slot.name())))
            .accessible(Role::Button, label.clone())
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                handler(action, window, cx);
            });
        if slot.is_center() {
            let content = match action {
                DeckAction::Copy | DeckAction::Save => div()
                    .font_family(UI_FONT)
                    .text_size(px(GLYPH * s))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(black(0.85))
                    .child(label)
                    .into_any_element(),
                _ => Icon::new(action_icon(action))
                    .size(px(GLYPH * s))
                    .text_color(black(0.85))
                    .into_any_element(),
            };
            return base
                .px(px(CAPSULE_PAD_X * s))
                .py(px(CAPSULE_PAD_Y * s))
                .rounded_full()
                .bg(white(CAPSULE_FILL))
                .child(content)
                .into_any_element();
        }
        let inset = px(INSET * s);
        let positioned = base.absolute();
        let middle = px((self.size.width as f32 - CORNER_BUTTON * s) / 2.);
        let positioned = match slot {
            Slot::TopLeft => positioned.top(inset).left(inset),
            Slot::TopRight => positioned.top(inset).right(inset),
            Slot::TopCenter => positioned.top(inset).left(middle),
            Slot::BottomLeft => positioned.bottom(inset).left(inset),
            Slot::BottomCenter => positioned.bottom(inset).left(middle),
            _ => positioned.bottom(inset).right(inset),
        };
        positioned
            .size(px(CORNER_BUTTON * s))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(black(CORNER_FILL))
            .border(STROKE)
            .border_color(white(CORNER_RING))
            .child(
                Icon::new(action_icon(action))
                    .size(px(GLYPH * s))
                    .text_color(white(1.)),
            )
            .into_any_element()
    }

    fn tool_layer(&self) -> gpui::Div {
        let usable = |a: &DeckAction| self.usable(*a);
        let corners = self
            .layout
            .assignments()
            .filter(|(s, a)| !s.is_center() && usable(a))
            .map(|(s, a)| self.tool(s, a));
        let centre = [Slot::CenterLeft, Slot::CenterRight]
            .into_iter()
            .filter_map(|s| self.layout.get(s).filter(usable).map(|a| self.tool(s, a)));
        div()
            .absolute()
            .inset_0()
            .rounded(RADIUS)
            .bg(black(SCRIM))
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(INSET * self.scale))
                    .children(centre),
            )
            .children(corners)
    }
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let saving = self.item.status == ItemStatus::Saving;
        let loaded = self.image.is_some();
        let loaded_in = since_change(self.id("thumb"), loaded, CROSSFADE, window, cx);
        let saving_in = since_change(self.id("saving"), saving, CROSSFADE, window, cx);
        let saving_shown = if saving { saving_in } else { 1. - saving_in };
        let a11y = if self.item.video {
            fl!("deck-card-a11y-recording")
        } else {
            fl!("deck-card-a11y-screenshot")
        };
        let caption = |text: String| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .font_family(UI_FONT)
                .text_size(px(11.))
                .text_color(white(0.8))
                .child(text)
        };
        let thumbnail = match self.image.clone() {
            Some(image) => div()
                .size_full()
                .opacity(loaded_in)
                .child(img(image).size_full().object_fit(ObjectFit::Contain))
                .into_any_element(),
            None => caption(fl!("deck-card-loading")).into_any_element(),
        };
        let loading_out = (loaded && loaded_in < 1.)
            .then(|| caption(fl!("deck-card-loading")).opacity(1. - loaded_in));
        let target = if self.tools && !saving { 1. } else { 0. };
        let tools_in = tween(self.id("tools"), target, DECK_HOVER, window, cx);
        let tools = (target > 0. || tools_in > 0.).then(|| self.tool_layer().opacity(tools_in));
        let s = self.scale;
        let mark = (self.shared && tools_in < 1.).then(|| {
            div()
                .id(self.id("shared"))
                .accessible(Role::Image, fl!("deck-card-shared-a11y"))
                .absolute()
                .left(px(INSET * s))
                .bottom(px(INSET * s))
                .size(px(CORNER_BUTTON * s * 0.8))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(black(CORNER_FILL))
                .border(STROKE)
                .border_color(white(CORNER_RING))
                .opacity(1. - tools_in)
                .child(
                    Icon::new(IconName::CloudCheck)
                        .size(px(GLYPH * s))
                        .text_color(white(1.)),
                )
        });
        let open = self.on_action.clone();
        let keyed = self.on_action.clone();
        let dragged = DraggedCapture {
            file: self.item.file.clone(),
            image: self.image.clone(),
        };
        div()
            .id(self.id("card"))
            .track_focus(&self.focus)
            .accessible(Role::Button, a11y)
            .relative()
            .w(px(self.size.width as f32))
            .h(px(self.size.height as f32))
            .rounded(RADIUS)
            .overflow_hidden()
            .bg(black(0.9))
            .shadow(card_shadow())
            .when(!saving, |card| {
                card.on_click(move |_, window, cx| open(DeckAction::Edit, window, cx))
                    .on_key_down(move |e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key == "enter" {
                            keyed(DeckAction::Edit, window, cx);
                        }
                    })
                    .on_drag(dragged, |d, _, _, cx| {
                        cx.new(|_| DragPreview {
                            image: d.image.clone(),
                        })
                    })
                    .external_drag_payload(|d: &DraggedCapture, _, _| {
                        let paths = FileDragPaths::new([(d.file.clone(), false)]);
                        let paths = match d.image.clone() {
                            Some(icon) => paths.with_icon(icon),
                            None => paths,
                        };
                        Some(ExternalDragPayload::Files(paths))
                    })
            })
            .child(thumbnail)
            .children(loading_out)
            .children(mark)
            .children(tools)
            .when(saving_shown > 0., |card| {
                card.child(
                    caption(fl!("deck-card-saving"))
                        .rounded(RADIUS)
                        .bg(black(0.45))
                        .opacity(saving_shown),
                )
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(RADIUS)
                    .border(STROKE)
                    .border_color(white(0.2)),
            )
    }
}
