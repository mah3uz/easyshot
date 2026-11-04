use es_geom::PVec;
use es_model::style::page_stroke_width;
use es_model::{
    AnnoShape, AnnotationDocument, Arrowhead, BackgroundColor, BackgroundGradient,
    BackgroundSettings, BackgroundStyle, FillStyle, GeoKind, GeoProps, HighlighterProps,
    NumberedProps, RedactionKind, RedactionProps, ShapeKind, StylePreset, Swatch, TextProps,
    UnitPoint,
};
use serde::Deserialize;
use serde_json::Value;

pub(super) const VERSION: u32 = 1;
const MAX_OPS: usize = 500;
const MAX_COORD: f64 = 1_000_000.0;
const MAX_TEXT_CHARS: usize = 2000;
const COUNTER_MIN: f64 = 24.0;
const COUNTER_FRACTION: f64 = 0.039;
const MAX_GRADIENT_COLORS: usize = 8;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    ops: Vec<Value>,
    #[serde(default)]
    background: Option<Background>,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    Arrow(Arrow),
    Rect(Boxed),
    Ellipse(Boxed),
    Text(Text),
    Counter(Counter),
    Highlight(Area),
    Blur(Redact),
    Pixelate(Redact),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arrow {
    from: Vec<f64>,
    to: Vec<f64>,
    color: Option<String>,
    width: Option<f64>,
    head: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Boxed {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Option<String>,
    width: Option<f64>,
    fill: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    x: f64,
    y: f64,
    text: String,
    size: Option<f64>,
    color: Option<String>,
    background: Option<String>,
    font: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counter {
    x: f64,
    y: f64,
    n: Option<u32>,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Area {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Redact {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    amount: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Background {
    padding: Option<f64>,
    color: Option<String>,
    gradient: Option<Vec<String>>,
    corners: Option<f64>,
    shadow: Option<f64>,
}

struct Scope<'a> {
    style: &'a StylePreset,
    size: (f64, f64),
    index: usize,
    op: &'static str,
}

impl Scope<'_> {
    fn fail<T>(&self, message: impl std::fmt::Display) -> Result<T, String> {
        Err(format!("ops[{}] ({}): {message}", self.index, self.op))
    }

    fn edge(&self) -> f64 {
        self.size.0.max(self.size.1)
    }

    fn coord(&self, field: &str, v: f64) -> Result<f64, String> {
        if v.is_finite() && v.abs() <= MAX_COORD {
            Ok(v)
        } else {
            self.fail(format!("{field} must be a number within +-{MAX_COORD}"))
        }
    }

    fn point(&self, field: &str, v: &[f64]) -> Result<(f64, f64), String> {
        match v {
            [x, y] => Ok((self.coord(field, *x)?, self.coord(field, *y)?)),
            _ => self.fail(format!("{field} must be two numbers, [x, y]")),
        }
    }

    fn extent(&self, field: &str, v: f64) -> Result<f64, String> {
        if v.is_finite() && v > 0.0 && v <= MAX_COORD {
            Ok(v)
        } else {
            self.fail(format!(
                "{field} must be greater than 0 and at most {MAX_COORD}"
            ))
        }
    }

    fn color(&self, field: &str, text: Option<&str>, default: &Swatch) -> Result<Swatch, String> {
        match text {
            None => Ok(default.clone()),
            Some(t) => parse_color(t).ok_or_else(|| {
                format!(
                    "ops[{}] ({}): {field} must be #rrggbb or #rrggbbaa, not {t:?}",
                    self.index, self.op
                )
            }),
        }
    }

    fn stroke(&self, width: Option<f64>) -> Result<f64, String> {
        match width {
            None => Ok(page_stroke_width(
                self.style.stroke_width,
                self.size.0,
                self.size.1,
            )),
            Some(w) if w.is_finite() && (0.5..=self.edge()).contains(&w) => Ok(w),
            Some(_) => self.fail("width must be a number from 0.5 to the image's longest edge"),
        }
    }

    fn fraction(&self, field: &str, v: Option<f64>, default: f64) -> Result<f64, String> {
        match v {
            None => Ok(default),
            Some(v) if v.is_finite() && (0.0..=1.0).contains(&v) => Ok(v),
            Some(_) => self.fail(format!("{field} must be a number from 0 to 1")),
        }
    }
}

pub(super) fn parse_color(text: &str) -> Option<Swatch> {
    let hex = text.strip_prefix('#')?;
    if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let alpha = if hex.len() == 8 { byte(6)? } else { 255 };
    Some(Swatch::custom(byte(0)?, byte(2)?, byte(4)?, alpha))
}

fn head(scope: &Scope, text: Option<&str>) -> Result<Arrowhead, String> {
    let default = match scope.style.arrowhead_end {
        Arrowhead::None => Arrowhead::Arrow,
        other => other,
    };
    let Some(text) = text else {
        return Ok(default);
    };
    Ok(match text {
        "none" => Arrowhead::None,
        "arrow" => Arrowhead::Arrow,
        "triangle" => Arrowhead::Triangle,
        "square" => Arrowhead::Square,
        "dot" => Arrowhead::Dot,
        "diamond" => Arrowhead::Diamond,
        "inverted" => Arrowhead::Inverted,
        "bar" => Arrowhead::Bar,
        _ => {
            return scope
                .fail("head must be none, arrow, triangle, square, dot, diamond, inverted or bar");
        }
    })
}

/// The annotations and look the ops file describes, over an image of `size` pixels.
pub(super) fn build(
    text: &str,
    size: (u32, u32),
    style: &StylePreset,
) -> Result<(AnnotationDocument, usize), String> {
    let file: File = serde_json::from_str(text).map_err(|e| format!("the ops file: {e}"))?;
    if file.version != VERSION {
        return Err(format!(
            "the ops file: version must be {VERSION}, not {}",
            file.version
        ));
    }
    if file.ops.len() > MAX_OPS {
        return Err(format!(
            "the ops file: at most {MAX_OPS} ops, got {}",
            file.ops.len()
        ));
    }
    let size = (f64::from(size.0), f64::from(size.1));
    let mut doc = AnnotationDocument::default();
    for (index, value) in file.ops.iter().enumerate() {
        let name = value.get("op").and_then(Value::as_str).unwrap_or("?");
        let op: Op = Op::deserialize(value).map_err(|e| format!("ops[{index}] ({name}): {e}"))?;
        let scope = Scope {
            style,
            size,
            index,
            op: op_name(&op),
        };
        let shape = shape(&scope, op, &doc)?;
        doc.add_shape(shape);
    }
    if let Some(background) = &file.background {
        doc.background = background_settings(background, size)?;
    }
    Ok((doc, file.ops.len()))
}

fn op_name(op: &Op) -> &'static str {
    match op {
        Op::Arrow(_) => "arrow",
        Op::Rect(_) => "rect",
        Op::Ellipse(_) => "ellipse",
        Op::Text(_) => "text",
        Op::Counter(_) => "counter",
        Op::Highlight(_) => "highlight",
        Op::Blur(_) => "blur",
        Op::Pixelate(_) => "pixelate",
    }
}

fn shape(scope: &Scope, op: Op, doc: &AnnotationDocument) -> Result<AnnoShape, String> {
    let style = scope.style;
    match op {
        Op::Arrow(a) => {
            let (fx, fy) = scope.point("from", &a.from)?;
            let (tx, ty) = scope.point("to", &a.to)?;
            if (tx - fx).hypot(ty - fy) < 1.0 {
                return scope.fail("from and to must be at least 1 pixel apart");
            }
            let props = es_model::ArrowProps {
                start: PVec::new(0.0, 0.0),
                end: PVec::new(tx - fx, ty - fy),
                arrowhead_start: style.arrowhead_start,
                arrowhead_end: head(scope, a.head.as_deref())?,
                swatch: scope.color("color", a.color.as_deref(), &style.swatch)?,
                stroke_width: scope.stroke(a.width)?,
                dash: style.dash,
                ..es_model::ArrowProps::default()
            };
            Ok(AnnoShape::new(fx, fy, ShapeKind::Arrow(props)))
        }
        Op::Rect(b) => boxed(scope, &b, GeoKind::Rectangle),
        Op::Ellipse(b) => boxed(scope, &b, GeoKind::Ellipse),
        Op::Text(t) => {
            let (x, y) = (scope.coord("x", t.x)?, scope.coord("y", t.y)?);
            let chars = t.text.chars().count();
            if chars == 0 || chars > MAX_TEXT_CHARS {
                return scope.fail(format!("text must have 1 to {MAX_TEXT_CHARS} characters"));
            }
            let font_size = match t.size {
                None => style.text_font_size,
                Some(s) if s.is_finite() && (4.0..=2000.0).contains(&s) => s,
                Some(_) => return scope.fail("size must be a number from 4 to 2000"),
            };
            let fill = match t.background.as_deref() {
                Some(c) => Some(scope.color("background", Some(c), &style.swatch)?),
                None => style.text_fill.clone(),
            };
            let props = TextProps {
                text: t.text,
                swatch: scope.color("color", t.color.as_deref(), &style.swatch)?,
                font_size,
                font_family: t
                    .font
                    .map_or_else(|| style.text_font_family.clone(), font_family),
                is_bold: style.text_bold,
                is_italic: style.text_italic,
                is_underline: style.text_underline,
                align: style.text_align,
                fill,
                border: style.text_border.clone(),
                ..TextProps::default()
            };
            Ok(AnnoShape::new(x, y, ShapeKind::Text(props)))
        }
        Op::Counter(c) => {
            let (x, y) = (scope.coord("x", c.x)?, scope.coord("y", c.y)?);
            let value = match c.n {
                None => doc.next_numbered_value(),
                Some(n) if (1..=9999).contains(&n) => n,
                Some(_) => return scope.fail("n must be a whole number from 1 to 9999"),
            };
            let diameter = COUNTER_MIN.max(scope.edge() * COUNTER_FRACTION);
            let props = NumberedProps {
                value,
                diameter,
                swatch: scope.color("color", c.color.as_deref(), &style.swatch)?,
            };
            Ok(AnnoShape::new(
                x - diameter / 2.0,
                y - diameter / 2.0,
                ShapeKind::Numbered(props),
            ))
        }
        Op::Highlight(a) => {
            let (x, y) = (scope.coord("x", a.x)?, scope.coord("y", a.y)?);
            let (w, h) = (scope.extent("w", a.w)?, scope.extent("h", a.h)?);
            let half = (w - h).max(1.0) / 2.0;
            let mid = h / 2.0;
            let props = HighlighterProps {
                points: vec![
                    PVec::new(w / 2.0 - half, mid),
                    PVec::new(w / 2.0 + half, mid),
                ],
                swatch: scope.color("color", a.color.as_deref(), &Swatch::yellow())?,
                stroke_width: h,
            };
            Ok(AnnoShape::new(x, y, ShapeKind::Highlighter(props)))
        }
        Op::Blur(r) => redaction(scope, &r, RedactionKind::Blur),
        Op::Pixelate(r) => redaction(scope, &r, RedactionKind::Pixelate),
    }
}

fn boxed(scope: &Scope, b: &Boxed, geo: GeoKind) -> Result<AnnoShape, String> {
    let (x, y) = (scope.coord("x", b.x)?, scope.coord("y", b.y)?);
    let props = GeoProps {
        geo,
        w: scope.extent("w", b.w)?,
        h: scope.extent("h", b.h)?,
        swatch: scope.color("color", b.color.as_deref(), &scope.style.swatch)?,
        stroke_width: scope.stroke(b.width)?,
        fill: if b.fill == Some(true) {
            FillStyle::Solid
        } else {
            FillStyle::None
        },
        dash: scope.style.dash,
        ..GeoProps::default()
    };
    Ok(AnnoShape::new(x, y, ShapeKind::Geo(props)))
}

fn redaction(scope: &Scope, r: &Redact, kind: RedactionKind) -> Result<AnnoShape, String> {
    let (x, y) = (scope.coord("x", r.x)?, scope.coord("y", r.y)?);
    let props = RedactionProps {
        kind,
        w: scope.extent("w", r.w)?,
        h: scope.extent("h", r.h)?,
        density: scope.fraction("amount", r.amount, scope.style.redaction_density)?,
    };
    Ok(AnnoShape::new(x, y, ShapeKind::Redaction(props)))
}

fn background_settings(b: &Background, size: (f64, f64)) -> Result<BackgroundSettings, String> {
    let fail = |m: &str| Err(format!("background: {m}"));
    let short = size.0.min(size.1);
    let edge = size.0.max(size.1);
    let mut look = BackgroundSettings::default();
    let color = |text: &str, field: &str| {
        parse_color(text)
            .map(|s| BackgroundColor {
                alpha: s.alpha,
                ..BackgroundColor::custom(s.red, s.green, s.blue)
            })
            .ok_or_else(|| {
                format!("background: {field} must be #rrggbb or #rrggbbaa, not {text:?}")
            })
    };
    look.style = match (&b.color, &b.gradient) {
        (Some(_), Some(_)) => return fail("give color or gradient, not both"),
        (Some(c), None) => BackgroundStyle::Solid(color(c, "color")?),
        (None, Some(list)) => {
            if !(2..=MAX_GRADIENT_COLORS).contains(&list.len()) {
                return fail(&format!("gradient needs 2 to {MAX_GRADIENT_COLORS} colors"));
            }
            let colors = list
                .iter()
                .map(|c| color(c, "gradient"))
                .collect::<Result<Vec<_>, _>>()?;
            BackgroundStyle::Gradient(BackgroundGradient {
                id: "custom".into(),
                title: "Custom".into(),
                colors,
                start: UnitPoint { x: 0.0, y: 0.0 },
                end: UnitPoint { x: 1.0, y: 1.0 },
            })
        }
        (None, None) => BackgroundStyle::Solid(BackgroundColor::new(
            "graphite", "Graphite", 0.17, 0.18, 0.21,
        )),
    };
    if let Some(p) = b.padding {
        if !p.is_finite() || !(0.0..=edge * 2.0).contains(&p) {
            return fail("padding must be a number of pixels from 0 to twice the longest edge");
        }
        look.padding = p / short;
    }
    if let Some(c) = b.corners {
        if !c.is_finite() || !(0.0..=short / 2.0).contains(&c) {
            return fail("corners must be a radius in pixels from 0 to half the shorter edge");
        }
        look.corner_radius = c / short;
    }
    if let Some(s) = b.shadow {
        if !s.is_finite() || !(0.0..=1.0).contains(&s) {
            return fail("shadow must be a number from 0 to 1");
        }
        look.shadow = s;
    }
    Ok(look)
}

/// A bundled family by its picker name, any other name as an installed family.
fn font_family(name: String) -> es_model::FontFamily {
    match name.to_lowercase().as_str() {
        "pro" | "inter" => es_model::FontFamily::Pro,
        "compact" | "inter tight" => es_model::FontFamily::Compact,
        "rounded" | "nunito" => es_model::FontFamily::Rounded,
        _ => es_model::FontFamily::System(name),
    }
}
