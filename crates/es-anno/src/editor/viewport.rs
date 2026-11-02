use es_geom::{Point, Size};
use es_model::style::{page_stroke_width, slider_stroke_width};

/// `origin` is where page (0, 0) sits on screen; `scale` is screen units per page unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub origin: Point,
    pub scale: f64,
    pub image_size: Size,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            origin: Point::ZERO,
            scale: 1.0,
            image_size: Size::ZERO,
        }
    }
}

impl Viewport {
    fn scale(&self) -> f64 {
        if self.scale.is_finite() && self.scale > 0.0 {
            self.scale
        } else {
            1.0
        }
    }

    pub fn page_to_screen(&self, p: Point) -> Point {
        let s = self.scale();
        Point::new(self.origin.x + p.x * s, self.origin.y + p.y * s)
    }

    pub fn screen_to_page(&self, p: Point) -> Point {
        let s = self.scale();
        Point::new((p.x - self.origin.x) / s, (p.y - self.origin.y) / s)
    }

    /// Keeps hit margins and handles a constant screen size at every zoom.
    pub fn page_distance(&self, screen: f64) -> f64 {
        screen / self.scale()
    }

    fn edge(&self) -> f64 {
        self.image_size.width.max(self.image_size.height)
    }

    pub fn page_stroke_width(&self, slider: f64) -> f64 {
        if self.edge() > 0.0 {
            page_stroke_width(slider, self.image_size.width, self.image_size.height)
        } else {
            slider
        }
    }

    pub fn slider_stroke_width(&self, page: f64) -> f64 {
        if self.edge() > 0.0 {
            slider_stroke_width(page, self.image_size.width, self.image_size.height)
        } else {
            page
        }
    }
}
