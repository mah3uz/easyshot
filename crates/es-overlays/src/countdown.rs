use crate::capture::strings::fl;
use es_app::capture_coordinator::{Countdown as CountdownHook, Timer};
use es_app::errors::{Error, Result};
use es_app::platform_ui::{LayerShellSurfaces, SurfaceHandle, SurfaceRole};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::motion::{Easing, reduce_motion};
use es_ui::theme::UI_FONT;
use es_ui::tokens::{Appearance, Tokens};
use gpui::{
    App, AppContext as _, Bounds, Context, Corners, Entity, FontWeight, Hsla,
    InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels, Render,
    Styled as _, Task, Window, canvas, div, fill, hsla, point, px, quad,
};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_secs(1);
const EXIT: Duration = Duration::from_millis(300);
const EXIT_REDUCED: Duration = Duration::from_millis(120);
// The overlay's frame callback stops once it is unmapped, so a frame period stands in for it.
const UNMAP_SETTLE: Duration = Duration::from_millis(34);
const DIAL: f32 = 152.;
const DIGIT: f32 = 68.;
const RING: f32 = 5.;
const RING_INSET: f32 = 5.;
const SCRIM: f32 = 0.10;
const EXIT_SCALE: f32 = 0.82;
const BADGE: f32 = 160.;
const BADGE_RADIUS: f32 = 28.;
const BADGE_DIGIT: f32 = 76.;
pub const MIN_TIMED_SECONDS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountdownStyle {
    Capture,
    Recording,
}

pub type Sleep = Rc<dyn Fn(Duration, &mut App) -> Task<()>>;

/// Time as the countdown sees it, so tests can drive it.
#[derive(Clone)]
pub struct Clock {
    pub now: Rc<dyn Fn() -> Instant>,
    pub sleep: Sleep,
}

impl Clock {
    pub fn real() -> Self {
        Clock {
            now: Rc::new(Instant::now),
            sleep: Rc::new(|d, cx| cx.background_executor().timer(d)),
        }
    }
}

pub fn exit_duration(reduce_motion: bool) -> Duration {
    if reduce_motion { EXIT_REDUCED } else { EXIT }
}

pub struct Countdown {
    style: CountdownStyle,
    seconds: u32,
    remaining: u32,
    started: Instant,
    exit: Option<(Instant, Duration)>,
    finished: bool,
    clock: Clock,
}

