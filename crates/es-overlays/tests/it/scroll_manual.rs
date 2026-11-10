use es_overlays::capture::scroll::{self, Command, Outcome, Params, Source};
use es_scroll::Frame;
use std::ops::Range;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const W: usize = 240;
const VIEW: usize = 300;
const PAGE: usize = 2400;
const SECTION: usize = 260;
const BG: [u8; 4] = [250, 250, 250, 255];
const INK: [u8; 4] = [30, 30, 40, 255];
const NOTCH: f64 = 100.0;
const GLIDE_MS: f64 = 180.0;

fn hash(a: usize, b: usize) -> usize {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h ^ (h >> 32)) as usize
}

/// A plain web page: on a light background, each section has a heading, lines of text with gaps between them, and a picture.
fn pixel(row: usize, x: usize) -> [u8; 4] {
    let (section, r) = (row / SECTION, row % SECTION);
    if (20..44).contains(&r) {
        let end = 20 + 60 + hash(section, 1) % 100;
        if (20..end).contains(&x) && (x / 6) % 4 != 3 {
            return INK;
        }
    }
    if (60..150).contains(&r) {
        let (line, lr) = ((r - 60) / 18, (r - 60) % 18);
        let end = 20 + 40 + hash(section * 8 + line, 2) % 160;
        if lr < 10 && (20..end).contains(&x) && !hash(section * 8 + line, x / 4).is_multiple_of(3) {
            return INK;
        }
    }
    if (160..250).contains(&r) && (40..200).contains(&x) {
        let shade = 60 + (hash(section * 4096 + r, x / 8) % 160) as u8;
        return [shade, 120, 200 - shade / 2, 255];
    }
    BG
}

fn page() -> Vec<u8> {
    (0..PAGE)
        .flat_map(|y| (0..W).flat_map(move |x| pixel(y, x)))
        .collect()
}

/// The user scrolls the page with the wheel: each notch glides `NOTCH` rows down over `GLIDE_MS`, the way a browser smooth-scrolls.
struct Manual {
    page: Vec<u8>,
    notches: Vec<u64>,
    /// Screen rows and columns where something plays in place, fixed on screen.
    animation: Option<(Range<usize>, Range<usize>)>,
    start: Instant,
    elapsed: Duration,
    stop_at: u64,
    commands: mpsc::Sender<Command>,
    grabs: usize,
}

impl Manual {
    fn offset_at(&self, ms: u64) -> usize {
        let moved: f64 = self
            .notches
            .iter()
            .filter(|&&t| t <= ms)
            .map(|&t| {
                let k = ((ms - t) as f64 / GLIDE_MS).min(1.0);
                NOTCH * (1.0 - (1.0 - k).powi(3))
            })
            .sum();
        (moved.round() as usize).min(PAGE - VIEW)
    }

    fn final_offset(&self) -> usize {
        self.offset_at(u64::from(u32::MAX))
    }
}

impl Source for Manual {
    fn grab(&mut self) -> Option<Frame> {
        self.grabs += 1;
        let o = self.offset_at(self.elapsed.as_millis() as u64);
        let mut data = self.page[o * W * 4..(o + VIEW) * W * 4].to_vec();
        if let Some((rows, cols)) = self.animation.clone() {
            for y in rows {
                for x in cols.clone() {
                    let i = (y * W + x) * 4;
                    let v = (hash(self.grabs * 7919 + y, x) % 256) as u8;
                    data[i..i + 4].copy_from_slice(&[v, 255 - v, v / 2, 255]);
                }
            }
        }
        Frame::new(W, VIEW, data)
    }

    fn scroll(&mut self, _: i32) -> bool {
        panic!("manual mode never scrolls for the user")
    }

    fn pointer_inside(&self) -> bool {
        true
    }

    fn sleep(&mut self, d: Duration) {
        let before = self.elapsed.as_millis() as u64;
        self.elapsed += d;
        if before < self.stop_at && self.elapsed.as_millis() as u64 >= self.stop_at {
            let _ = self.commands.send(Command::Stop);
        }
    }

    fn now(&self) -> Instant {
        self.start + self.elapsed
    }
}

fn capture(notches: Vec<u64>, animation: Option<(Range<usize>, Range<usize>)>) -> (Manual, Frame) {
    let (tx, rx) = mpsc::channel();
    let stop_at = notches.last().copied().unwrap_or(0) + 1500;
    let mut src = Manual {
        page: page(),
        notches,
        animation,
        start: Instant::now(),
        elapsed: Duration::ZERO,
        stop_at,
        commands: tx,
        grabs: 0,
    };
    let params = Params {
        auto_available: false,
        preview: (50, 200),
        max_height: es_scroll::MAX_HEIGHT,
    };
    let outcome = scroll::run(&mut src, &rx, params, &mut |_| {});
    let Outcome::Done(image) = outcome else {
        panic!("stop ends with an image, got {outcome:?}");
    };
    (src, image)
}

/// A second of stillness, then notches at uneven times: two close together, three in a quick burst, and four in one fast spin that moves more than the area holds within a sixth of a second.
const NOTCHES: [u64; 10] = [1200, 1290, 1900, 2600, 2640, 2700, 3500, 3520, 3540, 3560];

// The user scrolls at their own speed, so frames must be taken often enough that consecutive ones still overlap; one lost overlap leaves every later frame unplaced.
#[test]
fn manual_scroll_with_uneven_gliding_notches_after_a_pause_joins_the_whole_way() {
    let (src, image) = capture(NOTCHES.to_vec(), None);
    let end = src.final_offset();
    assert!(end > 2 * VIEW, "the page moved well past the first view");
    assert_eq!(image.height, end + VIEW, "every view placed once");
    assert!(
        image.data == src.page[..(end + VIEW) * W * 4],
        "every row in its place"
    );
}

// A page playing an animation while the user has not scrolled yet must teach the stitcher where it plays, or the scroll that follows matches the animation instead of the page.
#[test]
fn manual_scroll_over_a_page_with_an_animation_joins_the_page_around_it() {
    let (rows, cols) = (40..100, 40..120);
    let (src, image) = capture(NOTCHES[..6].to_vec(), Some((rows, cols.clone())));
    let end = src.final_offset();
    assert_eq!(image.height, end + VIEW, "every view placed once");
    for y in 0..image.height {
        for x in (0..W).filter(|x| !cols.contains(x)) {
            let i = (y * W + x) * 4;
            assert_eq!(
                image.data[i..i + 4],
                src.page[i..i + 4],
                "page row {y}, column {x}"
            );
        }
    }
}
