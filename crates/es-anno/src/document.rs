use crate::ShapeId;
use es_geom::arrow::{self, ArrowBody, ArrowInfo, ArrowInput, Bound};
use es_geom::freehand::{self, StrokeOptions};
use es_geom::geometry2d::{Arc2d, Circle2d, Edge2d, Ellipse2d, Geometry2d, Polyline2d};
use es_geom::path::{GeoShape, geo_path};
use es_geom::{BBox, BezPath, Mat, PVec, Size};
use es_model::{
    AnnoShape, AnnoShapeId, AnnotationDocument, ArrowBinding, ArrowTerminal, Arrowhead, DrawProps,
    FillStyle, GeoKind, ShapeKind, TextProps,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const CURVE_RESOLUTION: usize = 12;

pub trait TextMeasure {
    /// The laid-out box of the text, in page units.
    fn measure(&self, props: &TextProps) -> Size;
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub shapes: Vec<AnnoShape>,
    pub bindings: Vec<ArrowBinding>,
}

#[derive(Default)]
struct Cached {
    revision: u64,
    geometry: Option<Arc<dyn Geometry2d>>,
    arrow: Option<Arc<ArrowInfo>>,
    ink: Option<Arc<BezPath>>,
}

fn geometry_slot(c: &mut Cached) -> &mut Option<Arc<dyn Geometry2d>> {
    &mut c.geometry
}

fn arrow_slot(c: &mut Cached) -> &mut Option<Arc<ArrowInfo>> {
    &mut c.arrow
}

fn ink_slot(c: &mut Cached) -> &mut Option<Arc<BezPath>> {
    &mut c.ink
}

struct Target {
    handle: u64,
    geometry: Arc<dyn Geometry2d>,
    transform: Mat,
    page_bounds: BBox,
    stroke_width: f64,
    anchor: PVec,
    precise: bool,
    exact: bool,
}

impl Target {
    fn bound(&self) -> Bound<'_> {
        Bound {
            id: self.handle,
            geometry: &*self.geometry,
            transform: self.transform,
            page_bounds: self.page_bounds,
            stroke_width: self.stroke_width,
            normalized_anchor: self.anchor,
            precise: self.precise,
            exact: self.exact,
        }
    }
}

pub fn page_transform(shape: &AnnoShape) -> Mat {
    let flip = if shape.mirrored { -1.0 } else { 1.0 };
    Mat::multiply(
        Mat::compose(shape.x, shape.y, shape.rotation),
        Mat::scale(flip, 1.0),
    )
}

/// Mutators return the changed handles, arrows that follow included; empty means no change.
pub struct Document {
    model: AnnotationDocument,
    index: HashMap<AnnoShapeId, usize>,
    handles: HashMap<AnnoShapeId, ShapeId>,
    ids: HashMap<ShapeId, AnnoShapeId>,
    revisions: HashMap<AnnoShapeId, u64>,
    clock: u64,
    cache: RefCell<HashMap<AnnoShapeId, Cached>>,
    measure: Box<dyn TextMeasure>,
}

impl Document {
    pub fn new(mut model: AnnotationDocument, measure: Box<dyn TextMeasure>) -> Self {
        let mut seen = HashSet::new();
        for shape in &mut model.shapes {
            if !seen.insert(shape.id) {
                shape.id = AnnoShapeId::new();
            }
        }
        let mut doc = Self {
            model,
            index: HashMap::new(),
            handles: HashMap::new(),
            ids: HashMap::new(),
            revisions: HashMap::new(),
            clock: 0,
            cache: RefCell::new(HashMap::new()),
            measure,
        };
        doc.reindex();
        let all: Vec<_> = doc.model.shapes.iter().map(|s| s.id).collect();
        doc.after_change(&all);
        doc
    }

    pub fn model(&self) -> &AnnotationDocument {
        &self.model
    }

    pub fn shapes(&self) -> &[AnnoShape] {
        &self.model.shapes
    }

