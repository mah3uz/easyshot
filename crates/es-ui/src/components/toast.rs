use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    App, BoxShadow, Context, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, SharedString, StatefulInteractiveElement, Styled, Task, Window, div, hsla,
    prelude::FluentBuilder, px, relative,
};
use gpui_component::Icon;

use crate::{
    a11y::{Accessible, Range, Role},
    icons::IconName,
    motion::{
        Clocked, TOAST_IN, TOAST_IN_SCALE, TOAST_OUT, TOAST_OUT_SCALE, TOAST_REDUCED, TOAST_RISE,
        Transition,
    },
    theme::tokens,
    tokens::Typeset,
};

use super::button::{ButtonKind, EditorButton};

const RADIUS: Pixels = px(14.);
const PADDING_X: Pixels = px(16.);
const PADDING_Y: Pixels = px(11.);
const GAP: Pixels = px(10.);
/// A settings row's space between its title and its control.
const ACTION_GAP: Pixels = px(16.);
const ICON_BOX: Pixels = px(32.);
const GLYPH: Pixels = px(18.);
const MAX_WIDTH: Pixels = px(400.);
const SHADOW_INK: f32 = 0.18;
const BAR: Pixels = px(4.);
const BAR_WIDTH: Pixels = px(140.);
const TICK: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastStyle {
    #[default]
    Normal,
    Error,
}

type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Time left on a toast; it does not run down while the pointer is over the card.
#[derive(Clone, Copy, Debug)]
pub struct Countdown {
    remaining: Duration,
}

impl Countdown {
    pub fn new(total: Duration) -> Self {
        Self { remaining: total }
    }

    pub fn remaining(&self) -> Duration {
        self.remaining
    }

    /// Returns true once the time is used up.
    pub fn advance(&mut self, elapsed: Duration, hovered: bool) -> bool {
        if !hovered {
            self.remaining = self.remaining.saturating_sub(elapsed);
        }
        self.remaining.is_zero()
    }
}

/// A shown toast's time, the only one: it runs down while the pointer is off the card given it with [`Toast::clock`].
pub struct ToastClock {
    hovered: Rc<Cell<bool>>,
    _run: Task<()>,
}

impl ToastClock {
    /// Calls `done` on the entity that keeps the clock once the time is used up; dropping the clock stops it.
    pub fn start<T: 'static>(
        label: impl Into<SharedString>,
        duration: Duration,
        cx: &mut Context<T>,
        done: impl FnOnce(&mut T, &mut Context<T>) + 'static,
    ) -> Self {
        let hovered = Rc::new(Cell::new(false));
        let over = hovered.clone();
        let label: SharedString = label.into();
        let run = cx.spawn(async move |this, cx| {
            let mut countdown = Countdown::new(duration);
            let mut last = cx.background_executor().now();
            let mut was_over = false;
            loop {
                cx.background_executor().timer(TICK).await;
                let now = cx.background_executor().now();
                let is_over = over.get();
                if is_over != was_over {
                    was_over = is_over;
                    let left = countdown.remaining();
                    tracing::debug!(target: "easyshot::toast", toast = %label, ?left, hovered = is_over, "toast hover changed");
                    let state = if is_over { "paused" } else { "resumed" };
                    tracing::debug!(target: "easyshot::toast", toast = %label, ?left, "toast countdown {state}");
                }
                if countdown.advance(now - last, is_over) {
                    break;
                }
                last = now;
            }
            let _ = this.update(cx, done);
        });
        Self { hovered, _run: run }
    }

    pub fn hovered(&self) -> bool {
        self.hovered.get()
    }
}

/// The card only: whoever shows it places it, runs its [`ToastClock`], then marks it leaving for [`TOAST_OUT`].
#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    title: SharedString,
    message: Option<SharedString>,
    icon: Option<IconName>,
    style: ToastStyle,
    leaving: bool,
    settled: bool,
    actions: Vec<(SharedString, ActionHandler)>,
    progress: Option<f32>,
    hovered: Option<Rc<Cell<bool>>>,
}

impl Toast {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: None,
            icon: None,
            style: ToastStyle::Normal,
            leaving: false,
            settled: false,
            actions: Vec::new(),
            progress: None,
            hovered: None,
        }
    }

    /// The pointer on this card holds the clock.
    pub fn clock(mut self, clock: &ToastClock) -> Self {
        self.hovered = Some(clock.hovered.clone());
        self
    }

    /// A button on the card, such as Undo; each call adds one after the last.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.actions.push((label.into(), Rc::new(on_click)));
        self
    }

    /// A bar under the text, filled `done` of the way, from 0 to 1.
    pub fn progress(mut self, done: f32) -> Self {
        self.progress = Some(done.clamp(0., 1.));
        self
    }

    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn style(mut self, style: ToastStyle) -> Self {
        self.style = style;
        self
    }

    /// Plays the fade out; the host drops the toast once [`TOAST_OUT`] has run.
    pub fn leaving(mut self, leaving: bool) -> Self {
        self.leaving = leaving;
        self
    }

    /// Draws the card at rest with no entrance, for previews that are not being shown.
    pub fn settled(mut self) -> Self {
        self.settled = true;
        self
    }
}

