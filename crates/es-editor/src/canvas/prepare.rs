use super::grid::{self, GridKey};
use super::paths::{self, Bucket, Painted, Placement};
use super::tiles::{self, BaseTiles};
use super::underlay::{self, UnderlayKey};
use super::{EditorCanvas, FrameStats, ShapeEntry, SpotEntry, text_shadow};
use es_anno::draw::{SPOTLIGHT_DIM, draw_items};
use es_anno::{BuildContext, DrawItem, Viewport, build_draw_list};
use es_geom::{Rect, Rgba, Size};
use es_model::AnnoShapeId;
use gpui::{Bounds, Context, Corners, Pixels, RenderImage, Window, point, px, size};
use std::sync::Arc;

pub struct Prepared {
    pub grid: Option<(Arc<RenderImage>, Vec<Bounds<Pixels>>)>,
    pub underlay: Option<(Bounds<Pixels>, Arc<RenderImage>)>,
    /// When set, it stands in for the underlay, tiles, redactions and shapes.
    pub effects: Option<(Bounds<Pixels>, Arc<RenderImage>)>,
    /// Drawn over the flat canvas; a composite carries its own.
    pub watermark: Option<(Bounds<Pixels>, Arc<RenderImage>)>,
    pub tiles: Vec<(Bounds<Pixels>, Corners<Pixels>, Arc<RenderImage>)>,
    pub redactions: Vec<DrawItem>,
    pub viewport: Viewport,
    pub image: Bounds<Pixels>,
    pub spotlight: Vec<Painted>,
    pub shapes: Vec<Painted>,
    /// Text shadows, each painted just before the shape path at its index.
    pub shadows: Vec<(usize, Bounds<Pixels>, Arc<RenderImage>)>,
    pub retired: Vec<Arc<RenderImage>>,
}

fn window_rect(origin: gpui::Point<Pixels>, r: Rect) -> Bounds<Pixels> {
    Bounds::new(
        origin + point(px(r.x0 as f32), px(r.y0 as f32)),
        size(px(r.width() as f32), px(r.height() as f32)),
    )
}

fn place_entry(placed: &mut Vec<Painted>, built: &[Painted], at: Placement) {
    placed.clear();
    placed.extend(built.iter().map(|p| Painted {
        path: at.place(&p.path),
        color: p.color,
    }));
}

