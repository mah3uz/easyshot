use es_overlays::capture::scroll::{self, Command, Outcome, Params, Source, Update};
use es_scroll::Frame;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const W: usize = 64;
const VIEW: usize = 120;

fn shade(n: usize) -> u8 {
    let mut h = (n as u32).wrapping_mul(0x9E37_79B9) ^ 0x5bd1_e995;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    (h >> 24) as u8
}

/// A page whose every row has its own brightness, so any overlap registers at one place only.
fn page(rows: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(rows * W * 4);
    for y in 0..rows {
        let base = shade(y);
        for x in 0..W {
            let v = base.wrapping_add((x / 8) as u8 * 3);
            data.extend_from_slice(&[v, v / 2, 255 - v, 255]);
        }
    }
    data
}

/// A scrolling app seen through the area: wheel input moves it, or a script moves it over time.
struct Fake {
    page: Vec<u8>,
    rows: usize,
    offset: usize,
    script: Vec<(u64, usize)>,
    start: Instant,
    elapsed: Duration,
    commands: mpsc::Sender<Command>,
    at: Vec<(u64, Command)>,
    flicker: bool,
    blink: bool,
    /// How many rows one wheel click moves this app.
    per_click: usize,
    /// An offset whose content shows nothing of the page the first time the app stops there.
    unready: Option<usize>,
    grabs: usize,
    scrolls: usize,
    /// Where each scroll left the app.
    offsets: Vec<usize>,
    sleeps: Vec<Duration>,
    /// Smooth scrolling: a scroll starts moving the page this many ms later and glides over the second number.
    glide: Option<(u64, u64)>,
    glide_from: usize,
    glide_to: usize,
    glide_at: u64,
}

impl Fake {
    fn new(rows: usize, commands: mpsc::Sender<Command>) -> Self {
        Fake {
            page: page(rows),
            rows,
            offset: 0,
            script: Vec::new(),
            start: Instant::now(),
            elapsed: Duration::ZERO,
            commands,
            at: Vec::new(),
            flicker: false,
            blink: false,
            per_click: 15,
            unready: None,
            grabs: 0,
            scrolls: 0,
            offsets: Vec::new(),
            sleeps: Vec::new(),
            glide: None,
            glide_from: 0,
            glide_to: 0,
            glide_at: 0,
        }
    }

    fn rows(&self, from: usize, n: usize) -> Vec<u8> {
        self.page[from * W * 4..(from + n) * W * 4].to_vec()
    }
}

impl Source for Fake {
    fn grab(&mut self) -> Option<Frame> {
        self.grabs += 1;
        let ms = self.elapsed.as_millis() as u64;
        if let Some(&(_, o)) = self.script.iter().rev().find(|(t, _)| *t <= ms) {
            self.offset = o;
        }
        if let Some((delay, dur)) = self.glide {
            let t = ms.saturating_sub(self.glide_at + delay);
            let k = (t as f64 / dur as f64).min(1.0);
            let (a, b) = (self.glide_from as f64, self.glide_to as f64);
            self.offset = (a + (b - a) * k).round() as usize;
        }
        let mut data = self.rows(self.offset, VIEW);
        if self.flicker {
            data.rotate_left(self.grabs % VIEW * W * 4);
        }
        if self.blink {
            data[0] = self.grabs as u8;
        }
        if self.unready == Some(self.offset) {
            data.reverse();
        }
        Frame::new(W, VIEW, data)
    }

    fn scroll(&mut self, clicks: i32) -> bool {
        self.scrolls += 1;
        if self.unready == Some(self.offset) {
            self.unready = None;
        }
        let moved = clicks as isize * self.per_click as isize;
        let target = self
            .offset
            .saturating_add_signed(moved)
            .min(self.rows - VIEW);
        if self.glide.is_some() {
            self.glide_from = self.offset;
            self.glide_to = target;
            self.glide_at = self.elapsed.as_millis() as u64;
        } else {
            self.offset = target;
        }
        self.offsets.push(target);
        true
    }

    fn sleep(&mut self, d: Duration) {
        self.sleeps.push(d);
        self.elapsed += d;
        let ms = self.elapsed.as_millis() as u64;
        let due: Vec<Command> = self
            .at
            .iter()
            .filter(|(t, _)| *t <= ms)
            .map(|(_, c)| *c)
            .collect();
        self.at.retain(|(t, _)| *t > ms);
        for c in due {
            let _ = self.commands.send(c);
        }
    }

