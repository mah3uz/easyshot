use crate::session::io::Choice;
use crate::strings::fl;
use crate::window::{EditorWindow, Job};
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton},
    tokens::{Tokens, Typeset},
};
use gpui::{
    AnyElement, App, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, Task, Window, black, div, px,
};

impl EditorWindow {
    /// True when the window may close now. Otherwise a guard decides and closes the window itself if it agrees.
    pub fn should_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.session() else {
            return true;
        };
        let (dirty, saving) = {
            let io = session.io.read(cx);
            (io.is_dirty(cx), io.is_saving())
        };
        if !dirty && !saving {
            return true;
        }
        let decision = self.request_close(cx);
        let handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            if decision.await {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        })
        .detach();
        false
    }

    /// The one path every close goes through: resolves `true` when the window may close.
    pub fn request_close(&mut self, cx: &mut Context<Self>) -> Task<bool> {
        let Some(session) = self.session() else {
            return Task::ready(true);
        };
        let io = session.io.clone();
        cx.spawn(async move |this, cx| {
            let saving = io.update(cx, |io, _| io.wait_for_save());
            if let Some(done) = saving {
                return done.await.unwrap_or(false);
            }
            if !io.read_with(cx, |io, cx| io.is_dirty(cx)) {
                return true;
            }
            let prompt = io.update(cx, |io, cx| io.open_prompt(cx));
            match prompt.await.unwrap_or(Choice::Cancel) {
                Choice::Cancel => false,
                Choice::Discard => {
                    io.update(cx, |io, cx| io.drop_journal(cx));
                    true
                }
                Choice::Save => {
                    let started = this
                        .update(cx, |w, cx| w.request(Job::Save, cx))
                        .unwrap_or(false);
                    if !started {
                        return false;
                    }
                    match io.update(cx, |io, _| io.wait_for_save()) {
                        Some(done) => done.await.unwrap_or(false),
                        None => false,
                    }
                }
            }
        })
    }

    pub(crate) fn close_dialog(&self, t: &Tokens, cx: &mut Context<Self>) -> Option<AnyElement> {
        let session = self.session()?;
        let io = session.io.clone();
        if !io.read(cx).prompt_open() {
            return None;
        }
        let name: SharedString = session
            .doc
            .read(cx)
            .display()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| fl!("editor-close-guard-this-screenshot"))
            .into();
        let message = fl!("editor-close-guard-message", name = name.to_string());
        let button = |id: &'static str, label: String, choice: Choice, kind: ButtonKind| {
            let io: Entity<_> = io.clone();
            EditorButton::new(id)
                .kind(kind)
                .label(label)
                .on_click(move |_, _, cx: &mut App| {
                    io.update(cx, |io, cx| io.choose(choice, cx));
                })
        };
        Some(
            div()
                .id("close-guard")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().opacity(0.4))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("close-guard-panel")
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .w(px(420.))
                        .p(px(20.))
                        .rounded(t.metrics.popover_radius)
                        .bg(t.window)
                        .border(t.metrics.hairline)
                        .border_color(t.border)
                        .text_color(t.text)
                        .accessible(Role::AlertDialog, fl!("editor-close-guard-a11y"))
                        .child(
                            div()
                                .typeset(t.type_scale.label)
                                .child(SharedString::from(message)),
                        )
                        .child(
                            div()
                                .text_color(t.text_secondary)
                                .typeset(t.type_scale.label)
                                .child(fl!("editor-close-guard-info")),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(button(
                                    "guard-cancel",
                                    fl!("editor-close-guard-cancel"),
                                    Choice::Cancel,
                                    ButtonKind::Bordered,
                                ))
                                .child(button(
                                    "guard-discard",
                                    fl!("editor-close-guard-discard"),
                                    Choice::Discard,
                                    ButtonKind::Bordered,
                                ))
                                .child(button(
                                    "guard-save",
                                    fl!("editor-close-guard-save"),
                                    Choice::Save,
                                    ButtonKind::Prominent,
                                )),
                        ),
                )
                .into_any_element(),
        )
    }
}
