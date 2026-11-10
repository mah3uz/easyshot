use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, Global, Hsla, IntoElement, Pixels, Rgba,
    TooltipTiming, Window, px,
};
use gpui_component::tooltip::TooltipDefaults;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseOut,
    EaseIn,
    /// `cubic-bezier(.2, .8, .2, 1)`: a fast start that settles softly, close to a critically damped spring.
    Snappy,
    Spring(Spring),
}

impl Easing {
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseOut => cubic_bezier([0.0, 0.0, 0.58, 1.0], t),
            Self::EaseIn => cubic_bezier([0.42, 0.0, 1.0, 1.0], t),
            Self::Snappy => cubic_bezier([0.2, 0.8, 0.2, 1.0], t),
            Self::Spring(_) if t >= 1.0 => 1.0,
            Self::Spring(spring) => spring.at(t),
        }
    }
}

/// A damped spring let go at rest at 0 and pulled to 1, timed in fractions of its transition.
/// Below a damping of 100 % it passes 1 once by [`Spring::overshoot`] and swings back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spring {
    /// Thousandths of the transition one undamped swing takes.
    response: u16,
    damping: u16,
}

impl Spring {
    fn zeta(self) -> f32 {
        f32::from(self.damping) / 100.0
    }

    fn omega(self) -> f32 {
        std::f32::consts::TAU * 1000.0 / f32::from(self.response.max(1))
    }

    pub fn at(self, t: f32) -> f32 {
        let (zeta, omega) = (self.zeta(), self.omega());
        let decay = (-zeta * omega * t).exp();
        if zeta >= 1.0 {
            return 1.0 - decay * (1.0 + omega * t);
        }
        let swing = omega * (1.0 - zeta * zeta).sqrt();
        1.0 - decay * ((swing * t).cos() + zeta * omega / swing * (swing * t).sin())
    }

    /// How far past 1 the spring swings, as a fraction of its travel.
    pub fn overshoot(self) -> f32 {
        let zeta = self.zeta();
        if zeta >= 1.0 {
            return 0.0;
        }
        (-zeta * std::f32::consts::PI / (1.0 - zeta * zeta).sqrt()).exp()
    }
}

