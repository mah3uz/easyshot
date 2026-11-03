use es_ui::cursors::{SCALES, annotation_plus, rotate, window_camera};
use gpui::CursorImage;

struct Bitmap<'a> {
    side: usize,
    scale: f32,
    bgra: &'a [u8],
}

impl Bitmap<'_> {
    fn px(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.side + x) * 4;
        [
            self.bgra[i],
            self.bgra[i + 1],
            self.bgra[i + 2],
            self.bgra[i + 3],
        ]
    }

    fn at(&self, x: f32, y: f32) -> [u8; 4] {
        self.px((x * self.scale) as usize, (y * self.scale) as usize)
    }

    fn opaque_black(&self, x: usize, y: usize) -> bool {
        self.px(x, y) == [0, 0, 0, 255]
    }

    fn mirrors_about(&self, hotspot: u32, horizontal: bool, vertical: bool) -> bool {
        let h = hotspot as f32 * self.scale;
        if h.fract() != 0.0 {
            return true;
        }
        let m = |v: usize| 2 * h as usize - 1 - v;
        (0..self.side * self.side).all(|i| {
            let (x, y) = (i % self.side, i / self.side);
            let (mx, my) = (
                if horizontal { m(x) } else { x },
                if vertical { m(y) } else { y },
            );
            self.px(x, y)
                .iter()
                .zip(self.px(mx, my))
                .all(|(a, b)| a.abs_diff(b) <= 2)
        })
    }

    fn ink_bounds(&self) -> (f32, f32, f32, f32) {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..self.side {
            for x in 0..self.side {
                if self.px(x, y)[3] > 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        let s = self.scale;
        (x0 as f32 / s, y0 as f32 / s, x1 as f32 / s, y1 as f32 / s)
    }
}

fn bitmaps(image: &CursorImage) -> Vec<Bitmap<'_>> {
    image
        .images
        .iter()
        .map(|(scale, bgra)| Bitmap {
            side: (image.size.width as f32 * scale).round() as usize,
            scale: *scale,
            bgra,
        })
        .collect()
}

fn all() -> Vec<(String, CursorImage)> {
    let mut all = vec![
        ("annotationPlus".to_string(), annotation_plus()),
        ("windowCamera".to_string(), window_camera()),
    ];
    all.extend((0..8).map(|k| (format!("rotate {k}"), rotate(k))));
    all
}

#[test]
fn every_cursor_is_drawn_at_each_scale_inside_its_canvas() {
    for (name, image) in all() {
        let scales: Vec<f32> = image.images.iter().map(|(s, _)| *s).collect();
        assert_eq!(scales, SCALES, "{name}");
        for b in bitmaps(&image) {
            assert_eq!(b.bgra.len(), b.side * b.side * 4, "{name} at {}", b.scale);
            let black = (0..b.side * b.side)
                .filter(|i| b.opaque_black(i % b.side, i / b.side))
                .count();
            let halo = b
                .bgra
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] >= 192)
                .count();
            assert!(
                black as f32 > 20.0 * b.scale * b.scale,
                "{name} at {}: only {black} black pixels",
                b.scale
            );
            assert!(
                halo as f32 > 20.0 * b.scale * b.scale,
                "{name} at {}: only {halo} halo pixels",
                b.scale
            );
            let edge = (0..b.side).flat_map(|i| {
                [
                    b.px(i, 0),
                    b.px(i, b.side - 1),
                    b.px(0, i),
                    b.px(b.side - 1, i),
                ]
            });
            assert!(
                edge.into_iter().all(|p| p[3] == 0),
                "{name} at {} is clipped by its canvas",
                b.scale
            );
            for p in b.bgra.as_chunks::<4>().0 {
                assert!(
                    p[0] <= p[3] && p[0] == p[1] && p[1] == p[2],
                    "{name}: {p:?} is not premultiplied grey"
                );
            }
        }
    }
}

#[test]
fn the_hotspot_is_where_the_glyph_points() {
    let plus = annotation_plus();
    assert_eq!(
        (plus.size.width, plus.hotspot.x, plus.hotspot.y),
        (22, 11, 11)
    );
    for b in bitmaps(&plus) {
        let h = (11.0 * b.scale) as usize;
        assert!(
            b.opaque_black(h, h),
            "the cross centre at {} is not under the hotspot",
            b.scale
        );
        assert!(
            b.mirrors_about(11, true, true),
            "the cross at {} is off-centre",
            b.scale
        );
    }

    let camera = window_camera();
    assert_eq!(
        (camera.size.width, camera.hotspot.x, camera.hotspot.y),
        (22, 11, 11)
    );
    for b in bitmaps(&camera) {
        let (x0, y0, x1, y1) = b.ink_bounds();
        let centre = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        assert!(
            (centre.0 - 11.0).abs() <= 0.5 && (centre.1 - 11.0).abs() <= 0.5,
            "camera centred at {centre:?}"
        );
        assert!(
            b.mirrors_about(11, true, false),
            "the camera at {} is off-centre",
            b.scale
        );
    }

    for k in 0..8 {
        let image = rotate(k);
        assert_eq!(
            (image.size.width, image.hotspot.x, image.hotspot.y),
            (24, 12, 12),
            "rotate {k}"
        );
    }
}

#[test]
fn annotation_plus_is_drawn_at_its_own_resolution() {
    let plus = annotation_plus();
    let b = &bitmaps(&plus)[1];
    assert_eq!((b.scale, b.side), (1.5, 33));
    let column = 8;
    let rows: Vec<bool> = (13..20).map(|y| b.opaque_black(column, y)).collect();
    assert_eq!(
        rows,
        [false, false, true, true, true, false, false],
        "the 2 pt arm is not 3 crisp pixels at 1.5x"
    );
}

#[test]
fn rotate_cursors_turn_about_the_hotspot_clockwise() {
    let images: Vec<CursorImage> = (0..8).map(rotate).collect();
    for scale in [0, 2] {
        let base = &bitmaps(&images[0])[scale];
        let n = base.side;
        for quarter in 1..4 {
            let turned = &bitmaps(&images[quarter * 2])[scale];
            for y in 0..n {
                for x in 0..n {
                    let (mut sx, mut sy) = (x, y);
                    for _ in 0..quarter {
                        (sx, sy) = (sy, n - 1 - sx);
                    }
                    let (a, b) = (turned.px(x, y), base.px(sx, sy));
                    assert!(
                        a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 2),
                        "rotate {} at ({x}, {y})",
                        quarter * 2
                    );
                }
            }
        }
    }

    for (k, image) in images.iter().enumerate() {
        let (sin, cos) = (k as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
        let b = &bitmaps(image)[2];
        let ahead = b.at(12.0 + cos, 12.0 + sin);
        let behind = b.at(12.0 - 4.0 * cos, 12.0 - 4.0 * sin);
        assert_eq!(
            ahead,
            [0, 0, 0, 255],
            "rotate {k} does not bulge toward {}°",
            k * 45
        );
        assert_eq!(behind[3], 0, "rotate {k} is not open away from {}°", k * 45);
    }

    for i in 0..8 {
        for j in i + 1..8 {
            assert_ne!(
                images[i].images[0].1, images[j].images[0].1,
                "rotate {i} and {j} are identical"
            );
        }
    }
}
