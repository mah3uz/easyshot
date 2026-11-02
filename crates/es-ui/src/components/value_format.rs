use gpui::SharedString;

use crate::fl;

/// The model keeps its own units; `multiplier` only changes what is shown and typed, so 0.45 reads as 45%.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueFormat {
    pub multiplier: f64,
    pub fraction_digits: usize,
    pub suffix: SharedString,
    pub signed: bool,
    pub step: f64,
    pub accepted_suffixes: Vec<SharedString>,
    pub named: Vec<(f64, SharedString)>,
}

fn step_for(digits: usize) -> f64 {
    10f64.powi(-(digits as i32))
}

fn suffixes(unit: &str, aliases: &[&'static str]) -> Vec<SharedString> {
    std::iter::once(SharedString::from(unit.to_owned()))
        .chain(aliases.iter().map(|a| SharedString::new_static(a)))
        .collect()
}

impl ValueFormat {
    fn new(multiplier: f64, digits: usize, suffix: String, accepted: Vec<SharedString>) -> Self {
        Self {
            multiplier,
            fraction_digits: digits,
            suffix: suffix.into(),
            signed: false,
            step: step_for(digits) / multiplier,
            accepted_suffixes: accepted,
            named: Vec::new(),
        }
    }

    pub fn integer() -> Self {
        Self::new(1., 0, String::new(), Vec::new())
    }

    pub fn decimal(digits: usize) -> Self {
        Self::new(1., digits, String::new(), Vec::new())
    }

    pub fn pixels() -> Self {
        let unit = fl!("controls-unit-pixels");
        let accepted = suffixes(&unit, &["pixels", "pixel", "px"]);
        Self::new(1., 0, format!(" {unit}"), accepted)
    }

    pub fn points() -> Self {
        let unit = fl!("controls-unit-points");
        let accepted = suffixes(&unit, &["points", "point", "pt"]);
        Self::new(1., 0, format!(" {unit}"), accepted).step(4.)
    }

    pub fn percent() -> Self {
        let unit = fl!("controls-unit-percent");
        let accepted = suffixes(&unit, &["%"]);
        Self::new(100., 0, unit, accepted)
    }

    pub fn degrees() -> Self {
        let unit = fl!("controls-unit-degrees");
        let accepted = suffixes(&unit, &["degrees", "degree", "deg", "°"]);
        Self::new(1., 0, unit, accepted)
    }

    pub fn magnification(digits: usize) -> Self {
        let unit = fl!("controls-unit-times");
        let accepted = suffixes(&unit, &["×", "x"]);
        Self::new(1., digits, unit, accepted)
    }

    pub fn seconds() -> Self {
        let unit = fl!("controls-unit-seconds");
        let accepted = suffixes(&unit, &["seconds", "second", "s"]);
        Self::new(1., 0, unit, accepted)
    }

    /// Changes the shown precision and, with it, the step.
    pub fn digits(mut self, digits: usize) -> Self {
        self.fraction_digits = digits;
        self.step = step_for(digits) / self.multiplier;
        self
    }

    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    pub fn named(mut self, value: f64, name: impl Into<SharedString>) -> Self {
        self.named.push((value, name.into()));
        self
    }

    fn rounded(&self, scaled: f64) -> f64 {
        let scale = 10f64.powi(self.fraction_digits as i32);
        let rounded = (scaled * scale).round() / scale;
        if rounded.abs() < f64::EPSILON {
            0.
        } else {
            rounded
        }
    }

    fn number(&self, value: f64) -> String {
        let rounded = self.rounded(value * self.multiplier);
        format!("{rounded:.*}", self.fraction_digits)
    }

    pub fn display(&self, value: f64) -> SharedString {
        if let Some((_, name)) = self.named.iter().find(|(v, _)| *v == value) {
            return name.clone();
        }
        let sign = if self.signed && self.rounded(value * self.multiplier) > 0. {
            "+"
        } else {
            ""
        };
        format!("{sign}{}{}", self.number(value), self.suffix).into()
    }

    /// What the field holds while it is edited: the bare number, so typing replaces it cleanly.
    pub fn editing(&self, value: f64) -> String {
        self.number(value)
    }

    pub fn parse(&self, text: &str) -> Option<f64> {
        let text = text.trim();
        if let Some((value, _)) = self
            .named
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(text))
        {
            return Some(*value);
        }
        let mut number = text.replace('−', "-");
        for suffix in &self.accepted_suffixes {
            let suffix = suffix.trim();
            if suffix.is_empty() {
                continue;
            }
            let lower = number.to_lowercase();
            if lower.ends_with(&suffix.to_lowercase()) {
                let cut = lower.len() - suffix.to_lowercase().len();
                if number.is_char_boundary(cut) {
                    number.truncate(cut);
                }
            }
        }
        let parsed: f64 = number.trim().parse().ok()?;
        parsed.is_finite().then(|| parsed / self.multiplier)
    }
}
