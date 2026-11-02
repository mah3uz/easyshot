use super::paint::{accent, b};
use super::strings::fl;
use es_geom::{Point, Rect};
use es_ui::a11y::{Accessible as _, Role};
use es_ui::components::icon_button::IconButton;
use es_ui::convert::hsla as to_hsla;
use es_ui::icons::IconName;
use es_ui::theme::UI_FONT;
use es_ui::tokens::{Colours, LIGHT, Tokens};
use gpui::{
    App, Context, Corners, EventEmitter, FontWeight, Hsla, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, RenderImage, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, canvas, div, px,
};
use std::sync::Arc;

pub const HUD: (f64, f64) = (400.0, 36.0);
const HUD_GAP: f64 = 6.0;
const EDGE: f64 = 4.0;
const OUTLINE: f64 = 2.0;
pub const PREVIEW_WIDTH: f64 = 200.0;
const PREVIEW_MARGIN: f64 = 12.0;
const PREVIEW_TOP: f64 = 20.0;
const PREVIEW_MIN: f64 = 100.0;
const RADIUS: f32 = 12.0;
const PILL_HEIGHT: f32 = 24.0;
const AUTO_WIDTH: f32 = 90.0;
const STOP_WIDTH: f32 = 56.0;

/// Below the area, else above it, kept inside the output.
pub fn hud_origin(area: Rect, visible: Rect) -> Point {
    let (w, h) = HUD;
    let below = area.y1 + HUD_GAP;
    let y = if below + h <= visible.y1 - EDGE {
        below
    } else {
        area.y0 - HUD_GAP - h
    };
    let x = area.center().x - w / 2.0;
    let clamp = |v: f64, lo: f64, hi: f64| v.min(hi).max(lo);
    Point::new(
        clamp(x, visible.x0 + EDGE, visible.x1 - EDGE - w),
        clamp(y, visible.y0 + EDGE, visible.y1 - EDGE - h),
    )
}

/// The whole column the preview may grow into, right of the area when there is room, else left.
pub fn preview_column(area: Rect, visible: Rect) -> Option<Rect> {
    let room = PREVIEW_WIDTH + 2.0 * PREVIEW_MARGIN;
    let x0 = if visible.x1 - area.x1 >= room {
        area.x1 + PREVIEW_MARGIN
    } else if area.x0 - visible.x0 >= room {
        area.x0 - PREVIEW_MARGIN - PREVIEW_WIDTH
    } else {
        return None;
    };
    let y0 = visible.y0 + PREVIEW_TOP;
    let y1 = area.y1.min(visible.y1);
    (y1 - y0 >= PREVIEW_MIN).then(|| Rect::new(x0, y0, x0 + PREVIEW_WIDTH, y1))
}

/// Strips just outside the area, so the outline is never in a captured frame; any that would leave the output are dropped.
pub fn outline(area: Rect, visible: Rect) -> Vec<Rect> {
    let o = OUTLINE;
    [
        Rect::new(area.x0 - o, area.y0 - o, area.x1 + o, area.y0),
        Rect::new(area.x0 - o, area.y1, area.x1 + o, area.y1 + o),
        Rect::new(area.x0 - o, area.y0, area.x0, area.y1),
        Rect::new(area.x1, area.y0, area.x1 + o, area.y1),
    ]
    .into_iter()
    .filter(|r| {
        r.x0 >= visible.x0 && r.y0 >= visible.y0 && r.x1 <= visible.x1 && r.y1 <= visible.y1
    })
    .collect()
}

fn colours(cx: &App) -> Colours<Hsla> {
    cx.try_global::<Tokens>()
        .map(|t| t.colours)
        .unwrap_or_else(|| LIGHT.map(to_hsla))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudEvent {
    Cancel,
    Stop,
    Auto(bool),
}

pub struct ScrollHud {
    /// Points; `None` until the first frame is in.
    size: Option<(u32, u32)>,
    /// `None` when there is no virtual pointer to scroll with.
    auto: Option<bool>,
}

impl EventEmitter<HudEvent> for ScrollHud {}

impl ScrollHud {
    pub fn new(auto_available: bool) -> Self {
        ScrollHud {
            size: None,
            auto: auto_available.then_some(false),
        }
    }

    pub fn size(&self) -> Option<(u32, u32)> {
        self.size
    }

    pub fn auto(&self) -> Option<bool> {
        self.auto
    }

    pub fn set_size(&mut self, w: u32, h: u32, cx: &mut Context<Self>) {
        self.size = Some((w, h));
        cx.notify();
    }

    pub fn set_auto(&mut self, running: bool, cx: &mut Context<Self>) {
        if self.auto.is_some() {
            self.auto = Some(running);
            cx.notify();
        }
    }

    pub fn title(&self) -> String {
        match self.size {
            Some((w, h)) => fl!("scroll-title-size", w = w, h = h),
            None => fl!("scroll-title"),
        }
    }
}

fn pill(
    id: &'static str,
    width: f32,
    text: String,
    label: String,
    fill: Hsla,
    ink: Hsla,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .accessible(Role::Button, label)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(width))
        .h(px(PILL_HEIGHT))
        .rounded(px(PILL_HEIGHT / 2.))
        .bg(fill)
        .text_color(ink)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .hover(move |s| s.bg(fill.opacity(fill.a * 0.85)))
        .child(text)
}

