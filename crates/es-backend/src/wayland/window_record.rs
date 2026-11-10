use es_record::{
    Error, FrameSource, RecordSource, RecordSpec, Recorder, Result, SourceEvent, WindowRecorder,
};
use es_wayland::{StreamEvent, StreamOptions, ToplevelStream};
use std::sync::Arc;
use std::time::Duration;

struct WindowFrames(ToplevelStream);

impl FrameSource for WindowFrames {
    fn recv_timeout(&mut self, timeout: Duration) -> Option<SourceEvent> {
        Some(match self.0.recv_timeout(timeout)? {
            StreamEvent::Frame(f) => SourceEvent::Frame(f),
            StreamEvent::Stopped => SourceEvent::Stopped,
            StreamEvent::Failed(e) => SourceEvent::Failed(e.to_string()),
        })
    }
}

fn open(spec: &RecordSpec) -> Result<Box<dyn FrameSource>> {
    let RecordSource::Window(w) = &spec.source else {
        return Err(Error::Unsupported);
    };
    let options = StreamOptions {
        fps: spec.fps,
        paint_cursor: spec.cursor,
    };
    let stream =
        ToplevelStream::open(&w.id.0, options).map_err(|e| Error::Capture(e.to_string()))?;
    Ok(Box::new(WindowFrames(stream)))
}

/// A recorder of one window's own frames, or `None` when GStreamer or an H.264 encoder is missing.
pub fn window_recorder() -> Option<Arc<dyn Recorder>> {
    let recorder =
        WindowRecorder::new(open).with_audio_runner(Arc::new(es_record::project::SystemRunner));
    recorder.probe().available.then(|| Arc::new(recorder) as _)
}