    pub fn bindings(&self) -> &[ArrowBinding] {
        &self.model.bindings
    }

    pub fn shape(&self, id: AnnoShapeId) -> Option<&AnnoShape> {
        self.index.get(&id).map(|&i| &self.model.shapes[i])
    }

    pub fn index_of(&self, id: AnnoShapeId) -> Option<usize> {
        self.index.get(&id).copied()
    }

    /// Stable for the editor's lifetime, also across undo, redo and deletion.
    pub fn handle(&self, id: AnnoShapeId) -> Option<ShapeId> {
        self.handles.get(&id).copied()
    }

    pub fn id_of(&self, handle: ShapeId) -> Option<AnnoShapeId> {
        self.ids.get(&handle).copied()
    }

    /// Changes whenever the shape's cached geometry would; never reused for another state.
    pub fn revision(&self, id: AnnoShapeId) -> Option<u64> {
        self.revisions.get(&id).copied()
    }

    pub fn measure_text(&self, props: &TextProps) -> Size {
        self.measure.measure(props)
    }

    /// Local space; map through `page_transform` for page space.
    pub fn geometry(&self, id: AnnoShapeId) -> Option<Arc<dyn Geometry2d>> {
        self.lookup(id, geometry_slot, |doc, shape| {
            Some(doc.build_geometry(shape))
        })
    }

    pub fn arrow_info(&self, id: AnnoShapeId) -> Option<Arc<ArrowInfo>> {
        self.lookup(id, arrow_slot, |doc, shape| {
            doc.resolve_arrow(shape).map(Arc::new)
        })
    }

    /// The filled ink outline of a freehand stroke, in local space.
    pub fn ink_path(&self, id: AnnoShapeId) -> Option<Arc<BezPath>> {
        self.lookup(id, ink_slot, |_, shape| match &shape.kind {
            ShapeKind::Draw(p) => {
                let opts =
                    StrokeOptions::for_draw_shape(p.is_pen, p.stroke_width, p.is_complete, false);
                Some(Arc::new(freehand::ink_path(&p.points, &opts)))
            }
            _ => None,
        })
    }

    pub fn page_bounds(&self, id: AnnoShapeId) -> Option<BBox> {
        let shape = self.shape(id)?;
        let geometry = self.geometry(id)?;
        let points = page_transform(shape).apply_to_points(geometry.vertices());
        Some(BBox::from_points(&points))
    }

    /// Topmost first; filled shapes hit anywhere inside, the rest only near their outline.
    pub fn hit_test(&self, page: PVec, margin: f64) -> Option<AnnoShapeId> {
        self.model.shapes.iter().rev().find_map(|shape| {
            let geometry = self.geometry(shape.id)?;
            let local = page_transform(shape).inverse().apply_to_point(page);
            geometry
                .hit_test_point(local, margin, shape.is_filled())
                .then_some(shape.id)
        })
    }

