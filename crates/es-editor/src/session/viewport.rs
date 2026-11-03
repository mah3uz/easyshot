use crate::session::doc::DocModel;
use es_anno::Viewport;
use es_geom::{Point, Rect, Size};
use es_model::BackgroundSettings;
use es_render::beautify::{Layout, layout};
use gpui::{Context, Entity};

pub const MIN_ZOOM_PERCENT: f64 = 10.0;
pub const MAX_ZOOM_PERCENT: f64 = 400.0;
pub const ZOOM_IN_STEP: f64 = 1.25;
pub const ZOOM_OUT_STEP: f64 = 0.8;
pub const WHEEL_ZOOM_RATE: f64 = 0.0025;
/// Horizontal and vertical breathing room between the workspace edge and a fitted canvas.
pub const CANVAS_INSET: Size = Size::new(34.0, 28.0);

/// Zoom and pan maths, in the canvas element's logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewMath {
    pub container: Size,
    pub scale_factor: f64,
    pub layout: Layout,
    pub fit: bool,
    pub manual_scale: f64,
    pub pan: Point,
}

impl ViewMath {
    pub fn new(layout: Layout) -> Self {
        Self {
            container: Size::new(1.0, 1.0),
            scale_factor: 1.0,
            layout,
            fit: true,
            manual_scale: 1.0,
            pan: Point::ZERO,
        }
    }

    fn canvas(&self) -> Size {
        Size::new(
            self.layout.canvas_width.max(1.0),
            self.layout.canvas_height.max(1.0),
        )
    }

    pub fn fit_scale(&self) -> f64 {
        let c = self.canvas();
        let w = (self.container.width - CANVAS_INSET.width * 2.0).max(1.0);
        let h = (self.container.height - CANVAS_INSET.height * 2.0).max(1.0);
        (w / c.width).min(h / c.height)
    }

    fn scale_limits(&self) -> (f64, f64) {
        let sf = self.scale_factor.max(f64::EPSILON);
        (MIN_ZOOM_PERCENT / 100.0 / sf, MAX_ZOOM_PERCENT / 100.0 / sf)
    }

    /// Logical pixels per page pixel.
    pub fn scale(&self) -> f64 {
        if self.fit {
            self.fit_scale()
        } else {
            self.manual_scale
        }
    }

    /// 100 % is one image pixel per physical pixel.
    pub fn zoom_percent(&self) -> f64 {
        (self.scale() * self.scale_factor * 100.0).round().max(1.0)
    }

    pub fn max_pan(&self) -> Size {
        let c = self.canvas();
        let s = self.scale();
        Size::new(
            ((c.width * s - self.container.width) / 2.0).max(0.0),
            ((c.height * s - self.container.height) / 2.0).max(0.0),
        )
    }

    pub fn can_pan(&self) -> bool {
        let m = self.max_pan();
        m.width > 0.25 || m.height > 0.25
    }

    /// Where the beautify canvas sits in the element.
    pub fn canvas_rect(&self) -> Rect {
        let c = self.canvas();
        let s = self.scale();
        let x = (self.container.width - c.width * s) / 2.0 + self.pan.x;
        let y = (self.container.height - c.height * s) / 2.0 + self.pan.y;
        Rect::new(x, y, x + c.width * s, y + c.height * s)
    }

    pub fn viewport(&self) -> Viewport {
        let r = self.canvas_rect();
        let s = self.scale();
        Viewport {
            origin: Point::new(
                r.x0 + self.layout.image.x0 * s,
                r.y0 + self.layout.image.y0 * s,
            ),
            scale: s,
            image_size: Size::new(self.layout.image.width(), self.layout.image.height()),
        }
    }

    fn clamp_pan(&mut self) {
        let m = self.max_pan();
        self.pan = Point::new(
            self.pan.x.clamp(-m.width, m.width),
            self.pan.y.clamp(-m.height, m.height),
        );
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64) {
        self.pan = Point::new(self.pan.x + dx, self.pan.y + dy);
        self.clamp_pan();
    }

