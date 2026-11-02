use super::OutputSource;
use es_geom::Point;
use es_model::platform::{OutputId, OutputInfo};
use std::sync::{Arc, RwLock};

/// The screen the selector opens on when "A specific screen" is chosen; empty means the pointer's.
#[derive(Clone, Default)]
pub struct PinnedOutput(Arc<RwLock<Option<String>>>);

impl PinnedOutput {
    pub fn set(&self, follows_pointer: bool, name: &str) {
        let pinned = (!follows_pointer && !name.is_empty()).then(|| name.to_owned());
        if let Ok(mut slot) = self.0.write() {
            *slot = pinned;
        }
    }

    fn get(&self) -> Option<String> {
        self.0.read().ok()?.clone()
    }
}

/// Reports the pinned screen as the pointer's while it is connected, so the selector's
/// keyboard focus and first overlay go there.
pub struct PinnedOutputSource {
    inner: Arc<dyn OutputSource>,
    pinned: PinnedOutput,
}

impl PinnedOutputSource {
    pub fn new(inner: Arc<dyn OutputSource>, pinned: PinnedOutput) -> Self {
        Self { inner, pinned }
    }
}

impl OutputSource for PinnedOutputSource {
    fn outputs(&self) -> Vec<OutputInfo> {
        self.inner.outputs()
    }

    fn pointer_output(&self) -> Option<OutputId> {
        self.pinned
            .get()
            .and_then(|name| {
                self.inner
                    .outputs()
                    .into_iter()
                    .find(|o| o.id.0 == name)
                    .map(|o| o.id)
            })
            .or_else(|| self.inner.pointer_output())
    }

    fn focused_output(&self) -> Option<OutputId> {
        self.inner.focused_output()
    }

    fn pointer_global(&self) -> Option<Point> {
        self.inner.pointer_global()
    }
}