    /// Shapes that overlap the box, not only those inside it, back to front.
    pub fn overlapping(&self, page: BBox) -> Vec<AnnoShapeId> {
        let polygon = page.corners();
        self.model
            .shapes
            .iter()
            .filter(|shape| {
                let local = page_transform(shape).inverse().apply_to_points(&polygon);
                self.geometry(shape.id)
                    .is_some_and(|g| g.overlaps_polygon(&local))
            })
            .map(|s| s.id)
            .collect()
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            shapes: self.model.shapes.clone(),
            bindings: self.model.bindings.clone(),
        }
    }

    pub fn matches(&self, snapshot: &Snapshot) -> bool {
        self.model.shapes == snapshot.shapes && self.model.bindings == snapshot.bindings
    }

    /// Refuses a shape whose id is already in the document.
    pub fn add(&mut self, shape: AnnoShape) -> Vec<ShapeId> {
        if self.index.contains_key(&shape.id) {
            return Vec::new();
        }
        let id = shape.id;
        self.model.add_shape(shape);
        self.reindex();
        self.after_change(&[id])
    }

    /// The closure cannot change the shape's id.
    pub fn update(&mut self, id: AnnoShapeId, f: impl FnOnce(&mut AnnoShape)) -> Vec<ShapeId> {
        let Some(&i) = self.index.get(&id) else {
            return Vec::new();
        };
        let shape = &mut self.model.shapes[i];
        let before = shape.clone();
        f(shape);
        shape.id = id;
        if *shape == before {
            return Vec::new();
        }
        self.after_change(&[id])
    }

    pub fn delete(&mut self, ids: &[AnnoShapeId]) -> Vec<ShapeId> {
        let mut stale: Vec<AnnoShapeId> = ids
            .iter()
            .copied()
            .filter(|id| self.index.contains_key(id))
            .collect();
        if stale.is_empty() {
            return Vec::new();
        }
        for b in &self.model.bindings {
            if stale.contains(&b.to_id) && !stale.contains(&b.arrow_id) {
                stale.push(b.arrow_id);
            }
        }
        for id in ids {
            self.model.remove_shape(*id);
        }
        self.reindex();
        self.after_change(&stale)
    }

    pub fn set_binding(&mut self, binding: ArrowBinding) -> Vec<ShapeId> {
        let arrow = binding.arrow_id;
        if self.model.binding(arrow, binding.terminal) == Some(&binding) {
            return Vec::new();
        }
        self.model.set_binding(binding);
        self.after_change(&[arrow])
    }

    pub fn remove_binding(&mut self, arrow: AnnoShapeId, terminal: ArrowTerminal) -> Vec<ShapeId> {
        if self.model.binding(arrow, terminal).is_none() {
            return Vec::new();
        }
        self.model.remove_binding(arrow, terminal);
        self.after_change(&[arrow])
    }

    /// Accepts only a permutation of the current shapes; returns whether the order changed.
    pub fn reorder(&mut self, order: &[AnnoShapeId]) -> bool {
        let unique: HashSet<_> = order.iter().collect();
        if order.len() != self.model.shapes.len()
            || unique.len() != order.len()
            || order.iter().any(|id| !self.index.contains_key(id))
        {
            return false;
        }
        if order
            .iter()
            .zip(&self.model.shapes)
            .all(|(id, s)| *id == s.id)
        {
            return false;
        }
        let mut shapes = std::mem::take(&mut self.model.shapes);
        shapes.sort_by_key(|s| order.iter().position(|id| *id == s.id));
        self.model.shapes = shapes;
        self.reindex();
        true
    }

    pub fn restore(&mut self, snapshot: Snapshot) {
        self.model.shapes = snapshot.shapes;
        self.model.bindings = snapshot.bindings;
        self.reindex();
        self.cache.get_mut().clear();
        self.revisions.retain(|id, _| self.index.contains_key(id));
        let all: Vec<_> = self.model.shapes.iter().map(|s| s.id).collect();
        for id in all {
            self.touch(id);
        }
    }

    fn reindex(&mut self) {
        self.index.clear();
        for (i, shape) in self.model.shapes.iter().enumerate() {
            self.index.insert(shape.id, i);
            if !self.handles.contains_key(&shape.id) {
                let handle = ShapeId(self.handles.len() as u64 + 1);
                self.handles.insert(shape.id, handle);
                self.ids.insert(handle, shape.id);
            }
        }
    }

    fn touch(&mut self, id: AnnoShapeId) {
        self.clock += 1;
        self.revisions.insert(id, self.clock);
    }

    fn after_change(&mut self, ids: &[AnnoShapeId]) -> Vec<ShapeId> {
        let mut affected = ids.to_vec();
        for b in &self.model.bindings {
            if ids.contains(&b.to_id) && !affected.contains(&b.arrow_id) {
                affected.push(b.arrow_id);
            }
        }
        for &id in &affected {
            if self.index.contains_key(&id) {
                self.touch(id);
            } else {
                self.revisions.remove(&id);
                self.cache.get_mut().remove(&id);
            }
        }
        for &id in &affected {
            self.sync_arrow(id);
        }
        affected
            .iter()
            .filter_map(|id| self.handles.get(id).copied())
            .collect()
    }

    /// Bound terminals are written back so a document read without its bindings draws the same.
    fn sync_arrow(&mut self, id: AnnoShapeId) {
        let start = self.model.binding(id, ArrowTerminal::Start).is_some();
        let end = self.model.binding(id, ArrowTerminal::End).is_some();
        if !start && !end {
            return;
        }
        let Some(info) = self.arrow_info(id) else {
            return;
        };
        let (s, e) = info.synced_terminals();
        let Some(&i) = self.index.get(&id) else {
            return;
        };
        if let ShapeKind::Arrow(p) = &mut self.model.shapes[i].kind {
            if start {
                p.start = s;
            }
            if end {
                p.end = e;
            }
        }
    }

    fn lookup<T: Clone>(
        &self,
        id: AnnoShapeId,
        slot: fn(&mut Cached) -> &mut Option<T>,
        build: impl FnOnce(&Self, &AnnoShape) -> Option<T>,
    ) -> Option<T> {
        let revision = *self.revisions.get(&id)?;
        if let Some(entry) = self.cache.borrow_mut().get_mut(&id)
            && entry.revision == revision
            && let Some(value) = slot(entry)
        {
            return Some(value.clone());
        }
        let value = build(self, self.shape(id)?)?;
        let mut cache = self.cache.borrow_mut();
        let entry = cache.entry(id).or_default();
        if entry.revision != revision {
            *entry = Cached {
                revision,
                ..Cached::default()
            };
        }
        *slot(entry) = Some(value.clone());
        Some(value)
    }

    fn build_geometry(&self, shape: &AnnoShape) -> Arc<dyn Geometry2d> {
        match &shape.kind {
            ShapeKind::Geo(p) => {
                let filled = p.fill != FillStyle::None;
                match p.geo {
                    GeoKind::Rectangle => {
                        if p.corner_radius > 0.5 {
                            let path = geo_path(GeoShape::Rectangle, p.w, p.h, p.corner_radius);
                            let vertices = path.vertices(CURVE_RESOLUTION);
                            if vertices.len() >= 3 {
                                return Arc::new(Polyline2d::polygon(vertices, filled));
                            }
                        }
                        Arc::new(Polyline2d::rectangle(0.0, 0.0, p.w, p.h, filled))
                    }
                    GeoKind::Ellipse => Arc::new(Ellipse2d::new(p.w, p.h, filled)),
                }
            }
            ShapeKind::Draw(p) => draw_geometry(p),
            ShapeKind::Arrow(_) => match self.arrow_info(shape.id) {
                Some(info) => arrow_geometry(&info),
                None => Arc::new(Polyline2d::polyline(vec![
                    PVec::new(0.0, 0.0),
                    PVec::new(1.0, 1.0),
                ])),
            },
            ShapeKind::Text(p) => {
                let size = self.measure.measure(p);
                filled_rect(size.width, size.height)
            }
            ShapeKind::Redaction(p) => filled_rect(p.w, p.h),
            ShapeKind::Highlight(p) => filled_rect(p.w, p.h),
            ShapeKind::Numbered(p) => Arc::new(Ellipse2d::new(p.diameter, p.diameter, true)),
        }
    }

    fn resolve_arrow(&self, shape: &AnnoShape) -> Option<ArrowInfo> {
        let ShapeKind::Arrow(p) = &shape.kind else {
            return None;
        };
        let start = self.target(shape.id, ArrowTerminal::Start);
        let end = self.target(shape.id, ArrowTerminal::End);
        let input = ArrowInput {
            transform: page_transform(shape),
            start: p.start,
            end: p.end,
            bend: p.bend,
            head_start: head(p.arrowhead_start),
            head_end: head(p.arrowhead_end),
            stroke_width: p.stroke_width,
            start_bound: start.as_ref().map(Target::bound),
            end_bound: end.as_ref().map(Target::bound),
        };
        Some(arrow::resolve(&input))
    }

    /// Bad targets resolve as unbound, which also keeps arrow resolution from recursing.
    fn target(&self, arrow: AnnoShapeId, terminal: ArrowTerminal) -> Option<Target> {
        let binding = self.model.binding(arrow, terminal)?;
        let shape = self.shape(binding.to_id)?;
        if shape.id == arrow || shape.is_arrow() {
            return None;
        }
        let geometry = self.geometry(shape.id)?;
        let transform = page_transform(shape);
        let page_bounds = BBox::from_points(&transform.apply_to_points(geometry.vertices()));
        Some(Target {
            handle: self.handle(shape.id)?.0,
            geometry,
            transform,
            page_bounds,
            stroke_width: shape.stroke_width(),
            anchor: PVec::new(binding.normalized_anchor.x, binding.normalized_anchor.y),
            precise: binding.is_precise,
            exact: binding.is_exact,
        })
    }
}

