use super::options::{StrokeOptions, resolve_taper};
use crate::math::easing;
use crate::vec::PVec;

const MIN_PRESSURE: f64 = 0.025;
const RATE_OF_PRESSURE_CHANGE: f64 = 0.275;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePoint {
    pub point: PVec,
    pub input: PVec,
    pub pressure: f64,
    pub distance: f64,
    pub running_length: f64,
    pub radius: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Pipeline {
    stage: Vec<PVec>,
    pub points: Vec<StrokePoint>,
}

fn z_of(p: PVec, clamp_z: bool) -> f64 {
    if clamp_z && p.z < MIN_PRESSURE {
        MIN_PRESSURE
    } else {
        p.z
    }
}

impl Pipeline {
    pub fn run(&mut self, raw: &[PVec], opts: &StrokeOptions) {
        self.ingest(raw, opts);
        self.compute_radii(opts);
    }

    fn ingest(&mut self, raw: &[PVec], opts: &StrokeOptions) {
        self.points.clear();
        self.stage.clear();
        let Some(&first) = raw.first() else {
            return;
        };
        let size = opts.size;
        let simulate = opts.simulate_pressure;
        let t = 0.15 + (1.0 - opts.streamline) * 0.85;
        let min_dist2 = (size / 3.0) * (size / 3.0);
        let clamp_z = !simulate;

        let mut first_z = z_of(first, clamp_z);
        let mut start_idx = 1;
        while start_idx < raw.len() {
            let pt = raw[start_idx];
            let dx = pt.x - first.x;
            let dy = pt.y - first.y;
            if dx * dx + dy * dy > min_dist2 {
                break;
            }
            first_z = first_z.max(z_of(pt, clamp_z));
            start_idx += 1;
        }

        let stage = &mut self.stage;
        stage.reserve(raw.len() + 8);
        stage.push(PVec::with_z(first.x, first.y, first_z));
        for pt in &raw[start_idx..] {
            stage.push(PVec::with_z(pt.x, pt.y, z_of(*pt, clamp_z)));
        }

        let mut removed_from_end = 0usize;
        let m = stage.len();
        if m > 1 {
            let last = stage[m - 1];
            let mut j = m as isize - 2;
            while j >= 0 {
                let p = stage[j as usize];
                let dx = p.x - last.x;
                let dy = p.y - last.y;
                if dx * dx + dy * dy > min_dist2 {
                    break;
                }
                j -= 1;
                removed_from_end += 1;
            }
            if j < m as isize - 2 {
                let at = (j + 1) as usize;
                stage[at] = last;
                stage.truncate(at + 1);
            }
        }

        let m = stage.len();
        let last_segment_is_short = m > 1 && {
            let dx = stage[m - 1].x - stage[m - 2].x;
            let dy = stage[m - 1].y - stage[m - 2].y;
            dx * dx + dy * dy < size * size
        };
        let is_complete = opts.last || !simulate || last_segment_is_short || removed_from_end > 0;

        if stage.len() == 2 && simulate {
            let (p0, p1) = (stage[0], stage[1]);
            stage.resize(5, p0);
            for (i, s) in stage.iter_mut().enumerate().skip(1) {
                let u = i as f64 / 4.0;
                *s = PVec::with_z(
                    p0.x + (p1.x - p0.x) * u,
                    p0.y + (p1.y - p0.y) * u,
                    ((p0.z + (p1.z - p0.z)) * i as f64) / 4.0,
                );
            }
        }

        let p0 = stage[0];
        self.points.push(StrokePoint {
            point: p0,
            input: p0,
            pressure: if simulate { 0.5 } else { p0.z },
            distance: 0.0,
            running_length: 0.0,
            radius: 1.0,
        });

        if is_complete && opts.streamline > 0.0 {
            let l = stage[stage.len() - 1];
            stage.push(l);
        }

        let m = stage.len();
        let mut total = 0.0;
        let (mut prev_x, mut prev_y) = (p0.x, p0.y);
        let u = 1.0 - t;
        for (i, &s) in stage.iter().enumerate().skip(1) {
            let (x, y) = if t == 0.0 || (opts.last && i == m - 1) {
                (s.x, s.y)
            } else {
                (s.x + (prev_x - s.x) * u, s.y + (prev_y - s.y) * u)
            };
            if (prev_x - x).abs() < 0.0001 && (prev_y - y).abs() < 0.0001 {
                continue;
            }
            let distance = ((y - prev_y) * (y - prev_y) + (x - prev_x) * (x - prev_x)).sqrt();
            total += distance;
            if i < 4 && total < size {
                continue;
            }
            self.points.push(StrokePoint {
                point: PVec::with_z(x, y, s.z),
                input: s,
                pressure: if simulate { 0.5 } else { s.z },
                distance,
                running_length: total,
                radius: 1.0,
            });
            prev_x = x;
            prev_y = y;
        }

        if total < 1.0 {
            let max_p = self.points.iter().fold(0.5_f64, |a, p| a.max(p.pressure));
            for p in &mut self.points {
                p.pressure = max_p;
            }
        }
    }

    fn compute_radii(&mut self, opts: &StrokeOptions) {
        let n = self.points.len();
        if n == 0 {
            return;
        }
        let size = opts.size;
        let thinning = opts.thinning;
        let simulate = opts.simulate_pressure;
        let ease = opts.easing;
        let total = self.points[n - 1].running_length;

        if !simulate && total < size {
            let max_p = self.points.iter().fold(0.5_f64, |a, p| a.max(p.pressure));
            let r = size * ease(0.5 - thinning * (0.5 - max_p));
            for p in &mut self.points {
                p.pressure = max_p;
                p.radius = r;
            }
            return;
        }

        let mut prev_p = self.points[0].pressure;
        for p in &self.points {
            if p.running_length > size * 5.0 {
                break;
            }
            let sp = (p.distance / size).min(1.0);
            let np = if simulate {
                let rp = (1.0 - sp).min(1.0);
                (prev_p + (rp - prev_p) * (sp * RATE_OF_PRESSURE_CHANGE)).min(1.0)
            } else {
                (prev_p + (p.pressure - prev_p) * 0.5).min(1.0)
            };
            prev_p += (np - prev_p) * 0.5;
        }

        let taper_start = resolve_taper(opts.start.taper, size, total);
        let taper_end = resolve_taper(opts.end.taper, size, total);
        let has_taper = taper_start != 0.0 || taper_end != 0.0;
        let start_ease = opts.start.easing.unwrap_or(easing::ease_out_quad);
        let end_ease = opts.end.easing.unwrap_or(easing::ease_out_cubic);

        for p in &mut self.points {
            let mut radius;
            if thinning != 0.0 {
                let sp = (p.distance / size).min(1.0);
                let pressure = if simulate {
                    let rp = (1.0 - sp).min(1.0);
                    (prev_p + (rp - prev_p) * (sp * RATE_OF_PRESSURE_CHANGE)).min(1.0)
                } else {
                    (prev_p + (p.pressure - prev_p) * (sp * RATE_OF_PRESSURE_CHANGE)).min(1.0)
                };
                radius = size * ease(0.5 - thinning * (0.5 - pressure));
                prev_p = pressure;
            } else {
                radius = size / 2.0;
            }
            if has_taper {
                let run = p.running_length;
                let ts = if taper_start != 0.0 && run < taper_start {
                    start_ease(run / taper_start)
                } else {
                    1.0
                };
                let te = if taper_end != 0.0 && total - run < taper_end {
                    end_ease((total - run) / taper_end)
                } else {
                    1.0
                };
                radius = (radius * ts.min(te)).max(0.01);
            }
            p.radius = radius;
        }
    }
}