fn cubic_bezier([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
    let curve = |p1: f32, p2: f32, t: f32| {
        let u = 1.0 - t;
        3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
    };
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    let mut t = x;
    for _ in 0..30 {
        let at = curve(x1, x2, t);
        if (at - x).abs() < 1e-6 {
            break;
        }
        if at < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) / 2.0;
    }
    curve(y1, y2, t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub duration: Duration,
    pub easing: Easing,
    pub fade: bool,
}

const fn moves(ms: u64, easing: Easing) -> Transition {
    Transition {
        duration: Duration::from_millis(ms),
        easing,
        fade: false,
    }
}

const fn fades(ms: u64, easing: Easing) -> Transition {
    Transition {
        duration: Duration::from_millis(ms),
        easing,
        fade: true,
    }
}

/// A spring of `response_ms` per undamped swing and `damping` percent, cut at `ms` once it has
/// come to rest within half a percent of its travel.
const fn springs(ms: u64, response_ms: u64, damping: u16) -> Transition {
    Transition {
        duration: Duration::from_millis(ms),
        easing: Easing::Spring(Spring {
            response: (response_ms * 1000 / ms) as u16,
            damping,
        }),
        fade: false,
    }
}

/// Quick, with a small overshoot: a control answering.
pub const SNAPPY_SPRING: Transition = springs(360, 300, 75);
/// Lively, with a visible overshoot: something arriving.
pub const BOUNCY_SPRING: Transition = springs(600, 400, 62);
/// Soft, all but critically damped: a layout settling.
pub const GENTLE_SPRING: Transition = springs(380, 350, 85);
/// The gentle spring, half again as long: a whole page crossing a window.
pub const CALM_SPRING: Transition = springs(570, 525, 85);

pub const POPOVER_OPEN: Transition = moves(130, Easing::EaseOut);
pub const DECK_HOVER: Transition = fades(120, Easing::EaseOut);
pub const DECK_FAN: Transition = GENTLE_SPRING;
pub const DECK_ENTER: Transition = BOUNCY_SPRING;
pub const DECK_ENTER_REDUCED: Transition = fades(120, Easing::Linear);
pub const DECK_ENTER_SCALE: f32 = 0.88;
pub const DECK_ENTER_RISE: Pixels = px(14.);
pub const TOAST_IN: Transition = SNAPPY_SPRING;
pub const TOAST_IN_SCALE: f32 = 0.92;
pub const TOAST_RISE: Pixels = px(6.);
pub const TOAST_OUT: Transition = moves(160, Easing::EaseIn);
pub const TOAST_OUT_SCALE: f32 = 0.96;
pub const TOAST_REDUCED: Transition = fades(120, Easing::Linear);
pub const SECTION_EXPAND: Transition = moves(180, Easing::Snappy);
pub const FLASH: Transition = fades(200, Easing::Snappy);
pub const FLASH_HOLD: Duration = Duration::from_millis(1400);
pub const SLIDER_MARKERS: Transition = fades(120, Easing::EaseOut);
pub const HOVER_FILL: Transition = fades(120, Easing::EaseOut);
pub const CROSSFADE: Transition = fades(150, Easing::EaseOut);
/// The exact composite fading in over the frame it replaces when a look change settles.
pub const COMPOSITE_FADE: Transition = fades(180, Easing::EaseOut);
pub const FILL_SWITCH: Transition = fades(160, Easing::Snappy);
pub const SETTLE: Transition = moves(180, Easing::Snappy);
pub const PRESET_APPLY: Transition = moves(200, Easing::Snappy);
pub const STAGE_IN: Transition = BOUNCY_SPRING;
pub const CROP_COMMIT: Transition = moves(220, Easing::Snappy);

pub const CROP_RESHAPE: Transition = moves(180, Easing::Snappy);
pub const KNOB_GROW: Transition = moves(120, Easing::Snappy);
pub const BUTTON_POP: Transition = moves(180, Easing::Snappy);
pub const BUTTON_POP_SCALE: f32 = 0.82;
pub const NOTICE_EXPAND: Transition = moves(220, Easing::Snappy);

pub const PAGE_CHANGE: Transition = moves(180, Easing::EaseOut);
pub const SETTINGS_PAGE: Transition = moves(270, Easing::EaseOut);
pub const PAGE_RISE: Pixels = px(8.);
pub const STEP_CHANGE: Transition = moves(200, Easing::EaseOut);
pub const TAB_SLIDE: Transition = CALM_SPRING;
pub const THUMB_GLIDE: Transition = SNAPPY_SPRING;
pub const COUNTDOWN_IN: Transition = BOUNCY_SPRING;
pub const COUNTDOWN_IN_REDUCED: Transition = fades(120, Easing::Linear);
pub const COUNTDOWN_ENTRY_SCALE: f32 = 0.82;
pub const COUNTDOWN_OUT: Transition = moves(300, Easing::Snappy);
pub const COUNTDOWN_OUT_REDUCED: Transition = fades(120, Easing::Linear);
pub const DIGIT_ROLL: Transition = moves(300, Easing::Snappy);
pub const HANDOFF: Transition = moves(200, Easing::Snappy);
pub const HANDOFF_IN: Transition = SNAPPY_SPRING;
pub const HANDOFF_SCALE: f32 = 0.94;
pub const TOOLTIP_DELAY: Duration = Duration::from_millis(700);

const REDUCED_FADE_LIMIT: Duration = Duration::from_millis(120);

impl Transition {
    /// A short fade carries no movement, so it survives Reduce Motion; everything else jumps to its end.
    pub fn plays(self, reduce_motion: bool) -> bool {
        !reduce_motion || (self.fade && self.duration <= REDUCED_FADE_LIMIT)
    }

    pub fn animation(self) -> Animation {
        let easing = self.easing;
        Animation::new(self.duration).with_easing(move |t| easing.at(t))
    }
}

/// A transition timed on the app's clock instead of the wall clock, so tests can step it and its
/// progress can drive painting that is not an element style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clocked {
    transition: Transition,
    started: Instant,
}

impl Clocked {
    /// `None` when Reduce Motion skips the transition.
    pub fn start(transition: Transition, cx: &App) -> Option<Self> {
        transition.plays(reduce_motion(cx)).then(|| Self {
            transition,
            started: cx.background_executor().now(),
        })
    }

