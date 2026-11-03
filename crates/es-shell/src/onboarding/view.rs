use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use es_backend::{BindHelper, Caps};
use es_protocol::Trigger;
use es_store::paths::Paths;
use es_ui::{
    a11y::{Accessible, Role},
    components::{ButtonKind, EditorButton},
    icons::IconName,
    motion::{Clocked, STEP_CHANGE},
    theme::{UI_FONT, tokens},
    tokens::Tokens,
};
use gpui::{
    AnyElement, Context, Div, EventEmitter, FocusHandle, Focusable, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ObjectFit, ParentElement, Render,
    StatefulInteractiveElement, Styled, StyledImage, Window, div, img, linear_color_stop,
    linear_gradient, px,
};
use gpui_component::{Icon, switch::Switch};

use super::rows::{caption, check_row, form_row, group, row_title};
use super::{checklist, complete};
use crate::settings::ClipboardSink;
use crate::strings::fl;

pub const WIDTH: f32 = 760.;
pub const HEIGHT: f32 = 680.;
const PAGE_WIDTH: f32 = 520.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Desktop,
    Shortcuts,
    Saving,
    Practice,
}

impl Step {
    pub const ALL: [Step; 5] = [
        Step::Welcome,
        Step::Desktop,
        Step::Shortcuts,
        Step::Saving,
        Step::Practice,
    ];

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone)]
pub struct OnboardingInput {
    pub caps: Caps,
    pub binds: Option<Arc<dyn BindHelper>>,
    pub clipboard: Option<Rc<dyn ClipboardSink>>,
    /// Global shortcuts already saved in the config: catalogue id to trigger.
    pub global_shortcuts: BTreeMap<String, String>,
    pub capture_shortcut: Option<Trigger>,
    pub save_dir: Option<PathBuf>,
    pub autosave: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingEvent {
    /// The introduction is over and `state.json` records it as seen.
    Closed,
    ChooseFolder,
    SetAutosave(bool),
    CaptureArea,
    OpenPractice,
    /// The global shortcuts to save in the config, which now include the chosen keys.
    SetShortcuts(BTreeMap<String, String>),
}

pub struct Onboarding {
    pub(super) paths: Paths,
    pub(super) input: OnboardingInput,
    step: Step,
    step_motion: Option<(Clocked, Step)>,
    practice: Option<PathBuf>,
    pub(super) shortcuts: super::shortcuts::State,
    focus: FocusHandle,
}

impl EventEmitter<OnboardingEvent> for Onboarding {}

impl Focusable for Onboarding {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Onboarding {
    pub fn new(
        paths: Paths,
        input: OnboardingInput,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let this = cx.entity().downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = this.update(cx, |o, cx| o.finish(cx));
            true
        });
        Self {
            paths,
            input,
            step: Step::Welcome,
            step_motion: None,
            practice: None,
            shortcuts: Default::default(),
            focus,
        }
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn go(&mut self, step: Step, cx: &mut Context<Self>) {
        if step != self.step {
            self.step_motion = Clocked::start(STEP_CHANGE, cx).map(|c| (c, self.step));
        }
        self.step = step;
        if step == Step::Shortcuts {
            self.enter_shortcuts(cx);
        }
        cx.notify();
    }

    pub fn advance(&mut self, cx: &mut Context<Self>) {
        match Step::ALL.get(self.step.index() + 1) {
            Some(&next) => self.go(next, cx),
            None => self.finish(cx),
        }
    }

    pub fn back(&mut self, cx: &mut Context<Self>) {
        match self.step.index().checked_sub(1) {
            Some(i) => self.go(Step::ALL[i], cx),
            None => self.finish(cx),
        }
    }

    /// Done, Skip and closing the window all end here.
    pub fn finish(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = complete(&self.paths) {
            tracing::warn!(%error, "cannot record the finished introduction");
        }
        cx.emit(OnboardingEvent::Closed);
    }

    pub fn set_input(&mut self, input: OnboardingInput, cx: &mut Context<Self>) {
        self.input = input;
        cx.notify();
    }

