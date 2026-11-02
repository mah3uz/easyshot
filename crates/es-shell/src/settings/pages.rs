use super::widgets::{group, row, switch};
use super::{Page, SettingsWindow, Toggle};
use crate::strings::fl;
use es_model::Config;
use es_ui::components::{ButtonKind, EditorButton, InspectorSlider, Segment, Segmented};
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, px,
};
use gpui_component::h_flex;

const SEGMENT_CHAR: f32 = 6.5;
const SEGMENT_PAD: f32 = 20.;
const SEGMENT_MIN: f32 = 52.;
const SEGMENTED_MAX: f32 = 380.;

fn text(s: String) -> Option<SharedString> {
    Some(s.into())
}

impl SettingsWindow {
    pub(super) fn toggle_row(
        &self,
        toggle: Toggle,
        title: String,
        help: Option<String>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = toggle.get(&self.config(cx));
        let control = switch(
            toggle.id(),
            title.clone(),
            on,
            cx.listener(move |this, on: &bool, _, cx| this.set_toggle(toggle, *on, cx)),
        );
        row(title, help.map(Into::into), control, cx)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn choice_row<T: PartialEq + Clone + 'static>(
        &self,
        id: &'static str,
        title: String,
        help: Option<String>,
        options: Vec<(T, String)>,
        current: &T,
        set: fn(&mut Config, T),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = options.iter().position(|(v, _)| v == current).unwrap_or(0);
        let values: Vec<T> = options.iter().map(|(v, _)| v.clone()).collect();
        let longest = options
            .iter()
            .map(|(_, l)| l.chars().count())
            .max()
            .unwrap_or(0);
        let width =
            (longest as f32 * SEGMENT_CHAR + SEGMENT_PAD).max(SEGMENT_MIN) * options.len() as f32;
        let this = cx.entity().downgrade();
        let control = Segmented::new(
            id,
            title.clone(),
            options.into_iter().map(|(_, label)| Segment::text(label)),
        )
        .selected(selected)
        .on_select(move |i, _, cx| {
            if let Some(v) = values.get(i).cloned() {
                this.update(cx, |this, cx| this.edit(move |c| set(c, v), cx))
                    .ok();
            }
        });
        let control = div().w(px(width.min(SEGMENTED_MAX))).child(control);
        row(title, help.map(Into::into), control, cx)
    }

    pub(super) fn slider_row(&self, slider: &Entity<InspectorSlider>) -> AnyElement {
        div()
            .px(px(16.))
            .py(px(6.))
            .child(slider.clone())
            .into_any_element()
    }

