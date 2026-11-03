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