    pub fn show_practice(&mut self, image: PathBuf, cx: &mut Context<Self>) {
        self.practice = Some(image);
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match e.keystroke.key.as_str() {
            "enter" => self.advance(cx),
            "escape" if self.step == Step::Welcome => self.finish(cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn header(&self, t: &Tokens) -> Div {
        let (icon, title, subtitle) = match self.step {
            Step::Welcome => (
                IconName::Easyshot,
                fl!("onboarding-step1-title"),
                fl!("onboarding-step1-subtitle"),
            ),
            Step::Desktop => (
                IconName::Monitor,
                fl!("onboarding-step2-title"),
                fl!("onboarding-step2-subtitle"),
            ),
            Step::Shortcuts => (
                IconName::Keyboard,
                fl!("onboarding-step-shortcuts-title"),
                fl!("onboarding-step-shortcuts-subtitle"),
            ),
            Step::Saving => (
                IconName::Folder,
                fl!("onboarding-step3-title"),
                fl!("onboarding-step3-subtitle"),
            ),
            Step::Practice => (
                IconName::Scan,
                fl!("onboarding-step4-title"),
                fl!("onboarding-step4-subtitle"),
            ),
        };
        let top = Hsla {
            l: (t.accent.l + 0.12).min(1.),
            ..t.accent
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .size(px(88.))
                    .rounded(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(top, 0.),
                        linear_color_stop(t.accent, 1.),
                    ))
                    .text_color(t.on_accent)
                    .child(Icon::new(icon).size(px(48.))),
            )
            .child(
                div()
                    .text_size(px(26.))
                    .font_weight(FontWeight::BOLD)
                    .text_center()
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(15.))
                    .text_color(t.text_secondary)
                    .text_center()
                    .child(subtitle),
            )
    }

    fn welcome(&self, t: &Tokens) -> Div {
        let capture = match &self.input.capture_shortcut {
            Some(trigger) => fl!("onboarding-bullet-capture", shortcut = trigger.label()),
            None => fl!("onboarding-bullet-capture-unbound"),
        };
        let bullet = |icon: IconName, text: String, tint: Hsla| {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().text_color(tint).child(Icon::new(icon).size(px(20.))))
                .child(div().text_size(px(14.)).child(text))
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(bullet(IconName::SquareDashed, capture, t.accent))
            .child(bullet(
                IconName::PencilLine,
                fl!("onboarding-bullet-annotate"),
                t.text_secondary,
            ))
            .child(bullet(
                IconName::CircleDot,
                fl!("onboarding-bullet-record"),
                t.text_secondary,
            ))
            .child(bullet(
                IconName::Easyshot,
                fl!("onboarding-bullet-tray"),
                t.text_secondary,
            ))
    }

    fn desktop(&self, t: &Tokens) -> Div {
        let mut list = group(t);
        for (i, row) in checklist(self.input.caps).into_iter().enumerate() {
            list = list.child(check_row(t, row, i > 0));
        }
        div().flex().flex_col().gap(px(16.)).w_full().child(list)
    }

    fn saving(&self, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let autosave = self.input.autosave;
        let help = if autosave {
            fl!("onboarding-autosave-on-help")
        } else {
            fl!("onboarding-autosave-off-help")
        };
        let folder = self
            .input
            .save_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let entity = cx.entity().downgrade();
        let chooser = cx.entity().downgrade();
        group(t)
            .w_full()
            .child(
                form_row(t, false)
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(row_title(fl!("onboarding-autosave")))
                            .child(caption(t, help)),
                    )
                    .child(
                        Switch::new("onboarding-autosave")
                            .checked(autosave)
                            .accessibility_label(fl!("onboarding-autosave"))
                            .on_click(move |on, _, cx| {
                                let on = *on;
                                let _ = entity.update(cx, |o, cx| {
                                    o.input.autosave = on;
                                    cx.emit(OnboardingEvent::SetAutosave(on));
                                    cx.notify();
                                });
                            }),
                    ),
            )
            .child(
                form_row(t, true)
                    .child(row_title(fl!("onboarding-save-folder")))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(t.text_secondary)
                            .text_right()
                            .truncate()
                            .child(folder),
                    )
                    .child(
                        EditorButton::new("onboarding-choose-folder")
                            .label(fl!("onboarding-save-folder-choose"))
                            .kind(ButtonKind::Bordered)
                            .on_click(move |_, _, cx| {
                                let _ = chooser
                                    .update(cx, |_, cx| cx.emit(OnboardingEvent::ChooseFolder));
                            }),
                    ),
            )
    }

