#![allow(clippy::unwrap_used)]

use es_scroll::{Frame, Push, Stitcher};

const W: usize = 200;
const VIEW: usize = 200;
const BOX: std::ops::Range<usize> = 60..150;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 24) as u8
    }
}

fn noise(height: usize, seed: u64) -> Frame {
    let mut rng = Rng(seed);
    let mut data = Vec::with_capacity(W * height * 4);
    for _ in 0..W * height {
        data.extend_from_slice(&[rng.next(), rng.next(), rng.next(), 255]);
    }
    Frame::new(W, height, data).unwrap()
}

/// The view at `off`, with a banner on page rows `BOX` that shows a new picture on every grab and scrolls with the page.
fn view(page: &Frame, off: usize, grab: u64) -> Frame {
    let s = W * 4;
    let mut f = Frame::new(W, VIEW, page.data[off * s..(off + VIEW) * s].to_vec()).unwrap();
    let banner = noise(BOX.len(), 1000 + grab);
    for page_row in BOX {
        if (off..off + VIEW).contains(&page_row) {
            let (y, b) = (page_row - off, page_row - BOX.start);
            f.data[y * s..(y + 1) * s].copy_from_slice(&banner.data[b * s..(b + 1) * s]);
        }
    }
    f
}

// A carousel or video near the top of a page scrolls up the screen; masking the screen rows it first covered would leave it in every later match.
#[test]
fn an_animation_that_scrolls_with_the_page_does_not_stop_the_stitch_after_the_first_view() {
    let page = noise(1400, 41);
    let mut st = Stitcher::new();
    assert_eq!(st.push(&view(&page, 0, 0)), Push::Appended(VIEW));
    assert_eq!(st.push(&view(&page, 0, 1)), Push::Duplicate, "unscrolled");
    let offsets = [50, 100, 150, 200, 260, 320];
    for (i, off) in offsets.into_iter().enumerate() {
        let r = st.push(&view(&page, off, 2 + i as u64));
        assert!(matches!(r, Push::Appended(_)), "offset {off}: {r:?}");
    }
    let image = st.finish().unwrap();
    assert_eq!(image.height, 320 + VIEW, "every view placed");
    let s = W * 4;
    for y in (0..image.height).filter(|y| !BOX.contains(y)) {
        assert!(
            image.data[y * s..(y + 1) * s] == page.data[y * s..(y + 1) * s],
            "page row {y} in its place"
        );
    }
}