impl Countdown {
    pub fn new(style: CountdownStyle, seconds: u32, clock: Clock) -> Self {
        Countdown {
            style,
            seconds,
            remaining: seconds,
            started: (clock.now)(),
            exit: None,
            finished: false,
            clock,
        }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    pub fn exiting(&self) -> bool {
        self.exit.is_some()
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Counts down one tick per second, plays the exit, then resolves.
    pub fn run(this: &Entity<Self>, cx: &mut App) -> Task<()> {
        let this = this.clone();
        cx.spawn(async move |cx| {
            let (clock, seconds) = cx.update(|cx| {
                this.update(cx, |c, cx| {
                    c.started = (c.clock.now)();
                    cx.notify();
                    (c.clock.clone(), c.seconds)
                })
            });
            for n in (1..=seconds).rev() {
                cx.update(|cx| {
                    this.update(cx, |c, cx| {
                        c.remaining = n;
                        cx.notify();
                    })
                });
                cx.update(|cx| (clock.sleep)(TICK, cx)).await;
            }
            let exit = cx.update(|cx| exit_duration(reduce_motion(cx)));
            cx.update(|cx| {
                this.update(cx, |c, cx| {
                    c.exit = Some(((c.clock.now)(), exit));
                    cx.notify();
                })
            });
            cx.update(|cx| (clock.sleep)(exit, cx)).await;
            cx.update(|cx| {
                this.update(cx, |c, cx| {
                    c.finished = true;
                    cx.notify();
                })
            });
        })
    }

    fn sweep(&self, now: Instant, reduced: bool) -> f32 {
        if reduced || self.seconds == 0 {
            return 1.0;
        }
        let total = TICK.as_secs_f32() * self.seconds as f32;
        (1.0 - now.saturating_duration_since(self.started).as_secs_f32() / total).clamp(0.0, 1.0)
    }

    /// Opacity, and size as a share of full size.
    fn presence(&self, now: Instant, reduced: bool) -> (f32, f32) {
        if self.finished {
            return (0.0, if reduced { 1.0 } else { EXIT_SCALE });
        }
        let Some((since, length)) = self.exit else {
            return (1.0, 1.0);
        };
        let t = now.saturating_duration_since(since).as_secs_f32() / length.as_secs_f32();
        let t = t.clamp(0.0, 1.0);
        if reduced {
            (1.0 - t, 1.0)
        } else {
            let e = Easing::Snappy.at(t);
            (1.0 - e, 1.0 - (1.0 - EXIT_SCALE) * e)
        }
    }
}

fn circle_points(centre: (f32, f32), radius: f32, from: f32, to: f32) -> Vec<gpui::Point<Pixels>> {
    let steps = (((to - from).abs() / TAU) * 120.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = from + (to - from) * i as f32 / steps as f32;
            point(
                px(centre.0 + radius * a.cos()),
                px(centre.1 + radius * a.sin()),
            )
        })
        .collect()
}

fn arc(
    centre: (f32, f32),
    radius: f32,
    from: f32,
    to: f32,
    width: f32,
    colour: Hsla,
    window: &mut Window,
) {
    let mut path = PathBuilder::stroke(px(width));
    for (i, p) in circle_points(centre, radius, from, to)
        .into_iter()
        .enumerate()
    {
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    match path.build() {
        Ok(path) => window.paint_path(path, colour),
        Err(error) => tracing::warn!(%error, "countdown ring not painted"),
    }
}

fn dot(at: gpui::Point<Pixels>, diameter: f32, colour: Hsla, window: &mut Window) {
    let r = px(diameter / 2.);
    window.paint_quad(
        fill(
            Bounds::new(
                point(at.x - r, at.y - r),
                gpui::size(px(diameter), px(diameter)),
            ),
            colour,
        )
        .corner_radii(Corners::all(r)),
    );
}

struct Dial {
    sweep: f32,
    opacity: f32,
    scale: f32,
    face: Hsla,
    track: Hsla,
    accent: Hsla,
}

impl Dial {
    fn paint(self, bounds: Bounds<Pixels>, window: &mut Window) {
        window.paint_quad(fill(bounds, hsla(0., 0., 0., SCRIM * self.opacity)));
        let c = bounds.center();
        let centre = (f32::from(c.x), f32::from(c.y));
        let d = DIAL * self.scale;
        let face = Bounds::new(
            point(c.x - px(d / 2.), c.y - px(d / 2.)),
            gpui::size(px(d), px(d)),
        );
        window.paint_quad(quad(
            face,
            Corners::all(px(d / 2.)),
            self.face.opacity(self.face.a * self.opacity),
            px(0.5),
            hsla(0., 0., 0., 0.12 * self.opacity),
            Default::default(),
        ));
        let radius = (d / 2. - RING_INSET * self.scale) - RING * self.scale / 2.;
        let width = RING * self.scale;
        arc(
            centre,
            radius,
            0.,
            TAU,
            width,
            self.track.opacity(self.track.a * self.opacity),
            window,
        );
        if self.sweep > 0.0 {
            let (from, to) = (-FRAC_PI_2, -FRAC_PI_2 + TAU * self.sweep);
            let accent = self.accent.opacity(self.opacity);
            arc(
                centre,
                radius,
                from,
                to,
                width + 4.,
                accent.opacity(0.18 * self.opacity),
                window,
            );
            arc(centre, radius, from, to, width, accent, window);
            let ends = circle_points(centre, radius, from, to);
            for end in [ends.first(), ends.last()].into_iter().flatten() {
                dot(*end, width, accent, window);
            }
        }
    }
}

impl Render for Countdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reduced = reduce_motion(cx);
        let now = (self.clock.now)();
        let (opacity, scale) = self.presence(now, reduced);
        if !self.finished && (!reduced || self.exit.is_some()) {
            window.request_animation_frame();
        }
        let tokens = cx.try_global::<Tokens>();
        let dark = tokens.is_some_and(|t| t.appearance == Appearance::Dark);
        let text = tokens.map_or(hsla(0., 0., 0.1, 1.), |t| t.colours.text);
        let label = fl!("countdown-a11y", count = self.remaining);
        let digit = |size: f32, colour: Hsla| {
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .font_family(UI_FONT)
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(px(size * scale))
                .line_height(px(size * scale))
                .text_color(colour.opacity(colour.a * opacity))
                .child(self.remaining.to_string())
        };
        let root = div()
            .id("countdown")
            .accessible(Role::Timer, label)
            .relative()
            .size_full();
        match self.style {
            CountdownStyle::Capture => {
                let dial = Dial {
                    sweep: self.sweep(now, reduced),
                    opacity,
                    scale,
                    face: if dark {
                        hsla(0., 0., 0.16, 0.92)
                    } else {
                        hsla(0., 0., 0.98, 0.92)
                    },
                    track: text.opacity(0.12),
                    accent: crate::capture::paint::accent(cx),
                };
                root.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| dial.paint(bounds, window),
                    )
                    .absolute()
                    .size_full(),
                )
                .child(digit(DIGIT, text))
            }
            CountdownStyle::Recording => {
                let side = BADGE * scale;
                root.flex().items_center().justify_center().child(
                    div()
                        .relative()
                        .w(px(side))
                        .h(px(side))
                        .rounded(px(BADGE_RADIUS * scale))
                        .bg(hsla(0., 0., 0., 0.55 * opacity))
                        .child(digit(BADGE_DIGIT, hsla(0., 0., 1., 1.))),
                )
            }
        }
    }
}

