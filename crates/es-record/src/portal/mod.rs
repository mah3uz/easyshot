mod capture;
mod encoder;

pub use capture::{CaptureSource, GstCapture, element_available, gst_init};
pub use encoder::{ENCODERS, Encoder, pick_encoder};

use crate::{
    Clock, Error, RecordSource, RecordSpec, Recorder, RecorderProbe, RecordingHandle, Result,
    SystemClock,
};
use ashpd::desktop::PersistMode;
use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, StartCastOptions,
};
use std::sync::{Arc, Mutex};

pub struct PortalRecorder {
    restore_token: Mutex<Option<String>>,
    clock: Arc<dyn Clock>,
}

impl PortalRecorder {
    pub fn new(restore_token: Option<String>) -> Self {
        Self {
            restore_token: Mutex::new(restore_token),
            clock: Arc::new(SystemClock::default()),
        }
    }

    /// The token the portal last handed back; the caller stores it for the next session.
    pub fn restore_token(&self) -> Option<String> {
        self.restore_token.lock().ok().and_then(|t| t.clone())
    }
}

fn portal_err(e: ashpd::Error) -> Error {
    match e {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => Error::Cancelled,
        other => Error::Protocol(other.to_string()),
    }
}

fn crop_for(source: &RecordSource, size: Option<(i32, i32)>) -> Option<[i32; 4]> {
    let (RecordSource::Region { logical, .. }, Some((w, h))) = (source, size) else {
        return None;
    };
    let clamp = |v: f64, max: i32| (v.round() as i32).clamp(0, max);
    Some([
        clamp(logical.x0, w),
        clamp(logical.y0, h),
        clamp(f64::from(w) - logical.x1, w),
        clamp(f64::from(h) - logical.y1, h),
    ])
}

async fn open_session(
    spec: &RecordSpec,
    token: Option<String>,
) -> Result<(CaptureSource, Option<String>, Box<dyn FnOnce() + Send>)> {
    let sc = Screencast::new().await.map_err(portal_err)?;
    let session = sc
        .create_session(Default::default())
        .await
        .map_err(portal_err)?;
    let sources = match spec.source {
        RecordSource::Window(_) => SourceType::Window,
        _ => SourceType::Monitor,
    };
    let cursor = if spec.cursor {
        CursorMode::Embedded
    } else {
        CursorMode::Hidden
    };
    sc.select_sources(
        &session,
        SelectSourcesOptions::default()
            .set_cursor_mode(cursor)
            .set_sources(ashpd::enumflags2::BitFlags::from(sources))
            .set_multiple(false)
            .set_persist_mode(PersistMode::ExplicitlyRevoked)
            .set_restore_token(token.as_deref()),
    )
    .await
    .map_err(portal_err)?
    .response()
    .map_err(portal_err)?;
    let streams = sc
        .start(&session, None, StartCastOptions::default())
        .await
        .map_err(portal_err)?
        .response()
        .map_err(portal_err)?;
    let stream = streams
        .streams()
        .first()
        .ok_or_else(|| Error::Protocol("the portal returned no stream".into()))?;
    let node = stream.pipe_wire_node_id();
    let crop = crop_for(&spec.source, stream.size());
    let new_token = streams.restore_token().map(str::to_owned);
    let fd = sc
        .open_pipe_wire_remote(&session, Default::default())
        .await
        .map_err(portal_err)?;
    let cleanup: Box<dyn FnOnce() + Send> = Box::new(move || {
        let _ = futures::executor::block_on(session.close());
    });
    Ok((
        CaptureSource::PipeWire { fd, node, crop },
        new_token,
        cleanup,
    ))
}

impl Recorder for PortalRecorder {
    fn name(&self) -> &'static str {
        "portal-pipewire"
    }

    fn probe(&self) -> RecorderProbe {
        let problem = if capture::gst_init().is_err() {
            Some("GStreamer failed to initialise")
        } else if !element_available("pipewiresrc") {
            Some("the GStreamer pipewire plugin is missing")
        } else if !element_available("matroskamux") {
            Some("the GStreamer matroska plugin is missing")
        } else if pick_encoder(element_available).is_none() {
            Some("no GStreamer video encoder is installed")
        } else {
            None
        };
        RecorderProbe {
            available: problem.is_none(),
            reason: problem.map(str::to_owned),
        }
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        std::fs::create_dir_all(&spec.dir).map_err(|source| Error::Io {
            path: spec.dir.clone(),
            source,
        })?;
        let token = self.restore_token();
        let (source, new_token, cleanup) = futures::executor::block_on(open_session(&spec, token))?;
        if let (Some(t), Ok(mut slot)) = (new_token, self.restore_token.lock()) {
            *slot = Some(t);
        }
        let out = spec.dir.join("screen.mkv");
        let capture = GstCapture::start(source, &out, None, self.clock.clone(), Some(cleanup))?;
        Ok(Box::new(capture))
    }
}
