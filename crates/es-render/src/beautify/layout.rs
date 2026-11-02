use es_geom::Rect;
use es_model::{Alignment, BackgroundSettings};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub canvas_width: f64,
    pub canvas_height: f64,
    pub card: Rect,
    pub image: Rect,
    pub padding: f64,
    pub border_width: f64,
    /// Top-left, top-right, bottom-right, bottom-left.
    pub image_radii: [f64; 4],
    pub card_radii: [f64; 4],
}

impl Layout {
    pub fn canvas_pixels(&self) -> (u32, u32) {
        (
            self.canvas_width.ceil() as u32,
            self.canvas_height.ceil() as u32,
        )
    }

    /// The image keeps its exact size; only its origin moves onto the pixel grid.
    pub fn pixel_aligned(mut self) -> Self {
        let dx = self.image.x0.round() - self.image.x0;
        let dy = self.image.y0.round() - self.image.y0;
        let shift = |r: Rect| Rect::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy);
        self.image = shift(self.image);
        self.card = shift(self.card);
        self
    }
}

fn empty() -> Layout {
    Layout {
        canvas_width: 0.0,
        canvas_height: 0.0,
        card: Rect::ZERO,
        image: Rect::ZERO,
        padding: 0.0,
        border_width: 0.0,
        image_radii: [0.0; 4],
        card_radii: [0.0; 4],
    }
}

pub fn layout(width: f64, height: f64, settings: &BackgroundSettings) -> Layout {
    if width <= 0.0 || height <= 0.0 {
        return empty();
    }
    if !settings.requires_canvas_layout() {
        let rect = Rect::new(0.0, 0.0, width, height);
        return Layout {
            canvas_width: width,
            canvas_height: height,
            card: rect,
            image: rect,
            ..empty()
        };
    }

    let alignment = settings.effective_canvas_alignment();
    let uses_stage = settings.uses_canvas_layout();
    let border = settings.border.pixel_thickness(width, height);
    let card_w = width + border * 2.0;
    let card_h = height + border * 2.0;
    let padding = if uses_stage {
        (width.min(height) * settings.padding).max(0.0)
    } else {
        0.0
    };
    let edge = |sticks: bool| if sticks { 0.0 } else { padding };
    let (top, bottom) = (
        edge(alignment.sticks_to_top()),
        edge(alignment.sticks_to_bottom()),
    );
    let (leading, trailing) = (
        edge(alignment.sticks_to_leading()),
        edge(alignment.sticks_to_trailing()),
    );

    let mut canvas_w = card_w + leading + trailing;
    let mut canvas_h = card_h + top + bottom;
    if let Some(ratio) = settings.aspect_ratio.value().filter(|_| uses_stage) {
        if canvas_w / canvas_h < ratio {
            canvas_w = canvas_h * ratio;
        } else {
            canvas_h = canvas_w / ratio;
        }
    }

    let avail_w = canvas_w - leading - trailing;
    let avail_h = canvas_h - top - bottom;
    let x = leading + (avail_w - card_w).max(0.0) * alignment.x_factor();
    let y = top + (avail_h - card_h).max(0.0) * alignment.y_factor();
    let card = Rect::new(x, y, x + card_w, y + card_h);
    let image = Rect::new(
        x + border,
        y + border,
        x + border + width,
        y + border + height,
    );

    let base = settings.corner_radius.max(0.0) * width.min(height);
    let card_base = if base > 0.0 { base + border } else { 0.0 };
    Layout {
        canvas_width: canvas_w,
        canvas_height: canvas_h,
        card,
        image,
        padding,
        border_width: border,
        image_radii: corner_radii(base, alignment),
        card_radii: corner_radii(card_base, alignment),
    }
}

fn corner_radii(base: f64, alignment: Alignment) -> [f64; 4] {
    let m = alignment.corner_multipliers();
    [
        base * f64::from(m.top_left),
        base * f64::from(m.top_right),
        base * f64::from(m.bottom_right),
        base * f64::from(m.bottom_left),
    ]
}
