use super::Start;
use es_app::errors::{Error, Result};
use es_geom::Point;
use es_model::platform::{OutputId, OutputInfo};
use es_overlays::record::{Recorded, RecordingTarget};
use es_record::RecordSource;

pub fn resolve(start: &Start, outputs: &[OutputInfo]) -> Result<(Recorded, RecordSource)> {
    let origin = |id: &OutputId| {
        outputs
            .iter()
            .find(|o| o.id == *id)
            .map(|o| Point::new(o.logical.x0, o.logical.y0))
    };
    match &start.request.target {
        RecordingTarget::Area => {
            let (output, area) = start
                .area
                .clone()
                .ok_or_else(|| Error::NotFound("an area to record".into()))?;
            let at = origin(&output).ok_or_else(|| Error::NotFound(output.0.clone()))?;
            let logical = area - at.to_vec2();
            Ok((
                Recorded::Area(area),
                RecordSource::Region { output, logical },
            ))
        }
        RecordingTarget::Display(id) => Ok((
            Recorded::Display(id.clone()),
            RecordSource::Output(id.clone()),
        )),
        RecordingTarget::Window(w) => {
            let area = w.logical.ok_or(Error::Unsupported)?;
            Ok((Recorded::Area(area), RecordSource::Window(w.clone())))
        }
    }
}

pub fn output_name(source: &RecordSource) -> String {
    match source {
        RecordSource::Output(id) | RecordSource::Region { output: id, .. } => id.0.clone(),
        RecordSource::Window(w) => w.output.as_ref().map(|o| o.0.clone()).unwrap_or_default(),
    }
}