    pub(super) fn restore_row(
        &self,
        page: Page,
        help: Option<String>,
        reset: fn(&mut Config),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control = if self.confirm_restore == Some(page) {
            h_flex()
                .gap(px(8.))
                .child(
                    EditorButton::new("restore-cancel")
                        .label(fl!("settings-restore-cancel"))
                        .kind(ButtonKind::Bordered)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.confirm_restore = None;
                            cx.notify();
                        })),
                )
                .child(
                    EditorButton::new("restore-confirm")
                        .label(fl!("general-restore-confirm"))
                        .kind(ButtonKind::Prominent)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.confirm_restore = None;
                            this.edit(reset, cx);
                        })),
                )
                .into_any_element()
        } else {
            EditorButton::new("restore")
                .label(fl!("general-restore"))
                .kind(ButtonKind::Bordered)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.confirm_restore = Some(page);
                    cx.notify();
                }))
                .into_any_element()
        };
        group(
            None,
            vec![row(help.unwrap_or_default(), None, control, cx)],
            None,
            cx,
        )
        .into_any_element()
    }

    pub(super) fn render_capture(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let timer = [0u32, 3, 5, 10]
            .into_iter()
            .map(|s| {
                let label = if s == 0 {
                    fl!("capture-timer-off")
                } else {
                    fl!("capture-timer-seconds", seconds = s)
                };
                (s, label)
            })
            .collect();
        let on_release_help = if c.capture.capture_on_release {
            fl!("capture-on-release-on-help")
        } else {
            fl!("capture-on-release-off-help")
        };
        let preselect_help = if c.capture.preselect_previous {
            fl!("capture-preselect-on-help")
        } else {
            fl!("capture-preselect-off-help")
        };
        let mut region = vec![
            self.toggle_row(
                Toggle::CaptureOnRelease,
                fl!("capture-on-release"),
                Some(on_release_help),
                cx,
            ),
            self.toggle_row(
                Toggle::Preselect,
                fl!("capture-preselect"),
                Some(preselect_help),
                cx,
            ),
            self.choice_row(
                "selector-on",
                fl!("capture-selector-on"),
                None,
                vec![
                    (true, fl!("capture-selector-on-pointer")),
                    (false, fl!("capture-selector-on-specific")),
                ],
                &c.capture.selector_follows_pointer,
                |c, v| c.capture.selector_follows_pointer = v,
                cx,
            ),
        ];
        if !c.capture.selector_follows_pointer {
            let pinned = c.capture.pinned_output.clone();
            if self.outputs.contains(&pinned) || pinned.is_empty() {
                let options = self
                    .outputs
                    .iter()
                    .map(|o| (o.clone(), o.clone()))
                    .collect();
                region.push(self.choice_row(
                    "selector-screen",
                    fl!("capture-selector-screen"),
                    None,
                    options,
                    &pinned,
                    |c, v| c.capture.pinned_output = v,
                    cx,
                ));
            } else {
                region.push(row(
                    fl!("capture-selector-screen"),
                    text(fl!("capture-selector-screen-stale")),
                    div(),
                    cx,
                ));
            }
        }
        let region_footer = if c.capture.preselect_previous {
            fl!("capture-footer-preselect")
        } else {
            fl!("capture-footer-default")
        };
        vec![
            group(
                text(fl!("capture-timer")),
                vec![self.choice_row(
                    "countdown",
                    fl!("capture-countdown"),
                    Some(fl!("capture-countdown-help")),
                    timer,
                    &c.capture.self_timer,
                    |c, v| c.capture.self_timer = v,
                    cx,
                )],
                None,
                cx,
            )
            .into_any_element(),
            group(text(fl!("capture-region")), region, text(region_footer), cx).into_any_element(),
            group(
                text(fl!("capture-after")),
                vec![
                    self.toggle_row(
                        Toggle::OpenEditor,
                        fl!("capture-open-editor"),
                        Some(fl!("capture-open-editor-help")),
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::KeepPreviews,
                        fl!("capture-keep-previews"),
                        Some(fl!("capture-keep-previews-help")),
                        cx,
                    ),
                ],
                None,
                cx,
            )
            .into_any_element(),
            self.restore_row(
                Page::Capture,
                Some(fl!("capture-restore-help")),
                |c| c.capture = Default::default(),
                cx,
            ),
        ]
    }

    pub(super) fn render_overlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let o = &c.overlay;
        let s = |v: &str| v.to_owned();
        vec![
            group(
                text(fl!("overlay-quick-setup")),
                vec![
                    self.choice_row(
                        "overlay-layout",
                        fl!("overlay-layout"),
                        None,
                        vec![
                            (s("standard"), fl!("overlay-layout-standard")),
                            (s("sharing"), fl!("overlay-layout-sharing")),
                            (s("minimal"), fl!("overlay-layout-minimal")),
                        ],
                        &o.layout.clone(),
                        |c, v| c.overlay.layout = v,
                        cx,
                    ),
                    self.choice_row(
                        "overlay-position",
                        fl!("overlay-position"),
                        None,
                        vec![
                            (s("bottom-right"), fl!("overlay-position-bottom-right")),
                            (s("bottom-left"), fl!("overlay-position-bottom-left")),
                        ],
                        &o.position.clone(),
                        |c, v| c.overlay.position = v,
                        cx,
                    ),
                    self.choice_row(
                        "overlay-size",
                        fl!("overlay-card-size"),
                        None,
                        vec![
                            (s("small"), fl!("overlay-card-size-small")),
                            (s("medium"), fl!("overlay-card-size-medium")),
                            (s("large"), fl!("overlay-card-size-large")),
                        ],
                        &o.card_size.clone(),
                        |c, v| c.overlay.card_size = v,
                        cx,
                    ),
                ],
                text(fl!("overlay-quick-setup-footer")),
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("overlay-behavior")),
                vec![
                    self.toggle_row(
                        Toggle::AlwaysShowActions,
                        fl!("overlay-always-show-actions"),
                        Some(fl!("overlay-always-show-actions-help")),
                        cx,
                    ),
                    self.slider_row(&self.sliders.margin),
                    self.slider_row(&self.sliders.hide_after),
                ],
                text(fl!("overlay-behavior-footer")),
                cx,
            )
            .into_any_element(),
            self.restore_row(
                Page::Overlay,
                Some(fl!("overlay-restore-help")),
                |c| c.overlay = Default::default(),
                cx,
            ),
        ]
    }

    pub(super) fn render_recording(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let r = &c.recording;
        let s = |v: &str| v.to_owned();
        let mut mics = vec![
            (String::new(), fl!("recording-microphone-off")),
            (s("default"), fl!("recording-microphone-default")),
        ];
        if !r.mic.is_empty() && r.mic != "default" {
            mics.push((r.mic.clone(), r.mic.clone()));
        }
        let countdown = [0u32, 1, 3, 5]
            .into_iter()
            .map(|n| {
                let label = if n == 0 {
                    fl!("recording-countdown-none")
                } else {
                    fl!("recording-countdown-seconds", count = n)
                };
                (n, label)
            })
            .collect();
        vec![
            group(
                text(fl!("recording-include")),
                vec![
                    self.choice_row(
                        "mic",
                        fl!("recording-microphone"),
                        None,
                        mics,
                        &r.mic.clone(),
                        |c, v| c.recording.mic = v,
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::SystemAudio,
                        fl!("recording-system-audio"),
                        Some(fl!("recording-system-audio-help")),
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::Keystrokes,
                        fl!("recording-keystrokes"),
                        Some(fl!("recording-keystrokes-help")),
                        cx,
                    ),
                ],
                text(fl!("recording-include-footer")),
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("recording-before")),
                vec![self.choice_row(
                    "start-delay",
                    fl!("recording-countdown"),
                    Some(fl!("recording-countdown-help")),
                    countdown,
                    &r.start_delay,
                    |c, v| c.recording.start_delay = v,
                    cx,
                )],
                None,
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("recording-after")),
                vec![
                    self.toggle_row(
                        Toggle::RecordingOpenEditor,
                        fl!("recording-open-editor"),
                        Some(fl!("recording-open-editor-help")),
                        cx,
                    ),
                    self.toggle_row(
                        Toggle::RecordingSave,
                        fl!("recording-save-to-folder"),
                        Some(fl!("recording-save-to-folder-help")),
                        cx,
                    ),
                ],
                None,
                cx,
            )
            .into_any_element(),
            group(
                text(fl!("recording-export")),
                vec![
                    self.choice_row(
                        "fps",
                        fl!("recording-export-fps"),
                        Some(fl!("recording-export-fps-help")),
                        [30u32, 60]
                            .into_iter()
                            .map(|f| (f, fl!("recording-export-fps-value", fps = f)))
                            .collect(),
                        &r.fps,
                        |c, v| c.recording.fps = v,
                        cx,
                    ),
                    self.choice_row(
                        "codec",
                        fl!("recording-export-codec"),
                        None,
                        vec![
                            (s("h264"), fl!("recording-export-codec-h264")),
                            (s("hevc"), fl!("recording-export-codec-hevc")),
                            (s("av1"), fl!("recording-export-codec-av1")),
                        ],
                        &r.codec.clone(),
                        |c, v| c.recording.codec = v,
                        cx,
                    ),
                    self.choice_row(
                        "container",
                        fl!("recording-export-container"),
                        None,
                        vec![
                            (s("mkv"), fl!("recording-export-container-mkv")),
                            (s("mp4"), fl!("recording-export-container-mp4")),
                        ],
                        &r.container.clone(),
                        |c, v| c.recording.container = v,
                        cx,
                    ),
                    self.choice_row(
                        "quality",
                        fl!("recording-export-quality"),
                        None,
                        vec![
                            (s("medium"), fl!("recording-export-quality-medium")),
                            (s("high"), fl!("recording-export-quality-high")),
                            (s("very_high"), fl!("recording-export-quality-very-high")),
                            (s("ultra"), fl!("recording-export-quality-ultra")),
                        ],
                        &r.quality.clone(),
                        |c, v| c.recording.quality = v,
                        cx,
                    ),
                    self.choice_row(
                        "recorder",
                        fl!("recording-recorder"),
                        Some(fl!("recording-recorder-help")),
                        vec![
                            (s("auto"), fl!("recording-recorder-auto")),
                            (s("gsr-native"), fl!("recording-recorder-gsr")),
                            (s("wf-recorder"), fl!("recording-recorder-wf")),
                            (s("portal-pipewire"), fl!("recording-recorder-portal")),
                        ],
                        &r.recorder.clone(),
                        |c, v| c.recording.recorder = v,
                        cx,
                    ),
                ],
                text(fl!("recording-export-footer")),
                cx,
            )
            .into_any_element(),
            self.restore_row(
                Page::Recording,
                None,
                |c| c.recording = Default::default(),
                cx,
            ),
        ]
    }
}
