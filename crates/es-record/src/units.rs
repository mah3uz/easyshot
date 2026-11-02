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

#[derive(Debug, Clone, PartialEq)]
pub struct PhysicalMap {
    pub output: OutputId,
    pub logical: Rect,
    pub scale: f64,
    pub gsr: GsrMonitor,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum RegionUnits {
    #[default]
    Logical,
    Physical(Vec<PhysicalMap>),
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
            .rsplit(|c: char| c.is_whitespace() || c == ',' || c == ':')
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
    let mut maps = Vec::new();
    for o in outputs {
        let Some(g) = printed.iter().find(|g| g.name == o.id.0) else {
            continue;
        };
        let logical = (
            o.logical.width().round() as u32,
            o.logical.height().round() as u32,
        );
        if (g.width, g.height) != logical && (g.width, g.height) == o.physical {
            maps.push(PhysicalMap {
                output: o.id.clone(),
                logical: o.logical,
                scale: o.scale,
                gsr: g.clone(),
            });
        }
    }
    if maps.is_empty() {
        RegionUnits::Logical
    } else {
        RegionUnits::Physical(maps)
    }
}

pub fn needs_unit_probe(outputs: &[OutputInfo]) -> bool {
    outputs.iter().any(|o| (o.scale - 1.0).abs() > f64::EPSILON)
}

impl RegionUnits {
    pub fn region_target(&self, output: &OutputId, logical: Rect) -> String {
        let plain = (logical.x0, logical.y0, logical.width(), logical.height());
        let (x, y, w, h) = match self {
            RegionUnits::Physical(maps) => match maps.iter().find(|m| &m.output == output) {
                Some(m) => (
                    m.gsr.x as f64 + (logical.x0 - m.logical.x0) * m.scale,
                    m.gsr.y as f64 + (logical.y0 - m.logical.y0) * m.scale,
                    logical.width() * m.scale,
                    logical.height() * m.scale,
                ),
                None => plain,
            },
            RegionUnits::Logical => plain,
        };
        format!("{}x{}+{}+{}", w.round(), h.round(), x.round(), y.round())
    }
}
