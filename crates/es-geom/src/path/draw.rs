use super::builder::{Command, Kind, PathBuilder, cubic_length, pt};
use crate::math::{HALF_PI, Rng, clamp, modulate};
use crate::vec::PVec;
use kurbo::BezPath;
use std::f64::consts::PI;

#[derive(Debug, Clone, PartialEq)]
pub struct DrawOptions {
    pub stroke_width: f64,
    pub random_seed: String,
    pub offset: Option<f64>,
    pub roundness: Option<f64>,
    pub passes: usize,
}

impl DrawOptions {
    pub fn new(stroke_width: f64, random_seed: impl Into<String>) -> Self {
        Self {
            stroke_width,
            random_seed: random_seed.into(),
            offset: None,
            roundness: None,
            passes: 2,
        }
    }
}

#[derive(Clone, Copy)]
struct CommandInfo {
    tangent_start: PVec,
    tangent_end: PVec,
    length: f64,
}

struct DrawCommand {
    command: Command,
    offset_amount: f64,
    roundness_before: f64,
    roundness_after: f64,
    tangent_to_prev: Option<PVec>,
    tangent_to_next: Option<PVec>,
}

fn supports_roundness(command: &Command) -> bool {
    !matches!(command.kind, Kind::Cubic { .. })
}

fn command_info(commands: &[Command]) -> Vec<Option<CommandInfo>> {
    let mut info = vec![None; commands.len()];
    for i in 1..commands.len() {
        let previous = commands[i - 1].point;
        let current = &commands[i];
        info[i] = match current.kind {
            Kind::Move => None,
            Kind::Line => {
                let t = PVec::sub(previous, current.point).uni();
                Some(CommandInfo {
                    tangent_start: t,
                    tangent_end: t,
                    length: PVec::dist(previous, current.point),
                })
            }
            Kind::Cubic { cp1, cp2 } => Some(CommandInfo {
                tangent_start: PVec::sub(cp1, previous).uni(),
                tangent_end: PVec::sub(cp2, current.point).uni(),
                length: cubic_length(previous, cp1, cp2, current.point),
            }),
        };
    }
    info
}

fn draw_commands(commands: &[Command], opts: &DrawOptions) -> Vec<DrawCommand> {
    let default_offset = opts.offset.unwrap_or(opts.stroke_width / 3.0);
    let default_roundness = opts.roundness.unwrap_or(opts.stroke_width * 2.0);
    let info = command_info(commands);

    let mut result: Vec<DrawCommand> = Vec::with_capacity(commands.len());
    let mut last_move_idx: Option<usize> = None;
    let mut close_roundness: Vec<(usize, f64)> = Vec::new();

    for (i, command) in commands.iter().enumerate() {
        let offset = command.opts.offset.unwrap_or(default_offset);
        let roundness = command.opts.roundness.unwrap_or(default_roundness);

        if command.is_move() {
            last_move_idx = Some(i);
        }

        let next_idx = if command.is_close {
            last_move_idx.map(|m| m + 1)
        } else if i + 1 < commands.len() && !commands[i + 1].is_move() {
            Some(i + 1)
        } else {
            None
        };

        let next = next_idx.filter(|n| *n < commands.len() && !commands[*n].is_move());
        let next_info = next.and_then(|n| info[n]);

        let next_supports =
            next_idx.is_some_and(|n| n < commands.len() && supports_roundness(&commands[n]));
        let current_info = info[i];
        let tangent_to_prev = current_info.map(|c| c.tangent_end);
        let tangent_to_next = next_info.map(|n| n.tangent_start);

        let mut angle_roundness = 0.0;
        if supports_roundness(command)
            && next_supports
            && let (Some(tp), Some(tn)) = (tangent_to_prev, tangent_to_next)
            && tp.len2() > 0.01
            && tn.len2() > 0.01
        {
            angle_roundness = modulate(
                PVec::angle_between(tp, tn).abs(),
                (HALF_PI, PI),
                (roundness, 0.0),
                true,
            );
        }

        let current_len = current_info.map_or(f64::INFINITY, |c| c.length);
        let next_len = next_info.map_or(f64::INFINITY, |n| n.length);
        let offset_limit = current_len.min(next_len) - angle_roundness * 2.0;
        let offset_amount = clamp(offset, 0.0, (offset_limit / 4.0).max(0.0));

        let roundness_before = angle_roundness.min(current_len / 4.0);
        let roundness_after = angle_roundness.min(next_len / 4.0);

        result.push(DrawCommand {
            command: *command,
            offset_amount,
            roundness_before,
            roundness_after,
            tangent_to_prev,
            tangent_to_next,
        });

        // A closed path's corner at the move point is the one the closing segment makes.
        if command.is_close
            && let Some(m) = last_move_idx
        {
            close_roundness.push((m, roundness_after));
        }
    }

    for (idx, r) in close_roundness {
        if let Some(d) = result.get_mut(idx) {
            d.roundness_after = r;
        }
    }
    result
}

impl PathBuilder {
    pub fn draw_path(&self, opts: &DrawOptions) -> BezPath {
        let draws = draw_commands(self.commands(), opts);
        let mut path = BezPath::new();

        for pass in 0..opts.passes {
            let mut rng = Rng::new(&format!("{}{}", opts.random_seed, pass));
            let mut last_move_offset = PVec::new(0.0, 0.0);

            for draw in &draws {
                let command = &draw.command;
                let offset = if command.is_close {
                    last_move_offset
                } else {
                    let x = rng.next();
                    let y = rng.next();
                    let direction = PVec::new(x, y).uni();
                    let magnitude = rng.next().abs().sqrt() * draw.offset_amount;
                    PVec::mul(direction, magnitude)
                };
                if command.is_move() {
                    last_move_offset = offset;
                }

                let offset_point = PVec::add(command.point, offset);

                let end_point = match draw.tangent_to_next {
                    Some(t) if draw.roundness_after > 0.0 => {
                        PVec::add(PVec::mul(t, -draw.roundness_after), offset_point)
                    }
                    _ => offset_point,
                };
                let start_point = match draw.tangent_to_prev {
                    Some(t) if draw.roundness_before > 0.0 => {
                        PVec::add(PVec::mul(t, draw.roundness_before), offset_point)
                    }
                    _ => offset_point,
                };
                let has_corner = !PVec::equals(end_point, offset_point)
                    && !PVec::equals(start_point, offset_point);

                match command.kind {
                    Kind::Move => path.move_to(pt(end_point)),
                    Kind::Line => {
                        if has_corner {
                            path.line_to(pt(start_point));
                            path.quad_to(pt(offset_point), pt(end_point));
                        } else {
                            path.line_to(pt(end_point));
                        }
                    }
                    Kind::Cubic { cp1, cp2 } => {
                        let c1 = pt(PVec::add(cp1, offset));
                        let c2 = pt(PVec::add(cp2, offset));
                        if has_corner {
                            path.curve_to(c1, c2, pt(offset_point));
                            path.quad_to(pt(offset_point), pt(end_point));
                        } else {
                            path.curve_to(c1, c2, pt(end_point));
                        }
                    }
                }
            }
        }
        path
    }
}