    fn now(&self) -> Instant {
        self.start + self.elapsed
    }
}

fn params(max_height: usize) -> Params {
    Params {
        auto_available: true,
        preview: (50, 200),
        max_height,
    }
}

fn run(fake: &mut Fake, rx: &mpsc::Receiver<Command>, max_height: usize) -> (Outcome, Vec<Update>) {
    let mut updates = Vec::new();
    let outcome = scroll::run(fake, rx, params(max_height), &mut |u| updates.push(u));
    (outcome, updates)
}

fn heights(updates: &[Update]) -> Vec<usize> {
    updates
        .iter()
        .filter_map(|u| match u {
            Update::Height(h) => Some(*h),
            _ => None,
        })
        .collect()
}

#[test]
fn auto_scroll_runs_to_the_end_of_the_page_and_stitches_it_exactly() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(1000, tx.clone());
    tx.send(Command::Auto(true)).expect("queued");
    let (outcome, updates) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    let Outcome::Done(image) = outcome else {
        panic!("auto scroll ends with an image, got {outcome:?}");
    };
    assert_eq!(image.height, 1000, "the whole page, top to bottom, once");
    assert!(image.data == fake.page, "every row in its place");
    let to_bottom = (1000 - VIEW).div_ceil(45);
    assert_eq!(
        fake.scrolls,
        to_bottom + 8,
        "it gives up after eight steps that add nothing"
    );
    assert!(updates.contains(&Update::Auto(true)));
    assert_eq!(heights(&updates).last(), Some(&1000));
}

// Chromium shows the old page for a few frames before it starts to glide; two grabs of it must not read as the page having settled.
#[test]
fn auto_scroll_waits_for_a_smooth_scroll_to_start_before_settling() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(1000, tx.clone());
    fake.glide = Some((60, 150));
    tx.send(Command::Auto(true)).expect("queued");
    let (outcome, updates) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    let Outcome::Done(image) = outcome else {
        panic!("auto scroll ends with an image, got {outcome:?}");
    };
    assert!(image.data == fake.page, "every row in its place");
    let to_bottom = (1000 - VIEW).div_ceil(45);
    assert_eq!(
        fake.scrolls,
        to_bottom + 8,
        "every step appended its full shift, none read as a duplicate"
    );
    let h = heights(&updates);
    assert!(
        h.windows(2).all(|w| w[1] > w[0]),
        "each step grew the image: {h:?}"
    );
}

fn auto_capture(per_click: usize) -> (Fake, Outcome) {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(1000, tx.clone());
    fake.per_click = per_click;
    tx.send(Command::Auto(true)).expect("queued");
    let (outcome, _) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    (fake, outcome)
}

// One click moves a browser or terminal about a tenth of a typical area; a step sized in points jumped past the area entirely.
#[test]
fn auto_scroll_learns_how_far_a_click_moves_and_never_jumps_past_the_area() {
    let (fake, outcome) = auto_capture(40);
    let Outcome::Done(image) = outcome else {
        panic!("auto scroll ends with an image, got {outcome:?}");
    };
    assert_eq!(
        image.height, 1000,
        "more than the first view: the whole page"
    );
    assert!(image.data == fake.page, "every row in its place");
}

#[test]
fn auto_scroll_reaches_the_end_of_an_app_that_moves_one_row_per_click() {
    let (fake, outcome) = auto_capture(1);
    let Outcome::Done(image) = outcome else {
        panic!("auto scroll ends with an image, got {outcome:?}");
    };
    assert_eq!(
        image.height, 1000,
        "tiny clicks are not taken for the end of the page"
    );
    assert!(image.data == fake.page);
}

// A page still loading or animating can fail one match; scrolling on from there would leave too little overlap for every later step.
#[test]
fn a_step_that_finds_no_overlap_is_retried_nearer_never_further() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(1000, tx.clone());
    fake.unready = Some(105);
    tx.send(Command::Auto(true)).expect("queued");
    let (outcome, _) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    let Outcome::Done(image) = outcome else {
        panic!("auto scroll ends with an image, got {outcome:?}");
    };
    assert!(image.data == fake.page, "every row in its place");
    let failed = fake
        .offsets
        .iter()
        .position(|&o| o == 105)
        .expect("reached");
    let (anchor, retry) = (fake.offsets[failed - 1], fake.offsets[failed + 1]);
    assert!(
        (anchor + 1..105).contains(&retry),
        "after the failed step the app goes back between the last placed frame ({anchor}) and 105, went to {retry}"
    );
    assert!(
        fake.offsets.windows(2).all(|w| w[1] <= w[0] + 45),
        "no step goes further than one step: {:?}",
        fake.offsets
    );
}