impl Render for ScrollHud {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = colours(cx);
        let (w, h) = self.size.unwrap_or((0, 0));
        let label = match self.size {
            Some(_) => fl!("scroll-hud-a11y", w = w, h = h),
            None => fl!("scroll-title"),
        };
        let cancel = IconButton::new("scroll-cancel", IconName::X, fl!("scroll-cancel-a11y"))
            .size(px(20.), px(11.))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(HudEvent::Cancel)));
        let mut bar = div()
            .id("scroll-hud")
            .accessible(Role::Toolbar, label)
            .size_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .rounded(px(RADIUS))
            .bg(t.inspector)
            .border_1()
            .border_color(t.border)
            .font_family(UI_FONT)
            .text_color(t.text)
            .child(cancel)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(SharedString::from(self.title())),
            );
        if let Some(running) = self.auto {
            let (text, label, fill, ink) = if running {
                (
                    fl!("scroll-scrolling"),
                    fl!("scroll-pause-auto-a11y"),
                    t.accent_tint,
                    t.on_accent,
                )
            } else {
                (
                    fl!("scroll-auto"),
                    fl!("scroll-start-auto-a11y"),
                    t.accent,
                    t.on_accent,
                )
            };
            bar = bar.child(
                pill("scroll-auto", AUTO_WIDTH, text, label, fill, ink)
                    .toggled(running)
                    .on_click(cx.listener(move |_, _, _, cx| cx.emit(HudEvent::Auto(!running)))),
            );
        }
        bar.child(
            pill(
                "scroll-stop",
                STOP_WIDTH,
                fl!("scroll-stop"),
                fl!("scroll-stop-a11y"),
                t.destructive,
                t.on_accent,
            )
            .on_click(cx.listener(|_, _, _, cx| cx.emit(HudEvent::Stop))),
        )
    }
}

/// The stitched image so far, scaled to the column and growing up from the area's bottom edge.
pub struct ScrollPreview {
    image: Option<(Arc<RenderImage>, f64, f64)>,
}

impl Default for ScrollPreview {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollPreview {
    pub fn new() -> Self {
        ScrollPreview { image: None }
    }

    /// `w` and `h` are logical points.
    pub fn set_image(&mut self, image: Arc<RenderImage>, w: f64, h: f64, cx: &mut Context<Self>) {
        // Every update is a new image, so the old one leaves the atlas rather than piling up there.
        if let Some((old, _, _)) = self.image.replace((image, w, h)) {
            cx.drop_image(old, None);
        }
        cx.notify();
    }
}

impl Render for ScrollPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = colours(cx);
        let image = self.image.clone();
        let height = image
            .as_ref()
            .map_or(PREVIEW_MIN, |(_, _, h)| h.max(PREVIEW_MIN));
        div().size_full().flex().flex_col().justify_end().child(
            div()
                .id("scroll-preview")
                .accessible(Role::Image, fl!("scroll-title"))
                .w_full()
                .h(px(height as f32))
                .max_h_full()
                .rounded(px(6.))
                .overflow_hidden()
                .bg(t.inspector)
                .border_1()
                .border_color(t.border)
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let Some((image, w, h)) = &image else { return };
                            let x = f64::from(bounds.size.width) / 2.0 - w / 2.0;
                            let at = b(Rect::new(x, 0.0, x + w, *h)
                                + Point::new(
                                    f64::from(bounds.origin.x),
                                    f64::from(bounds.origin.y),
                                )
                                .to_vec2());
                            if let Err(error) = window.paint_image(
                                at,
                                at,
                                Corners::default(),
                                image.clone(),
                                0,
                                false,
                            ) {
                                tracing::warn!(%error, "cannot paint the scroll preview");
                            }
                        },
                    )
                    .size_full(),
                ),
        )
    }
}

pub struct Strip;

impl Render for Strip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(accent(cx))
    }
}
