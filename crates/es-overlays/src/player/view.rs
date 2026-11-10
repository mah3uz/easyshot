use super::engine::{AudioOut, Engine, Frame, Signal};
use super::export::{ExportJob, ExportMode, FfmpegRunner, export};
use super::options::{BRIGHTNESS_LIMIT, ExportOptions};
use super::scrubber::follow_drags;
use super::strip::{StripSpec, strip};
use super::trim::Trim;
use super::volume::{Volumes, levels};
use crate::capture::strings::fl;
use es_record::CommandRunner;
use es_store::Paths;
use es_ui::a11y::{Accessible, Role};
use es_ui::components::{InspectorSlider, SliderEvent, ValueFormat};
use es_ui::theme::tokens;
use gpui::{
    App, AppContext as _, Bounds, Context, Corners, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, ObjectFit, ParentElement, Pixels, Render,
    RenderImage, Styled, Subscription, Task, Window, canvas, div, px,
};
use std::path::PathBuf;
use std::sync::Arc;

pub const TITLE_BAR_HEIGHT: Pixels = px(38.);
const PANEL_X: Pixels = px(20.);
const STAGE_INSET: Pixels = px(16.);
pub(super) const STRIP_HEIGHT: Pixels = px(44.);

pub struct PlayerConfig {
    pub video: PathBuf,
    /// Asked once per export with the file extension, so each export can get a name of its own.
    pub output: std::rc::Rc<dyn Fn(&str) -> PathBuf>,
    pub options: ExportOptions,
    pub audio: AudioOut,
    pub paths: Paths,
    pub commands: Arc<dyn CommandRunner>,
    pub ffmpeg: Arc<dyn FfmpegRunner>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    Exported(PathBuf),
    /// The choices an export started with, for the next time.
    Chosen(ExportOptions),
    OpenStudio,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExportState {
    Idle,
    Running(f64),
    Done(PathBuf),
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Grab {
    Start,
    End,
    Playhead,
}

pub struct PlayerWindow {
    pub(super) config: PlayerConfig,
    pub(super) engine: Option<Engine>,
    pub(super) failure: Option<String>,
    focus: FocusHandle,
    focused: bool,
    image: Option<Arc<RenderImage>>,
    pub(super) trim: Trim,
    pub(super) position: f64,
    pub(super) playing: bool,
    pub(super) strip: Option<Arc<RenderImage>>,
    strip_asked: bool,
    pub(super) track: Bounds<Pixels>,
    scale: f32,
    fit: (u32, u32),
    pub(super) grab: Option<Grab>,
    pub(super) mode: ExportMode,
    pub(super) remove_audio: bool,
    pub(super) options: ExportOptions,
    brightness: Option<(Entity<InspectorSlider>, Subscription)>,
    pub(super) volumes: Option<Volumes>,
    pub(super) export: ExportState,
    frames: u64,
    dropped: u64,
    _signals: Option<Task<()>>,
}

impl EventEmitter<PlayerEvent> for PlayerWindow {}

impl Focusable for PlayerWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn render_image(width: u32, height: u32, bgra: Vec<u8>) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

fn load_strip(path: &std::path::Path) -> Option<Arc<RenderImage>> {
    let mut rgba = es_store::decode::open_image(path).ok()?.to_rgba8();
    for p in rgba.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    let (w, h) = rgba.dimensions();
    render_image(w, h, rgba.into_raw())
}

impl PlayerWindow {
    pub fn new(config: PlayerConfig, cx: &mut Context<Self>) -> Self {
        let opened = Engine::open(
            &config.video,
            config.audio,
            (1280, 720),
            &levels(&config.options),
        );
        let (engine, failure) = match opened {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e)),
        };
        let signals = engine.as_ref().map(|e| {
            let rx = e.signals();
            cx.spawn(async move |this, cx| {
                while let Ok(signal) = rx.recv().await {
                    if this.update(cx, |p, cx| p.on_signal(signal, cx)).is_err() {
                        break;
                    }
                }
            })
        });
        let options = config.options;
        let this = PlayerWindow {
            config,
            engine,
            failure,
            focus: cx.focus_handle(),
            focused: false,
            image: None,
            trim: Trim::new(0.),
            position: 0.,
            playing: false,
            strip: None,
            strip_asked: false,
            track: Bounds::default(),
            scale: 1.,
            fit: (1280, 720),
            grab: None,
            mode: ExportMode::default(),
            remove_audio: false,
            options,
            brightness: None,
            volumes: None,
            export: ExportState::Idle,
            frames: 0,
            dropped: 0,
            _signals: signals,
        };
        this.apply_tone();
        this
    }

    pub fn frames_shown(&self) -> u64 {
        self.frames
    }

