use super::dash::{DashStyle, perfect_dash_props};
use crate::math::{HALF_PI, PI2, approximately};
use crate::vec::PVec;
use kurbo::{BezPath, Point};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CommandOpts {
    pub offset: Option<f64>,
    pub roundness: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Move,
    Line,
    Cubic { cp1: PVec, cp2: PVec },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Command {
    pub kind: Kind,
    pub point: PVec,
    pub is_close: bool,
    pub opts: CommandOpts,
}

impl Command {
    pub(super) fn is_move(&self) -> bool {
        matches!(self.kind, Kind::Move)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashedSegment {
    pub path: BezPath,
    pub dashes: Vec<f64>,
    pub phase: f64,
}

#[derive(Debug, Clone, Default)]
pub struct PathBuilder {
    commands: Vec<Command>,
    last_move_idx: Option<usize>,
}

pub(super) fn pt(v: PVec) -> Point {
    Point::new(v.x, v.y)
}

fn cubic_at(p0: PVec, p1: PVec, p2: PVec, p3: PVec, t: f64) -> PVec {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    PVec::new(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

pub(super) fn cubic_length(p0: PVec, p1: PVec, p2: PVec, p3: PVec) -> f64 {
    const SAMPLES: usize = 16;
    let mut length = 0.0;
    let mut prev = p0;
    for i in 1..=SAMPLES {
        let point = cubic_at(p0, p1, p2, p3, i as f64 / SAMPLES as f64);
        length += PVec::dist(prev, point);
        prev = point;
    }
    length
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn move_to(&mut self, p: PVec) -> &mut Self {
        self.move_to_with(p, CommandOpts::default())
    }

    pub fn move_to_with(&mut self, p: PVec, opts: CommandOpts) -> &mut Self {
        self.commands.push(Command {
            kind: Kind::Move,
            point: p,
            is_close: false,
            opts,
        });
        self.last_move_idx = Some(self.commands.len() - 1);
        self
    }

    pub fn line_to(&mut self, p: PVec) -> &mut Self {
        self.line_to_with(p, CommandOpts::default())
    }

    pub fn line_to_with(&mut self, p: PVec, opts: CommandOpts) -> &mut Self {
        self.commands.push(Command {
            kind: Kind::Line,
            point: p,
            is_close: false,
            opts,
        });
        self
    }

    pub fn cubic_to(&mut self, cp1: PVec, cp2: PVec, p: PVec) -> &mut Self {
        self.cubic_to_with(cp1, cp2, p, CommandOpts::default())
    }

    pub fn cubic_to_with(&mut self, cp1: PVec, cp2: PVec, p: PVec, opts: CommandOpts) -> &mut Self {
        self.commands.push(Command {
            kind: Kind::Cubic { cp1, cp2 },
            point: p,
            is_close: false,
            opts,
        });
        self
    }

    pub fn close(&mut self) -> &mut Self {
        let (Some(move_idx), Some(last)) = (self.last_move_idx, self.commands.last()) else {
            return self;
        };
        let move_point = self.commands[move_idx].point;
        if approximately(move_point.x, last.point.x, 1e-6)
            && approximately(move_point.y, last.point.y, 1e-6)
        {
            if let Some(last) = self.commands.last_mut() {
                last.is_close = true;
            }
        } else {
            self.commands.push(Command {
                kind: Kind::Line,
                point: move_point,
                is_close: true,
                opts: CommandOpts::default(),
            });
        }
        self
    }

    pub fn circular_arc(
        &mut self,
        radius: f64,
        large_arc: bool,
        sweep: bool,
        to: PVec,
    ) -> &mut Self {
        self.arc(radius, radius, large_arc, sweep, 0.0, to)
    }

    pub fn arc(
        &mut self,
        rx: f64,
        ry: f64,
        large_arc: bool,
        sweep: bool,
        x_axis_rotation: f64,
        to: PVec,
    ) -> &mut Self {
        self.arc_with(
            rx,
            ry,
            large_arc,
            sweep,
            x_axis_rotation,
            to,
            CommandOpts::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn arc_with(
        &mut self,
        rx: f64,
        ry: f64,
        large_arc: bool,
        sweep: bool,
        x_axis_rotation: f64,
        p2: PVec,
        opts: CommandOpts,
    ) -> &mut Self {
        let Some(last) = self.commands.last() else {
            return self;
        };
        let p1 = last.point;
        if p1.x == p2.x && p1.y == p2.y {
            return self;
        }
        if rx == 0.0 || ry == 0.0 {
            return self.line_to_with(p2, opts);
        }

        let (sin_phi, cos_phi) = x_axis_rotation.sin_cos();
        let mut rx1 = rx.abs();
        let mut ry1 = ry.abs();

        let dx = (p1.x - p2.x) / 2.0;
        let dy = (p1.y - p2.y) / 2.0;
        let x1p = cos_phi * dx + sin_phi * dy;
        let y1p = -sin_phi * dx + cos_phi * dy;

        let lambda = (x1p * x1p) / (rx1 * rx1) + (y1p * y1p) / (ry1 * ry1);
        if lambda > 1.0 {
            let s = lambda.sqrt();
            rx1 *= s;
            ry1 *= s;
        }

        let sign = if large_arc != sweep { 1.0 } else { -1.0 };
        let term = rx1 * rx1 * ry1 * ry1 - rx1 * rx1 * y1p * y1p - ry1 * ry1 * x1p * x1p;
        let numerator = rx1 * rx1 * y1p * y1p + ry1 * ry1 * x1p * x1p;
        let radicand = if numerator == 0.0 {
            0.0
        } else {
            term / numerator
        };
        let coef = sign * radicand.max(0.0).sqrt();

        let cxp = coef * ((rx1 * y1p) / ry1);
        let cyp = coef * (-(ry1 * x1p) / rx1);
        let cx = cos_phi * cxp - sin_phi * cyp + (p1.x + p2.x) / 2.0;
        let cy = sin_phi * cxp + cos_phi * cyp + (p1.y + p2.y) / 2.0;

        let ux = (x1p - cxp) / rx1;
        let uy = (y1p - cyp) / ry1;
        let vx = (-x1p - cxp) / rx1;
        let vy = (-y1p - cyp) / ry1;

        let start_angle = uy.atan2(ux);
        let mut end_angle = vy.atan2(vx);
        if !sweep && end_angle > start_angle {
            end_angle -= PI2;
        } else if sweep && end_angle < start_angle {
            end_angle += PI2;
        }
        let sweep_angle = end_angle - start_angle;

        let ellipse_point = |a: f64| {
            PVec::new(
                cx + rx1 * a.cos() * cos_phi - ry1 * a.sin() * sin_phi,
                cy + rx1 * a.cos() * sin_phi + ry1 * a.sin() * cos_phi,
            )
        };
        let ellipse_derivative = |a: f64| {
            PVec::new(
                -rx1 * a.sin() * cos_phi - ry1 * a.cos() * sin_phi,
                -rx1 * a.sin() * sin_phi + ry1 * a.cos() * cos_phi,
            )
        };

        let segments = ((sweep_angle.abs() / HALF_PI).ceil() as usize).clamp(1, 4);
        let per_segment = sweep_angle / segments as f64;

        for i in 0..segments {
            let theta1 = start_angle + i as f64 * per_segment;
            let theta2 = start_angle + (i + 1) as f64 * per_segment;
            let start = ellipse_point(theta1);
            let end = ellipse_point(theta2);
            let d1 = ellipse_derivative(theta1);
            let d2 = ellipse_derivative(theta2);
            let handle = (4.0 / 3.0) * ((theta2 - theta1) / 4.0).tan();
            let cp1 = PVec::new(start.x + handle * d1.x, start.y + handle * d1.y);
            let cp2 = PVec::new(end.x - handle * d2.x, end.y - handle * d2.y);
            let seg_opts = if i == 0 { opts } else { CommandOpts::default() };
            self.cubic_to_with(cp1, cp2, end, seg_opts);
        }
        self
    }

    pub fn line_through_points(points: &[PVec]) -> PathBuilder {
        let mut path = PathBuilder::new();
        if let Some((first, rest)) = points.split_first() {
            path.move_to(*first);
            for p in rest {
                path.line_to(*p);
            }
        }
        path
    }

    pub fn to_bez_path(&self) -> BezPath {
        let mut path = BezPath::new();
        for c in &self.commands {
            match c.kind {
                Kind::Move => path.move_to(pt(c.point)),
                Kind::Line => path.line_to(pt(c.point)),
                Kind::Cubic { cp1, cp2 } => path.curve_to(pt(cp1), pt(cp2), pt(c.point)),
            }
            if c.is_close {
                path.close_path();
            }
        }
        path
    }

    pub fn vertices(&self, curve_resolution: usize) -> Vec<PVec> {
        let mut points = Vec::new();
        let mut current = PVec::new(0.0, 0.0);
        for c in &self.commands {
            match c.kind {
                Kind::Move | Kind::Line => points.push(c.point),
                Kind::Cubic { cp1, cp2 } => {
                    for i in 1..=curve_resolution {
                        let t = i as f64 / curve_resolution as f64;
                        points.push(cubic_at(current, cp1, cp2, c.point, t));
                    }
                }
            }
            current = c.point;
        }
        points
    }

    pub fn dashed_segments(&self, stroke_width: f64, style: DashStyle) -> Vec<DashedSegment> {
        let mut segments = Vec::new();
        let mut current: Option<PVec> = None;
        for c in &self.commands {
            let from = current.replace(c.point);
            let Some(from) = from else { continue };
            let mut path = BezPath::new();
            path.move_to(pt(from));
            let length = match c.kind {
                Kind::Move => continue,
                Kind::Line => {
                    path.line_to(pt(c.point));
                    PVec::dist(from, c.point)
                }
                Kind::Cubic { cp1, cp2 } => {
                    path.curve_to(pt(cp1), pt(cp2), pt(c.point));
                    cubic_length(from, cp1, cp2, c.point)
                }
            };
            if length <= 0.0 {
                continue;
            }
            let props = perfect_dash_props(length, stroke_width, style, &Default::default());
            segments.push(DashedSegment {
                path,
                dashes: props.dashes,
                phase: props.phase,
            });
        }
        segments
    }
}
