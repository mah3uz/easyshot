use super::{Confirm, ControlBar, RADIUS};
use crate::capture::strings::fl;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

impl ControlBar {
    pub(super) fn render_confirm(
        &mut self,
        confirm: Confirm,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = tokens(cx).clone();
        let busy = self.phase.busy();
        let (title, message, action) = match confirm {
            Confirm::Restart => (
                fl!("rec-confirm-restart-title"),
                fl!("rec-confirm-restart-message"),
                fl!("rec-confirm-restart"),
            ),
            Confirm::Discard => (
                fl!("rec-confirm-discard-title"),
                fl!("rec-confirm-discard-message"),
                fl!("rec-confirm-discard"),
            ),
        };
        let button = |id: &'static str, text: String, fill: Hsla, ink: Hsla| {
            div()
                .id(id)
                .accessible(Role::Button, text.clone())
                .flex()
                .items_center()
                .justify_center()
                .w(px(88.))
                .h(px(26.))
                .rounded(px(7.))
                .bg(fill)
                .text_color(ink)
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .hover(move |s| s.bg(fill.opacity(fill.a * 0.85)))
                .child(text)
        };
        div()
            .id("rec-confirm")
            .accessible(Role::AlertDialog, title.clone())
            .flex()
            .flex_col()
            .justify_between()
            .size_full()
            .p(px(14.))
            .rounded(px(RADIUS))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.text_secondary)
                            .child(message),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .when(busy, |d| d.opacity(0.5))
                    .child(
                        button(
                            "rec-confirm-cancel",
                            fl!("rec-confirm-cancel"),
                            t.fill,
                            t.text,
                        )
                        .on_click(cx.listener(|bar, _, _, cx| bar.cancel(cx))),
                    )
                    .child(
                        button("rec-confirm-ok", action, t.destructive, t.on_accent)
                            .on_click(cx.listener(move |bar, _, _, cx| bar.confirmed(confirm, cx))),
                    ),
            )
            .into_any_element()
    }
}
