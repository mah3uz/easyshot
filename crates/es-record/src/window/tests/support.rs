use super::super::{Encoder, FrameSource, SourceEvent, WindowRecorder};
use crate::{Codec, Container, Error, Quality, RecordSource, RecordSpec, RecorderEvent};
use es_model::platform::{WindowId, WindowInfo};
use es_model::{PixelFormat, VideoFrame};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SECOND: u64 = 1_000_000_000;

pub enum Step {
    /// Frames of `size` at `rate` per second for `span`, each a different light gray.
    Frames {
        size: (u32, u32),
        rate: u32,
        span: Duration,
    },
    /// Dark frames at 60 per second for `span`, white for 200 ms from the pipeline clock time
    /// `at` holds once it is set, sent at that moment.
    Flash {
        size: (u32, u32),
        at: Arc<AtomicU64>,
        span: Duration,
    },
    Stop,
}

pub const DARK: u8 = 16;
pub const WHITE: u8 = 235;
const FLASH: u64 = 200_000_000;

pub fn clock_now() -> u64 {
    gst::init().expect("gstreamer");
    gst::SystemClock::obtain().time().nseconds()
}

fn sleep_until(t: u64) {
    std::thread::sleep(Duration::from_nanos(t.saturating_sub(clock_now())));
}

pub struct Feed {
    rx: Receiver<SourceEvent>,
    _open: Sender<SourceEvent>,
    dropped: Arc<AtomicBool>,
}

