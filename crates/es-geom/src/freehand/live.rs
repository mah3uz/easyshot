use super::ink::{Spec, chunk_ranges, partitions, render_partition};
use super::options::StrokeOptions;
use super::pipeline::{Pipeline, StrokePoint};
use super::tracks::TrackPoint;
use crate::vec::PVec;
use kurbo::BezPath;
use std::sync::Arc;

const MIN_INPUT_SPACING: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub index: u32,
    pub hash: u64,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub key: ChunkKey,
    pub path: Arc<BezPath>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Update {
    pub rebuilt: Vec<usize>,
    pub len: usize,
}

struct Hasher(u64);

impl Hasher {
    fn word(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn f(&mut self, v: f64) {
        self.word(v.to_bits());
    }
}

fn build_chunks(
    pts: &[StrokePoint],
    specs: &[Spec],
    opts: &StrokeOptions,
    prev: &[Chunk],
) -> (Vec<Chunk>, Vec<usize>) {
    let mut out: Vec<Chunk> = Vec::with_capacity(prev.len() + 2);
    let mut rebuilt = Vec::new();
    let mut scratch: Vec<TrackPoint> = Vec::with_capacity(super::ink::CHUNK_POINTS);
    for spec in specs {
        chunk_ranges(spec.len(), |lo, hi| {
            let index = out.len();
            let mut h = Hasher(0);
            h.word((hi - lo + 1) as u64);
            if lo == 0
                && let Some(a) = spec.anchor
            {
                h.word(1);
                h.f(a.x);
                h.f(a.y);
            }
            for k in lo..=hi {
                let t = spec.get(pts, k);
                h.f(t.p.x);
                h.f(t.p.y);
                h.f(t.input.x);
                h.f(t.input.y);
                h.f(t.radius);
                h.f(t.running);
                h.word(u64::from(t.is_cap));
            }
            let key = ChunkKey {
                index: index as u32,
                hash: h.0,
            };
            if let Some(p) = prev.get(index)
                && p.key == key
            {
                out.push(p.clone());
                return;
            }
            scratch.clear();
            scratch.extend((lo..=hi).map(|k| spec.get(pts, k)));
            if let Some(f) = scratch.first_mut() {
                f.is_cap = true;
            }
            if let Some(l) = scratch.last_mut() {
                l.is_cap = true;
            }
            let anchor = if lo == 0 { spec.anchor } else { None };
            let path = Arc::new(render_partition(&scratch, opts, anchor));
            rebuilt.push(index);
            out.push(Chunk { key, path });
        });
    }
    (out, rebuilt)
}

pub fn stroke_points(raw: &[PVec], opts: &StrokeOptions) -> Vec<StrokePoint> {
    let mut p = Pipeline::default();
    p.run(raw, opts);
    p.points
}

pub fn outline(raw: &[PVec], opts: &StrokeOptions) -> Vec<PVec> {
    super::outline::outline(&stroke_points(raw, opts), opts)
}

pub fn ink_chunks(raw: &[PVec], opts: &StrokeOptions) -> Vec<Chunk> {
    let pts = stroke_points(raw, opts);
    let mut specs = Vec::new();
    partitions(&pts, &mut specs);
    build_chunks(&pts, &specs, opts, &[]).0
}

pub fn ink_path(raw: &[PVec], opts: &StrokeOptions) -> BezPath {
    let mut path = BezPath::new();
    for c in ink_chunks(raw, opts) {
        path.extend(c.path.elements().iter().copied());
    }
    path
}

#[derive(Debug, Clone)]
pub struct LiveStroke {
    opts: StrokeOptions,
    raw: Vec<PVec>,
    pipe: Pipeline,
    specs: Vec<Spec>,
    chunks: Vec<Chunk>,
}

impl LiveStroke {
    pub fn new(opts: StrokeOptions) -> Self {
        Self {
            opts: opts.with_last(false),
            raw: Vec::new(),
            pipe: Pipeline::default(),
            specs: Vec::new(),
            chunks: Vec::new(),
        }
    }

    pub fn raw(&self) -> &[PVec] {
        &self.raw
    }

    pub fn points(&self) -> &[StrokePoint] {
        &self.pipe.points
    }

    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    pub fn push(&mut self, p: PVec) -> Option<Update> {
        if let Some(last) = self.raw.last()
            && PVec::dist_min(*last, p, MIN_INPUT_SPACING)
        {
            return None;
        }
        self.raw.push(p);
        Some(self.rebuild())
    }

    pub fn finish(&mut self) -> Update {
        self.opts.last = true;
        self.rebuild()
    }

    fn rebuild(&mut self) -> Update {
        self.pipe.run(&self.raw, &self.opts);
        partitions(&self.pipe.points, &mut self.specs);
        let (chunks, rebuilt) =
            build_chunks(&self.pipe.points, &self.specs, &self.opts, &self.chunks);
        self.chunks = chunks;
        Update {
            rebuilt,
            len: self.chunks.len(),
        }
    }
}
