use crate::capture::strings::fl;
use crate::countdown::Sleep;
use es_app::agent_access::{Answer, ChangeRequest, ConsentPrompt, ConsentRequest, Loosening};
use es_app::platform_ui::{LayerShellSurfaces, SurfaceHandle, SurfaceRole};
use es_geom::Size;
use es_protocol::agent::GrantSpan;
use es_ui::a11y::{Accessible as _, Role};
use es_ui::components::{ButtonKind, EditorButton};
use es_ui::icons::IconName;
use es_ui::theme::{UI_FONT, tokens};
use gpui::{
    App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Task, Window, div, px,
};
use gpui_component::{Icon, h_flex, v_flex};
use std::rc::Rc;
use std::time::Duration;

pub const WIDTH: f64 = 500.;
const HEIGHT: f64 = 196.;
const RADIUS: f32 = 16.;
const BADGE: f32 = 30.;
/// An unanswered dialog denies and closes after this long, so a request never waits for ever.
pub const ANSWER_WAIT: Duration = Duration::from_secs(60);

/// What the dialog asks about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    Access(ConsentRequest),
    Change(ChangeRequest),
}

/// One button: its element id, its label and the answer it gives.
pub struct Choice {
    pub id: &'static str,
    pub label: String,
    pub answer: Answer,
}

pub struct ConsentDialog {
    ask: Ask,
    focus: FocusHandle,
    focused: bool,
    answered: bool,
    _expiry: Task<()>,
}

impl EventEmitter<Answer> for ConsentDialog {}

impl Focusable for ConsentDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ConsentDialog {
    pub fn new(ask: Ask, cx: &mut Context<Self>) -> Self {
        Self::with_sleep(
            ask,
            &(Rc::new(|d, cx: &mut App| cx.background_executor().timer(d)) as Sleep),
            cx,
        )
    }

    pub fn with_sleep(ask: Ask, sleep: &Sleep, cx: &mut Context<Self>) -> Self {
        let wait = sleep(ANSWER_WAIT, cx);
        let expiry = cx.spawn(async move |this, cx| {
            wait.await;
            let _ = this.update(cx, |this, cx| this.answer(Answer::Deny, cx));
        });
        ConsentDialog {
            ask,
            focus: cx.focus_handle(),
            focused: false,
            answered: false,
            _expiry: expiry,
        }
    }

    pub fn size() -> Size {
        Size::new(WIDTH, HEIGHT)
    }

    /// Left to right, which is also the Tab order: refusing comes first.
    pub fn choices(&self) -> Vec<Choice> {
        let choice = |id, label, answer| Choice { id, label, answer };
        match &self.ask {
            Ask::Access(r) if r.remember => vec![
                choice("consent-deny", fl!("consent-deny"), Answer::Deny),
                choice(
                    "consent-hour",
                    fl!("consent-allow-hour"),
                    Answer::Allow(GrantSpan::Hour),
                ),
                choice(
                    "consent-session",
                    fl!("consent-allow-session"),
                    Answer::Allow(GrantSpan::Session),
                ),
                choice(
                    "consent-always",
                    fl!("consent-allow-always"),
                    Answer::Allow(GrantSpan::Always),
                ),
            ],
            Ask::Access(_) => vec![
                choice("consent-deny", fl!("consent-deny"), Answer::Deny),
                choice("consent-once", fl!("consent-allow-once"), Answer::Once),
            ],
            Ask::Change(_) => vec![
                choice("consent-cancel", fl!("consent-cancel"), Answer::Deny),
                choice("consent-allow", fl!("consent-allow"), Answer::Once),
            ],
        }
    }

    pub fn title(&self) -> String {
        match &self.ask {
            Ask::Access(_) => fl!("consent-title"),
            Ask::Change(r) => match r.change {
                Loosening::TurnOn => fl!("consent-change-on-title"),
                Loosening::AlwaysAllow => fl!("consent-change-always-title"),
                Loosening::WiderScope => fl!("consent-change-scope-title"),
                Loosening::ShowSecrets => fl!("consent-change-secrets-title"),
                Loosening::ShorterLog => fl!("consent-change-log-title"),
            },
        }
    }

    pub fn answer(&mut self, answer: Answer, cx: &mut Context<Self>) {
        if !self.answered {
            self.answered = true;
            cx.emit(answer);
        }
    }

