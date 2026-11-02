use es_record::{
    RecordSpec, Recorder, RecorderEvent, RecorderProbe, RecordingHandle, RecordingOutput, Result,
};
use futures::future::BoxFuture;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Records nothing. With `video`, stopping leaves a one-second test pattern from ffmpeg's `lavfi`
/// source, so the card has a real file behind it.
#[derive(Clone, Copy, Default)]
pub struct FakeRecorder {
    pub video: bool,
}

struct FakeHandle {
    dir: PathBuf,
    video: bool,
    tx: async_channel::Sender<RecorderEvent>,
    rx: async_channel::Receiver<RecorderEvent>,
}

impl Recorder for FakeRecorder {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn probe(&self) -> RecorderProbe {
        RecorderProbe {
            available: true,
            reason: None,
        }
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        let (tx, rx) = async_channel::unbounded();
        let _ = tx.try_send(RecorderEvent::Started);
        Ok(Box::new(FakeHandle {
            dir: spec.dir,
            video: self.video,
            tx,
            rx,
        }))
    }
}

impl RecordingHandle for FakeHandle {
    fn pause(&mut self) -> Result<()> {
        let _ = self.tx.try_send(RecorderEvent::Paused);
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        let _ = self.tx.try_send(RecorderEvent::Resumed);
        Ok(())
    }

    fn stop(self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
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
        Box::pin(async move {
            Ok(RecordingOutput {
                file,
                first_frame_monotonic_us: 0,
                segments: Vec::new(),
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
