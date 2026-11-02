use crate::beautify::layout::Layout;
use es_geom::Rect;
use es_model::CameraSettings;
use rayon::prelude::*;
use tiny_skia::Pixmap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Full,
    /// Sampling and output at half resolution, for live previews.
    Half,
}

/// Row-major 3x3 projective map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Homography(pub [f64; 9]);

impl Homography {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);

    pub fn apply(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let m = &self.0;
        let w = m[6] * x + m[7] * y + m[8];
        if w.abs() < 1e-12 {
            return None;
        }
        Some((
            (m[0] * x + m[1] * y + m[2]) / w,
            (m[3] * x + m[4] * y + m[5]) / w,
        ))
    }

    /// The map that applies `self` first and `other` second.
    pub fn then(&self, other: &Self) -> Self {
        let (a, b) = (&other.0, &self.0);
        let mut out = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                out[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
            }
        }
        Self(out)
    }

    pub fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f, g, h, i] = self.0;
        let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        if det.abs() < 1e-12 {
            return None;
        }
        let r = 1.0 / det;
        Some(Self([
            (e * i - f * h) * r,
            (c * h - b * i) * r,
            (b * f - c * e) * r,
            (f * g - d * i) * r,
            (a * i - c * g) * r,
            (c * d - a * f) * r,
            (d * h - e * g) * r,
            (b * g - a * h) * r,
            (a * e - b * d) * r,
        ]))
    }

    /// Maps `rect` onto the quad given as top-left, top-right, bottom-right, bottom-left.
    pub fn from_rect_to_quad(rect: Rect, quad: [(f64, f64); 4]) -> Option<Self> {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        let [p0, p1, p2, p3] = quad;
        let (dx1, dx2) = (p1.0 - p2.0, p3.0 - p2.0);
        let (dy1, dy2) = (p1.1 - p2.1, p3.1 - p2.1);
        let dx3 = p0.0 - p1.0 + p2.0 - p3.0;
        let dy3 = p0.1 - p1.1 + p2.1 - p3.1;
        let (gx, gy) = if dx3.abs() < 1e-9 && dy3.abs() < 1e-9 {
            (0.0, 0.0)
        } else {
            let den = dx1 * dy2 - dx2 * dy1;
            if den.abs() < 1e-9 {
                return None;
            }
            ((dx3 * dy2 - dx2 * dy3) / den, (dx1 * dy3 - dx3 * dy1) / den)
        };
        let unit = Self([
            p1.0 - p0.0 + gx * p1.0,
            p3.0 - p0.0 + gy * p3.0,
            p0.0,
            p1.1 - p0.1 + gx * p1.1,
            p3.1 - p0.1 + gy * p3.1,
            p0.1,
            gx,
            gy,
            1.0,
        ]);
        let (w, h) = (rect.width(), rect.height());
        let to_unit = Self([
            1.0 / w,
            0.0,
            -rect.x0 / w,
            0.0,
            1.0 / h,
            -rect.y0 / h,
            0.0,
            0.0,
            1.0,
        ]);
        Some(to_unit.then(&unit))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    pub quad: [(f64, f64); 4],
    pub forward: Homography,
    pub inverse: Homography,
}

fn rot_x(v: [f64; 3], a: f64) -> [f64; 3] {
    let (s, c) = a.sin_cos();
    [v[0], v[1] * c - v[2] * s, v[1] * s + v[2] * c]
}

