use gpui::{
    App, AppContext, ClickEvent, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, Pixels, Render, SharedString,
    Styled, Window, actions, div, prelude::FluentBuilder, px,
};
use gpui_component::{
    Icon, Sizable,
    input::{Input, InputState},
};

use crate::{
    a11y::{Accessible, Role},
    components::{
        Segment, Segmented,
        button::{ButtonKind, EditorButton},
    },
    fl,
    icons::IconName,
    theme::tokens,
    tokens::{Tokens, Typeset},
};

pub const SHARE_POPOVER_WIDTH: Pixels = px(280.);
pub const SHARE_DAYS: [u8; 3] = [1, 3, 7];
const KEY_CONTEXT: &str = "SharePopover";
const GROUP_GAP: Pixels = px(12.);
const BODY_LEADING: Pixels = px(18.);

actions!(share_popover, [Submit, Dismiss]);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Submit, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareView {
    Ready,
    /// A video longer than the destination takes even at 720p; `message` says how long fits.
    TooLong {
        message: SharedString,
    },
    /// `wait` replaces Retry with a disabled button reading it, such as "Try again in 3 min".
    Failed {
        message: SharedString,
        wait: Option<SharedString>,
    },
}

/// The popover never adopts a choice: the caller answers with the matching setter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareEvent {
    Share { title: Option<String> },
    Days(u8),
    SetUpBucket,
    Retry,
    Close,
}

/// The one share surface; the caller puts it in a [`super::Popover`] of [`SHARE_POPOVER_WIDTH`]
/// and passes this entity's focus handle to it, so Enter and Esc work as it opens.
pub struct SharePopover {
    focus: FocusHandle,
    field: Entity<InputState>,
    view: ShareView,
    days: u8,
    offer_days: bool,
    info: Option<SharedString>,
    busy: bool,
}

impl EventEmitter<ShareEvent> for SharePopover {}

impl Focusable for SharePopover {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SharePopover {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field =
            cx.new(|cx| InputState::new(window, cx).placeholder(fl!("share-title-placeholder")));
        Self {
            focus: cx.focus_handle(),
            field,
            view: ShareView::Ready,
            days: SHARE_DAYS[SHARE_DAYS.len() - 1],
            offer_days: true,
            info: None,
            busy: false,
        }
    }

    pub fn set_view(&mut self, view: ShareView, cx: &mut Context<Self>) {
        self.view = view;
        cx.notify();
    }

    /// Shares to the user's own bucket do not expire, so the lifetime is not asked.
    pub fn set_offer_days(&mut self, offer: bool, cx: &mut Context<Self>) {
        self.offer_days = offer;
        cx.notify();
    }

    /// One of [`SHARE_DAYS`].
    pub fn set_days(&mut self, days: u8, cx: &mut Context<Self>) {
        self.days = days;
        cx.notify();
    }

    /// What a video will be sent as, such as "1080p, about 38 MB".
    pub fn set_info(&mut self, info: Option<SharedString>, cx: &mut Context<Self>) {
        self.info = info;
        cx.notify();
    }

