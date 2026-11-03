use super::{Recordings, Start, finish};
use es_geom::Rect;
use es_model::config::Recording;
use es_model::platform::OutputId;
use es_overlays::record::{RecordingRequest, RecordingTarget};
use es_protocol::agent::Audio;
use gpui::App;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use uuid::Uuid;

/// What a start overrides in the recording settings, for this recording only.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub fps: Option<u32>,
    pub keystrokes: bool,
}

impl Options {
    pub fn apply(&self, cfg: &mut Recording) {
        if let Some(fps) = self.fps {
            cfg.fps = fps;
        }
        cfg.keystrokes |= self.keystrokes;
    }

    pub(super) fn keeps_keystrokes(&self, cfg: &Recording) -> bool {
        cfg.keystrokes || self.keystrokes
    }
}

impl Start {
    pub fn new(request: RecordingRequest, area: Option<(OutputId, Rect)>) -> Start {
        Start {
            request,
            area,
            options: Options::default(),
        }
    }
}

/// A finished recording's card; `saving` while the copy into the videos folder is still running.
#[derive(Debug, Clone)]
pub struct Kept {
    pub card: Uuid,
    pub file: PathBuf,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub saving: bool,
}

#[derive(Debug, Clone)]
pub enum Ended {
    Kept(Kept),
    Saved { card: Uuid, path: PathBuf },
    SaveFailed { card: Uuid },
    Discarded,
    Failed,
}

impl Ended {
    pub(super) fn kept(done: &finish::Completed, saving: bool) -> Ended {
        Ended::Kept(Kept {
            card: done.item.id,
            file: done.item.file.clone(),
            duration_ms: (done.capture.duration_secs * 1000.).round() as u64,
            width: done.capture.pixel_width,
            height: done.capture.pixel_height,
            saving,
        })
    }
}

impl gpui::Global for Recordings {}

pub type Observer = Rc<dyn Fn(&Ended, &mut App)>;

impl Recordings {
    /// Told how every recording ends, whoever started it; one observer at a time.
    pub fn observe(&self, observer: Observer) {
        *self.0.observer.borrow_mut() = Some(observer);
    }

    pub(super) fn ended(&self, ended: &Ended, cx: &mut App) {
        let observer = self.0.observer.borrow().clone();
        if let Some(observer) = observer {
            observer(ended, cx);
        }
    }

    pub fn elapsed(&self, cx: &App) -> Option<Duration> {
        let active = self.0.active.borrow();
        active.as_ref().map(|s| s.bar.read(cx).elapsed())
    }

    /// Starts at once; an omitted sound choice takes the recording settings.
    pub fn agent_request(
        &self,
        target: RecordingTarget,
        audio: Option<Audio>,
        cx: &App,
    ) -> RecordingRequest {
        let request = RecordingRequest {
            delay_secs: 0,
            ..self.configured(target, cx)
        };
        let Some(audio) = audio else { return request };
        let mic = self.0.deps.settings.read(cx).config().recording.mic.clone();
        RecordingRequest {
            mic: matches!(audio, Audio::Mic | Audio::Both).then_some(mic),
            system_audio: matches!(audio, Audio::System | Audio::Both),
            ..request
        }
    }
}