#[test]
fn manual_capture_follows_the_page_and_stop_keeps_the_last_small_move() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(600, tx);
    fake.script = vec![(0, 0), (300, 40), (600, 80), (900, 120), (1200, 125)];
    fake.at = vec![(1700, Command::Stop)];
    let (outcome, updates) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    let Outcome::Done(image) = outcome else {
        panic!("stop ends with an image, got {outcome:?}");
    };
    assert_eq!(
        heights(&updates),
        vec![VIEW, VIEW + 40, VIEW + 80, VIEW + 120, VIEW + 125],
        "a 5-row move is too small mid-capture and is kept only by the final frame"
    );
    assert_eq!(image.height, VIEW + 125);
    assert!(image.data == fake.rows(0, VIEW + 125));
    assert_eq!(fake.scrolls, 0, "manual mode never scrolls for the user");
}

#[test]
fn an_area_that_never_settles_is_retried_with_growing_pauses_then_taken() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(600, tx.clone());
    fake.flicker = true;
    tx.send(Command::Cancel).expect("queued");
    let (outcome, updates) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(
        fake.grabs, 8,
        "seven tries within 200 ms, the last frame is used, then one more shows what animates in place"
    );
    let ms: Vec<u128> = fake.sleeps.iter().map(|d| d.as_millis()).collect();
    assert_eq!(ms, vec![10, 15, 22, 33, 50, 75]);
    assert_eq!(heights(&updates), vec![VIEW]);
}

// Each tick waits for the area to settle, so content that animates in place must not hold it up.
#[test]
fn a_pixel_changing_in_place_does_not_keep_the_area_from_settling() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(600, tx.clone());
    fake.blink = true;
    tx.send(Command::Cancel).expect("queued");
    let (outcome, _) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(fake.grabs, 3, "two grabs settle, then one more");
}

#[test]
fn the_preview_shows_every_grab_even_when_nothing_is_added() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(600, tx.clone());
    fake.blink = true;
    fake.at = vec![(1000, Command::Cancel)];
    let (_, updates) = run(&mut fake, &rx, es_scroll::MAX_HEIGHT);
    let previews: Vec<&Frame> = updates
        .iter()
        .filter_map(|u| match u {
            Update::Preview(f) => Some(f),
            _ => None,
        })
        .collect();
    assert_eq!(heights(&updates), vec![VIEW], "nothing scrolled");
    assert!(
        previews.len() >= 3,
        "one preview per grab pushed, got {}",
        previews.len()
    );
    assert!(
        previews.windows(2).any(|w| w[0] != w[1]),
        "the preview follows the pixel that changes"
    );
}

#[test]
fn the_height_limit_ends_the_capture() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(1000, tx.clone());
    tx.send(Command::Auto(true)).expect("queued");
    let (outcome, _) = run(&mut fake, &rx, 300);
    let Outcome::Done(image) = outcome else {
        panic!("a full image is still an image, got {outcome:?}");
    };
    assert_eq!(image.height, 300);
    assert!(image.data == fake.rows(0, 300));
}

#[test]
fn auto_scroll_is_refused_without_a_virtual_pointer() {
    let (tx, rx) = mpsc::channel();
    let mut fake = Fake::new(600, tx.clone());
    tx.send(Command::Auto(true)).expect("queued");
    fake.at = vec![(600, Command::Stop)];
    let mut updates = Vec::new();
    let p = Params {
        auto_available: false,
        ..params(es_scroll::MAX_HEIGHT)
    };
    scroll::run(&mut fake, &rx, p, &mut |u| updates.push(u));
    assert_eq!(fake.scrolls, 0);
    assert!(updates.contains(&Update::Auto(false)));
}

#[test]
fn the_preview_fits_its_box_and_keeps_the_shape() {
    let tall = Frame::new(W, 1000, page(1000)).expect("frame");
    let p = scroll::fit(&tall, (50, 200));
    assert!(p.width <= 50 && p.height <= 200);
    assert_eq!((p.width, p.height), (13, 200), "the height is what limits");
    let small = Frame::new(W, VIEW, page(VIEW)).expect("frame");
    let p = scroll::fit(&small, (500, 500));
    assert_eq!((p.width, p.height), (W, VIEW), "never enlarged");
}
