use es_ui::{
    a11y::{Accessible, Role},
    components::{EditorButton, IconButton, ZoomCapsule},
    icons::IconName,
    theme::tokens,
    tokens::Typeset,
};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, div,
    prelude::FluentBuilder, px,
};

use crate::strings::fl;
use crate::window::{CHROME_PADDING_X, EditorWindow, FOOTER_HEIGHT, Job, Progress, busy, done};

impl EditorWindow {
    pub(crate) fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let ready = self.session().is_some();
        let low_res = self.low_res && ready;
        let expanded = self.low_res_expanded;
        let this = cx.entity().downgrade();
        div()
            .id("footer")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(FOOTER_HEIGHT)
            .px(CHROME_PADDING_X)
            .bg(t.window)
            .border_t(t.metrics.hairline)
            .border_color(t.border)
            .when(ready, |d| {
                d.child(ZoomCapsule::new("zoom", self.zoom_percent(cx)).on_command(
                    move |command, _, cx| {
                        let _ = this.update(cx, |w, cx| w.zoom(command, cx));
                    },
                ))
            })
            .when_some(self.redact_status(&t), |d, status| d.child(status))
            .when(low_res, |d| {
                d.child(
                    IconButton::new("low-res", IconName::Info, fl!("editor-lowres-help")).on_click(
                        cx.listener(|w, _, _, cx| {
                            w.low_res_expanded = !w.low_res_expanded;
                            cx.notify();
                        }),
                    ),
                )
                .when(expanded, |d| {
                    d.child(
                        div()
                            .id("low-res-text")
                            .text_color(t.text_secondary)
                            .typeset(t.type_scale.label)
                            .accessible(Role::Note, fl!("editor-lowres-text"))
                            .child(fl!("editor-lowres-text")),
                    )
                })
            })
            .child(div().flex_1())
            .child(self.crop_badge(cx).unwrap_or_else(|| self.copy_control(cx)))
    }

    fn copy_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = tokens(cx).clone();
        match self.progress(Job::Copy) {
            Progress::Busy => busy("copy", fl!("editor-copying-a11y"), false, &t),
            Progress::Done => done("copy", fl!("editor-copied"), &t),
            Progress::Idle => EditorButton::new("copy")
                .icon(IconName::Copy)
                .label(fl!("editor-copy"))
                .tooltip(fl!("editor-copy-help-plain"))
                .disabled(!self.can_run(Job::Copy, cx))
                .on_click(cx.listener(|w, _, _, cx| {
                    w.request(Job::Copy, cx);
                }))
                .into_any_element(),
        }
    }
}
