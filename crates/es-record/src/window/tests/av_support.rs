use super::support::SECOND;
use crate::mute::AudioSource;
use crate::project::CommandRunner;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const RATE: u64 = 48_000;

fn live(wave: &str, freq: f64) -> gst::Element {
    gst::init().expect("gstreamer");
    let desc = format!(
        "audiotestsrc is-live=true wave={wave} freq={freq} volume=0.5 samplesperbuffer=480 \
         ! capsfilter caps=audio/x-raw,format=S16LE,rate=48000,channels=1"
    );
    gst::parse::bin_from_description(&desc, true)
        .unwrap_or_else(|e| panic!("{desc}: {e}"))
        .upcast()
}

/// A live sine of `freq` Hz.
pub fn tone(freq: f64) -> gst::Element {
    live("sine", freq)
}

/// Live silence with a 5 ms full-scale click at the pipeline clock time `at` holds.
pub fn click(at: Arc<AtomicU64>) -> gst::Element {
    let bin = live("silence", 440.0).downcast::<gst::Bin>().expect("bin");
    let pad = bin.static_pad("src").expect("ghost pad");
    pad.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
        let t = at.load(Ordering::SeqCst);
        let base = pad
            .parent_element()
            .and_then(|e| e.base_time())
            .map_or(0, |b| b.nseconds());
        let pts = info
            .buffer()
            .and_then(|b| b.pts())
            .map_or(0, |p| p.nseconds());
        if t == 0 || t < base {
            return gst::PadProbeReturn::Ok;
        }
        let at = (t - base) as u128 * RATE as u128 / SECOND as u128;
        let first = pts as u128 * RATE as u128 / SECOND as u128;
        let Some(buffer) = info.buffer_mut() else {
            return gst::PadProbeReturn::Ok;
        };
        let len = (buffer.size() / 2) as u128;
        if at + 240 <= first || at >= first + len {
            return gst::PadProbeReturn::Ok;
        }
        let buffer = buffer.make_mut();
        buffer.unset_flags(gst::BufferFlags::GAP);
        let mut map = buffer.map_writable().expect("writable");
        for (i, s) in map
            .as_mut_slice()
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .enumerate()
        {
            let n = first + i as u128;
            if n >= at && n < at + 240 {
                let v = if (n - at) % 24 < 12 {
                    i16::MAX
                } else {
                    -i16::MAX
                };
                s.copy_from_slice(&v.to_le_bytes());
            }
        }
        gst::PadProbeReturn::Ok
    });
    bin.upcast()
}

/// Test sources by track: system sound and microphone each from `make`.
pub fn sources(
    make: impl Fn(AudioSource) -> gst::Element + Send + Sync + 'static,
) -> impl Fn(AudioSource, &crate::RecordSpec) -> crate::Result<gst::Element> + Send + Sync + 'static
{
    move |source, _| Ok(make(source))
}

#[derive(Debug, Default)]
pub struct Sound {
    /// Decoded mono samples, from file time `start`.
    pub samples: Vec<i16>,
    pub start: u64,
    /// The largest jump between where one decoded buffer ends and the next begins.
    pub widest_gap: u64,
}

impl Sound {
    pub fn seconds(&self) -> f64 {
        (self.start as f64 / SECOND as f64) + self.samples.len() as f64 / RATE as f64
    }

    fn window(&self, from: f64, to: f64) -> &[i16] {
        let at = |t: f64| {
            let s = ((t - self.start as f64 / SECOND as f64) * RATE as f64).max(0.0) as usize;
            s.min(self.samples.len())
        };
        &self.samples[at(from)..at(to)]
    }

    pub fn rms(&self, from: f64, to: f64) -> f64 {
        let w = self.window(from, to);
        let sum: f64 = w.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
        (sum / w.len().max(1) as f64).sqrt()
    }

    /// Zero crossings per second between file times `from` and `to`, half of which is the pitch.
    pub fn pitch(&self, from: f64, to: f64) -> f64 {
        let w = self.window(from, to);
        let crossings = w.windows(2).filter(|p| (p[0] < 0) != (p[1] < 0)).count();
        crossings as f64 / 2.0 / (w.len().max(1) as f64 / RATE as f64)
    }

    /// File time of the first sample louder than half scale.
    pub fn first_loud(&self) -> Option<f64> {
        let i = self
            .samples
            .iter()
            .position(|&s| s.unsigned_abs() > 16_000)?;
        Some(self.start as f64 / SECOND as f64 + i as f64 / RATE as f64)
    }
}

#[derive(Debug, Default)]
pub struct Av {
    /// Each video frame's file time and mean luma.
    pub video: Vec<(u64, f64)>,
    pub video_end: u64,
    /// Sound tracks in file order.
    pub sound: Vec<Sound>,
}

impl Av {
    pub fn video_seconds(&self) -> f64 {
        self.video_end as f64 / SECOND as f64
    }
}

