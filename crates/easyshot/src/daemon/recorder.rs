use super::recording::RecorderFor;
use es_app::errors::Error;
use es_backend::Backend;
use std::rc::Rc;
use std::sync::Arc;

/// Picks the recorder the settings name, else the best one the backend found; test mode fakes one.
pub fn for_backend(backend: Arc<Backend>) -> RecorderFor {
    struct Shared(Arc<Backend>, usize);
    impl es_record::Recorder for Shared {
        fn name(&self) -> &'static str {
            self.0.recorders[self.1].name()
        }
        fn probe(&self) -> es_record::RecorderProbe {
            self.0.recorders[self.1].probe()
        }
        fn start(
            &self,
            spec: es_record::RecordSpec,
        ) -> es_record::Result<Box<dyn es_record::RecordingHandle>> {
            self.0.recorders[self.1].start(spec)
        }
        fn prepare(
            &self,
            spec: &es_record::RecordSpec,
            outputs: &[es_model::platform::OutputInfo],
        ) {
            self.0.recorders[self.1].prepare(spec, outputs)
        }
        fn can_record_system_audio(&self, with_mic: bool) -> bool {
            self.0.recorders[self.1].can_record_system_audio(with_mic)
        }
    }
    Rc::new(move |wanted| {
        #[cfg(feature = "test-hooks")]
        if super::test_mode_requested() {
            return Ok(Arc::new(super::recording::fake::FakeRecorder {
                video: true,
                ..Default::default()
            }));
        }
        let at = backend
            .recorders
            .iter()
            .position(|r| wanted == "auto" || r.name() == wanted)
            .ok_or(Error::Unsupported)?;
        Ok(Arc::new(Shared(backend.clone(), at)))
    })
}

/// The recorder that records a window from its own frames, when the backend has one.
pub fn window_recorder(backend: &Backend) -> Option<Arc<dyn es_record::Recorder>> {
    let recorder = backend.window_recorder.clone()?;
    #[cfg(feature = "test-hooks")]
    if super::test_mode_requested() {
        return Some(Arc::new(Silent(recorder)));
    }
    Some(recorder)
}

/// Test mode records a test-output window's frames, but never the user's own sound or microphone.
#[cfg(feature = "test-hooks")]
struct Silent(Arc<dyn es_record::Recorder>);

#[cfg(feature = "test-hooks")]
impl es_record::Recorder for Silent {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn probe(&self) -> es_record::RecorderProbe {
        self.0.probe()
    }
    fn start(
        &self,
        spec: es_record::RecordSpec,
    ) -> es_record::Result<Box<dyn es_record::RecordingHandle>> {
        self.0.start(es_record::RecordSpec {
            system_audio: false,
            mic: None,
            ..spec
        })
    }
    fn can_record_system_audio(&self, _with_mic: bool) -> bool {
        false
    }
}

#[cfg(all(test, feature = "test-hooks"))]
mod tests {
    use super::Silent;
    use es_record::{
        Codec, Container, Quality, RecordSource, RecordSpec, Recorder, RecorderProbe,
        RecordingHandle,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Spy(Mutex<Option<RecordSpec>>);

    impl Recorder for Spy {
        fn name(&self) -> &'static str {
            "window"
        }
        fn probe(&self) -> RecorderProbe {
            RecorderProbe {
                available: true,
                reason: None,
            }
        }
        fn start(&self, spec: RecordSpec) -> es_record::Result<Box<dyn RecordingHandle>> {
            *self.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(spec);
            Err(es_record::Error::Unsupported)
        }
    }

    // A test run shares the user's sound server, whose sound and microphone must stay out of reach.
    #[test]
    fn a_test_mode_window_recording_asks_for_no_sound() {
        let spy = Arc::new(Spy::default());
        let silent = Silent(spy.clone());
        assert!(!silent.can_record_system_audio(true));
        let _ = silent.start(RecordSpec {
            source: RecordSource::Output(es_model::platform::OutputId("test".into())),
            fps: 30,
            codec: Codec::H264,
            container: Container::Mkv,
            quality: Quality::High,
            system_audio: true,
            mic: Some("default".into()),
            cursor: true,
            dir: std::env::temp_dir(),
        });
        let started = spy.0.lock().unwrap_or_else(|e| e.into_inner()).take();
        let started = started.expect("the window recorder was started");
        assert!(!started.system_audio && started.mic.is_none());
    }
}
