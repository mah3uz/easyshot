use crate::capture::strings::fl;
use es_record::gst_plugins::{self, Gap};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioOut {
    Speakers,
    /// Keeps the audio clock running without making a sound.
    Silent,
}

impl AudioOut {
    fn factory(self) -> &'static str {
        match self {
            AudioOut::Speakers => "autoaudiosink",
            AudioOut::Silent => "fakeaudiosink",
        }
    }
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub pts: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Signal {
    Frame,
    /// A state change or seek finished; position and duration are now answerable.
    Settled,
    End,
    Failed(String),
}

type Latest = Arc<Mutex<Option<Frame>>>;

/// Every sound track gets its own volume into one mixer, so all of them are heard at once.
struct Mix {
    levels: Vec<f64>,
    volumes: Vec<gst::Element>,
    mixer: Option<gst::Element>,
}

impl Mix {
    fn level(&self, track: usize) -> f64 {
        self.levels.get(track).copied().unwrap_or(1.)
    }
}

type Shared = Arc<Mutex<Mix>>;

pub struct Engine {
    playbin: gst::Pipeline,
    mix: Shared,
    filter: gst::Element,
    balance: gst::Element,
    latest: Latest,
    signals: async_channel::Receiver<Signal>,
}

/// The plain install message for what the player lacks, or `None` when it has everything.
fn install_message(gaps: &[Gap]) -> Option<String> {
    if gaps.is_empty() {
        return None;
    }
    Some(fl!(
        "player-missing-plugins",
        packages = gst_plugins::packages(gaps).join(" ")
    ))
}

fn make(factory: &str) -> Result<gst::Element, String> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| format!("GStreamer element {factory} is missing"))
}

/// Frames arrive no larger than `w` x `h`, keeping their shape; smaller videos stay at their own size.
fn fit_caps(w: u32, h: u32) -> gst::Caps {
    gst::Caps::builder("video/x-raw")
        .field("format", "BGRA")
        .field("width", gst::IntRange::new(1, w.max(1) as i32))
        .field("height", gst::IntRange::new(1, h.max(1) as i32))
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
        .build()
}

fn attach_audio(
    pipeline: &gst::Pipeline,
    mix: &Shared,
    out: AudioOut,
    pad: &gst::Pad,
) -> Result<(), String> {
    let mut mix = mix.lock().unwrap_or_else(|e| e.into_inner());
    let mut added: Vec<gst::Element> = Vec::new();
    if mix.mixer.is_none() {
        let (mixer, convert, resample, sink) = (
            make("audiomixer")?,
            make("audioconvert")?,
            make("audioresample")?,
            make(out.factory())?,
        );
        let tail = [&mixer, &convert, &resample, &sink];
        pipeline.add_many(tail).map_err(|e| e.to_string())?;
        gst::Element::link_many(tail).map_err(|e| e.to_string())?;
        added.extend(tail.into_iter().cloned());
        mix.mixer = Some(mixer);
    }
    let mixer = mix.mixer.clone().ok_or("no audio mixer")?;
    let (convert, resample, volume) = (
        make("audioconvert")?,
        make("audioresample")?,
        make("volume")?,
    );
    volume.set_property("volume", mix.level(mix.volumes.len()));
    let branch = [&convert, &resample, &volume];
    pipeline.add_many(branch).map_err(|e| e.to_string())?;
    gst::Element::link_many(branch).map_err(|e| e.to_string())?;
    let request = mixer
        .request_pad_simple("sink_%u")
        .ok_or("the audio mixer gave no input")?;
    let from = volume.static_pad("src").ok_or("volume has no src pad")?;
    from.link(&request).map_err(|e| e.to_string())?;
    added.extend(branch.into_iter().cloned());
    for element in added.iter().rev() {
        element
            .sync_state_with_parent()
            .map_err(|e| e.to_string())?;
    }
    let sink = convert
        .static_pad("sink")
        .ok_or("audioconvert has no sink pad")?;
    pad.link(&sink).map_err(|e| e.to_string())?;
    mix.volumes.push(volume);
    Ok(())
}