    fn key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        if e.keystroke.key == "escape" {
            self.answer(Answer::Deny, cx);
        }
    }

    fn lines(&self) -> (String, String, String) {
        let program = |pid: Option<u32>, comm: &Option<String>| match (comm, pid) {
            (Some(comm), Some(pid)) => fl!("consent-program", program = comm.clone(), pid = pid),
            (Some(comm), None) => comm.clone(),
            (None, Some(pid)) => fl!("consent-program-unnamed", pid = pid),
            (None, None) => fl!("consent-program-unknown"),
        };
        match &self.ask {
            Ask::Access(r) => {
                let wants = match (r.route, &r.target) {
                    ("agent/list-outputs", _) => fl!("consent-wants-outputs"),
                    ("agent/list-windows", _) => fl!("consent-wants-windows"),
                    ("agent/capture", Some(target)) => {
                        fl!("consent-wants-capture", target = target.clone())
                    }
                    (route, Some(target)) => fl!(
                        "consent-wants-route-target",
                        route = route,
                        target = target.clone()
                    ),
                    (route, None) => fl!("consent-wants-route", route = route),
                };
                let note = if r.remember {
                    fl!("consent-note-remember")
                } else {
                    fl!("consent-note-once")
                };
                (program(r.pid, &r.comm), wants, note)
            }
            Ask::Change(r) => {
                let wants = match r.change {
                    Loosening::TurnOn => fl!("consent-change-on"),
                    Loosening::AlwaysAllow => fl!("consent-change-always"),
                    Loosening::WiderScope => fl!("consent-change-scope"),
                    Loosening::ShowSecrets => fl!("consent-change-secrets"),
                    Loosening::ShorterLog => fl!("consent-change-log"),
                };
                (program(r.pid, &r.comm), wants, fl!("consent-change-note"))
            }
        }
    }
}

impl Render for ConsentDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        let t = tokens(cx).clone();
        let title = self.title();
        let (program, wants, note) = self.lines();
        let mut buttons = h_flex().gap(px(8.));
        for (i, choice) in self.choices().into_iter().enumerate() {
            let answer = choice.answer;
            if i == 1 {
                buttons = buttons.child(div().flex_1());
            }
            buttons = buttons.child(
                EditorButton::new(choice.id)
                    .label(choice.label)
                    .kind(ButtonKind::Bordered)
                    .on_click(cx.listener(move |this, _, _, cx| this.answer(answer, cx))),
            );
        }
        v_flex()
            .id("consent")
            .accessible(Role::AlertDialog, title.clone())
            .aria_description(format!("{program} {wants}"))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e, _, cx| this.key(e, cx)))
            .size_full()
            .justify_between()
            .p(px(18.))
            .rounded(px(RADIUS))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .text_color(t.text)
            .child(
                h_flex()
                    .items_start()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(BADGE))
                            .rounded_full()
                            .bg(t.accent_tint)
                            .child(
                                Icon::new(IconName::KeyRound)
                                    .size(px(16.))
                                    .text_color(t.accent),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(program))
                                    .child(div().child(wants)),
                            )
                            .child(
                                div()
                                    .pt(px(2.))
                                    .text_size(px(12.))
                                    .text_color(t.text_secondary)
                                    .child(note),
                            ),
                    ),
            )
            .child(buttons)
    }
}

/// The daemon's prompt: a dialog above every window on the output under the pointer, answered
/// by a click; it refuses whatever it cannot show.
pub struct LayerPrompt {
    surfaces: Rc<LayerShellSurfaces>,
}

impl LayerPrompt {
    pub fn new(surfaces: Rc<LayerShellSurfaces>) -> Self {
        LayerPrompt { surfaces }
    }

    fn show(&self, ask: Ask, cx: &mut App) -> Task<Answer> {
        let source = self.surfaces.source();
        let output = self
            .surfaces
            .test_output()
            .cloned()
            .or_else(|| source.pointer_output())
            .or_else(|| source.outputs().first().map(|o| o.id.clone()));
        let Some(output) = output else {
            tracing::warn!("no output for the consent dialog; refused");
            return Task::ready(Answer::Deny);
        };
        let handle = match self.surfaces.open_surface(
            SurfaceRole::Consent,
            &output,
            ConsentDialog::size(),
            cx,
        ) {
            Ok(handle) => handle,
            Err(e) => {
                tracing::warn!(error = %e, "consent dialog not shown; refused");
                return Task::ready(Answer::Deny);
            }
        };
        let view = cx.new(|cx| ConsentDialog::new(ask, cx));
        handle.set_content(view.clone().into(), cx);
        let (tx, rx) = futures::channel::oneshot::channel();
        let mut tx = Some(tx);
        let subscription = cx.subscribe(&view, move |_, answer: &Answer, _| {
            if let Some(tx) = tx.take() {
                let _ = tx.send(*answer);
            }
        });
        cx.spawn(async move |cx| {
            let answer = rx.await.unwrap_or(Answer::Deny);
            drop(subscription);
            cx.update(|cx| handle.close(cx));
            answer
        })
    }
}

impl ConsentPrompt for LayerPrompt {
    fn ask(&self, request: ConsentRequest, cx: &mut App) -> Task<Answer> {
        self.show(Ask::Access(request), cx)
    }

    fn confirm(&self, request: ChangeRequest, cx: &mut App) -> Task<bool> {
        let answer = self.show(Ask::Change(request), cx);
        cx.spawn(async move |_| answer.await.allows())
    }
}