impl FrameSource for Feed {
    fn recv_timeout(&mut self, timeout: Duration) -> Option<SourceEvent> {
        self.rx.recv_timeout(timeout).ok()
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

fn frame((w, h): (u32, u32), shade: u8) -> VideoFrame {
    VideoFrame {
        width: w,
        height: h,
        stride: w * 4,
        format: PixelFormat::Xrgb8888,
        data: vec![shade; (w * h * 4) as usize].into(),
        presented: 0,
    }
}

/// A window that plays `steps` on its own thread and stays open after them until dropped.
pub fn feed(steps: Vec<Step>) -> (Feed, Arc<AtomicBool>) {
    let (tx, rx) = channel();
    let dropped = Arc::new(AtomicBool::new(false));
    let producer = tx.clone();
    std::thread::spawn(move || {
        let mut i = 0u32;
        for step in steps {
            match step {
                Step::Frames { size, rate, span } => {
                    let start = Instant::now();
                    let period = Duration::from_secs(1) / rate;
                    let mut n = 0;
                    while start.elapsed() < span {
                        let shade = 100 + (i % 150) as u8;
                        if producer
                            .send(SourceEvent::Frame(frame(size, shade)))
                            .is_err()
                        {
                            return;
                        }
                        i += 1;
                        n += 1;
                        let next = start + period * n;
                        std::thread::sleep(next.saturating_duration_since(Instant::now()));
                    }
                }
                Step::Flash { size, at, span } => {
                    let end = clock_now() + span.as_nanos() as u64;
                    let mut next = clock_now();
                    while next < end {
                        let t = at.load(Ordering::SeqCst);
                        if t != 0 && next < t && t < next + SECOND / 60 {
                            next = t;
                        }
                        sleep_until(next);
                        let lit = t != 0 && (t..t + FLASH).contains(&next);
                        let shade = if lit { WHITE } else { DARK };
                        if producer
                            .send(SourceEvent::Frame(frame(size, shade)))
                            .is_err()
                        {
                            return;
                        }
                        next += SECOND / 60;
                    }
                }
                Step::Stop => {
                    let _ = producer.send(SourceEvent::Stopped);
                    return;
                }
            }
        }
    });
    let feed = Feed {
        rx,
        _open: tx,
        dropped: dropped.clone(),
    };
    (feed, dropped)
}

/// Encoders under test: `EASYSHOT_TEST_ENCODERS` (element names, comma separated), else x264enc,
/// which runs on any machine.
pub fn test_encoders() -> Vec<Encoder> {
    let wanted = std::env::var("EASYSHOT_TEST_ENCODERS").unwrap_or_else(|_| "x264enc".into());
    let picked: Vec<_> = Encoder::ORDER
        .into_iter()
        .filter(|e| wanted.split(',').any(|w| w.trim() == e.element()))
        .collect();
    assert!(!picked.is_empty(), "no known encoder in {wanted}");
    picked
}

pub fn recorder(feed: Feed, encoders: &[Encoder]) -> WindowRecorder {
    let feed = Mutex::new(Some(feed));
    WindowRecorder::new(move |_| {
        let f = feed.lock().ok().and_then(|mut f| f.take());
        f.map(|f| Box::new(f) as Box<dyn FrameSource>)
            .ok_or(Error::Unsupported)
    })
    .with_encoders(encoders)
}

pub fn spec(dir: &Path, fps: u32) -> RecordSpec {
    RecordSpec {
        source: RecordSource::Window(WindowInfo {
            id: WindowId("synthetic".into()),
            app_id: "synthetic".into(),
            title: "synthetic".into(),
            pid: None,
            logical: None,
            output: None,
            workspace: None,
            floating: false,
            focus_rank: None,
        }),
        fps,
        codec: Codec::H264,
        container: Container::Mkv,
        quality: Quality::High,
        system_audio: false,
        mic: None,
        cursor: false,
        dir: dir.to_path_buf(),
    }
}

pub fn wait_for(events: &async_channel::Receiver<RecorderEvent>, want: &RecorderEvent) -> bool {
    let until = Instant::now() + Duration::from_secs(10);
    while Instant::now() < until {
        match events.try_recv() {
            Ok(e) if &e == want => return true,
            Ok(_) => {}
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    false
}

#[derive(Debug, Default)]
pub struct Decoded {
    pub frames: u64,
    pub sizes: Vec<(u32, u32)>,
    /// End of the last frame, in nanoseconds of file time.
    pub duration: u64,
    /// The luma plane of the frame shown at the `grab` times, in order.
    pub grabbed: Vec<Vec<u8>>,
}

impl Decoded {
    pub fn seconds(&self) -> f64 {
        self.duration as f64 / SECOND as f64
    }
}

/// Decodes the whole file, the way a player would, and counts what it shows.
pub fn decode(path: &Path, grab: &[u64]) -> Decoded {
    gst::init().expect("gstreamer");
    let pipeline = gst::parse::launch(
        "filesrc name=src ! decodebin ! videoconvert ! video/x-raw,format=GRAY8 \
         ! appsink name=sink sync=false",
    )
    .expect("decode pipeline")
    .downcast::<gst::Pipeline>()
    .expect("pipeline");
    let src = pipeline.by_name("src").expect("filesrc");
    src.set_property("location", path.to_string_lossy().as_ref());
    let sink = pipeline
        .by_name("sink")
        .and_then(|s| s.downcast::<gst_app::AppSink>().ok())
        .expect("appsink");
    pipeline.set_state(gst::State::Playing).expect("play");
    let mut d = Decoded::default();
    let mut wanted = grab.iter().copied().peekable();
    while let Ok(sample) = sink.pull_sample() {
        let s = sample.caps().and_then(|c| c.structure(0)).expect("caps");
        let size = (
            s.get::<i32>("width").expect("width") as u32,
            s.get::<i32>("height").expect("height") as u32,
        );
        if d.sizes.last() != Some(&size) {
            d.sizes.push(size);
        }
        let buf = sample.buffer().expect("buffer");
        let pts = buf.pts().map(|t| t.nseconds()).unwrap_or(0);
        let end = pts + buf.duration().map(|t| t.nseconds()).unwrap_or(0);
        d.frames += 1;
        d.duration = d.duration.max(end);
        while wanted.peek().is_some_and(|&t| t < end) {
            wanted.next();
            let map = buf.map_readable().expect("map");
            d.grabbed
                .push(map.as_slice()[..(size.0 * size.1) as usize].to_vec());
        }
    }
    let error = pipeline
        .bus()
        .and_then(|b| b.pop_filtered(&[gst::MessageType::Error]));
    let _ = pipeline.set_state(gst::State::Null);
    assert!(error.is_none(), "the file decodes: {error:?}");
    d
}

/// Mean luma of column `x` of a `width`-wide frame.
pub fn column(frame: &[u8], width: u32, x: u32) -> f64 {
    let rows = frame.len() / width as usize;
    let sum: u64 = (0..rows)
        .map(|y| u64::from(frame[y * width as usize + x as usize]))
        .sum();
    sum as f64 / rows as f64
}

pub fn spans_seconds(segments: &[(u64, u64)]) -> f64 {
    segments.iter().map(|(a, b)| b - a).sum::<u64>() as f64 / 1e6
}