fn read_frame(sample: &gst::Sample) -> Option<Frame> {
    let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
    let buffer = sample.buffer()?;
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).ok()?;
    let (w, h) = (info.width(), info.height());
    let stride = usize::try_from(*frame.plane_stride().first()?).ok()?;
    let data = frame.plane_data(0).ok()?;
    let row = w as usize * 4;
    let mut bgra = Vec::with_capacity(row * h as usize);
    for y in 0..h as usize {
        bgra.extend_from_slice(data.get(y * stride..y * stride + row)?);
    }
    Some(Frame {
        width: w,
        height: h,
        bgra,
        pts: buffer.pts().map(|t| Duration::from_nanos(t.nseconds())),
    })
}

/// Only the newest frame is kept; the UI is woken once per batch, not once per frame.
fn deliver(sample: &gst::Sample, latest: &Latest, tx: &async_channel::Sender<Signal>) {
    let Some(frame) = read_frame(sample) else {
        return;
    };
    let mut slot = latest.lock().unwrap_or_else(|e| e.into_inner());
    if slot.replace(frame).is_none() {
        let _ = tx.try_send(Signal::Frame);
    }
}

fn secs(t: gst::ClockTime) -> f64 {
    t.nseconds() as f64 / 1e9
}

impl Engine {
    /// Opens paused on the first frame. `levels` are the volumes of the sound tracks in file order; a track without one plays at full volume.
    pub fn open(
        video: &Path,
        audio: AudioOut,
        fit: (u32, u32),
        levels: &[f64],
    ) -> Result<Self, String> {
        gst::init().map_err(|e| e.to_string())?;
        let gaps = gst_plugins::playback_gaps(&gst_plugins::in_registry);
        if let Some(message) = install_message(&gaps) {
            for gap in &gaps {
                tracing::warn!(
                    element = gap.element,
                    package = gap.package,
                    "GStreamer element missing"
                );
            }
            return Err(message);
        }
        let uri = gst::glib::filename_to_uri(video, None).map_err(|e| e.to_string())?;
        let playbin = gst::Pipeline::new();
        let source = gst::ElementFactory::make("uridecodebin")
            .property("uri", uri.as_str())
            .build()
            .map_err(|_| "GStreamer element uridecodebin is missing".to_owned())?;
        playbin.add(&source).map_err(|e| e.to_string())?;
        let mix: Shared = Arc::new(Mutex::new(Mix {
            levels: levels.to_vec(),
            volumes: Vec::new(),
            mixer: None,
        }));
        let (balance, convert, scale, filter) = (
            make("videobalance")?,
            make("videoconvert")?,
            make("videoscale")?,
            make("capsfilter")?,
        );
        filter.set_property("caps", fit_caps(fit.0, fit.1));
        let sink = gst_app::AppSink::builder()
            .sync(true)
            .max_buffers(2)
            .drop(true)
            .build();
        let bin = gst::Bin::new();
        let parts = [&balance, &convert, &scale, &filter, sink.upcast_ref()];
        bin.add_many(parts).map_err(|e| e.to_string())?;
        gst::Element::link_many(parts).map_err(|e| e.to_string())?;
        let pad = balance
            .static_pad("sink")
            .ok_or_else(|| "videobalance has no sink pad".to_owned())?;
        let ghost = gst::GhostPad::with_target(&pad).map_err(|e| e.to_string())?;
        bin.add_pad(&ghost).map_err(|e| e.to_string())?;
        playbin.add(&bin).map_err(|e| e.to_string())?;
        let (weak, weak_mix, video_in) = (playbin.downgrade(), mix.clone(), ghost.clone());
        source.connect_pad_added(move |_, pad| {
            let Some(pipeline) = weak.upgrade() else {
                return;
            };
            let kind = pad
                .current_caps()
                .or_else(|| Some(pad.query_caps(None)))
                .and_then(|c| c.structure(0).map(|s| s.name().to_string()))
                .unwrap_or_default();
            if kind.starts_with("video/") {
                if !video_in.is_linked() {
                    let _ = pad.link(&video_in);
                }
            } else if kind.starts_with("audio/")
                && let Err(error) = attach_audio(&pipeline, &weak_mix, audio, pad)
            {
                tracing::warn!(%error, "audio track not played");
            }
        });

        let (tx, signals) = async_channel::unbounded();
        let latest: Latest = Arc::default();
        let (on_sample, on_preroll) = ((latest.clone(), tx.clone()), (latest.clone(), tx.clone()));
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |s| {
                    let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    deliver(&sample, &on_sample.0, &on_sample.1);
                    Ok(gst::FlowSuccess::Ok)
                })
                .new_preroll(move |s| {
                    let sample = s.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                    deliver(&sample, &on_preroll.0, &on_preroll.1);
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        let bus = playbin
            .bus()
            .ok_or_else(|| "playbin has no bus".to_owned())?;
        bus.set_sync_handler(move |_, msg| {
            let signal = match msg.view() {
                gst::MessageView::AsyncDone(_) => Some(Signal::Settled),
                gst::MessageView::Eos(_) => Some(Signal::End),
                gst::MessageView::Error(e) => Some(Signal::Failed(e.error().to_string())),
                _ => None,
            };
            if let Some(signal) = signal {
                let _ = tx.try_send(signal);
            }
            gst::BusSyncReply::Drop
        });
        playbin
            .set_state(gst::State::Paused)
            .map_err(|e| e.to_string())?;
        Ok(Engine {
            playbin,
            mix,
            filter,
            balance,
            latest,
            signals,
        })
    }

    pub fn signals(&self) -> async_channel::Receiver<Signal> {
        self.signals.clone()
    }

    pub fn take_frame(&self) -> Option<Frame> {
        self.latest.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub fn play(&self) {
        let _ = self.playbin.set_state(gst::State::Playing);
    }

    pub fn pause(&self) {
        let _ = self.playbin.set_state(gst::State::Paused);
    }

    pub fn seek(&self, secs: f64) {
        let at = gst::ClockTime::from_nseconds((secs.max(0.) * 1e9) as u64);
        let _ = self
            .playbin
            .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, at);
    }

    pub fn position(&self) -> Option<f64> {
        self.playbin.query_position::<gst::ClockTime>().map(secs)
    }

    pub fn duration(&self) -> Option<f64> {
        self.playbin.query_duration::<gst::ClockTime>().map(secs)
    }

    pub fn set_tone(&self, brightness: f64, contrast: f64, saturation: f64) {
        self.balance.set_property("brightness", brightness);
        self.balance.set_property("contrast", contrast);
        self.balance.set_property("saturation", saturation);
    }

    pub fn tone(&self) -> (f64, f64, f64) {
        (
            self.balance.property("brightness"),
            self.balance.property("contrast"),
            self.balance.property("saturation"),
        )
    }

    /// The sound tracks found in the file; known once the first state change has settled.
    pub fn audio_tracks(&self) -> usize {
        self.mix
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .volumes
            .len()
    }

    pub fn set_level(&self, track: usize, level: f64) {
        let mut mix = self.mix.lock().unwrap_or_else(|e| e.into_inner());
        if mix.levels.len() <= track {
            mix.levels.resize(track + 1, 1.);
        }
        mix.levels[track] = level;
        if let Some(volume) = mix.volumes.get(track) {
            volume.set_property("volume", level);
        }
    }

    pub fn level(&self, track: usize) -> Option<f64> {
        let mix = self.mix.lock().unwrap_or_else(|e| e.into_inner());
        mix.volumes.get(track).map(|v| v.property("volume"))
    }

    pub fn fit(&self, w: u32, h: u32) {
        self.filter.set_property("caps", fit_caps(w, h));
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_plugin_tells_the_user_what_to_install_not_an_element_name() {
        let gaps = gst_plugins::playback_gaps(&|e| e != "videobalance");
        let message = install_message(&gaps).unwrap();
        assert!(message.contains("gst-plugins-good"), "{message}");
        assert!(!message.contains("videobalance"), "{message}");
    }

    #[test]
    fn nothing_missing_gives_no_message() {
        assert_eq!(install_message(&[]), None);
    }
}