    pub fn images_dropped(&self) -> u64 {
        self.dropped
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn trim(&self) -> Trim {
        self.trim
    }

    pub fn has_strip(&self) -> bool {
        self.strip.is_some()
    }

    /// Where the thumbnail strip was last drawn, in window coordinates.
    pub fn track(&self) -> Bounds<Pixels> {
        self.track
    }

    pub fn export_state(&self) -> &ExportState {
        &self.export
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    fn on_signal(&mut self, signal: Signal, cx: &mut Context<Self>) {
        match signal {
            Signal::Frame => {
                if let Some(frame) = self.engine.as_ref().and_then(Engine::take_frame) {
                    self.show(frame, cx);
                }
            }
            Signal::Settled => {
                let duration = self.engine.as_ref().and_then(Engine::duration);
                if let Some(d) = duration.filter(|_| self.trim.duration() <= 0.) {
                    self.trim = Trim::new(d);
                }
                self.ask_strip(cx);
            }
            Signal::End => {
                self.playing = false;
                self.position = self.trim.duration();
                if let Some(e) = &self.engine {
                    e.pause();
                }
            }
            Signal::Failed(reason) => {
                self.playing = false;
                self.failure = Some(reason);
            }
        }
        cx.notify();
    }

    fn show(&mut self, frame: Frame, cx: &mut Context<Self>) {
        let Some(image) = render_image(frame.width, frame.height, frame.bgra) else {
            return;
        };
        self.frames += 1;
        if let Some(old) = self.image.replace(image) {
            cx.drop_image(old, None);
            self.dropped += 1;
        }
        if !self.playing || self.grab.is_some() {
            return;
        }
        let at = self.engine.as_ref().and_then(Engine::position);
        let at = at.or_else(|| frame.pts.map(|t| t.as_secs_f64()));
        if let Some(at) = at {
            self.position = at;
        }
        if self.position >= self.trim.end() {
            self.position = self.trim.end();
            self.pause();
        }
    }

    fn ask_strip(&mut self, cx: &mut Context<Self>) {
        let width = f64::from(f32::from(self.track.size.width) * self.scale);
        if self.strip_asked || width <= 0. || self.trim.duration() <= 0. {
            return;
        }
        self.strip_asked = true;
        let spec = StripSpec::fitting(width, f64::from(f32::from(STRIP_HEIGHT) * self.scale));
        let (runner, paths) = (self.config.commands.clone(), self.config.paths.clone());
        let (video, duration) = (self.config.video.clone(), self.trim.duration());
        let made = cx.background_spawn(async move {
            match strip(runner.as_ref(), &paths, &video, duration, spec) {
                Ok(path) => load_strip(&path),
                Err(error) => {
                    tracing::warn!(%error, "thumbnail strip failed");
                    None
                }
            }
        });
        cx.spawn(async move |this, cx| {
            let image = made.await;
            let _ = this.update(cx, |p, cx| {
                p.strip = image;
                cx.notify();
            });
        })
        .detach();
    }

    fn pause(&mut self) {
        self.playing = false;
        if let Some(e) = &self.engine {
            e.pause();
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = &self.engine else { return };
        if self.playing {
            self.pause();
        } else {
            let (start, end) = (self.trim.start(), self.trim.end());
            if self.position >= end - 0.01 || self.position < start {
                self.position = start;
                engine.seek(start);
            }
            engine.play();
            self.playing = true;
        }
        cx.notify();
    }

    pub fn seek(&mut self, secs: f64, cx: &mut Context<Self>) {
        self.position = secs.clamp(0., self.trim.duration());
        if let Some(e) = &self.engine {
            e.seek(self.position);
        }
        cx.notify();
    }

    pub(super) fn drag_to(&mut self, grab: Grab, secs: f64, cx: &mut Context<Self>) {
        let at = match grab {
            Grab::Start => {
                self.trim.set_start(secs);
                self.trim.start()
            }
            Grab::End => {
                self.trim.set_end(secs);
                self.trim.end()
            }
            Grab::Playhead => secs,
        };
        self.seek(at, cx);
    }

    pub fn set_mode(&mut self, mode: ExportMode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    pub fn set_remove_audio(&mut self, remove: bool, cx: &mut Context<Self>) {
        self.remove_audio = remove;
        cx.notify();
    }

    pub fn options(&self) -> ExportOptions {
        self.options
    }

    /// The preview's brightness, contrast and saturation, as the player is showing them.
    pub fn previewed_tone(&self) -> Option<(f64, f64, f64)> {
        self.engine.as_ref().map(Engine::tone)
    }

    fn apply_tone(&self) {
        let Some(engine) = &self.engine else { return };
        let (contrast, saturation) = self.options.look.tone();
        engine.set_tone(self.options.brightness, contrast, saturation);
        if !self.playing {
            engine.seek(self.position);
        }
    }

    pub fn set_options(&mut self, options: ExportOptions, cx: &mut Context<Self>) {
        self.options = options;
        self.apply_tone();
        self.apply_volumes();
        cx.notify();
    }

    fn ensure_brightness(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InspectorSlider> {
        if let Some((slider, _)) = &self.brightness {
            return slider.clone();
        }
        let start = self.options.brightness;
        let slider = cx.new(|cx| {
            InspectorSlider::new(
                fl!("player-brightness"),
                start,
                -BRIGHTNESS_LIMIT..=BRIGHTNESS_LIMIT,
                ValueFormat::percent().signed(),
                window,
                cx,
            )
        });
        let sub = cx.subscribe(&slider, |p, _, event: &SliderEvent, cx| {
            if let SliderEvent::Changed(v) = event {
                let options = ExportOptions {
                    brightness: *v,
                    ..p.options
                };
                p.set_options(options, cx);
            }
        });
        self.brightness = Some((slider.clone(), sub));
        slider
    }

    pub fn export(&mut self, cx: &mut Context<Self>) {
        if matches!(self.export, ExportState::Running(_)) || self.trim.duration() <= 0. {
            return;
        }
        self.pause();
        let job = ExportJob {
            input: self.config.video.clone(),
            output: (self.config.output)(self.options.format.id()),
            trim: self.trim,
            mode: self.mode,
            remove_audio: self.remove_audio,
            audio_tracks: self.engine.as_ref().map_or(0, Engine::audio_tracks),
            options: self.options,
        };
        cx.emit(PlayerEvent::Chosen(self.options));
        let runner = self.config.ffmpeg.clone();
        let (tx, rx) = async_channel::unbounded();
        let run = cx.background_spawn(async move {
            export(runner.as_ref(), &job, &mut |p| {
                let _ = tx.try_send(p);
            })
        });
        self.export = ExportState::Running(0.);
        cx.notify();
        cx.spawn(async move |this, cx| {
            while let Ok(p) = rx.recv().await {
                let _ = this.update(cx, |w, cx| {
                    w.export = ExportState::Running(p);
                    cx.notify();
                });
            }
            let result = run.await;
            let _ = this.update(cx, |w, cx| {
                w.export = match result {
                    Ok(path) => {
                        cx.emit(PlayerEvent::Exported(path.clone()));
                        ExportState::Done(path)
                    }
                    Err(e) => ExportState::Failed(e.to_string()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// Frames are decoded no larger than the stage shows them.
    fn refit(&mut self, window: &Window) {
        let Some(engine) = &self.engine else { return };
        self.scale = window.scale_factor();
        let view = window.viewport_size();
        let w = (f32::from(view.width) * self.scale) as u32;
        let h = (f32::from(view.height) * self.scale) as u32;
        if (w, h) != self.fit && w > 0 && h > 0 {
            self.fit = (w, h);
            engine.fit(w, h);
            // A paused pipeline only renegotiates on its next flush.
            if !self.playing {
                engine.seek(self.position);
            }
        }
    }
}

impl Render for PlayerWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.focus, cx);
        }
        self.refit(window);
        let brightness = self.ensure_brightness(window, cx);
        let volumes = self.ensure_volumes(window, cx);
        if !self.strip_asked {
            self.ask_strip(cx);
        }
        let t = tokens(cx).clone();
        // An `img` sizes itself from the picture; the stage must size the picture instead.
        let stage = self.image.clone().map(|image| {
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let fit = ObjectFit::Contain.get_bounds(bounds, image.size(0));
                    let _ =
                        window.paint_image(fit, fit, Corners::default(), image.clone(), 0, false);
                },
            )
            .absolute()
            .top(STAGE_INSET)
            .bottom(STAGE_INSET)
            .left(STAGE_INSET)
            .right(STAGE_INSET)
        });
        let root = div()
            .id("player")
            .accessible(Role::Application, fl!("player-a11y"))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|p, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "space" {
                    p.toggle_play(cx);
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(t.window)
            .child(self.title_bar(cx))
            .child(
                div()
                    .id("player-stage")
                    .accessible(Role::Image, fl!("player-video-a11y"))
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .bg(t.workspace)
                    .children(stage),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .px(PANEL_X)
                    .pt(px(12.))
                    .pb(px(14.))
                    .border_t_1()
                    .border_color(t.separator)
                    .child(self.transport(cx))
                    .child(self.scrubber(cx))
                    .child(self.options_panel(&brightness, volumes, cx))
                    .child(self.status(cx)),
            );
        follow_drags(root, cx)
    }
}
