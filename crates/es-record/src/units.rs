use es_geom::Rect;
use es_model::platform::{OutputId, OutputInfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GsrMonitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One output's logical rectangle and the rectangle the recorder printed for it.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorMap {
    pub output: OutputId,
    pub logical: Rect,
    pub gsr: GsrMonitor,
}

/// `Logical` passes global logical coordinates, the form slurp prints.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum RegionUnits {
    #[default]
    Logical,
    Mapped(Vec<MonitorMap>),
}

fn parse_rect(s: &str) -> Option<(u32, u32, i32, i32)> {
    let (w, rest) = s.split_once('x')?;
    let hend = rest.find(['+', '-'])?;
    let (h, pos) = rest.split_at(hend);
    let yat = pos[1..].find(['+', '-'])? + 1;
    let (x, y) = pos.split_at(yat);
    Some((
        w.parse().ok()?,
        h.parse().ok()?,
        x.parse().ok()?,
        y.parse().ok()?,
    ))
}

pub fn parse_monitor_list(text: &str) -> Vec<GsrMonitor> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let inner = &rest[open + 1..open + close];
        let name = rest[..open]
            .trim_end()
            .trim_end_matches('"')
            .rsplit(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | '"'))
            .next()
            .unwrap_or("");
        if let (Some((width, height, x, y)), false) = (parse_rect(inner), name.is_empty()) {
            out.push(GsrMonitor {
                name: name.to_owned(),
                x,
                y,
                width,
                height,
            });
        }
        rest = &rest[open + close + 1..];
    }
    out
}

pub fn decide_units(printed: &[GsrMonitor], outputs: &[OutputInfo]) -> RegionUnits {
    let maps: Vec<MonitorMap> = outputs
        .iter()
        .filter_map(|o| {
            let g = printed.iter().find(|g| g.name == o.id.0)?;
            (g.width > 0 && g.height > 0 && o.logical.width() > 0.0 && o.logical.height() > 0.0)
                .then(|| MonitorMap {
                    output: o.id.clone(),
                    logical: o.logical,
                    gsr: g.clone(),
                })
        })
        .collect();
    if maps.is_empty() {
        RegionUnits::Logical
    } else {
        RegionUnits::Mapped(maps)
    }
}

/// With one output at the origin and scale 1, every coordinate system agrees.
pub fn needs_unit_probe(outputs: &[OutputInfo]) -> bool {
    outputs
        .iter()
        .any(|o| (o.scale - 1.0).abs() > f64::EPSILON || o.logical.x0 != 0.0 || o.logical.y0 != 0.0)
}

impl RegionUnits {
    /// `logical` is in global logical points.
    pub fn region_target(&self, output: &OutputId, logical: Rect) -> String {
        let plain = (logical.x0, logical.y0, logical.width(), logical.height());
        let map = match self {
            RegionUnits::Mapped(maps) => maps.iter().find(|m| &m.output == output),
            RegionUnits::Logical => None,
        };
        let (x, y, w, h) = match map {
            Some(m) => {
                let sx = f64::from(m.gsr.width) / m.logical.width();
                let sy = f64::from(m.gsr.height) / m.logical.height();
                let x0 = f64::from(m.gsr.x) + (logical.x0 - m.logical.x0) * sx;
                let y0 = f64::from(m.gsr.y) + (logical.y0 - m.logical.y0) * sy;
                let x1 = f64::from(m.gsr.x) + (logical.x1 - m.logical.x0) * sx;
                let y1 = f64::from(m.gsr.y) + (logical.y1 - m.logical.y0) * sy;
                (
                    x0.round(),
                    y0.round(),
                    x1.round() - x0.round(),
                    y1.round() - y0.round(),
                )
            }
            None => plain,
        };
        format!("{}x{}+{}+{}", w.round(), h.round(), x.round(), y.round())
    }
}
