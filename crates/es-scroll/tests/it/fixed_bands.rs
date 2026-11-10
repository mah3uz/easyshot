#![allow(clippy::unwrap_used)]

use es_scroll::{Frame, Push, Stitcher};

const W: usize = 200;
const VIEW: usize = 300;
const OFFSETS: [usize; 6] = [0, 110, 230, 350, 470, 600];
const OPAQUE: u16 = 255;

fn noise(width: usize, height: usize, mut seed: u64) -> Frame {
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 24) as u8
    };
    let mut data = Vec::with_capacity(width * height * 4);
    for _ in 0..width * height {
        data.extend_from_slice(&[next(), next(), next(), 255]);
    }
    Frame::new(width, height, data).unwrap()
}

fn rows(page: &Frame, from: usize, count: usize) -> Frame {
    let s = page.width * 4;
    Frame::new(
        page.width,
        count,
        page.data[from * s..(from + count) * s].to_vec(),
    )
    .unwrap()
}

fn paint(frame: &mut Frame, at: usize, bar: &Frame, alpha: u16) {
    let start = at * frame.width * 4;
    for (i, &b) in bar.data.iter().enumerate() {
        let under = &mut frame.data[start + i];
        if i % 4 != 3 {
            *under = ((u16::from(b) * alpha + u16::from(*under) * (255 - alpha)) / 255) as u8;
        }
    }
}

struct Bars {
    header: Option<(Frame, u16)>,
    header_from: usize,
    footer: Option<Frame>,
}

impl Bars {
    fn view(&self, page: &Frame, index: usize) -> Frame {
        let mut f = rows(page, OFFSETS[index], VIEW);
        if let Some((header, alpha)) = &self.header
            && index >= self.header_from
        {
            paint(&mut f, 0, header, *alpha);
        }
        if let Some(footer) = &self.footer {
            paint(&mut f, VIEW - footer.height, footer, OPAQUE);
        }
        f
    }

    fn stitch(&self, page: &Frame) -> Frame {
        let mut st = Stitcher::new();
        for i in 0..OFFSETS.len() {
            let r = st.push(&self.view(page, i));
            assert!(matches!(r, Push::Appended(_)), "frame {i}: {r:?}");
        }
        st.finish().unwrap()
    }
}

fn page() -> Frame {
    noise(W, 1200, 21)
}

fn whole_height() -> usize {
    OFFSETS[OFFSETS.len() - 1] + VIEW
}

#[test]
fn an_opaque_header_shown_from_the_start_is_kept_once_at_the_top() {
    let (page, header) = (page(), noise(W, 40, 31));
    let bars = Bars {
        header: Some((header.clone(), OPAQUE)),
        header_from: 0,
        footer: None,
    };
    let mut want = rows(&page, 0, whole_height());
    paint(&mut want, 0, &header, OPAQUE);
    assert!(bars.stitch(&page) == want);
}

// A nav bar that a site reveals once the reader scrolls must neither repeat down the page nor be lost.
#[test]
fn a_header_that_appears_after_the_first_scroll_is_kept_once_at_the_top() {
    let (page, header) = (page(), noise(W, 40, 32));
    let bars = Bars {
        header: Some((header.clone(), OPAQUE)),
        header_from: 1,
        footer: None,
    };
    let mut want = rows(&page, 0, whole_height());
    paint(&mut want, 0, &header, OPAQUE);
    assert!(bars.stitch(&page) == want);
}

#[test]
fn a_translucent_header_keeps_the_first_frames_copy_and_never_repeats() {
    let (page, header) = (page(), noise(W, 40, 33));
    let bars = Bars {
        header: Some((header.clone(), 160)),
        header_from: 0,
        footer: None,
    };
    let mut want = rows(&page, 0, whole_height());
    paint(&mut want, 0, &header, 160);
    assert!(bars.stitch(&page) == want);
}

// Without a clean copy of a translucent bar there is nothing to put at the top, but it must still never land mid-page.
#[test]
fn a_translucent_header_that_appears_late_never_lands_mid_page() {
    let (page, header) = (page(), noise(W, 40, 34));
    let bars = Bars {
        header: Some((header, 160)),
        header_from: 1,
        footer: None,
    };
    assert!(bars.stitch(&page) == rows(&page, 0, whole_height()));
}

#[test]
fn a_fixed_bottom_bar_is_kept_once_at_the_end() {
    let (page, footer) = (page(), noise(W, 30, 35));
    let bars = Bars {
        header: None,
        header_from: 0,
        footer: Some(footer.clone()),
    };
    let mut want = rows(&page, 0, whole_height());
    paint(&mut want, whole_height() - 30, &footer, OPAQUE);
    assert!(bars.stitch(&page) == want);
}

#[test]
fn a_header_and_a_bottom_bar_together_each_appear_once() {
    let (page, header, footer) = (page(), noise(W, 40, 36), noise(W, 30, 37));
    let bars = Bars {
        header: Some((header.clone(), OPAQUE)),
        header_from: 1,
        footer: Some(footer.clone()),
    };
    let mut want = rows(&page, 0, whole_height());
    paint(&mut want, 0, &header, OPAQUE);
    paint(&mut want, whole_height() - 30, &footer, OPAQUE);
    assert!(bars.stitch(&page) == want);
}

// A hover highlight or a late-loading image differs between two grabs of the same rows; the rows placed first stay, so a seam never shows two versions.
#[test]
fn rows_already_placed_are_never_rewritten() {
    let page = page();
    let mut st = Stitcher::new();
    assert_eq!(st.push(&rows(&page, 0, VIEW)), Push::Appended(VIEW));
    let mut second = rows(&page, 110, VIEW);
    paint(&mut second, 100, &noise(W, 10, 38), OPAQUE);
    assert_eq!(st.push(&second), Push::Appended(110));
    assert!(st.finish().unwrap() == rows(&page, 0, VIEW + 110));
}
