use es_desktop::app_name;
use es_geom::{Point, Rect};
use es_model::platform::{OutputId, WindowId, WindowInfo};

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: WindowId,
    pub app_id: String,
    /// The app's display name, from its desktop entry when it has one.
    pub app: String,
    pub title: String,
    /// Logical, relative to the output's top-left; None when the compositor gives no geometry.
    pub local: Option<Rect>,
}

fn rank(w: &WindowInfo) -> (bool, u32) {
    (!w.floating, w.focus_rank.unwrap_or(u32::MAX))
}

fn ordered(snapshot: &[WindowInfo], own_pid: u32) -> Vec<&WindowInfo> {
    let mut out: Vec<&WindowInfo> = snapshot.iter().filter(|w| w.pid != Some(own_pid)).collect();
    out.sort_by_key(|w| rank(w));
    out
}

/// Front to back: floating windows first, then the most recently focused.
pub fn on_output(
    snapshot: &[WindowInfo],
    output: &OutputId,
    output_logical: Rect,
    own_pid: u32,
) -> Vec<Candidate> {
    ordered(snapshot, own_pid)
        .into_iter()
        .filter_map(|w| {
            let global = w.logical?;
            let here = match &w.output {
                Some(o) => o == output,
                None => global.intersect(output_logical).area() > 0.0,
            };
            let local = (global - output_logical.origin().to_vec2())
                .intersect(Rect::from_origin_size((0.0, 0.0), output_logical.size()));
            (here && local.area() > 0.0).then(|| Candidate {
                id: w.id.clone(),
                app_id: w.app_id.clone(),
                app: app_name(&w.app_id),
                title: w.title.clone(),
                local: Some(local),
            })
        })
        .collect()
}

pub fn listed(snapshot: &[WindowInfo], own_pid: u32) -> Vec<Candidate> {
    ordered(snapshot, own_pid)
        .into_iter()
        .map(|w| Candidate {
            id: w.id.clone(),
            app_id: w.app_id.clone(),
            app: app_name(&w.app_id),
            title: w.title.clone(),
            local: None,
        })
        .collect()
}

pub fn hit(candidates: &[Candidate], p: Point) -> Option<usize> {
    candidates
        .iter()
        .position(|c| c.local.is_some_and(|r| r.contains(p)))
}