    /// Keeps the canvas point under `anchor` where it is, unless the pan clamp forbids it.
    pub fn zoom_at(&mut self, factor: f64, anchor: Point) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let (lo, hi) = self.scale_limits();
        let s = self.scale();
        let next = (s * factor).clamp(lo, hi);
        let r = self.canvas_rect();
        let page = Point::new((anchor.x - r.x0) / s, (anchor.y - r.y0) / s);
        let c = self.canvas();
        self.fit = false;
        self.manual_scale = next;
        self.pan = Point::new(
            anchor.x - page.x * next - (self.container.width - c.width * next) / 2.0,
            anchor.y - page.y * next - (self.container.height - c.height * next) / 2.0,
        );
        self.clamp_pan();
    }

    pub fn centre(&self) -> Point {
        Point::new(self.container.width / 2.0, self.container.height / 2.0)
    }

    pub fn set_percent(&mut self, percent: f64) {
        let (lo, hi) = self.scale_limits();
        self.fit = false;
        self.manual_scale = (percent / 100.0 / self.scale_factor.max(f64::EPSILON)).clamp(lo, hi);
        self.pan = Point::ZERO;
    }

    pub fn fit(&mut self) {
        self.fit = true;
        self.pan = Point::ZERO;
    }
}

/// The only writer of the engine's viewport.
pub struct ViewportModel {
    doc: Entity<DocModel>,
    math: ViewMath,
    revision: u64,
    anchor_at_pointer: bool,
    settings: BackgroundSettings,
}

impl ViewportModel {
    pub fn new(doc: Entity<DocModel>, cx: &mut Context<Self>) -> Self {
        let d = doc.read(cx);
        let size = d.image_size();
        let lay = layout(size.width, size.height, d.background()).pixel_aligned();
        let settings = d.background().clone();
        let mut this = Self {
            doc,
            math: ViewMath::new(lay),
            revision: 1,
            anchor_at_pointer: true,
            settings,
        };
        this.push(cx);
        this
    }

    pub fn math(&self) -> &ViewMath {
        &self.math
    }

    pub fn viewport(&self) -> Viewport {
        self.math.viewport()
    }

    /// Bumped on every pan, zoom, resize or layout change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn zoom_percent(&self) -> f64 {
        self.math.zoom_percent()
    }

    pub fn is_fit(&self) -> bool {
        self.math.fit
    }

    /// `false` zooms wheel and pinch gestures about the centre instead of the pointer.
    pub fn set_zoom_anchor_pointer(&mut self, at_pointer: bool) {
        self.anchor_at_pointer = at_pointer;
    }

    pub fn zoom_anchor_pointer(&self) -> bool {
        self.anchor_at_pointer
    }

    pub fn set_container(&mut self, size: Size, scale_factor: f64, cx: &mut Context<Self>) {
        self.update(cx, |m| {
            m.container = size;
            m.scale_factor = scale_factor;
            m.clamp_pan();
        });
    }

    pub fn set_background(&mut self, settings: &BackgroundSettings, cx: &mut Context<Self>) {
        let size = self.doc.read(cx).image_size();
        let lay = layout(size.width, size.height, settings).pixel_aligned();
        self.settings = settings.clone();
        self.update(cx, |m| {
            m.layout = lay;
            m.clamp_pan();
        });
    }

    /// Lays out for a replaced base image and fits it.
    pub fn image_changed(&mut self, cx: &mut Context<Self>) {
        let size = self.doc.read(cx).image_size();
        let lay = layout(size.width, size.height, &self.settings).pixel_aligned();
        self.update(cx, |m| {
            m.layout = lay;
            m.fit();
        });
    }

    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.update(cx, |m| m.zoom_at(ZOOM_IN_STEP, m.centre()));
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.update(cx, |m| m.zoom_at(ZOOM_OUT_STEP, m.centre()));
    }

    pub fn set_percent(&mut self, percent: f64, cx: &mut Context<Self>) {
        self.update(cx, |m| m.set_percent(percent));
    }

    pub fn actual_size(&mut self, cx: &mut Context<Self>) {
        self.set_percent(100.0, cx);
    }

    pub fn fit(&mut self, cx: &mut Context<Self>) {
        self.update(cx, ViewMath::fit);
    }

    pub fn pan_by(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        self.update(cx, |m| m.pan_by(dx, dy));
    }

    /// For wheel and pinch: about `pointer`, or the centre when so configured.
    pub fn zoom_at(&mut self, factor: f64, pointer: Point, cx: &mut Context<Self>) {
        let at_pointer = self.anchor_at_pointer;
        self.update(cx, |m| {
            let anchor = if at_pointer { pointer } else { m.centre() };
            m.zoom_at(factor, anchor);
        });
    }

    fn update(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut ViewMath)) {
        let before = self.math;
        f(&mut self.math);
        if self.math != before {
            self.revision += 1;
            self.push(cx);
            cx.notify();
        }
    }

    fn push(&mut self, cx: &mut Context<Self>) {
        let viewport = self.math.viewport();
        self.doc.update(cx, |d, _| d.set_viewport(viewport));
    }
}