/// Each clock carries whether it moves the card or, under Reduce Motion, only fades it.
struct Motion {
    entry: Option<(Clocked, bool)>,
    exit: Option<(Clocked, bool)>,
}

fn clock(transition: Transition, cx: &App) -> Option<(Clocked, bool)> {
    Clocked::start(transition, cx)
        .map(|c| (c, true))
        .or_else(|| Clocked::start(TOAST_REDUCED, cx).map(|c| (c, false)))
}

/// The card's scale, how far below its place it is drawn, and its opacity.
fn pose(window: &mut Window, cx: &mut App, id: ElementId, leaving: bool) -> (f32, Pixels, f32) {
    let motion = window.use_keyed_state(id, cx, |_, cx| Motion {
        entry: clock(TOAST_IN, cx),
        exit: None,
    });
    let (entry, exit) = motion.update(cx, |m, cx| {
        if leaving && m.exit.is_none() {
            m.exit = clock(TOAST_OUT, cx);
        }
        (m.entry, m.exit)
    });
    if leaving {
        let Some((c, moves)) = exit else {
            return (1., px(0.), 0.);
        };
        let o = c.progress(window, cx);
        let scale = if moves {
            1. - (1. - TOAST_OUT_SCALE) * o
        } else {
            1.
        };
        return (scale, px(0.), 1. - o);
    }
    let Some((c, moves)) = entry else {
        return (1., px(0.), 1.);
    };
    let e = c.progress(window, cx);
    if !moves {
        return (1., px(0.), e);
    }
    (
        TOAST_IN_SCALE + (1. - TOAST_IN_SCALE) * e,
        TOAST_RISE * (1. - e),
        (e * 1.5).clamp(0., 1.),
    )
}

impl RenderOnce for Toast {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = tokens(cx).clone();
        let error = self.style == ToastStyle::Error;
        let title = self.title.clone();
        let (scale, rise, alpha) = if self.settled && !self.leaving {
            (1., px(0.), 1.)
        } else {
            pose(window, cx, self.id.clone(), self.leaving)
        };
        let inset = relative((1. - scale) / 2.);
        let shell = div()
            .absolute()
            .top(inset)
            .bottom(inset)
            .left(inset)
            .right(inset)
            .m(-t.metrics.hairline)
            .rounded(RADIUS)
            .bg(t.dropdown)
            .border(t.metrics.hairline)
            .border_color(t.border)
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), hsla(0., 0., 0., SHADOW_INK))
                    .blur_radius(px(9.))
                    .spread_radius(px(-2.)),
            ]);
        div()
            .id(self.id)
            .when_some(self.hovered, |d, hovered| {
                d.on_hover(move |over, _, _| hovered.set(*over))
            })
            .relative()
            .top(rise)
            .when(alpha < 1., |d| d.opacity(alpha))
            .flex()
            .items_center()
            .gap(GAP)
            .max_w(MAX_WIDTH)
            .px(PADDING_X)
            .py(PADDING_Y)
            .border(t.metrics.hairline)
            .border_color(gpui::transparent_black())
            .child(shell)
            .text_color(t.text)
            .accessible(
                if error { Role::Alert } else { Role::Status },
                self.title.clone(),
            )
            .when_some(self.message.clone(), |d, message| {
                d.aria_description(message)
            })
            .when_some(self.icon, |d, icon| {
                d.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(ICON_BOX)
                        .text_color(if error {
                            t.destructive
                        } else {
                            t.text_secondary
                        })
                        .child(Icon::new(icon).size(GLYPH)),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().typeset(t.type_scale.title).child(self.title))
                    .when_some(self.message, |d, message| {
                        d.child(
                            div()
                                .typeset(t.type_scale.label)
                                .text_color(t.text_secondary)
                                .child(message),
                        )
                    })
                    .when_some(self.progress, |d, done| {
                        d.child(
                            div()
                                .id("toast-progress")
                                .accessible(Role::ProgressIndicator, title)
                                .range(Range {
                                    value: f64::from(done) * 100.,
                                    min: 0.,
                                    max: 100.,
                                    step: 1.,
                                })
                                .mt(px(4.))
                                .w(BAR_WIDTH)
                                .h(BAR)
                                .rounded_full()
                                .bg(t.track)
                                .child(
                                    div().h_full().w(relative(done)).rounded_full().bg(t.accent),
                                ),
                        )
                    }),
            )
            .when(!self.actions.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_none()
                        .gap(px(8.))
                        .ml(ACTION_GAP - GAP)
                        .children(self.actions.into_iter().enumerate().map(
                            |(i, (label, on_click))| {
                                EditorButton::new(("toast-action", i))
                                    .label(label)
                                    .kind(ButtonKind::Bordered)
                                    .on_click(move |_, window, cx| on_click(window, cx))
                            },
                        )),
                )
            })
    }
}
