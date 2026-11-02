use super::view::{Grab, PlayerWindow, STRIP_HEIGHT};
use crate::capture::strings::fl;
use es_ui::a11y::{Accessible, Range, Role};
use es_ui::theme::tokens;
use gpui::{
    Context, Div, ImageSource, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ObjectFit, ParentElement, Pixels, Stateful, Styled, StyledImage, canvas, div,
    img, px, relative,
};

pub const HANDLE_WIDTH: Pixels = px(12.);
const FRAME: Pixels = px(3.);
const SLOP: f32 = 4.;
const RADIUS: Pixels = px(6.);

impl PlayerWindow {
    fn fraction(&self, secs: f64) -> f32 {
        let d = self.trim.duration();
        if d > 0. {
            (secs / d).clamp(0., 1.) as f32
        } else {
            0.
        }
    }

    fn x_of(&self, secs: f64) -> f32 {
        f32::from(self.track.left()) + self.fraction(secs) * f32::from(self.track.size.width)
    }

    fn time_at(&self, x: Pixels) -> f64 {
        let width = f32::from(self.track.size.width).max(1.);
        let f = ((f32::from(x) - f32::from(self.track.left())) / width).clamp(0., 1.);
        f64::from(f) * self.trim.duration()
    }

    /// Handles sit outside the kept span, so the span can shrink to its minimum and both stay reachable.
    fn grab_at(&self, x: Pixels) -> Grab {
        let (x, w) = (f32::from(x), f32::from(HANDLE_WIDTH));
        let (s, e) = (self.x_of(self.trim.start()), self.x_of(self.trim.end()));
        let near_start = (s - w - SLOP..=s + SLOP).contains(&x);
        let near_end = (e - SLOP..=e + w + SLOP).contains(&x);
        match (near_start, near_end) {
            (true, true) if x < (s + e) / 2. => Grab::Start,
            (true, true) => Grab::End,
            (true, false) => Grab::Start,
            (false, true) => Grab::End,
            (false, false) => Grab::Playhead,
        }
    }

    pub(super) fn scrubber(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = tokens(cx).clone();
        let (s, e) = (
            self.fraction(self.trim.start()),
            self.fraction(self.trim.end()),
        );
        let at = self.fraction(self.position);
        let dim = t.window.opacity(0.62);
        let range = |value| Range {
            value,
            min: 0.,
            max: self.trim.duration(),
            step: 0.01,
        };
        let grip = || div().w(px(2.5)).h(px(16.)).rounded_full().bg(t.on_accent);
        let handle = |id: &'static str, label: String, value: f64| {
            div()
                .id(id)
                .accessible(Role::Slider, label)
                .range(range(value))
                .absolute()
                .top_0()
                .bottom_0()
                .w(HANDLE_WIDTH)
                .flex()
                .items_center()
                .justify_center()
                .bg(t.accent)
                .child(grip())
        };
        let view = cx.entity().downgrade();
        let track = div()
            .relative()
            .size_full()
            .child(div().absolute().inset_0().rounded(RADIUS).bg(t.track))
            .children(self.strip.clone().map(|strip| {
                img(ImageSource::Render(strip))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded(px(2.))
                    .object_fit(ObjectFit::Fill)
            }))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(relative(s))
                    .bg(dim),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(relative(1. - e))
                    .bg(dim),
            )
            .child(
                div()
                    .absolute()
                    .top(-FRAME)
                    .bottom(-FRAME)
                    .left(relative(s))
                    .w(relative(e - s))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(FRAME)
                            .bg(t.accent),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(FRAME)
                            .bg(t.accent),
                    )
                    .child(
                        handle(
                            "trim-start",
                            fl!("player-trim-start-a11y"),
                            self.trim.start(),
                        )
                        .left(-HANDLE_WIDTH)
                        .rounded_l(RADIUS),
                    )
                    .child(
                        handle("trim-end", fl!("player-trim-end-a11y"), self.trim.end())
                            .right(-HANDLE_WIDTH)
                            .rounded_r(RADIUS),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(-FRAME - px(7.))
                    .bottom(-FRAME - px(2.))
                    .left(relative(at))
                    .ml(px(-1.))
                    .w(px(2.))
                    .bg(t.destructive)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(-4.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(t.destructive),
                    ),
            )
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let _ = view.update(cx, |p, _| p.track = bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        div()
            .id("timeline")
            .accessible(Role::Slider, fl!("player-timeline-a11y"))
            .range(range(self.position))
            .h(STRIP_HEIGHT + FRAME * 2.)
            .px(HANDLE_WIDTH)
            .py(FRAME)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|p, e: &MouseDownEvent, _, cx| {
                    let grab = p.grab_at(e.position.x);
                    p.grab = Some(grab);
                    p.drag_to(grab, p.time_at(e.position.x), cx);
                    cx.stop_propagation();
                }),
            )
            .child(track)
    }
}

/// Drags follow the pointer anywhere in the window, not only over the strip.
pub(super) fn follow_drags(root: Stateful<Div>, cx: &mut Context<PlayerWindow>) -> Stateful<Div> {
    root.on_mouse_move(cx.listener(|p, e: &MouseMoveEvent, _, cx| {
        if let (Some(grab), Some(MouseButton::Left)) = (p.grab, e.pressed_button) {
            p.drag_to(grab, p.time_at(e.position.x), cx);
        }
    }))
    .on_mouse_up(
        MouseButton::Left,
        cx.listener(|p, _, _, cx| {
            if p.grab.take().is_some() {
                cx.notify();
            }
        }),
    )
}
