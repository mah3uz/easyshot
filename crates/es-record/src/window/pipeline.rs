use super::audio::Track;
use super::cut::Timeline;
use crate::mute::AudioSource;
use crate::{Error, Quality, Result};
use es_model::{PixelFormat, VideoFrame};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    Nvh264,
    Vah264,
    X264,
}

impl Encoder {
    /// Tried in this order; the first that encodes a frame is used.
    pub const ORDER: [Encoder; 3] = [Encoder::Nvh264, Encoder::Vah264, Encoder::X264];

    pub fn element(self) -> &'static str {
        match self {
            Encoder::Nvh264 => "nvh264enc",
            Encoder::Vah264 => "vah264enc",
            Encoder::X264 => "x264enc",
        }
    }

    pub fn installed(self) -> bool {
        gst::init().is_ok() && gst::ElementFactory::find(self.element()).is_some()
    }

    // nvh264enc takes the RGB layouts directly and converts on the GPU; the others need 4:2:0
    // so players without High 4:4:4 support can open the file.
    pub(crate) fn chain(self, kbps: u32) -> String {
        match self {
            Encoder::Nvh264 => format!("nvh264enc name=encoder rc-mode=cbr bitrate={kbps}"),
            Encoder::Vah264 => format!(
                "video/x-raw,format=NV12 ! vah264enc name=encoder rate-control=cbr bitrate={kbps}"
            ),
            Encoder::X264 => format!(
                "video/x-raw,format=I420 \
                 ! x264enc name=encoder speed-preset=ultrafast tune=zerolatency bitrate={kbps}"
            ),
        }
    }
}

/// Video bitrate in kbit/s: the canvas's pixels per second times the quality's bits per pixel,
/// within 1 to 80 Mbit/s.
pub(crate) fn bitrate_kbps(quality: Quality, (w, h): (u32, u32), fps: u32) -> u32 {
    let millibits_per_pixel = match quality {
        Quality::Medium => 50,
        Quality::High => 80,
        Quality::VeryHigh => 110,
        Quality::Ultra => 150,
    };
    let kbps = u64::from(w) * u64::from(h) * u64::from(fps) * millibits_per_pixel / 1_000_000;
    kbps.clamp(1_000, 80_000) as u32
}

/// The sound tracks to record, in file order, and where each one's capture comes from.
pub(crate) struct Sound<'a> {
    pub sources: &'a [AudioSource],
    pub capture: &'a dyn Fn(AudioSource) -> Result<gst::Element>,
    pub timeline: &'a Arc<Mutex<Timeline>>,
}

pub(crate) const ELEMENTS: [&str; 7] = [
    "appsrc",
    "videoscale",
    "videoconvert",
    "queue",
    "h264parse",
    "matroskamux",
    "filesink",
];

const ERROR_WINDOW: Duration = Duration::from_millis(300);
const FIRST_OUTPUT_LIMIT: Duration = Duration::from_secs(5);
const EOS_LIMIT: Duration = Duration::from_secs(10);

/// One frame as GStreamer holds it; repeating it shares the pixels.
#[derive(Clone)]
pub(crate) struct Picture {
    buffer: gst::Buffer,
    width: u32,
    height: u32,
    format: PixelFormat,
}

fn gst_format(f: PixelFormat) -> &'static str {
    match f {
        PixelFormat::Argb8888 => "BGRA",
        PixelFormat::Xrgb8888 => "BGRx",
        PixelFormat::Abgr8888 => "RGBA",
        PixelFormat::Xbgr8888 => "RGBx",
    }
}