    /// While the caller prepares the file: Share waits with a spinner.
    pub fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        cx.notify();
    }

    pub fn view(&self) -> &ShareView {
        &self.view
    }

    pub fn title(&self, cx: &App) -> Option<String> {
        let title = self.field.read(cx).value().trim().to_owned();
        (!title.is_empty()).then_some(title)
    }

    fn share(&mut self, cx: &mut Context<Self>) {
        if self.view == ShareView::Ready && !self.busy {
            let title = self.title(cx);
            cx.emit(ShareEvent::Share { title });
        }
    }

    // Enter on a button or link activates that control instead.
    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        let typing = self.field.read(cx).focus_handle(cx).is_focused(window);
        if typing || self.focus.is_focused(window) {
            self.share(cx);
        } else {
            cx.propagate();
        }
    }

    // Propagates so an enclosing popover closes as well.
    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(ShareEvent::Close);
        cx.propagate();
    }

    fn emit(
        &self,
        event: ShareEvent,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |_, _: &ClickEvent, _, cx| cx.emit(event.clone()))
    }

    fn ready(&self, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let days = self.offer_days;
        let selected = SHARE_DAYS.iter().position(|&d| d == self.days).unwrap_or(0);
        let this = cx.entity().downgrade();
        let expiry = Segmented::new(
            "share-days",
            fl!("share-expiry"),
            SHARE_DAYS.map(|days| Segment::text(fl!("share-days", days = days))),
        )
        .selected(selected)
        .height(px(28.))
        .glide()
        .on_select(move |i, _, cx| {
            let _ = this.update(cx, |_, cx| cx.emit(ShareEvent::Days(SHARE_DAYS[i])));
        });
        let share = cx.listener(|this, _: &ClickEvent, _, cx| this.share(cx));
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .typeset(t.type_scale.footer)
                    .text_color(t.text_secondary)
                    .when_some(self.info.clone(), |d, info| {
                        d.child(glyph(IconName::Film, t))
                            .child(div().min_w_0().truncate().child(info))
                    }),
            )
            .child(
                EditorButton::new("share-go")
                    .label(fl!("share-button"))
                    .when(self.busy, |b| {
                        b.icon(IconName::CloudUpload)
                            .accessible_label(fl!("share-preparing-a11y"))
                    })
                    .kind(ButtonKind::Prominent)
                    .busy(self.busy)
                    .disabled(self.busy)
                    .padding_x(px(14.))
                    .on_click(share),
            );
        div()
            .flex()
            .flex_col()
            .gap(GROUP_GAP)
            .child(heading(fl!("share-title"), None, t))
            .child(
                Input::new(&self.field)
                    .small()
                    .typeset(t.type_scale.footer)
                    .aria_label(fl!("share-title-field")),
            )
            .when(days, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(t.metrics.group_label_spacing)
                        .child(caption(fl!("share-expiry"), t))
                        .child(expiry),
                )
            })
            .child(footer)
    }

    fn too_long(&self, message: &SharedString, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let bucket = EditorButton::new("share-use-bucket")
            .label(fl!("share-use-bucket"))
            .kind(ButtonKind::Prominent)
            .on_click(self.emit(ShareEvent::SetUpBucket, cx));
        div()
            .flex()
            .flex_col()
            .gap(GROUP_GAP)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(heading(fl!("share-too-long-title"), None, t))
                    .child(body(message.clone(), t)),
            )
            .child(actions_row().child(bucket))
    }

    fn failed(
        &self,
        message: &SharedString,
        wait: Option<&SharedString>,
        t: &Tokens,
        cx: &mut Context<Self>,
    ) -> Div {
        let retry = EditorButton::new("share-retry")
            .label(wait.cloned().unwrap_or_else(|| fl!("share-retry").into()))
            .kind(ButtonKind::Prominent)
            .disabled(wait.is_some())
            .on_click(self.emit(ShareEvent::Retry, cx));
        div()
            .flex()
            .flex_col()
            .gap(GROUP_GAP)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(heading(
                        fl!("share-error-title"),
                        Some((IconName::CircleAlert, t.destructive)),
                        t,
                    ))
                    .child(body(message.clone(), t)),
            )
            .child(actions_row().child(retry))
    }
}

impl Render for SharePopover {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let view = self.view.clone();
        let content = match &view {
            ShareView::Ready => self.ready(&t, cx),
            ShareView::TooLong { message } => self.too_long(message, &t, cx),
            ShareView::Failed { message, wait } => self.failed(message, wait.as_ref(), &t, cx),
        };
        div()
            .id("share-popover")
            .accessible(Role::Group, fl!("share-a11y"))
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::dismiss))
            .text_color(t.text)
            .child(content)
    }
}

fn heading(text: String, icon: Option<(IconName, gpui::Hsla)>, t: &Tokens) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .typeset(t.type_scale.title)
        .when_some(icon, |d, (icon, colour)| {
            d.child(Icon::new(icon).size(px(15.)).text_color(colour))
        })
        .child(text)
}

fn caption(text: String, t: &Tokens) -> Div {
    div()
        .typeset(t.type_scale.label)
        .text_color(t.text_secondary)
        .child(text)
}

fn body(text: SharedString, t: &Tokens) -> Div {
    div()
        .typeset(t.type_scale.footer)
        .line_height(BODY_LEADING)
        .text_color(t.text_secondary)
        .child(text)
}

fn glyph(icon: IconName, t: &Tokens) -> Icon {
    Icon::new(icon)
        .size(px(14.))
        .flex_none()
        .text_color(t.text_secondary)
}

fn actions_row() -> Div {
    div().flex().justify_end().gap(px(8.))
}
