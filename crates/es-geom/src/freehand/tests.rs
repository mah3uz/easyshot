use super::*;
use crate::vec::PVec;
use kurbo::Shape;
use std::sync::Arc;

fn dot() -> Vec<PVec> {
    vec![PVec::with_z(10.0, 10.0, 0.5)]
}

fn line() -> Vec<PVec> {
    (0..20)
        .map(|i| PVec::with_z(f64::from(i) * 10.0, 0.0, 0.5))
        .collect()
}

fn curve() -> Vec<PVec> {
    (0..60)
        .map(|i| {
            let t = f64::from(i) / 59.0 * std::f64::consts::FRAC_PI_2;
            PVec::with_z(100.0 * t.sin(), 100.0 * (1.0 - t.cos()), 0.5)
        })
        .collect()
}

fn ramp() -> Vec<PVec> {
    (0..40)
        .map(|i| PVec::with_z(f64::from(i) * 6.0, 0.0, 0.1 + 0.9 * f64::from(i) / 39.0))
        .collect()
}

fn scribble(n: usize) -> Vec<PVec> {
    (0..n)
        .map(|i| {
            let t = i as f64;
            PVec::with_z(
                300.0 + 250.0 * (t * 0.031).sin() + 30.0 * (t * 0.9).sin(),
                200.0 + 150.0 * (t * 0.047 + 1.0).sin() + 30.0 * (t * 1.3).cos(),
                0.5,
            )
        })
        .collect()
}

#[derive(Debug)]
struct Summary {
    elements: usize,
    area: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    radii: f64,
}

fn summarize(raw: &[PVec], opts: &StrokeOptions) -> Summary {
    let path = ink_path(raw, opts);
    let bb = path.bounding_box();
    Summary {
        elements: path.elements().len(),
        area: path.area(),
        x0: bb.x0,
        y0: bb.y0,
        x1: bb.x1,
        y1: bb.y1,
        radii: stroke_points(raw, opts).iter().map(|p| p.radius).sum(),
    }
}

fn sim() -> StrokeOptions {
    StrokeOptions::simulated_pressure(16.0).with_last(true)
}

fn pen() -> StrokeOptions {
    StrokeOptions::real_pressure(8.0).with_last(true)
}

fn assert_golden(raw: &[PVec], o: &StrokeOptions, g: &Summary) {
    let s = summarize(raw, o);
    assert_eq!(s.elements, g.elements, "path element count");
    for (name, got, want) in [
        ("area", s.area, g.area),
        ("x0", s.x0, g.x0),
        ("y0", s.y0, g.y0),
        ("x1", s.x1, g.x1),
        ("y1", s.y1, g.y1),
        ("radii", s.radii, g.radii),
    ] {
        assert!((got - want).abs() < 1e-6, "{name}: {got} vs {want}");
    }
}

#[allow(clippy::too_many_arguments)]
fn g(elements: usize, area: f64, x0: f64, y0: f64, x1: f64, y1: f64, radii: f64) -> Summary {
    Summary {
        elements,
        area,
        x0,
        y0,
        x1,
        y1,
        radii,
    }
}

#[test]
fn golden_dot_is_a_disc_of_the_simulated_radius() {
    assert_golden(
        &dot(),
        &sim(),
        &g(
            6,
            402.23645858003283,
            -1.3137084989847594,
            -1.3137084989847594,
            21.31370849898476,
            21.31370849898476,
            11.31370849898476,
        ),
    );
}

#[test]
fn golden_straight_line() {
    assert_golden(
        &line(),
        &sim(),
        &g(
            14,
            4183.19455576911,
            -10.71429919428585,
            -10.71429919428585,
            199.11429753949355,
            10.71429919428585,
            193.0836013078695,
        ),
    );
}

#[test]
fn golden_curve() {
    assert_golden(
        &curve(),
        &sim(),
        &g(
            22,
            4680.3748349034695,
            -12.445211883241525,
            -12.445211988170168,
            113.4136671725908,
            113.41366675854049,
            718.856876712043,
        ),
    );
}

#[test]
fn golden_pressure_ramp() {
    assert_golden(
        &ramp(),
        &pen(),
        &g(
            14,
            2950.83982040492,
            -4.065023699955112,
            -8.558948756381788,
            242.5589487563818,
            8.558948756381788,
            232.01636516216254,
        ),
    );
}

#[test]
fn golden_long_scribble() {
    assert_golden(
        &scribble(1500),
        &sim(),
        &g(
            3980,
            351823.06687078124,
            13.767650861463288,
            13.169626697192307,
            582.73877882693,
            385.71963278479257,
            11624.44195377295,
        ),
    );
}

#[test]
fn pressure_ramp_thickens_the_stroke() {
    let pts = stroke_points(&ramp(), &pen());
    let first = pts.first().map(|p| p.radius).unwrap_or(0.0);
    let last = pts.last().map(|p| p.radius).unwrap_or(0.0);
    assert!(last > first * 1.5, "{first} -> {last}");
}

fn same_paths(a: &[Chunk], b: &[Chunk]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.key == y.key && x.path.elements() == y.path.elements())
}

