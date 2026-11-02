use super::remap::CropMap;
use es_geom::{Point, Rect, Size};
use kurbo::Vec2;

pub const MIN_CROP_PIXELS: f64 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectPreset {
    #[default]
    Freeform,
    Original,
    Square,
    Landscape16x9,
    Portrait9x16,
    Landscape4x3,
    Landscape3x2,
}

impl AspectPreset {
    pub const ALL: [AspectPreset; 7] = [
        AspectPreset::Freeform,
        AspectPreset::Original,
        AspectPreset::Square,
        AspectPreset::Landscape16x9,
        AspectPreset::Portrait9x16,
        AspectPreset::Landscape4x3,
        AspectPreset::Landscape3x2,
    ];

    /// Pixel width over height; `None` for freeform.
    pub fn ratio(self, image: Size) -> Option<f64> {
        match self {
            AspectPreset::Freeform => None,
            AspectPreset::Original => (image.height > 0.0).then(|| image.width / image.height),
            AspectPreset::Square => Some(1.0),
            AspectPreset::Landscape16x9 => Some(16.0 / 9.0),
            AspectPreset::Portrait9x16 => Some(9.0 / 16.0),
            AspectPreset::Landscape4x3 => Some(4.0 / 3.0),
            AspectPreset::Landscape3x2 => Some(3.0 / 2.0),
        }
    }

