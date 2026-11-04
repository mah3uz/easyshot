use es_geom::Point;
use es_model::platform::OutputId;
use es_model::state::BarSpot;
use std::collections::BTreeMap;

/// Where the user left the control bar on each output, as offsets from its top-left corner.
pub fn load(paths: &es_store::Paths) -> BTreeMap<String, Point> {
    match es_store::state::open(paths) {
        Ok(state) => state
            .value
            .control_bar_spots
            .iter()
            .map(|(k, s)| (k.clone(), Point::new(s.x, s.y)))
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "cannot read the control bar spots");
            BTreeMap::new()
        }
    }
}

pub fn remember(paths: &es_store::Paths, output: &OutputId, offset: Point) {
    let saved = es_store::state::open(paths).and_then(|mut state| {
        state.value.control_bar_spots.insert(
            output.0.clone(),
            BarSpot {
                x: offset.x,
                y: offset.y,
            },
        );
        state.save()
    });
    if let Err(error) = saved {
        tracing::warn!(%error, "cannot save the control bar spot");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_spot_is_there_for_the_next_recording_on_that_output_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = es_store::Paths::rooted(dir.path());
        remember(&paths, &OutputId("DP-1".into()), Point::new(40.0, 900.0));
        let spots = load(&paths);
        assert_eq!(spots.get("DP-1"), Some(&Point::new(40.0, 900.0)));
        assert_eq!(spots.get("HDMI-A-1"), None);
    }
}