    /// Eased from 0 to 1, asking the window for frames until it arrives.
    pub fn progress(&self, window: &mut Window, cx: &App) -> f32 {
        let t = self.linear(cx);
        if t < 1.0 {
            window.request_animation_frame();
        }
        self.transition.easing.at(t)
    }

    /// A spring passes 1 before it is done, so the end is read from the clock, not the value.
    pub fn done(&self, cx: &App) -> bool {
        self.linear(cx) >= 1.0
    }

    /// Eased from 0 to 1, without asking for frames.
    pub fn value(&self, cx: &App) -> f32 {
        self.transition.easing.at(self.linear(cx))
    }

    fn linear(&self, cx: &App) -> f32 {
        if self.transition.duration.is_zero() {
            return 1.0;
        }
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(self.started);
        (elapsed.as_secs_f32() / self.transition.duration.as_secs_f32()).min(1.0)
    }
}

pub fn progress(clocked: Option<Clocked>, window: &mut Window, cx: &App) -> f32 {
    clocked.map_or(1.0, |c| c.progress(window, cx))
}

struct Changed<T> {
    value: Option<T>,
    since: Option<Clocked>,
}

/// Eased progress of `transition` since `value` last changed for the element `key`: 1 when it has not.
pub fn since_change<T: PartialEq + 'static>(
    key: impl Into<ElementId>,
    value: T,
    transition: Transition,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let state = window.use_keyed_state(key, cx, |_, _| Changed {
        value: None,
        since: None,
    });
    let since = state.update(cx, |s, cx| {
        if s.value.as_ref() != Some(&value) {
            if s.value.is_some() {
                s.since = Clocked::start(transition, cx);
            }
            s.value = Some(value);
        }
        s.since
    });
    progress(since, window, cx)
}

struct Tween {
    to: f32,
    from: f32,
    since: Option<Clocked>,
}

impl Tween {
    fn at(&self, cx: &App) -> f32 {
        self.since
            .map_or(self.to, |c| self.from + (self.to - self.from) * c.value(cx))
    }
}

/// `target` for the element `key`, eased over `transition` from wherever it was drawn when the
/// target last changed; the first value shows at once.
pub fn tween(
    key: impl Into<ElementId>,
    target: f32,
    transition: Transition,
    window: &mut Window,
    cx: &mut App,
) -> f32 {
    let state = window.use_keyed_state(key, cx, |_, _| Tween {
        to: target,
        from: target,
        since: None,
    });
    let (at, moving) = state.update(cx, |s, cx| {
        if s.to != target {
            s.from = s.at(cx);
            s.to = target;
            s.since = Clocked::start(transition, cx);
        }
        (s.at(cx), s.since.is_some_and(|c| c.linear(cx) < 1.0))
    });
    if moving {
        window.request_animation_frame();
    }
    at
}

struct Hover {
    on: bool,
    from: f32,
    since: Option<Clocked>,
}

impl Hover {
    fn amount(&self, cx: &App) -> f32 {
        let to = if self.on { 1.0 } else { 0.0 };
        self.since
            .map_or(to, |c| self.from + (to - self.from) * c.value(cx))
    }
}

pub type HoverListener = Box<dyn Fn(&bool, &mut Window, &mut App)>;

/// How far the element `key`'s hover fill is in, 0 to 1, fading over [`HOVER_FILL`]; the listener
/// goes to the element's `on_hover`.
pub fn hover_fade(
    key: impl Into<ElementId>,
    window: &mut Window,
    cx: &mut App,
) -> (f32, HoverListener) {
    let state = window.use_keyed_state(key, cx, |_, _| Hover {
        on: false,
        from: 0.0,
        since: None,
    });
    let amount = state.read(cx).amount(cx);
    if state.read(cx).since.is_some() && amount != 0.0 && amount != 1.0 {
        window.request_animation_frame();
    }
    let listener = Box::new(move |on: &bool, _: &mut Window, cx: &mut App| {
        state.update(cx, |h, cx| {
            if h.on != *on {
                h.from = h.amount(cx);
                h.on = *on;
                h.since = Clocked::start(HOVER_FILL, cx);
                cx.notify();
            }
        });
    });
    (amount, listener)
}

