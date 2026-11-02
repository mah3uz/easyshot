#![allow(dead_code, clippy::unwrap_used)]
use es_geom::Rect;
use es_model::platform::OutputId;
use es_record::{Clock, Codec, Container, Quality, RecordSource, RecordSpec, Timing};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub struct FakeClock {
    virtual_now: Mutex<Duration>,
}

impl FakeClock {
    pub fn new() -> Self {
        Self {
            virtual_now: Mutex::new(Duration::ZERO),
        }
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Duration {
        *self.virtual_now.lock().unwrap()
    }

    fn sleep(&self, d: Duration) {
        *self.virtual_now.lock().unwrap() += d;
        std::thread::sleep(d / 10);
    }

    fn monotonic_us(&self) -> u64 {
        self.now().as_micros() as u64 + 1_000_000
    }
}

pub fn timing() -> Timing {
    Timing::default()
}

pub fn fake(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fakes")
        .join(name)
}

pub fn spec(dir: &Path, source: RecordSource) -> RecordSpec {
    RecordSpec {
        source,
        fps: 60,
        codec: Codec::H264,
        container: Container::Mkv,
        quality: Quality::VeryHigh,
        system_audio: false,
        mic: None,
        cursor: true,
        dir: dir.join("rec.easyshotrec"),
    }
}

pub fn output(name: &str) -> RecordSource {
    RecordSource::Output(OutputId(name.into()))
}

pub fn region(name: &str, x: f64, y: f64, w: f64, h: f64) -> RecordSource {
    RecordSource::Region {
        output: OutputId(name.into()),
        logical: Rect::new(x, y, x + w, y + h),
    }
}

pub fn read_log(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

pub fn pid_in(log: &[String]) -> u32 {
    log.iter()
        .find_map(|l| l.strip_prefix("pid "))
        .unwrap()
        .parse()
        .unwrap()
}

pub fn process_gone(pid: u32) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}
