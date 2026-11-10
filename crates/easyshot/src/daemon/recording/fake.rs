use es_record::{
    Clock, RecordSpec, Recorder, RecorderEvent, RecorderProbe, RecordingHandle, RecordingOutput,
    Result,
};
use futures::future::BoxFuture;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

/// Records nothing. With `video`, stopping leaves a one-second test pattern from ffmpeg's `lavfi`
/// source, so the card has a real file behind it. With `clock`, the output's first frame and
/// paused spans are stamped from it, as a real recorder's are.
#[derive(Clone, Default)]
pub struct FakeRecorder {
    pub video: bool,
    pub no_system_audio: bool,
    /// Named `window`, as the recorder of a window's own frames is.
    pub window: bool,
    /// Reports its window closed as soon as it has started.
    pub closes: bool,
    /// Reports its window can no longer be captured as soon as it has started.
    pub lost: bool,
    pub clock: Option<Arc<dyn Clock>>,
}

struct FakeHandle {
    dir: PathBuf,
    video: bool,
    tx: async_channel::Sender<RecorderEvent>,
    rx: async_channel::Receiver<RecorderEvent>,
    clock: Option<Arc<dyn Clock>>,
    first: u64,
    running_since: Option<u64>,
    segments: Vec<(u64, u64)>,
}

impl FakeHandle {
    fn now(&self) -> u64 {
        self.clock.as_ref().map_or(0, |c| c.monotonic_us())
    }
}

impl Recorder for FakeRecorder {
    fn name(&self) -> &'static str {
        if self.window { "window" } else { "fake" }
    }

    fn probe(&self) -> RecorderProbe {
        RecorderProbe {
            available: true,
            reason: None,
        }
    }

    fn can_record_system_audio(&self, _with_mic: bool) -> bool {
        !self.no_system_audio
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        let (tx, rx) = async_channel::unbounded();
        let _ = tx.try_send(RecorderEvent::Started);
        if self.closes {
            let _ = tx.try_send(RecorderEvent::WindowClosed);
        }
        if self.lost {
            let _ = tx.try_send(RecorderEvent::WindowLost);
        }
        let first = self.clock.as_ref().map_or(0, |c| c.monotonic_us());
        Ok(Box::new(FakeHandle {
            dir: spec.dir,
            video: self.video,
            tx,
            rx,
            clock: self.clock.clone(),
            first,
            running_since: Some(first),
            segments: Vec::new(),
        }))
    }
}

impl RecordingHandle for FakeHandle {
    fn pause(&mut self) -> Result<()> {
        if let Some(start) = self.running_since.take() {
            self.segments.push((start, self.now()));
        }
        let _ = self.tx.try_send(RecorderEvent::Paused);
        Ok(())
    }

    fn set_muted(&mut self, _: es_record::AudioSource, _: bool) -> Result<()> {
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if self.running_since.is_none() {
            self.running_since = Some(self.now());
        }
        let _ = self.tx.try_send(RecorderEvent::Resumed);
        Ok(())
    }

    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
        if let Some(start) = self.running_since.take() {
            let end = self.now();
            self.segments.push((start, end));
        }
        let segments = if self.clock.is_some() {
            std::mem::take(&mut self.segments)
        } else {
            Vec::new()
        };
        let file = self.dir.join(es_record::project::VIDEO);
        let made = self.video
            && Command::new("ffmpeg")
                .args(["-y", "-v", "error", "-f", "lavfi", "-i"])
                .arg("testsrc=duration=1:size=320x180:rate=10")
                .args(["-c:v", "mpeg4"])
                .arg(&file)
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("DISPLAY")
                .stdin(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
        if !made {
            let _ = std::fs::write(&file, b"fake video");
        }
        let first = self.first;
        Box::pin(async move {
            Ok(RecordingOutput {
                file,
                first_frame_monotonic_us: first,
                segments,
            })
        })
    }

    fn discard(self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.rx.clone()
    }
}

/// An input source with no devices behind it.
#[cfg(feature = "test-hooks")]
pub struct NoEvents;

#[cfg(feature = "test-hooks")]
impl es_record::input::EventSource for NoEvents {
    fn next_batch(&mut self, wait: std::time::Duration) -> Vec<es_record::RawEvent> {
        std::thread::sleep(wait);
        Vec::new()
    }
}