impl EditorCanvas {
    pub(super) fn prepare(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Prepared {
        let sf = window.scale_factor();
        let container = Size::new(f64::from(bounds.size.width), f64::from(bounds.size.height));
        self.bounds = bounds;
        let current = self.viewport.read(cx).math();
        if current.container != container || current.scale_factor != f64::from(sf) {
            self.viewport
                .update(cx, |v, cx| v.set_container(container, f64::from(sf), cx));
        }
        let math = *self.viewport.read(cx).math();
        let vp = math.viewport();
        let mut stats = FrameStats::default();
        let released_before = self.stats.images_released;
        let mut retired = std::mem::take(&mut self.retired);
        let t = es_ui::theme::tokens(cx);
        let accent = t.accent;
        let grid_key = GridKey {
            scale_factor: sf,
            workspace: t.workspace,
            dot: t.text_tertiary,
        };
        if self.grid.as_ref().is_none_or(|(k, _)| *k != grid_key) {
            if let Some((_, old)) = self.grid.take() {
                retired.push(old);
            }
            self.grid = grid::render(grid_key).map(|img| (grid_key, img));
        }
        let grid = self
            .grid
            .as_ref()
            .map(|(_, img)| (img.clone(), grid::cells(bounds)));

        let (base, base_revision) = {
            let doc = self.doc.read(cx);
            (doc.base().clone(), doc.base_revision())
        };
        if self
            .tiles
            .as_ref()
            .is_none_or(|t| t.revision() != base_revision)
        {
            let fresh = BaseTiles::new(base_revision, base.width(), base.height());
            if let Some(old) = self.tiles.replace(fresh) {
                retired.extend(old.into_images());
            }
        }

        let image_rect = Rect::new(
            vp.origin.x,
            vp.origin.y,
            vp.origin.x + vp.image_size.width * vp.scale,
            vp.origin.y + vp.image_size.height * vp.scale,
        );
        let page_visible = Rect::new(
            -vp.origin.x / vp.scale,
            -vp.origin.y / vp.scale,
            (container.width - vp.origin.x) / vp.scale,
            (container.height - vp.origin.y) / vp.scale,
        );
        let radii = math.layout.image_radii.map(|r| px((r * vp.scale) as f32));
        let mut tiles_out = Vec::new();
        if let Some(tiles) = self.tiles.as_mut() {
            let (w, h) = (base.width(), base.height());
            for i in tiles::visible(tiles.rects(), page_visible) {
                let t = tiles.rects()[i];
                let Some(image) = tiles.image(i, &base) else {
                    continue;
                };
                let r = t.page_rect();
                let screen = Rect::new(
                    vp.origin.x + r.x0 * vp.scale,
                    vp.origin.y + r.y0 * vp.scale,
                    vp.origin.x + r.x1 * vp.scale,
                    vp.origin.y + r.y1 * vp.scale,
                );
                let (left, top) = (t.x == 0, t.y == 0);
                let (right, bottom) = (t.x + t.w == w, t.y + t.h == h);
                let pick = |on: bool, r: Pixels| if on { r } else { px(0.) };
                let corners = Corners {
                    top_left: pick(left && top, radii[0]),
                    top_right: pick(right && top, radii[1]),
                    bottom_right: pick(right && bottom, radii[2]),
                    bottom_left: pick(left && bottom, radii[3]),
                };
                tiles_out.push((window_rect(bounds.origin, screen), corners, image));
            }
        }
        stats.tiles_painted = tiles_out.len();

        let canvas_edge = math.layout.canvas_width.max(math.layout.canvas_height);
        let key = UnderlayKey::new(
            &self.background,
            (base.width(), base.height()),
            canvas_edge,
            vp.scale * f64::from(sf),
        );
        self.request_underlay(key, &mut retired, cx);
        let underlay = self
            .underlay
            .as_ref()
            .map(|(_, img)| (window_rect(bounds.origin, math.canvas_rect()), img.clone()));

        let bucket = Bucket::for_scale(vp.scale, sf);
        let at = Placement::new(bounds.origin, &vp, bucket);
        self.sync_text_edit(cx);
        let doc = self.doc.read(cx);
        let editor = doc.editor();
        let document = editor.doc();
        let live = editor.live_stroke().map(|(id, _)| id);
        let text = doc.text().clone();
        let build = BuildContext::new(&*text);
        let mut redactions = Vec::new();
        let mut holes = Vec::new();
        let mut shapes_out = Vec::new();
        let mut shadows = Vec::new();
        for shape in document.shapes() {
            let Some(revision) = document.revision(shape.id) else {
                continue;
            };
            if Some(shape.id) == live {
                continue;
            }
            if shape.is_redaction() {
                redactions.extend(draw_items(document, shape, &build));
                continue;
            }
            if shape.is_highlight() {
                holes.push((shape.id, revision));
                continue;
            }
            if shape.is_text()
                && let Some((page, image)) = self
                    .text_shadows
                    .track(document, shape, revision, bucket, &build)
            {
                let screen = text_shadow::on_screen(&vp, page);
                shadows.push((shapes_out.len(), window_rect(bounds.origin, screen), image));
            }
            let entry = self.shapes.entry(shape.id).or_insert_with(|| ShapeEntry {
                revision: 0,
                bucket,
                built: Vec::new(),
                placement: None,
                placed: Vec::new(),
            });
            if entry.revision != revision || entry.bucket != bucket {
                entry.built = paths::tessellate(&draw_items(document, shape, &build), bucket);
                entry.revision = revision;
                entry.bucket = bucket;
                entry.placement = None;
                stats.shapes_tessellated += 1;
            }
            if entry.placement != Some(at) {
                place_entry(&mut entry.placed, &entry.built, at);
                entry.placement = Some(at);
                stats.shapes_placed += 1;
            }
            shapes_out.extend(entry.placed.iter().cloned());
        }
        shapes_out.extend(self.live.paths(editor, bucket, at, &mut stats));
        shapes_out.extend(self.text_edit_overlay(&vp, bounds.origin, bucket, accent, &text, cx));
        if self.shapes.len() > document.shapes().len() {
            self.shapes.retain(|id, _| document.shape(*id).is_some());
        }
        self.text_shadows
            .retain(|id| document.shape(id).is_some(), &mut retired);

        let spotlight = self.spotlight_paths(doc, &holes, bucket, at, &math.layout.image_radii);
        stats.paths_painted = shapes_out.len() + spotlight.len();
        self.stats = stats;
        self.start_text_shadows(cx);
        let effects = self.effects_frame(bounds, &math, f64::from(sf), &mut retired, cx);
        let flat = effects.is_none();
        let watermark = self.watermark_frame(bounds, &math, f64::from(sf), flat, &mut retired, cx);
        if !flat {
            self.stats.tiles_painted = 0;
            self.stats.paths_painted = 0;
        }
        let keep = |v: Vec<Painted>| if flat { v } else { Vec::new() };
        self.stats.images_released = released_before + retired.len();
        Prepared {
            grid,
            underlay: underlay.filter(|_| flat),
            effects,
            watermark,
            tiles: if flat { tiles_out } else { Vec::new() },
            redactions,
            viewport: vp,
            image: window_rect(bounds.origin, image_rect),
            spotlight: keep(spotlight),
            shapes: keep(shapes_out),
            shadows: if flat { shadows } else { Vec::new() },
            retired,
        }
    }

    fn spotlight_paths(
        &mut self,
        doc: &crate::session::doc::DocModel,
        holes: &[(AnnoShapeId, u64)],
        bucket: Bucket,
        at: Placement,
        radii: &[f64; 4],
    ) -> Vec<Painted> {
        if holes.is_empty() {
            self.spotlight = None;
            return Vec::new();
        }
        let stale = self
            .spotlight
            .as_ref()
            .is_none_or(|s| s.holes != holes || s.bucket != bucket);
        if stale {
            let document = doc.editor().doc();
            let others: Vec<AnnoShapeId> = document
                .shapes()
                .iter()
                .filter(|s| !s.is_highlight())
                .map(|s| s.id)
                .collect();
            let text = doc.text().clone();
            let mut build = BuildContext::new(&*text);
            build.skip = &others;
            let list = build_draw_list(document, &build);
            let cut: Vec<_> = list
                .items
                .into_iter()
                .flat_map(|item| match item {
                    DrawItem::Spotlight { holes } => holes,
                    _ => Vec::new(),
                })
                .collect();
            let size = doc.image_size();
            let clip = Rect::new(0.0, 0.0, size.width, size.height);
            let dim = es_ui::convert::hsla(Rgba::new(0.0, 0.0, 0.0, SPOTLIGHT_DIM));
            self.spotlight = Some(SpotEntry {
                holes: holes.to_vec(),
                bucket,
                built: paths::spotlight(clip, *radii, &cut, bucket, dim),
                placement: None,
                placed: Vec::new(),
            });
        }
        let Some(spot) = self.spotlight.as_mut() else {
            return Vec::new();
        };
        if spot.placement != Some(at) {
            place_entry(&mut spot.placed, &spot.built, at);
            spot.placement = Some(at);
        }
        spot.placed.clone()
    }

    fn request_underlay(
        &mut self,
        key: Option<UnderlayKey>,
        retired: &mut Vec<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = key else {
            if let Some((_, old)) = self.underlay.take() {
                retired.push(old);
            }
            self.underlay_pending = None;
            return;
        };
        let current = self.underlay.as_ref().is_some_and(|(k, _)| *k == key);
        let pending = self.underlay_pending.as_ref() == Some(&key);
        if current {
            self.underlay_pending = None;
            return;
        }
        if pending {
            return;
        }
        let job = key.clone();
        let rendering = cx
            .background_executor()
            .spawn(async move { underlay::render(&job).and_then(|p| underlay::render_image(&p)) });
        let task = cx.spawn(async move |this, cx| {
            let image = rendering.await;
            let _ = this.update(cx, |canvas, cx| {
                let Some(wanted) = canvas.underlay_pending.take() else {
                    return;
                };
                if let Some(image) = image
                    && let Some((_, old)) = canvas.underlay.replace((wanted, image))
                {
                    canvas.retired.push(old);
                }
                cx.notify();
            });
        });
        self.underlay_pending = Some(key);
        self.underlay_task = Some(task);
    }
}
