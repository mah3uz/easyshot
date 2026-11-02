use crate::math::{easing, modulate};
use std::f64::consts::PI;

pub type Easing = fn(f64) -> f64;

#[derive(Debug, Clone, Copy)]
pub struct End {
    pub cap: bool,
    pub taper: Option<f64>,
    pub easing: Option<Easing>,
}

impl Default for End {
    fn default() -> Self {
        Self {
            cap: true,
            taper: None,
            easing: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StrokeOptions {
    pub size: f64,
    pub thinning: f64,
    pub smoothing: f64,
    pub streamline: f64,
    pub easing: Easing,
    pub simulate_pressure: bool,
    pub start: End,
    pub end: End,
    pub last: bool,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            size: 16.0,
            thinning: 0.5,
            smoothing: 0.5,
            streamline: 0.5,
            easing: easing::linear,
            simulate_pressure: true,
            start: End::default(),
            end: End::default(),
            last: false,
        }
    }
}

pub fn pen_easing(t: f64) -> f64 {
    t * 0.65 + (t * PI / 2.0).sin() * 0.35
}

pub(crate) fn resolve_taper(taper: Option<f64>, size: f64, total_length: f64) -> f64 {
    match taper {
        None => 0.0,
        Some(0.0) => 0.0,
        Some(t) if t.is_infinite() => size.max(total_length),
        Some(t) => t,
    }
}

impl StrokeOptions {
    pub fn simulated_pressure(stroke_width: f64) -> Self {
        Self {
            size: stroke_width,
            thinning: 0.5,
            smoothing: 0.62,
            streamline: modulate(stroke_width, (9.0, 16.0), (0.64, 0.74), true),
            easing: easing::ease_out_sine,
            simulate_pressure: true,
            ..Self::default()
        }
    }

    pub fn real_pressure(stroke_width: f64) -> Self {
        Self {
            size: 1.0 + stroke_width * 1.2,
            thinning: 0.62,
            smoothing: 0.62,
            streamline: 0.62,
            easing: pen_easing,
            simulate_pressure: false,
            ..Self::default()
        }
    }

    pub fn solid(stroke_width: f64) -> Self {
        Self {
            size: stroke_width,
            thinning: 0.0,
            smoothing: 0.62,
            streamline: modulate(stroke_width, (9.0, 16.0), (0.64, 0.74), true),
            easing: easing::linear,
            simulate_pressure: false,
            ..Self::default()
        }
    }

    pub fn solid_real_pressure(stroke_width: f64) -> Self {
        Self {
            size: stroke_width,
            thinning: 0.0,
            smoothing: 0.62,
            streamline: 0.62,
            easing: easing::linear,
            simulate_pressure: false,
            ..Self::default()
        }
    }

    pub fn for_draw_shape(is_pen: bool, stroke_width: f64, last: bool, force_solid: bool) -> Self {
        let mut o = match (force_solid, is_pen) {
            (true, true) => Self::solid_real_pressure(stroke_width),
            (true, false) => Self::solid(stroke_width),
            (false, true) => Self::real_pressure(stroke_width),
            (false, false) => Self::simulated_pressure(stroke_width),
        };
        o.last = last;
        o
    }

    pub fn with_last(mut self, last: bool) -> Self {
        self.last = last;
        self
    }
}