#[derive(Default)]
struct Collected {
    video: Vec<(u64, f64)>,
    video_end: u64,
    sound: BTreeMap<usize, Sound>,
}

fn add_sound(sound: &mut Sound, pts: u64, data: &[u8]) {
    if sound.samples.is_empty() {
        sound.start = pts;
    } else {
        let end =
            sound.start + (sound.samples.len() as u128 * SECOND as u128 / RATE as u128) as u64;
        sound.widest_gap = sound.widest_gap.max(pts.abs_diff(end));
    }
    sound.samples.extend(
        data.as_chunks::<2>()
            .0
            .iter()
            .map(|&b| i16::from_le_bytes(b)),
    );
}

/// Decodes every track of the file, the way a player would.
pub fn decode_av(path: &Path) -> Av {
    gst::init().expect("gstreamer");
    let pipeline = gst::Pipeline::new();
    let src = gst::ElementFactory::make("filesrc")
        .property("location", path.to_string_lossy().as_ref())
        .build()
        .expect("filesrc");
    let demux = gst::ElementFactory::make("matroskademux")
        .build()
        .expect("demux");
    pipeline.add_many([&src, &demux]).expect("add");
    src.link(&demux).expect("link");
    let got = Arc::new(Mutex::new(Collected::default()));
    let tracks = Arc::new(AtomicUsize::new(0));
    let weak = pipeline.downgrade();
    let sink_got = got.clone();
    demux.connect_pad_added(move |_, pad| {
        let Some(pipeline) = weak.upgrade() else {
            return;
        };
        let name = pad.name();
        let video = name.starts_with("video");
        let tail = if video {
            "videoconvert ! video/x-raw,format=GRAY8"
        } else {
            "audioconvert ! audioresample \
             ! capsfilter caps=audio/x-raw,format=S16LE,rate=48000,channels=1"
        };
        let desc = format!("{tail} ! appsink name=sink sync=false");
        let bin = gst::parse::bin_from_description(&desc, true).expect("decoder");
        let decode = gst::ElementFactory::make("decodebin")
            .build()
            .expect("decodebin");
        let sink = bin
            .by_name("sink")
            .and_then(|s| s.downcast::<gst_app::AppSink>().ok())
            .expect("appsink");
        let index = if video {
            0
        } else {
            name.trim_start_matches("audio_")
                .parse()
                .expect("audio pad")
        };
        let got = sink_got.clone();
        tracks.fetch_add(1, Ordering::SeqCst);
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let pts = buffer.pts().map_or(0, |t| t.nseconds());
                    let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
                    let mut g = got.lock().expect("lock");
                    if video {
                        let px = map.as_slice();
                        let mean = px.iter().map(|&p| f64::from(p)).sum::<f64>() / px.len() as f64;
                        g.video.push((pts, mean));
                        let end = pts + buffer.duration().map_or(0, |d| d.nseconds());
                        g.video_end = g.video_end.max(end);
                    } else {
                        add_sound(g.sound.entry(index).or_default(), pts, map.as_slice());
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        let input = bin.static_pad("sink").expect("converter input");
        decode.connect_pad_added(move |_, out| {
            let _ = out.link(&input);
        });
        // Each track needs its own thread, or the first decoder to preroll blocks the others.
        let queue = gst::ElementFactory::make("queue").build().expect("queue");
        pipeline
            .add_many([&queue, &decode, bin.upcast_ref()])
            .expect("add decoder");
        queue.link(&decode).expect("link queue");
        for e in [bin.upcast_ref(), &decode, &queue] {
            e.sync_state_with_parent().expect("sync");
        }
        let sinkpad = queue.static_pad("sink").expect("decoder input");
        pad.link(&sinkpad).expect("link decoder");
    });
    pipeline.set_state(gst::State::Playing).expect("play");
    let bus = pipeline.bus().expect("bus");
    let msg = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("the file decodes to the end");
    let _ = pipeline.set_state(gst::State::Null);
    assert_eq!(msg.type_(), gst::MessageType::Eos, "decode: {msg:?}");
    let mut g = got.lock().expect("lock");
    let sound = std::mem::take(&mut g.sound).into_values().collect();
    Av {
        video: std::mem::take(&mut g.video),
        video_end: g.video_end,
        sound,
    }
}

/// A sound server whose source list holds `mic` for the first `polls` looks, and then not.
pub struct Unplugged {
    pub mic: String,
    pub polls: AtomicUsize,
}

impl CommandRunner for Unplugged {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<std::process::Output> {
        use std::os::unix::process::ExitStatusExt as _;
        assert_eq!(program, "pactl");
        let listed = args.iter().any(|a| a == "short")
            && self
                .polls
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok();
        let stdout = if listed {
            format!("7\t{}\tPipeWire\ts16le 2ch 48000Hz\tRUNNING\n", self.mic)
        } else {
            String::new()
        };
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
        })
    }
}
