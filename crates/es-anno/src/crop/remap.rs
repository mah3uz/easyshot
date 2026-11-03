use es_geom::{Mat, PVec, Point, Size};
use es_model::{AnnoShape, ShapeKind};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Image-pixel offset and scale applied to every shape when the image is cropped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropMap {
    pub origin: Point,
    pub sx: f64,
    pub sy: f64,
}

impl CropMap {
    pub fn inverse(self) -> CropMap {
        CropMap {
            origin: Point::new(-self.origin.x * self.sx, -self.origin.y * self.sy),
            sx: 1.0 / self.sx,
            sy: 1.0 / self.sy,
        }
    }

    pub fn apply_to(&self, shapes: &mut [AnnoShape]) {
        shapes.iter_mut().for_each(|s| self.apply(s));
    }

    /// The origin is translated then scaled; sizes and point lists, which are shape-relative, only scale.
    pub fn apply(&self, shape: &mut AnnoShape) {
        let (sx, sy) = (self.sx, self.sy);
        let uniform = (sx + sy) / 2.0;
        shape.x = (shape.x - self.origin.x) * sx;
        shape.y = (shape.y - self.origin.y) * sy;
        match &mut shape.kind {
            ShapeKind::Geo(p) => {
                p.w *= sx;
                p.h *= sy;
                p.stroke_width *= uniform;
                p.corner_radius *= uniform;
            }
            ShapeKind::Draw(p) => {
                for pt in &mut p.points {
                    pt.x *= sx;
                    pt.y *= sy;
                }
                p.stroke_width *= uniform;
            }
            ShapeKind::Arrow(p) => {
                for pt in [&mut p.start, &mut p.end] {
                    pt.x *= sx;
                    pt.y *= sy;
                }
                p.bend *= uniform;
                p.stroke_width *= uniform;
            }
            ShapeKind::Text(p) => {
                p.font_size *= uniform;
                p.w *= sx;
            }
            ShapeKind::Redaction(p) => {
                p.w *= sx;
                p.h *= sy;
            }
            ShapeKind::Highlight(p) => {
                p.w *= sx;
                p.h *= sy;
            }
            ShapeKind::Highlighter(p) => {
                for pt in &mut p.points {
                    pt.x *= sx;
                    pt.y *= sy;
                }
                p.stroke_width *= uniform;
            }
            ShapeKind::Numbered(p) => p.diameter *= uniform,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOp {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}

impl ImageOp {
    pub fn inverse(self) -> ImageOp {
        match self {
            ImageOp::RotateLeft => ImageOp::RotateRight,
            ImageOp::RotateRight => ImageOp::RotateLeft,
            flip => flip,
        }
    }

    pub fn size_after(self, size: Size) -> Size {
        match self {
            ImageOp::RotateLeft | ImageOp::RotateRight => Size::new(size.height, size.width),
            _ => size,
        }
    }

    /// Old pixel coordinates to new ones, for an image of the old `size`.
    pub fn matrix(self, size: Size) -> Mat {
        let (w, h) = (size.width, size.height);
        match self {
            ImageOp::RotateLeft => Mat::new(0.0, -1.0, 1.0, 0.0, 0.0, w),
            ImageOp::RotateRight => Mat::new(0.0, 1.0, -1.0, 0.0, h, 0.0),
            ImageOp::FlipHorizontal => Mat::new(-1.0, 0.0, 0.0, 1.0, w, 0.0),
            ImageOp::FlipVertical => Mat::new(1.0, 0.0, 0.0, -1.0, 0.0, h),
        }
    }

    pub fn apply_to(self, size: Size, shapes: &mut [AnnoShape]) {
        let m = self.matrix(size);
        for shape in shapes {
            let p = m.apply_to_point(PVec::new(shape.x, shape.y));
            shape.x = p.x;
            shape.y = p.y;
            match self {
                ImageOp::RotateLeft => shape.rotation -= FRAC_PI_2,
                ImageOp::RotateRight => shape.rotation += FRAC_PI_2,
                ImageOp::FlipHorizontal => {
                    shape.rotation = -shape.rotation;
                    shape.mirrored = !shape.mirrored;
                }
                ImageOp::FlipVertical => {
                    shape.rotation = PI - shape.rotation;
                    shape.mirrored = !shape.mirrored;
                }
            }
            shape.rotation = shape.rotation.rem_euclid(TAU);
        }
    }
}