/// `a` mixed `t` of the way to `b`, channel by channel.
pub fn mix(a: Hsla, b: Hsla, t: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let f = |x: f32, y: f32| x + (y - x) * t;
    Rgba {
        r: f(a.r, b.r),
        g: f(a.g, b.g),
        b: f(a.b, b.b),
        a: f(a.a, b.a),
    }
    .into()
}

struct ReduceMotion(bool);

impl Global for ReduceMotion {}

pub fn set_reduce_motion(reduce: bool, cx: &mut App) {
    cx.set_global(ReduceMotion(reduce));
    cx.refresh_windows();
}

pub fn reduce_motion(cx: &App) -> bool {
    cx.try_global::<ReduceMotion>().is_some_and(|r| r.0)
}

/// Plain element tooltips and the kit's managed ones keep separate clocks, so both are set. Neither
/// stays warm: a tooltip that opened at once on the next control reads as one jumping across.
pub fn install_tooltip_timing(cx: &mut App) {
    cx.set_global(TooltipTiming {
        delay: TOOLTIP_DELAY,
        warm: Duration::ZERO,
    });
    TooltipDefaults::new()
        .with_show_delay(TOOLTIP_DELAY)
        .with_grace_period(Duration::ZERO)
        .install(cx);
}

/// GPUI's own reduce-motion switch would also stop the short fades, so this gate is kept separately.
pub fn animate<E: IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    transition: Transition,
    cx: &App,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if transition.plays(reduce_motion(cx)) {
        element
            .with_animation(id, transition.animation(), animator)
            .into_any_element()
    } else {
        animator(element, 1.0).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_easing_starts_at_rest_and_arrives() {
        for easing in [
            Easing::Linear,
            Easing::EaseOut,
            Easing::EaseIn,
            Easing::Snappy,
        ] {
            assert!(easing.at(0.0).abs() < 1e-4, "{easing:?}");
            assert!((easing.at(1.0) - 1.0).abs() < 1e-4, "{easing:?}");
            let samples: Vec<f32> = (0..=20).map(|i| easing.at(i as f32 / 20.0)).collect();
            assert!(
                samples.windows(2).all(|w| w[1] >= w[0] - 1e-5),
                "{easing:?} goes backwards"
            );
        }
    }

    #[test]
    fn the_curves_have_their_character() {
        assert!(
            Easing::EaseOut.at(0.5) > 0.5,
            "ease-out is ahead at the midpoint"
        );
        assert!(Easing::EaseIn.at(0.5) < 0.5, "ease-in is behind");
        assert!(
            Easing::Snappy.at(0.25) > Easing::EaseOut.at(0.25),
            "snappy covers more ground early than ease-out"
        );
        assert!((Easing::Snappy.at(0.5) - 0.93).abs() < 0.02);
    }

    fn curve(t: Transition) -> Vec<f32> {
        (0..=400).map(|i| t.easing.at(i as f32 / 400.0)).collect()
    }

    fn spring(t: Transition) -> Spring {
        match t.easing {
            Easing::Spring(s) => s,
            other => panic!("{other:?} is not a spring"),
        }
    }

    #[test]
    fn a_spring_starts_at_rest_overshoots_by_its_damping_and_settles_by_its_end() {
        for (token, overshoot) in [
            (SNAPPY_SPRING, 0.028),
            (BOUNCY_SPRING, 0.084),
            (GENTLE_SPRING, 0.006),
            (CALM_SPRING, 0.006),
        ] {
            let samples = curve(token);
            assert_eq!(samples[0], 0.0, "{token:?}");
            assert!(
                samples[4] < 0.05,
                "{token:?} leaves at rest, gathering speed"
            );
            assert_eq!(*samples.last().unwrap_or(&0.0), 1.0, "{token:?}");
            let peak = samples.iter().copied().fold(0.0, f32::max);
            assert!(
                (peak - 1.0 - overshoot).abs() < 0.004,
                "{token:?} swings {} past its end, not {overshoot}",
                peak - 1.0
            );
            assert!((spring(token).overshoot() - overshoot).abs() < 0.002);
            let tail = &samples[samples.len() - 20..];
            assert!(
                tail.iter().all(|v| (v - 1.0).abs() < 0.006),
                "{token:?} has come to rest before its duration is up, so the cut does not show"
            );
        }
    }

    #[test]
    fn a_livelier_spring_swings_further_and_the_snappy_one_arrives_first() {
        let first_past = |t: Transition| curve(t).iter().position(|v| *v >= 1.0);
        let ms =
            |t: Transition, i: Option<usize>| i.map(|i| t.duration.as_millis() as usize * i / 400);
        assert!(
            spring(BOUNCY_SPRING).overshoot() > spring(SNAPPY_SPRING).overshoot()
                && spring(SNAPPY_SPRING).overshoot() > spring(GENTLE_SPRING).overshoot()
        );
        let snappy = ms(SNAPPY_SPRING, first_past(SNAPPY_SPRING));
        let bouncy = ms(BOUNCY_SPRING, first_past(BOUNCY_SPRING));
        assert!(
            snappy.is_some_and(|s| s < 200) && snappy < bouncy,
            "snappy reaches its end in {snappy:?} ms, bouncy in {bouncy:?} ms"
        );
        assert!(!SNAPPY_SPRING.plays(true) && !BOUNCY_SPRING.plays(true));
    }

    #[test]
    fn a_whole_page_moves_half_again_as_slowly_as_a_control_and_settles_the_same_way() {
        assert_eq!(curve(TAB_SLIDE), curve(GENTLE_SPRING));
        assert_eq!(TAB_SLIDE.duration * 2, GENTLE_SPRING.duration * 3);
        assert_eq!(SETTINGS_PAGE.duration * 2, PAGE_CHANGE.duration * 3);
        assert_eq!(SETTINGS_PAGE.easing, PAGE_CHANGE.easing);
        assert!(!TAB_SLIDE.plays(true) && !SETTINGS_PAGE.plays(true));
    }

    #[test]
    fn reduce_motion_keeps_only_short_fades() {
        assert!(SLIDER_MARKERS.plays(true));
        assert!(DECK_HOVER.plays(true));
        assert!(
            !TOAST_IN.plays(true) && !TOAST_OUT.plays(true) && TOAST_REDUCED.plays(true),
            "a toast only fades in and out under Reduce Motion"
        );
        assert!(
            !POPOVER_OPEN.plays(true),
            "the popover grows, which is movement"
        );
        assert!(!SECTION_EXPAND.plays(true));
        assert!(
            COUNTDOWN_IN_REDUCED.plays(true) && COUNTDOWN_OUT_REDUCED.plays(true),
            "the countdown keeps a short fade in and out"
        );
        assert!(!COUNTDOWN_IN.plays(true) && !COUNTDOWN_OUT.plays(true) && !DIGIT_ROLL.plays(true));
        assert!(
            !CROSSFADE.plays(true) && !FILL_SWITCH.plays(true) && !COMPOSITE_FADE.plays(true),
            "a cross-fade over 120 ms is too long to keep"
        );
        for t in [CROP_RESHAPE, KNOB_GROW, BUTTON_POP, NOTICE_EXPAND] {
            assert!(!t.plays(true), "{t:?} moves, so Reduce Motion skips it");
        }
        for t in [POPOVER_OPEN, TOAST_IN, TOAST_OUT, SECTION_EXPAND, FLASH] {
            assert!(t.plays(false));
        }
    }

    #[test]
    fn the_timed_motions_keep_their_durations() {
        let ms = |t: Transition| t.duration.as_millis();
        assert_eq!(
            (ms(POPOVER_OPEN), POPOVER_OPEN.easing),
            (130, Easing::EaseOut)
        );
        assert_eq!(ms(DECK_HOVER), 120);
        assert_eq!(
            (ms(TOAST_IN), TOAST_RISE, ms(TOAST_OUT)),
            (360, px(6.), 160)
        );
        assert_eq!(
            (ms(SECTION_EXPAND), SECTION_EXPAND.easing),
            (180, Easing::Snappy)
        );
        assert_eq!((ms(FLASH), FLASH_HOLD.as_millis()), (200, 1400));
        assert_eq!((ms(CROSSFADE), CROSSFADE.easing), (150, Easing::EaseOut));
        assert_eq!(
            (ms(COMPOSITE_FADE), COMPOSITE_FADE.easing),
            (180, Easing::EaseOut)
        );
        assert_eq!((ms(FILL_SWITCH), FILL_SWITCH.easing), (160, Easing::Snappy));
    }
}
