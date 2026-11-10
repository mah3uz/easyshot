//! Replays recorded frames (`NNNN-<unix ms>.ppm`) through the manual scroll loop on a virtual clock.
use es_overlays::capture::scroll::{self, Command, Outcome, Params, Source, Update};
use es_scroll::Frame;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct Replay {
    frames: Vec<(u64, PathBuf)>,
    start: Instant,
    elapsed: Duration,
    stop_at: u64,
    commands: mpsc::Sender<Command>,
}

fn read_ppm(path: &PathBuf) -> Option<Frame> {
    let bytes = std::fs::read(path).ok()?;
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&bytes[s..i]).to_string());
    }
    i += 1;
    let (w, h): (usize, usize) = (fields[1].parse().ok()?, fields[2].parse().ok()?);
    let rgb = &bytes[i..i + w * h * 3];
    let data = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    Frame::new(w, h, data)
}

impl Source for Replay {
    fn grab(&mut self) -> Option<Frame> {
        let now = self.frames[0].0 + self.elapsed.as_millis() as u64;
        let idx = self
            .frames
            .iter()
            .rposition(|(t, _)| *t <= now)
            .unwrap_or(0);
        read_ppm(&self.frames[idx].1)
    }
    fn scroll(&mut self, _: i32) -> bool {
        false
    }
    fn sleep(&mut self, d: Duration) {
        self.elapsed += d;
        let now = self.frames[0].0 + self.elapsed.as_millis() as u64;
        if now >= self.stop_at {
            let _ = self.commands.send(Command::Stop);
        }
    }
    fn now(&self) -> Instant {
        self.start + self.elapsed
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("EASYSHOT_LOG")
                .unwrap_or_else(|_| "es_overlays=debug,es_scroll=debug".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().expect("frame dir"));
    let out = PathBuf::from(args.next().expect("out png"));
    let mut frames: Vec<(u64, PathBuf)> = std::fs::read_dir(&dir)
        .expect("dir")
        .filter_map(|e| {
            let p = e.ok()?.path();
            let stem = p.file_stem()?.to_str()?.to_owned();
            let ms = stem.split('-').nth(1)?.parse().ok()?;
            Some((ms, p))
        })
        .collect();
    frames.sort();
    let (tx, rx) = mpsc::channel();
    let stop_at = frames.last().map(|f| f.0).unwrap_or(0) + 600;
    let mut src = Replay {
        frames,
        start: Instant::now(),
        elapsed: Duration::ZERO,
        stop_at,
        commands: tx,
    };
    let params = Params {
        auto_available: false,
        preview: (50, 200),
        max_height: es_scroll::MAX_HEIGHT,
    };
    let outcome = scroll::run(&mut src, &rx, params, &mut |u| {
        if let Update::Height(h) = u {
            eprintln!("height {h}");
        }
    });
    let Outcome::Done(img) = outcome else {
        eprintln!("outcome {outcome:?}");
        return;
    };
    eprintln!("final {}x{}", img.width, img.height);
    image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.data)
        .expect("image")
        .save(&out)
        .expect("save");
}