    pub fn locks_aspect(self) -> bool {
        self != AspectPreset::Freeform
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CropHandle {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl CropHandle {
    pub const ALL: [CropHandle; 8] = [
        CropHandle::TopLeft,
        CropHandle::Top,
        CropHandle::TopRight,
        CropHandle::Left,
        CropHandle::Right,
        CropHandle::BottomLeft,
        CropHandle::Bottom,
        CropHandle::BottomRight,
    ];

    pub fn is_corner(self) -> bool {
        matches!(
            self,
            CropHandle::TopLeft
                | CropHandle::TopRight
                | CropHandle::BottomLeft
                | CropHandle::BottomRight
        )
    }

    pub fn is_left(self) -> bool {
        matches!(
            self,
            CropHandle::TopLeft | CropHandle::Left | CropHandle::BottomLeft
        )
    }

    pub fn is_top(self) -> bool {
        matches!(
            self,
            CropHandle::TopLeft | CropHandle::Top | CropHandle::TopRight
        )
    }

    /// Position within the crop rect, 0..=1 on each axis.
    pub fn unit_point(self) -> Point {
        let x = match self {
            CropHandle::TopLeft | CropHandle::Left | CropHandle::BottomLeft => 0.0,
            CropHandle::Top | CropHandle::Bottom => 0.5,
            _ => 1.0,
        };
        let y = match self {
            CropHandle::TopLeft | CropHandle::Top | CropHandle::TopRight => 0.0,
            CropHandle::Left | CropHandle::Right => 0.5,
            _ => 1.0,
        };
        Point::new(x, y)
    }

    /// Edge handles are hidden while an aspect is locked.
    pub fn visible(self, aspect: AspectPreset) -> bool {
        self.is_corner() || !aspect.locks_aspect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropResult {
    pub region: PixelRect,
    pub map: CropMap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CropState {
    image: Size,
    rect: Rect,
    aspect: AspectPreset,
}

fn fit(w: f64, h: f64, aspect: f64) -> (f64, f64) {
    if h <= 0.0 || w / h > aspect {
        (h * aspect, h)
    } else {
        (w, w / aspect)
    }
}

impl CropState {
    pub fn new(image: Size) -> Self {
        Self {
            image,
            rect: Rect::from_origin_size(Point::ZERO, image),
            aspect: AspectPreset::Freeform,
        }
    }

    pub fn image_size(&self) -> Size {
        self.image
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn aspect(&self) -> AspectPreset {
        self.aspect
    }

    fn ratio(&self) -> Option<f64> {
        self.aspect.ratio(self.image).filter(|r| *r > 0.0)
    }

    fn bounds(&self) -> Rect {
        Rect::from_origin_size(Point::ZERO, self.image)
    }

    pub fn min_size(&self) -> Size {
        let axis = |len: f64| (len * 0.01).max((len * 0.5).min(MIN_CROP_PIXELS));
        Size::new(axis(self.image.width), axis(self.image.height))
    }

    /// The output size in whole pixels, as the footer shows it while cropping.
    pub fn output_size(&self) -> (u32, u32) {
        let r = self.pixel_region();
        (r.width, r.height)
    }

    pub fn is_full(&self) -> bool {
        self.rect == self.bounds()
    }

    pub fn reset(&mut self) {
        self.rect = self.bounds();
        if let Some(ratio) = self.ratio() {
            self.rect = self.fit_aspect(self.rect, ratio);
        }
    }

    pub fn cancel(&mut self) {
        self.aspect = AspectPreset::Freeform;
        self.rect = self.bounds();
    }

    /// Re-fits the largest rect of the new ratio centred on the current one.
    pub fn set_aspect(&mut self, aspect: AspectPreset) {
        self.aspect = aspect;
        if let Some(ratio) = self.ratio() {
            self.rect = self.fit_aspect(self.rect, ratio);
        }
    }

    pub fn move_by(&mut self, delta: Vec2) {
        let (w, h) = (self.rect.width(), self.rect.height());
        let x = (self.rect.x0 + delta.x).clamp(0.0, (self.image.width - w).max(0.0));
        let y = (self.rect.y0 + delta.y).clamp(0.0, (self.image.height - h).max(0.0));
        self.rect = Rect::new(x, y, x + w, y + h);
    }

    /// Corner drags keep a locked aspect; edge drags ignore it. `from_center` resizes about the centre.
    pub fn resize(&mut self, handle: CropHandle, to: Point, from_center: bool) {
        let p = Point::new(
            to.x.clamp(0.0, self.image.width),
            to.y.clamp(0.0, self.image.height),
        );
        self.rect = if from_center {
            self.resize_from_center(handle, p)
        } else if handle.is_corner() {
            self.resize_corner(handle, p)
        } else {
            self.resize_edge(handle, p)
        };
    }

    pub fn pixel_region(&self) -> PixelRect {
        let (iw, ih) = (self.image.width.round(), self.image.height.round());
        let x0 = self.rect.x0.round().clamp(0.0, iw);
        let y0 = self.rect.y0.round().clamp(0.0, ih);
        let x1 = self.rect.x1.round().clamp(x0, iw);
        let y1 = self.rect.y1.round().clamp(y0, ih);
        PixelRect {
            x: x0 as u32,
            y: y0 as u32,
            width: ((x1 - x0) as u32).max(1),
            height: ((y1 - y0) as u32).max(1),
        }
    }

    /// `None` when the crop covers the whole image.
    pub fn apply(&self) -> Option<CropResult> {
        if self.is_full() {
            return None;
        }
        let region = self.pixel_region();
        let (w, h) = (self.rect.width(), self.rect.height());
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        Some(CropResult {
            region,
            map: CropMap {
                origin: Point::new(self.rect.x0, self.rect.y0),
                sx: f64::from(region.width) / w,
                sy: f64::from(region.height) / h,
            },
        })
    }

    fn fit_aspect(&self, rect: Rect, ratio: f64) -> Rect {
        let (w, h) = fit(rect.width(), rect.height(), ratio);
        let (w, h) = fit(w.min(self.image.width), h.min(self.image.height), ratio);
        let x = (rect.center().x - w / 2.0).clamp(0.0, (self.image.width - w).max(0.0));
        let y = (rect.center().y - h / 2.0).clamp(0.0, (self.image.height - h).max(0.0));
        Rect::new(x, y, x + w, y + h)
    }

    fn resize_edge(&self, handle: CropHandle, p: Point) -> Rect {
        let min = self.min_size();
        let mut r = self.rect;
        match handle {
            CropHandle::Left => r.x0 = p.x.min(r.x1 - min.width),
            CropHandle::Right => r.x1 = p.x.max(r.x0 + min.width),
            CropHandle::Top => r.y0 = p.y.min(r.y1 - min.height),
            CropHandle::Bottom => r.y1 = p.y.max(r.y0 + min.height),
            _ => {}
        }
        r
    }

    fn resize_corner(&self, handle: CropHandle, p: Point) -> Rect {
        let min = self.min_size();
        let (left, top) = (handle.is_left(), handle.is_top());
        let ax = if left { self.rect.x1 } else { self.rect.x0 };
        let ay = if top { self.rect.y1 } else { self.rect.y0 };
        let mut cx = if left {
            p.x.min(ax - min.width)
        } else {
            p.x.max(ax + min.width)
        };
        let mut cy = if top {
            p.y.min(ay - min.height)
        } else {
            p.y.max(ay + min.height)
        };
        if let Some(ratio) = self.ratio() {
            let (mut w, mut h) = fit((cx - ax).abs(), (cy - ay).abs(), ratio);
            let max_w = if left { ax } else { self.image.width - ax };
            let max_h = if top { ay } else { self.image.height - ay };
            if w > max_w {
                w = max_w;
                h = w / ratio;
            }
            if h > max_h {
                h = max_h;
                w = h * ratio;
            }
            cx = if left { ax - w } else { ax + w };
            cy = if top { ay - h } else { ay + h };
        }
        Rect::new(cx.min(ax), cy.min(ay), cx.max(ax), cy.max(ay))
    }

    fn resize_from_center(&self, handle: CropHandle, p: Point) -> Rect {
        let min = self.min_size();
        let c = self.rect.center();
        let mut hw = self.rect.width() / 2.0;
        let mut hh = self.rect.height() / 2.0;
        let ratio = self.ratio().filter(|_| handle.is_corner());
        if handle.is_corner() {
            hw = (p.x - c.x).abs();
            hh = (p.y - c.y).abs();
            if let Some(ratio) = ratio {
                (hw, hh) = fit(hw, hh, ratio);
            }
        } else {
            match handle {
                CropHandle::Left | CropHandle::Right => hw = (p.x - c.x).abs(),
                _ => hh = (p.y - c.y).abs(),
            }
        }
        hw = hw.max(min.width / 2.0).min(c.x.min(self.image.width - c.x));
        hh = hh
            .max(min.height / 2.0)
            .min(c.y.min(self.image.height - c.y));
        if let Some(ratio) = ratio {
            (hw, hh) = fit(hw, hh, ratio);
        }
        Rect::new(c.x - hw, c.y - hh, c.x + hw, c.y + hh)
    }
}