fn rot_y(v: [f64; 3], a: f64) -> [f64; 3] {
    let (s, c) = a.sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

pub fn focal_distance(longest_edge: f64, field_of_view_degrees: f64) -> f64 {
    let fov = field_of_view_degrees.clamp(18.0, 80.0).to_radians();
    longest_edge * (0.35 + 0.5 / (fov / 2.0).tan().max(0.01))
}

/// Projects `card` as seen by the camera; the pivot is the middle of `image`.
pub fn projection(
    card: Rect,
    image: Rect,
    canvas: (f64, f64),
    camera: &CameraSettings,
) -> Option<Projection> {
    let pivot = (image.center().x, image.center().y);
    let focal = focal_distance(
        image.width().max(image.height()),
        camera.field_of_view_degrees,
    );
    let scale = camera.zoom.clamp(0.4, 2.5);
    let pan = (camera.pan_x * canvas.0, camera.pan_y * canvas.1);
    let (roll_s, roll_c) = camera.roll_degrees.to_radians().sin_cos();

    let project = |(px, py): (f64, f64)| {
        let mut v = [px - pivot.0, py - pivot.1, 0.0];
        v = rot_x(v, camera.rotation_x_degrees.to_radians());
        v = rot_y(v, camera.rotation_y_degrees.to_radians());
        v = rot_y(v, -camera.tilt_x_degrees.to_radians());
        v = rot_x(v, -camera.tilt_y_degrees.to_radians());
        let k = focal / (focal - v[2]).max(focal * 0.12);
        let (x, y) = (v[0] * k, v[1] * k);
        let (rx, ry) = (x * roll_c - y * roll_s, x * roll_s + y * roll_c);
        (pivot.0 + rx * scale + pan.0, pivot.1 + ry * scale + pan.1)
    };
    let quad = [
        project((card.x0, card.y0)),
        project((card.x1, card.y0)),
        project((card.x1, card.y1)),
        project((card.x0, card.y1)),
    ];
    let forward = Homography::from_rect_to_quad(card, quad)?;
    let inverse = forward.inverse()?;
    Some(Projection {
        quad,
        forward,
        inverse,
    })
}

/// Output is the layer's size at full quality and half of it (rounded up) at half quality.
pub fn warp(layer: &Pixmap, forward: &Homography, quality: Quality) -> Option<Pixmap> {
    let inverse = forward.inverse()?;
    match quality {
        Quality::Full => sample(layer, &inverse, layer.width(), layer.height()),
        Quality::Half => {
            let small = halve(layer)?;
            let scale = Homography([0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 1.0]);
            let unscale = Homography([2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0]);
            let inv = unscale.then(&inverse).then(&scale);
            sample(&small, &inv, small.width(), small.height())
        }
    }
}

/// Returns the layer unchanged when the camera does not move the card.
pub fn project(layer: Pixmap, lay: &Layout, camera: &CameraSettings, quality: Quality) -> Pixmap {
    if !camera.has_effect() {
        return layer;
    }
    let canvas = (f64::from(layer.width()), f64::from(layer.height()));
    projection(lay.card, lay.image, canvas, camera)
        .and_then(|p| warp(&layer, &p.forward, quality))
        .unwrap_or(layer)
}

fn halve(src: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let (ow, oh) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = Pixmap::new(ow as u32, oh as u32)?;
    let data = src.data();
    out.data_mut()
        .par_chunks_exact_mut(ow * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..ow {
                let mut sum = [0u32; 4];
                let mut n = 0;
                for sy in (y * 2)..(y * 2 + 2).min(h) {
                    for sx in (x * 2)..(x * 2 + 2).min(w) {
                        for (c, s) in sum.iter_mut().enumerate() {
                            *s += u32::from(data[(sy * w + sx) * 4 + c]);
                        }
                        n += 1;
                    }
                }
                for c in 0..4 {
                    row[x * 4 + c] = ((sum[c] + n / 2) / n) as u8;
                }
            }
        });
    Some(out)
}

fn sample(src: &Pixmap, inverse: &Homography, width: u32, height: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(width, height)?;
    let (sw, sh) = (i64::from(src.width()), i64::from(src.height()));
    let data = src.data();
    let m = inverse.0;
    out.data_mut()
        .par_chunks_exact_mut(width as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let py = y as f64 + 0.5;
            for x in 0..width as usize {
                let px = x as f64 + 0.5;
                let w = m[6] * px + m[7] * py + m[8];
                if w.abs() < 1e-12 {
                    continue;
                }
                let u = (m[0] * px + m[1] * py + m[2]) / w - 0.5;
                let v = (m[3] * px + m[4] * py + m[5]) / w - 0.5;
                if u <= -1.0 || v <= -1.0 || u >= sw as f64 || v >= sh as f64 {
                    continue;
                }
                let (x0, y0) = (u.floor(), v.floor());
                let (fx, fy) = ((u - x0) as f32, (v - y0) as f32);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let mut acc = [0.0f32; 4];
                for (dy, wy) in [(0, 1.0 - fy), (1, fy)] {
                    for (dx, wx) in [(0, 1.0 - fx), (1, fx)] {
                        let (tx, ty) = (x0 + dx, y0 + dy);
                        if tx < 0 || ty < 0 || tx >= sw || ty >= sh {
                            continue;
                        }
                        let i = (ty * sw + tx) as usize * 4;
                        let wgt = wx * wy;
                        for (c, a) in acc.iter_mut().enumerate() {
                            *a += f32::from(data[i + c]) * wgt;
                        }
                    }
                }
                let alpha = acc[3].round().clamp(0.0, 255.0) as u8;
                for c in 0..3 {
                    row[x * 4 + c] = (acc[c].round().clamp(0.0, 255.0) as u8).min(alpha);
                }
                row[x * 4 + 3] = alpha;
            }
        });
    Some(out)
}