fn filled_rect(w: f64, h: f64) -> Arc<dyn Geometry2d> {
    Arc::new(Polyline2d::rectangle(0.0, 0.0, w, h, true))
}

/// The streamlined centreline, or a filled dot when the stroke is too small to be a line.
fn draw_geometry(p: &DrawProps) -> Arc<dyn Geometry2d> {
    let sw = p.stroke_width + 1.0;
    let dot = || -> Arc<dyn Geometry2d> { Arc::new(Circle2d::new(-sw, -sw, sw, true)) };
    if p.points.is_empty() {
        return dot();
    }
    let bounds = BBox::from_points(&p.points);
    if bounds.w < sw * 2.0 && bounds.h < sw * 2.0 {
        return dot();
    }
    let opts = StrokeOptions::for_draw_shape(p.is_pen, sw, true, true);
    let points: Vec<PVec> = freehand::stroke_points(&p.points, &opts)
        .into_iter()
        .map(|s| s.point)
        .collect();
    if p.is_closed && points.len() > 2 {
        return Arc::new(Polyline2d::polygon(points, false));
    }
    if points.len() < 2 {
        return dot();
    }
    Arc::new(Polyline2d::polyline(points))
}

fn arrow_geometry(info: &ArrowInfo) -> Arc<dyn Geometry2d> {
    match info.body {
        ArrowBody::Straight { start, end } => {
            if PVec::equals(start, end) {
                return Arc::new(Polyline2d::polyline(vec![
                    start,
                    PVec::add_xy(end, 1.0, 1.0),
                ]));
            }
            Arc::new(Edge2d::new(start, end))
        }
        ArrowBody::Arc(arc) => {
            let (start, end) = (info.start.point, info.end.point);
            if !arc.radius.is_finite() || arc.radius <= 0.0 || PVec::equals(start, end) {
                return Arc::new(Edge2d::new(start, end));
            }
            Arc::new(Arc2d::new(arc.center, start, end, arc.sweep, arc.large_arc))
        }
    }
}

fn head(h: Arrowhead) -> arrow::Arrowhead {
    match h {
        Arrowhead::None => arrow::Arrowhead::None,
        Arrowhead::Arrow => arrow::Arrowhead::Arrow,
        Arrowhead::Triangle => arrow::Arrowhead::Triangle,
        Arrowhead::Square => arrow::Arrowhead::Square,
        Arrowhead::Dot => arrow::Arrowhead::Dot,
        Arrowhead::Diamond => arrow::Arrowhead::Diamond,
        Arrowhead::Inverted => arrow::Arrowhead::Inverted,
        Arrowhead::Bar => arrow::Arrowhead::Bar,
    }
}