impl Picture {
    pub fn new(frame: VideoFrame) -> Result<Picture> {
        let (w, h, stride) = (frame.width, frame.height, frame.stride);
        let row = w as usize * 4;
        if w == 0
            || h == 0
            || (stride as usize) < row
            || frame.data.len() < stride as usize * h as usize
        {
            return Err(Error::Pipeline(format!(
                "frame {w}x{h} stride {stride} has {} bytes",
                frame.data.len()
            )));
        }
        let buffer = if stride as usize == row {
            gst::Buffer::from_slice(frame.data)
        } else {
            let mut tight = Vec::with_capacity(row * h as usize);
            for line in frame.data.chunks(stride as usize).take(h as usize) {
                tight.extend_from_slice(&line[..row]);
            }
            gst::Buffer::from_mut_slice(tight)
        };
        Ok(Picture {
            buffer,
            width: w,
            height: h,
            format: frame.format,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn caps(&self, fps: u32) -> gst::Caps {
        gst::Caps::builder("video/x-raw")
            .field("format", gst_format(self.format))
            .field("width", self.width as i32)
            .field("height", self.height as i32)
            .field("framerate", gst::Fraction::new(fps as i32, 1))
            .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
            .build()
    }
}

/// The canvas every frame is fitted into: the first frame's size, rounded up to even for 4:2:0.
pub(crate) fn canvas((w, h): (u32, u32)) -> (u32, u32) {
    (w.max(2).next_multiple_of(2), h.max(2).next_multiple_of(2))
}

pub(crate) struct Pipeline {
    pipeline: gst::Pipeline,
    src: gst_app::AppSrc,
    encoder: Encoder,
    fps: u32,
    caps_for: Option<(u32, u32, PixelFormat)>,
    tracks: Vec<Track>,
    mic_lost: bool,
}

fn pipeline_err(e: impl std::fmt::Display) -> Error {
    Error::Pipeline(e.to_string())
}

fn describe(msg: &gst::Message) -> String {
    match msg.view() {
        gst::MessageView::Error(e) => match e.debug() {
            Some(d) => format!("{} ({d})", e.error()),
            None => e.error().to_string(),
        },
        _ => format!("{:?}", msg.type_()),
    }
}

impl Pipeline {
    /// Builds the pipeline with the first encoder in `order` that turns `first` into an encoded
    /// frame without a bus error, and leaves it playing with `first` pushed as tick 0.
    pub fn open(
        path: &Path,
        (canvas, fps, kbps): ((u32, u32), u32, u32),
        first: &Picture,
        order: &[Encoder],
        sound: &Sound,
    ) -> Result<Pipeline> {
        gst::init().map_err(pipeline_err)?;
        let mut failures = Vec::new();
        for &encoder in order {
            if !encoder.installed() {
                failures.push(format!("{}: not installed", encoder.element()));
                continue;
            }
            match Self::build(path, canvas, fps, &encoder.chain(kbps), encoder)
                .and_then(|p| p.with_sound(sound))
                .and_then(|mut p| {
                    p.preroll(first)?;
                    Ok(p)
                }) {
                Ok(p) => return Ok(p),
                Err(e @ Error::Audio(_)) => return Err(e),
                Err(e) => {
                    tracing::warn!(encoder = encoder.element(), error = %e, "video encoder unusable");
                    failures.push(format!("{}: {e}", encoder.element()));
                }
            }
        }
        Err(Error::NoEncoder(failures.join("; ")))
    }

    fn build(
        path: &Path,
        (cw, ch): (u32, u32),
        fps: u32,
        chain: &str,
        encoder: Encoder,
    ) -> Result<Pipeline> {
        let max_bytes = u64::from(cw) * u64::from(ch) * 4 * 3;
        let launch = format!(
            "appsrc name=src is-live=true format=time block=false max-bytes={max_bytes} \
             ! videoscale add-borders=true \
             ! video/x-raw,width={cw},height={ch},pixel-aspect-ratio=1/1 \
             ! queue max-size-buffers=8 max-size-bytes=0 max-size-time=0 \
             ! videoconvert ! {chain} ! h264parse name=parse \
             ! matroskamux name=mux ! filesink name=sink"
        );
        let pipeline = gst::parse::launch(&launch)
            .map_err(pipeline_err)?
            .downcast::<gst::Pipeline>()
            .map_err(|_| Error::Pipeline("not a pipeline".into()))?;
        // The sound server's clock stops with its device; ticks and sound share this one instead.
        pipeline.use_clock(Some(&gst::SystemClock::obtain()));
        let sink = pipeline
            .by_name("sink")
            .ok_or_else(|| Error::Pipeline("no file sink".into()))?;
        sink.set_property("location", path.to_string_lossy().as_ref());
        let src = pipeline
            .by_name("src")
            .and_then(|e| e.downcast::<gst_app::AppSrc>().ok())
            .ok_or_else(|| Error::Pipeline("no app source".into()))?;
        Ok(Pipeline {
            pipeline,
            src,
            encoder,
            fps,
            caps_for: None,
            tracks: Vec::new(),
            mic_lost: false,
        })
    }

    fn with_sound(mut self, sound: &Sound) -> Result<Pipeline> {
        let mux = self
            .pipeline
            .by_name("mux")
            .ok_or_else(|| Error::Pipeline("no muxer".into()))?;
        for &source in sound.sources {
            let capture = (sound.capture)(source)?;
            let track = Track::add(&self.pipeline, &mux, sound.timeline, source, capture)?;
            self.tracks.push(track);
        }
        Ok(self)
    }

    fn preroll(&mut self, first: &Picture) -> Result<()> {
        let parse = self
            .pipeline
            .by_name("parse")
            .and_then(|p| p.static_pad("src"))
            .ok_or_else(|| Error::Pipeline("no parser".into()))?;
        let (tx, through) = mpsc::channel();
        parse.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
            let _ = tx.send(());
            gst::PadProbeReturn::Remove
        });
        let bus = self
            .pipeline
            .bus()
            .ok_or_else(|| Error::Pipeline("no bus".into()))?;
        let began = Instant::now();
        if let Err(e) = self.pipeline.set_state(gst::State::Playing) {
            return Err(match bus.pop_filtered(&[gst::MessageType::Error]) {
                Some(m) => self.failure(&m),
                None => Error::Pipeline(e.to_string()),
            });
        }
        let period = 1_000_000_000 / u64::from(self.fps.max(1));
        self.push(first, 0, period)?;
        let mut out = false;
        loop {
            if let Some(msg) = bus.timed_pop_filtered(
                gst::ClockTime::from_mseconds(10),
                &[gst::MessageType::Error],
            ) {
                return Err(self.failure(&msg));
            }
            out = out || through.try_recv().is_ok();
            let waited = began.elapsed();
            if out && waited >= ERROR_WINDOW {
                return Ok(());
            }
            if waited >= FIRST_OUTPUT_LIMIT {
                return Err(Error::Timeout);
            }
        }
    }

    pub fn encoder(&self) -> Encoder {
        self.encoder
    }

    pub fn clock(&self) -> Result<gst::Clock> {
        self.pipeline
            .clock()
            .ok_or_else(|| Error::Pipeline("the pipeline has no clock".into()))
    }

    /// False when the encoder is behind and its input queue is full, so the frame was not taken.
    // Never blocks: a push blocked on a stalled encoder would hold up stop and error reporting.
    pub fn push(&mut self, picture: &Picture, pts: u64, duration: u64) -> Result<bool> {
        if self.src.current_level_bytes() >= self.src.max_bytes() {
            return Ok(false);
        }
        let key = (picture.width, picture.height, picture.format);
        if self.caps_for != Some(key) {
            self.src.set_caps(Some(&picture.caps(self.fps)));
            self.caps_for = Some(key);
        }
        let mut buffer = picture.buffer.copy();
        {
            let b = buffer.make_mut();
            b.set_pts(gst::ClockTime::from_nseconds(pts));
            b.set_duration(gst::ClockTime::from_nseconds(duration));
        }
        self.src
            .push_buffer(buffer)
            .map(|_| true)
            .map_err(|e| Error::Pipeline(format!("push: {e:?}")))
    }

    fn failure(&self, msg: &gst::Message) -> Error {
        match self.track_of(msg) {
            Some(_) => Error::Audio(describe(msg)),
            None => Error::Pipeline(describe(msg)),
        }
    }

    fn track_of(&self, msg: &gst::Message) -> Option<AudioSource> {
        self.tracks.iter().find(|t| t.posted(msg)).map(|t| t.source)
    }

    /// Whether `msg` is a capture stream failing; its track goes on as silence.
    fn sound_lost(&self, msg: &gst::Message) -> bool {
        let Some(source) = self.track_of(msg) else {
            return false;
        };
        tracing::warn!(?source, error = %describe(msg), "sound capture failed; its track goes on silent");
        true
    }

    pub fn base_time(&self) -> u64 {
        self.pipeline.base_time().map_or(0, |t| t.nseconds())
    }

    pub fn set_muted(&self, source: AudioSource, muted: bool) {
        let muted = muted || (source == AudioSource::Mic && self.mic_lost);
        for t in self.tracks.iter().filter(|t| t.source == source) {
            t.set_muted(muted);
        }
    }

    /// From now on the microphone's track is silence, whatever stream the sound server moves
    /// the capture to.
    pub fn mic_lost(&mut self) {
        self.mic_lost = true;
        self.set_muted(AudioSource::Mic, true);
    }

    pub fn unmute_streams(&self) {
        for t in &self.tracks {
            t.unmute_stream();
        }
    }

    /// The first error on the bus since the last look, without waiting; a failed capture stream is
    /// logged and passed over.
    pub fn error(&self) -> Option<String> {
        let bus = self.pipeline.bus()?;
        loop {
            let msg = bus.pop_filtered(&[gst::MessageType::Error])?;
            if !self.sound_lost(&msg) {
                return Some(describe(&msg));
            }
        }
    }

    /// Ends the video, and waits until the muxer has written the file's index. The sound tracks
    /// end once the timeline is stopped.
    pub fn finish(self) -> Result<()> {
        self.src
            .end_of_stream()
            .map_err(|e| Error::Pipeline(format!("{e:?}")))?;
        let bus = self
            .pipeline
            .bus()
            .ok_or_else(|| Error::Pipeline("no bus".into()))?;
        let until = Instant::now() + EOS_LIMIT;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let limit = gst::ClockTime::from_nseconds(left.as_nanos() as u64);
            match bus.timed_pop_filtered(limit, &[gst::MessageType::Eos, gst::MessageType::Error]) {
                Some(m) if m.type_() == gst::MessageType::Eos => return Ok(()),
                Some(m) if self.sound_lost(&m) => {}
                Some(m) => return Err(Error::Pipeline(describe(&m))),
                None => return Err(Error::Timeout),
            }
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}
