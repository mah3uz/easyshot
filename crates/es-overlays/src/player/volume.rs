use super::engine::Engine;
use super::options::{ExportOptions, VOLUME_LIMIT};
use super::view::PlayerWindow;
use crate::capture::strings::fl;
use es_ui::components::{InspectorSlider, SliderEvent, ValueFormat};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};

pub(super) type Volumes = [(Entity<InspectorSlider>, Subscription); 2];

pub(super) fn levels(o: &ExportOptions) -> [f64; 2] {
    [o.system_volume, o.mic_volume]
}

fn with_level(o: ExportOptions, track: usize, v: f64) -> ExportOptions {
    if track == 0 {
        ExportOptions {
            system_volume: v,
            ..o
        }
    } else {
        ExportOptions { mic_volume: v, ..o }
    }
}

impl PlayerWindow {
    /// Two sliders, only for a video that has both a system and a microphone track.
    pub(super) fn ensure_volumes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<[Entity<InspectorSlider>; 2]> {
        if self.volumes.is_none() && self.engine.as_ref().map_or(0, Engine::audio_tracks) >= 2 {
            let start = levels(&self.options);
            let labels = [fl!("player-volume-system"), fl!("player-volume-mic")];
            let [a, b] = [0, 1].map(|track| {
                let slider = cx.new(|cx| {
                    InspectorSlider::new(
                        labels[track].clone(),
                        start[track],
                        0.0..=VOLUME_LIMIT,
                        ValueFormat::percent(),
                        window,
                        cx,
                    )
                });
                let sub = cx.subscribe(&slider, move |p, _, event: &SliderEvent, cx| {
                    if let SliderEvent::Changed(v) = event {
                        p.set_options(with_level(p.options, track, *v), cx);
                    }
                });
                (slider, sub)
            });
            self.volumes = Some([a, b]);
        }
        self.volumes
            .as_ref()
            .map(|v| [v[0].0.clone(), v[1].0.clone()])
    }

    pub(super) fn apply_volumes(&self) {
        if let Some(engine) = &self.engine {
            for (track, level) in levels(&self.options).into_iter().enumerate() {
                engine.set_level(track, level);
            }
        }
    }

    /// The slider values, system then microphone; none while the video has fewer than two tracks.
    pub fn volume_sliders(&self, cx: &gpui::App) -> Option<[f64; 2]> {
        self.volumes
            .as_ref()
            .map(|v| [v[0].0.read(cx).value(), v[1].0.read(cx).value()])
    }
}