/// The coordinator's countdown: a dial on the pointer's output that takes no input, gone before it resolves.
/// Seconds a timer counts: its own, or the setting raised to the minimum.
pub fn timer_seconds(timer: Timer, setting: u32) -> u32 {
    match timer {
        Timer::Seconds(n) => n,
        Timer::Off | Timer::Setting => setting.max(MIN_TIMED_SECONDS),
    }
}

pub fn hook(surfaces: Rc<LayerShellSurfaces>, delay: Rc<dyn Fn(&App) -> u32>) -> CountdownHook {
    Rc::new(move |timer, cx: &mut App| -> Task<Result<()>> {
        let seconds = timer_seconds(timer, delay(cx));
        let source = surfaces.source();
        let Some(output) = source
            .pointer_output()
            .or_else(|| source.outputs().first().map(|o| o.id.clone()))
        else {
            return Task::ready(Err(Error::NotFound("an output for the countdown".into())));
        };
        let opened = surfaces.output_info(&output).and_then(|info| {
            surfaces.open_surface(SurfaceRole::Countdown, &output, info.logical.size(), cx)
        });
        let handle = match opened {
            Ok(handle) => handle,
            Err(error) => return Task::ready(Err(error)),
        };
        handle.set_input_region(&[], cx);
        let view = cx.new(|_| Countdown::new(CountdownStyle::Capture, seconds, Clock::real()));
        handle.set_content(view.clone().into(), cx);
        let run = Countdown::run(&view, cx);
        cx.spawn(async move |cx| {
            run.await;
            cx.update(|cx| handle.close(cx));
            cx.background_executor().timer(UNMAP_SETTLE).await;
            Ok(())
        })
    })
}
