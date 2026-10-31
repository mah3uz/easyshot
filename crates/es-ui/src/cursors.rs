use std::{
    f32::consts::{FRAC_PI_4, PI},
    sync::Arc,
};

use gpui::{App, CursorImage, CustomCursorId, point, size};

pub const SCALES: [f32; 3] = [1.0, 1.5, 2.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cursors {
    pub annotation_plus: CustomCursorId,
    // rotate[k] bulges toward k * 45° clockwise from +x in screen space (y down).
    pub rotate: [CustomCursorId; 8],
    pub window_camera: CustomCursorId,
}

pub fn register(cx: &mut App) -> Cursors {
    Cursors {
        annotation_plus: cx.register_cursor_image(annotation_plus()),
        rotate: std::array::from_fn(|k| cx.register_cursor_image(rotate(k))),
        window_camera: cx.register_cursor_image(window_camera()),
    }
}

pub fn annotation_plus() -> CursorImage {
    let arms = [
        Shape::Segment(P(5.0, 11.0), P(17.0, 11.0)),
        Shape::Segment(P(11.0, 5.0), P(11.0, 17.0)),
    ];
    cursor(22, 11, 1.5, move |p| distance(&arms, p) - 1.0)
}

pub fn rotate(orientation: usize) -> CursorImage {
    let angle = orientation as f32 * FRAC_PI_4;
    let (sin, cos) = angle.sin_cos();
    let centre = P(12.0, 12.0);
    let arc_centre = P(-6.0, 0.0);
    let arc = Shape::Arc {
        centre: arc_centre,
        radius: 7.0,
        mid: 0.0,
        half: FRAC_PI_4,
    };
    let heads = [-1.0f32, 1.0].map(|side| {
        let a = side * FRAC_PI_4;
        let end = arc_centre.add(P(a.cos(), a.sin()).mul(7.0));
        let normal = P(a.cos(), a.sin());
        let tangent = P(-a.sin(), a.cos()).mul(side);
        let back = end.sub(tangent.mul(0.5));
        Shape::Triangle(
            end.add(tangent.mul(3.5)),
            back.add(normal.mul(3.0)),
            back.sub(normal.mul(3.0)),
        )
    });
    cursor(24, 12, 1.25, move |p| {
        let d = p.sub(centre);
        let local = P(d.0 * cos + d.1 * sin, d.1 * cos - d.0 * sin);
        (arc.distance(local) - 1.0)
            .min(heads[0].distance(local))
            .min(heads[1].distance(local))
    })
}

pub fn window_camera() -> CursorImage {
    let r = 2.0;
    let deg = |d: f32| d * PI / 180.0;
    let arc = |cx: f32, cy: f32, from: f32, to: f32| Shape::Arc {
        centre: P(cx, cy),
        radius: r,
        mid: deg((from + to) / 2.0),
        half: deg((to - from).abs() / 2.0),
    };
    let outline = [
        arc(13.997, 6.0, -90.0, -28.36),
        Shape::Segment(P(15.757, 5.05), P(16.243, 5.95)),
        arc(18.003, 5.0, 90.0, 151.64),
        Shape::Segment(P(18.003, 7.0), P(20.0, 7.0)),
        arc(20.0, 9.0, -90.0, 0.0),
        Shape::Segment(P(22.0, 9.0), P(22.0, 18.0)),
        arc(20.0, 18.0, 0.0, 90.0),
        Shape::Segment(P(20.0, 20.0), P(4.0, 20.0)),
        arc(4.0, 18.0, 90.0, 180.0),
        Shape::Segment(P(2.0, 18.0), P(2.0, 9.0)),
        arc(4.0, 9.0, 180.0, 270.0),
        Shape::Segment(P(4.0, 7.0), P(5.997, 7.0)),
        arc(5.997, 5.0, 28.42, 90.0),
        Shape::Segment(P(7.756, 5.952), P(8.245, 5.048)),
        arc(10.004, 6.0, -151.58, -90.0),
        Shape::Segment(P(10.004, 4.0), P(13.997, 4.0)),
        Shape::Arc {
            centre: P(12.0, 13.0),
            radius: 3.0,
            mid: 0.0,
            half: PI,
        },
    ];
    let icon = 18.0 / 24.0;
    let offset = (22.0 - 18.0) / 2.0;
    cursor(22, 11, 1.5, move |p| {
        distance(&outline, p.sub(P(offset, offset)).mul(1.0 / icon)) * icon - 0.75
    })
}

fn cursor(side: u32, hotspot: u32, halo: f32, ink: impl Fn(P) -> f32) -> CursorImage {
    let images = SCALES
        .iter()
        .map(|&scale| (scale, raster(side, scale, halo, &ink)))
        .collect();
    CursorImage {
        images,
        size: size(side, side),
        hotspot: point(hotspot, hotspot),
    }
}

fn raster(side: u32, scale: f32, halo: f32, ink: &impl Fn(P) -> f32) -> Arc<[u8]> {
    let pixels = (side as f32 * scale).round() as u32;
    let coverage = |d: f32| (0.5 - d * scale).clamp(0.0, 1.0);
    let mut out = Vec::with_capacity((pixels * pixels * 4) as usize);
    for y in 0..pixels {
        for x in 0..pixels {
            let d = ink(P((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale));
            let black = coverage(d);
            let white = coverage(d - halo) * (1.0 - black);
            let grey = (white * 255.0).round() as u8;
            out.extend([grey, grey, grey, ((black + white) * 255.0).round() as u8]);
        }
    }
    out.into()
}

#[derive(Clone, Copy, Debug)]
struct P(f32, f32);

impl P {
    fn add(self, o: P) -> P {
        P(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: P) -> P {
        P(self.0 - o.0, self.1 - o.1)
    }
    fn mul(self, k: f32) -> P {
        P(self.0 * k, self.1 * k)
    }
    fn dot(self, o: P) -> f32 {
        self.0 * o.0 + self.1 * o.1
    }
    fn cross(self, o: P) -> f32 {
        self.0 * o.1 - self.1 * o.0
    }
    fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Segment(P, P),
    Arc {
        centre: P,
        radius: f32,
        mid: f32,
        half: f32,
    },
    Triangle(P, P, P),
}

impl Shape {
    fn distance(&self, p: P) -> f32 {
        match *self {
            Shape::Segment(a, b) => segment(p, a, b),
            Shape::Arc {
                centre,
                radius,
                mid,
                half,
            } => {
                let d = p.sub(centre);
                let off = (d.1.atan2(d.0) - mid + PI).rem_euclid(2.0 * PI) - PI;
                if off.abs() <= half {
                    (d.len() - radius).abs()
                } else {
                    let end = |a: f32| centre.add(P(a.cos(), a.sin()).mul(radius)).sub(p).len();
                    end(mid - half).min(end(mid + half))
                }
            }
            Shape::Triangle(a, b, c) => {
                let edge = segment(p, a, b).min(segment(p, b, c)).min(segment(p, c, a));
                let sides = [
                    b.sub(a).cross(p.sub(a)),
                    c.sub(b).cross(p.sub(b)),
                    a.sub(c).cross(p.sub(c)),
                ];
                let inside = sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0);
                if inside { -edge } else { edge }
            }
        }
    }
}

fn segment(p: P, a: P, b: P) -> f32 {
    let ab = b.sub(a);
    let t = (p.sub(a).dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    p.sub(a.add(ab.mul(t))).len()
}

fn distance(shapes: &[Shape], p: P) -> f32 {
    shapes
        .iter()
        .map(|s| s.distance(p))
        .fold(f32::INFINITY, f32::min)
}