    fn practice(&self, t: &Tokens, cx: &mut Context<Self>) -> Div {
        let hint = match &self.input.capture_shortcut {
            Some(trigger) => fl!(
                "onboarding-practice-hint-shortcut",
                shortcut = trigger.label()
            ),
            None => fl!("onboarding-practice-hint-button"),
        };
        let shown: AnyElement = match &self.practice {
            Some(image) => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .id("onboarding-practice-thumbnail")
                        .accessible(Role::Image, fl!("onboarding-practice-thumbnail-a11y"))
                        .child(
                            img(image.clone())
                                .max_h(px(110.))
                                .rounded(px(8.))
                                .object_fit(ObjectFit::Contain),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .child(fl!("onboarding-practice-done")),
                )
                .into_any_element(),
            None => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .child(
                    div()
                        .text_color(t.accent)
                        .child(Icon::new(IconName::Scan).size(px(34.))),
                )
                .child(caption(t, hint))
                .into_any_element(),
        };
        let emit = |event: OnboardingEvent, cx: &mut Context<Self>| {
            let entity = cx.entity().downgrade();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                let event = event.clone();
                let _ = entity.update(cx, |_, cx| cx.emit(event));
            }
        };
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .min_h(px(130.))
                    .p(px(20.))
                    .rounded(px(14.))
                    .bg(t.bordered_fill)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(shown),
            )
            .child(
                div()
                    .flex()
                    .justify_center()
                    .gap(px(10.))
                    .child(
                        EditorButton::new("onboarding-capture-area")
                            .icon(IconName::SquareDashed)
                            .label(fl!("onboarding-practice-capture"))
                            .kind(ButtonKind::Bordered)
                            .on_click(emit(OnboardingEvent::CaptureArea, cx)),
                    )
                    .child(
                        EditorButton::new("onboarding-open-practice")
                            .icon(IconName::Image)
                            .label(fl!("onboarding-practice-open"))
                            .kind(ButtonKind::Bordered)
                            .on_click(emit(OnboardingEvent::OpenPractice, cx)),
                    ),
            )
    }

    fn footer(&self, t: &Tokens, arrived: f32, from: Step, cx: &mut Context<Self>) -> Div {
        let first = self.step == Step::Welcome;
        let last = self.step == Step::Practice;
        let (back, next) = (cx.entity().downgrade(), cx.entity().downgrade());
        let total = Step::ALL.len();
        let n = self.step.index() + 1;
        let dots = div()
            .id("onboarding-progress")
            .accessible(
                Role::ProgressIndicator,
                fl!("onboarding-step-a11y", n = n, total = total),
            )
            .flex()
            .items_center()
            .gap(px(6.))
            .children(Step::ALL.iter().map(|&s| {
                let active = s == self.step;
                let width = |current: bool| if current { 20. } else { 7. };
                let (was, is) = (width(s == from), width(active));
                div()
                    .h(px(7.))
                    .w(px(was + (is - was) * arrived))
                    .rounded_full()
                    .bg(if active { t.accent } else { t.text_tertiary })
            }));
        div()
            .flex()
            .items_center()
            .p(px(20.))
            .border_t(t.metrics.hairline)
            .border_color(t.separator)
            .child(
                div().flex_1().flex().child(
                    EditorButton::new("onboarding-back")
                        .label(if first {
                            fl!("onboarding-skip")
                        } else {
                            fl!("onboarding-back")
                        })
                        .kind(ButtonKind::Bordered)
                        .padding_x(px(14.))
                        .on_click(move |_, _, cx| {
                            let _ = back.update(cx, |o, cx| o.back(cx));
                        }),
                ),
            )
            .child(dots)
            .child(
                div().flex_1().flex().justify_end().child(
                    EditorButton::new("onboarding-next")
                        .label(if last {
                            fl!("onboarding-done")
                        } else {
                            fl!("onboarding-continue")
                        })
                        .kind(ButtonKind::Prominent)
                        .padding_x(px(14.))
                        .on_click(move |_, _, cx| {
                            let _ = next.update(cx, |o, cx| o.advance(cx));
                        }),
                ),
            )
    }
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let (arrived, from) = match self.step_motion {
            Some((clocked, from)) => (clocked.progress(window, cx), from),
            None => (1.0, self.step),
        };
        let toward = if self.step.index() < from.index() {
            -1.
        } else {
            1.
        };
        let slide = window.viewport_size().width * toward * (1.0 - arrived);
        let body = match self.step {
            Step::Welcome => self.welcome(&t),
            Step::Desktop => self.desktop(&t),
            Step::Shortcuts => self.shortcuts_step(&t, cx),
            Step::Saving => self.saving(&t, cx),
            Step::Practice => self.practice(&t, cx),
        };
        div()
            .id("onboarding")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(t.window)
            .text_color(t.text)
            .font_family(UI_FONT)
            .child(
                div()
                    .id("onboarding-page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(px(36.))
                    .pb(px(12.))
                    .px(px(40.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(PAGE_WIDTH))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(28.))
                            .relative()
                            .left(slide)
                            .opacity(arrived)
                            .child(self.header(&t))
                            .child(body),
                    ),
            )
            .child(self.footer(&t, arrived, from, cx))
    }
}
