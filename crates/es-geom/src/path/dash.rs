use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DashStyle {
    Draw,
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DashTerminal {
    #[default]
    Outset,
    Skip,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DashOptions {
    pub snap: u32,
    pub start: DashTerminal,
    pub end: DashTerminal,
    pub length_ratio: f64,
    pub closed: bool,
}

impl Default for DashOptions {
    fn default() -> Self {
        Self {
            snap: 1,
            start: DashTerminal::Outset,
            end: DashTerminal::Outset,
            length_ratio: 2.0,
            closed: false,
        }
    }
}

/// Empty `dashes` means a solid stroke. `phase` is the negated SVG dash offset.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DashProps {
    pub dashes: Vec<f64>,
    pub phase: f64,
}

pub fn perfect_dash_props(
    total_length: f64,
    stroke_width: f64,
    style: DashStyle,
    opts: &DashOptions,
) -> DashProps {
    let none = DashProps::default();
    let mut total = total_length;
    let mut stroke_dash_offset = 0.0;
    let ratio;
    let mut dash_length;
    match style {
        DashStyle::Dashed => {
            ratio = 1.0;
            dash_length = (stroke_width * opts.length_ratio).min(total / 4.0);
        }
        DashStyle::Dotted => {
            ratio = 100.0;
            dash_length = stroke_width / ratio;
        }
        DashStyle::Solid | DashStyle::Draw => return none,
    }
    if dash_length <= 0.0 || dash_length.is_nan() {
        return none;
    }

    if !opts.closed {
        match opts.start {
            DashTerminal::Outset => {
                total += dash_length / 2.0;
                stroke_dash_offset += dash_length / 2.0;
            }
            DashTerminal::Skip => {
                total -= dash_length;
                stroke_dash_offset -= dash_length;
            }
            DashTerminal::None => {}
        }
        match opts.end {
            DashTerminal::Outset => total += dash_length / 2.0,
            DashTerminal::Skip => total -= dash_length,
            DashTerminal::None => {}
        }
    }

    let mut dash_count = (total / dash_length / (2.0 * ratio)).floor() as i64;
    dash_count -= dash_count % i64::from(opts.snap.max(1));

    let gap_length;
    if dash_count < 3 && style == DashStyle::Dashed {
        if total / stroke_width < 4.0 {
            dash_length = total;
            gap_length = 0.0;
        } else {
            dash_length = total / 3.0;
            gap_length = total / 3.0;
        }
    } else {
        if dash_count <= 0 {
            return none;
        }
        let count = dash_count as f64;
        dash_length = total / count / (2.0 * ratio);
        if opts.closed {
            stroke_dash_offset = dash_length / 2.0;
            gap_length = (total - count * dash_length) / count;
        } else {
            gap_length = (total - count * dash_length) / (dash_count - 1).max(1) as f64;
        }
    }

    if !dash_length.is_finite() || !gap_length.is_finite() || dash_length <= 0.0 {
        return none;
    }
    DashProps {
        dashes: vec![dash_length, gap_length.max(0.0)],
        phase: -stroke_dash_offset,
    }
}
