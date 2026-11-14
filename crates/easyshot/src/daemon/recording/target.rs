use super::Start;
use es_app::errors::{Error, Result};
use es_geom::Rect;
use es_model::platform::OutputInfo;
use es_overlays::record::{Recorded, RecordingTarget};
use es_record::RecordSource;

/// Only the window recorder, which records the window alone, takes a window no screen shows; it
/// has no area on screen for the bar to keep clear of or for clicks to land in.
pub fn resolve(
    start: &Start,
    outputs: &[OutputInfo],
    window_recorder: bool,
) -> Result<(Recorded, RecordSource)> {
    match &start.request.target {
        RecordingTarget::Area => {
            let (output, area) = start
                .area
                .clone()
                .ok_or_else(|| Error::NotFound("an area to record".into()))?;
            if !outputs.iter().any(|o| o.id == output) {
                return Err(Error::NotFound(output.0.clone()));
            }
            Ok((
                Recorded::Area(area),
                RecordSource::Region {
                    output,
                    logical: area,
                },
            ))
        }
        RecordingTarget::Display(id) => Ok((
            Recorded::Display(id.clone()),
            RecordSource::Output(id.clone()),
        )),
        RecordingTarget::Window(w) => {
            let area = match w.logical {
                Some(area) => area,
                None if window_recorder => Rect::ZERO,
                None => return Err(Error::Unsupported),
            };
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