#[test]
fn incremental_chunks_equal_the_full_rebuild_at_every_step() {
    let raw = scribble(900);
    let live_opts = StrokeOptions::simulated_pressure(16.0);
    let mut live = LiveStroke::new(live_opts);
    for (i, p) in raw.iter().enumerate() {
        live.push(*p);
        if i % 37 == 0 || i == raw.len() - 1 {
            let full = ink_chunks(live.raw(), &live_opts);
            assert!(same_paths(live.chunks(), &full), "step {i}");
        }
    }
    live.finish();
    let full = ink_chunks(live.raw(), &live_opts.with_last(true));
    assert!(same_paths(live.chunks(), &full));
}

#[test]
fn an_append_rebuilds_only_the_tail_chunks() {
    let mut live = LiveStroke::new(StrokeOptions::simulated_pressure(16.0));
    let mut worst = 0;
    for (i, p) in scribble(1500).iter().enumerate() {
        let Some(u) = live.push(*p) else { continue };
        if i > 400 {
            worst = worst.max(u.rebuilt.len());
            let tail = u.len.saturating_sub(3);
            assert!(u.rebuilt.iter().all(|&c| c >= tail), "step {i}: {u:?}");
        }
    }
    assert!(live.chunks().len() > 10);
    assert!(worst <= 3);
}

#[test]
fn unchanged_chunks_keep_their_path_allocation() {
    let raw = scribble(800);
    let mut live = LiveStroke::new(StrokeOptions::simulated_pressure(16.0));
    for p in &raw[..799] {
        live.push(*p);
    }
    let before: Vec<Chunk> = live.chunks().to_vec();
    live.push(raw[799]);
    let reused = before
        .iter()
        .zip(live.chunks())
        .filter(|(a, b)| Arc::ptr_eq(&a.path, &b.path))
        .count();
    assert!(reused + 3 >= before.len(), "{reused} of {}", before.len());
}

fn covered_by(chunks: &[Chunk], p: kurbo::Point) -> bool {
    chunks.iter().any(|c| c.path.winding(p) != 0)
}

#[test]
fn chunk_overlap_leaves_no_gap() {
    let raw: Vec<PVec> = (0..3000)
        .map(|i| {
            let t = f64::from(i) * 0.004;
            PVec::with_z(
                (300.0 + t * 40.0) * t.cos(),
                (300.0 + t * 40.0) * t.sin(),
                0.5,
            )
        })
        .collect();
    let o = sim();
    let pts = stroke_points(&raw, &o);
    let chunks = ink_chunks(&raw, &o);
    assert!(chunks.len() > 5, "{} chunks", chunks.len());
    for w in pts.windows(2) {
        let (a, b) = (w[0].point, w[1].point);
        assert!(covered_by(&chunks, kurbo::Point::new(a.x, a.y)), "{a:?}");
        let mid = kurbo::Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        assert!(covered_by(&chunks, mid), "{mid:?}");
    }
}

#[test]
fn last_changes_only_the_tail() {
    let raw = scribble(1200);
    let live_opts = StrokeOptions::simulated_pressure(16.0);
    let mut live = LiveStroke::new(live_opts);
    for p in &raw {
        live.push(*p);
    }
    let before: Vec<ChunkKey> = live.chunks().iter().map(|c| c.key).collect();
    let u = live.finish();
    assert!(before.len() > 8);
    let head = before.len().min(u.len).saturating_sub(4);
    for (i, k) in before.iter().enumerate().take(head) {
        assert_eq!(*k, live.chunks()[i].key, "chunk {i}");
    }
    assert!(!u.rebuilt.is_empty());
}

#[test]
fn a_partition_is_chunked_at_128_points_with_one_point_shared() {
    let raw: Vec<PVec> = (0..400)
        .map(|i| PVec::with_z(f64::from(i) * 4.0, 0.0, 0.5))
        .collect();
    let o = StrokeOptions::solid(8.0).with_last(true);
    let n = stroke_points(&raw, &o).len();
    let chunks = ink_chunks(&raw, &o);
    assert_eq!(chunks.len(), (n - 1).div_ceil(127), "n = {n}");
}

#[test]
fn nothing_is_drawn_for_no_input() {
    assert!(ink_chunks(&[], &sim()).is_empty());
    assert!(outline(&[], &sim()).is_empty());
}

#[test]
fn outline_polygon_encloses_the_stroke_centerline() {
    let raw = curve();
    let poly = outline(&raw, &sim());
    assert!(poly.len() > 20);
    let mut path = kurbo::BezPath::new();
    for (i, p) in poly.iter().enumerate() {
        if i == 0 {
            path.move_to((p.x, p.y));
        } else {
            path.line_to((p.x, p.y));
        }
    }
    path.close_path();
    for p in stroke_points(&raw, &sim()) {
        assert_ne!(path.winding(kurbo::Point::new(p.point.x, p.point.y)), 0);
    }
}

#[cfg(not(debug_assertions))]
#[test]
fn append_at_5000_points_stays_under_half_a_millisecond_at_p95() {
    use std::time::Instant;
    let raw = scribble(5200);
    let mut live = LiveStroke::new(StrokeOptions::simulated_pressure(16.0));
    let mut times = Vec::new();
    for p in &raw {
        let t = Instant::now();
        let pushed = live.push(*p);
        let dt = t.elapsed();
        if pushed.is_some() && live.raw().len() > 5000 {
            times.push(dt.as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(f64::total_cmp);
    let p50 = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    eprintln!(
        "append at {} raw points: p50 {p50:.3} ms, p95 {p95:.3} ms",
        live.raw().len()
    );
    assert!(p95 < 0.5, "p95 {p95} ms");
}
