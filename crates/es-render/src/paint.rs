use crate::effects::redaction_patch;
use crate::text::skia_path;
use es_anno::draw::{DrawItem, DrawList, SPOTLIGHT_DIM};
use es_geom::{Affine, Cap, FillRule, Join, Rect, Rgba};
use kurbo::{RoundedRect, RoundedRectRadii, Shape};
use tiny_skia::{
    BlendMode, Color, LineCap, LineJoin, Paint, Path, Pixmap, PixmapPaint, Stroke, Transform,
};

/// The screenshot's outline in page space; the spotlight dim is confined to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipShape {
    pub rect: Rect,
    /// Top-left, top-right, bottom-right, bottom-left.
    pub radii: [f64; 4],
}

impl ClipShape {
    pub fn page(width: f64, height: f64) -> Self {
        Self {
            rect: Rect::new(0.0, 0.0, width, height),
            radii: [0.0; 4],
        }
    }

    fn path(&self) -> Option<Path> {
        let [tl, tr, br, bl] = self.radii;
        let rr = RoundedRect::from_rect(self.rect, RoundedRectRadii::new(tl, tr, br, bl));
        skia_path(&rr.to_path(0.05))
    }
}

pub fn paint_draw_list(
    list: &DrawList,
    target: &mut Pixmap,
    page_to_pixels: Affine,
    clip: ClipShape,
    snapshot: &Pixmap,
) {
    let cx = Cx {
        clip,
        snapshot,
        page: page_to_pixels,
    };
    paint_items(&list.items, target, &cx, to_transform(page_to_pixels), 1.0);
}

struct Cx<'a> {
    clip: ClipShape,
    snapshot: &'a Pixmap,
    page: Affine,
}

fn to_transform(a: Affine) -> Transform {
    let [a, b, c, d, e, f] = a.as_coeffs();
    Transform::from_row(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32)
}

fn paint_for(color: Rgba, opacity: f64) -> Paint<'static> {
    let mut p = Paint::default();
    let a = (color.a as f64 * opacity).clamp(0.0, 1.0) as f32;
    p.set_color(
        Color::from_rgba(
            color.r.clamp(0.0, 1.0),
            color.g.clamp(0.0, 1.0),
            color.b.clamp(0.0, 1.0),
            a,
        )
        .unwrap_or(Color::TRANSPARENT),
    );
    p.anti_alias = true;
    p
}

fn paint_items(items: &[DrawItem], target: &mut Pixmap, cx: &Cx, ts: Transform, opacity: f64) {
    for item in items {
        match item {
            DrawItem::Fill { path, color, rule } => {
                if let Some(p) = skia_path(path) {
                    target.fill_path(&p, &paint_for(*color, opacity), fill_rule(*rule), ts, None);
                }
            }
            DrawItem::Stroke {
                path,
                color,
                width,
                cap,
                join,
                ..
            } => {
                if let Some(p) = skia_path(path) {
                    let stroke = Stroke {
                        width: *width as f32,
                        line_cap: match cap {
                            Cap::Butt => LineCap::Butt,
                            Cap::Round => LineCap::Round,
                            Cap::Square => LineCap::Square,
                        },
                        line_join: match join {
                            Join::Miter => LineJoin::Miter,
                            Join::Round => LineJoin::Round,
                            Join::Bevel => LineJoin::Bevel,
                        },
                        ..Stroke::default()
                    };
                    target.stroke_path(&p, &paint_for(*color, opacity), &stroke, ts, None);
                }
            }
            DrawItem::Text {
                glyphs,
                color,
                underline,
            } => {
                let paint = paint_for(*color, opacity);
                if let Some(p) = skia_path(glyphs) {
                    target.fill_path(&p, &paint, tiny_skia::FillRule::Winding, ts, None);
                }
                for r in underline {
                    if let Some(r) = tiny_skia::Rect::from_ltrb(
                        r.x0 as f32,
                        r.y0 as f32,
                        r.x1 as f32,
                        r.y1 as f32,
                    ) {
                        target.fill_rect(r, &paint, ts, None);
                    }
                }
            }
            DrawItem::Group {
                transform,
                opacity: o,
                items,
            } => {
                let inner = ts.pre_concat(to_transform(*transform));
                paint_items(items, target, cx, inner, opacity * o);
            }
            DrawItem::Spotlight { holes } => spotlight(holes, target, cx),
            DrawItem::Redaction {
                rect,
                rotation,
                mirrored,
                kind,
                density,
            } => {
                let [a, b, c, d, e, f] = cx.page.as_coeffs();
                let s = (a * a + b * b).sqrt();
                let origin = (a * rect.x0 + c * rect.y0 + e, b * rect.x0 + d * rect.y0 + f);
                let r = Rect::new(
                    origin.0,
                    origin.1,
                    origin.0 + rect.width() * s,
                    origin.1 + rect.height() * s,
                );
                if let Some(patch) =
                    redaction_patch(cx.snapshot, r, *rotation, *mirrored, *kind, *density, s)
                {
                    patch.draw_onto(target);
                }
            }
        }
    }
}

fn fill_rule(r: FillRule) -> tiny_skia::FillRule {
    match r {
        FillRule::NonZero => tiny_skia::FillRule::Winding,
        FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
    }
}

fn spotlight(holes: &[es_geom::BezPath], target: &mut Pixmap, cx: &Cx) {
    let Some(clip) = cx.clip.path() else { return };
    let Some(mut layer) = Pixmap::new(target.width(), target.height()) else {
        return;
    };
    let ts = to_transform(cx.page);
    layer.fill_path(
        &clip,
        &paint_for(Rgba::new(0.0, 0.0, 0.0, SPOTLIGHT_DIM), 1.0),
        tiny_skia::FillRule::Winding,
        ts,
        None,
    );
    let clear = Paint {
        blend_mode: BlendMode::Clear,
        anti_alias: true,
        ..Paint::default()
    };
    for h in holes {
        if let Some(p) = skia_path(h) {
            layer.fill_path(&p, &clear, tiny_skia::FillRule::Winding, ts, None);
        }
    }
    target.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}
