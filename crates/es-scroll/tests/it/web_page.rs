#![allow(clippy::unwrap_used)]

use es_scroll::{Frame, Push, Stitcher};

const W: usize = 480;
const VIEW: usize = 300;
const PAGE: usize = 1600;
const STEP: usize = 110;
const SECTION: usize = 260;
const BG: [u8; 4] = [14, 16, 22, 255];
const HEAD: [u8; 4] = [120, 180, 240, 255];
const TEXT: [u8; 4] = [210, 210, 215, 255];
const BORDER: [u8; 4] = [130, 150, 200, 255];
const NAV: [u8; 4] = [180, 190, 200, 255];
const NAV_ROWS: std::ops::Range<usize> = 4..16;
const NAV_COLS: std::ops::Range<usize> = 40..440;
const TRACK: [u8; 4] = [30, 30, 34, 255];
const THUMB: [u8; 4] = [90, 90, 96, 255];
const HEADING_FROM: usize = 100;
const TEXT_FROM: usize = 140;
const PICTURE: std::ops::Range<usize> = 140..400;
const LEFT_GLOW: usize = 130;
const RIGHT_GLOW: usize = 370;
const SCROLLBAR: usize = 470;
/// The lowest red any ink has; the glow and the background stay well below it.
const INK_RED: u8 = 110;

fn hash(a: usize, b: usize) -> usize {
    let mut h = (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b as u64);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (h ^ (h >> 32)) as usize
}

/// The page's own ink at a page row: a nav bar across the top, and in each section a heading that starts left of the text, lines of text, and a bordered picture wider than the text.
fn ink(row: usize, x: usize) -> Option<[u8; 4]> {
    if NAV_ROWS.contains(&row) {
        return NAV_COLS.contains(&x).then_some(NAV);
    }
    let (section, r) = (row / SECTION, row % SECTION);
    if (20..44).contains(&r) {
        let end = HEADING_FROM + 120 + hash(section, 1) % 120;
        return ((HEADING_FROM..end).contains(&x) && (x / 6) % 4 != 3).then_some(HEAD);
    }
    if (60..150).contains(&r) {
        let (line, lr) = ((r - 60) / 18, (r - 60) % 18);
        let end = TEXT_FROM + 60 + hash(section * 8 + line, 2) % 200;
        return (lr < 10 && (TEXT_FROM..end).contains(&x) && (x / 5) % 3 != 2).then_some(TEXT);
    }
    if (160..250).contains(&r) && PICTURE.contains(&x) {
        let edge = !(162..248).contains(&r) || !(PICTURE.start + 2..PICTURE.end - 2).contains(&x);
        let shade = 120 + (hash(section * 4096 + r, x / 8) % 120) as u8;
        return Some(if edge { BORDER } else { [200, shade, 60, 255] });
    }
    None
}

/// A glow fixed on the screen behind the side margins, the way a page's decorative background stays put while its content scrolls over it.
fn glow(y: usize, x: usize, glowing: bool) -> [u8; 4] {
    if !glowing || (LEFT_GLOW..RIGHT_GLOW).contains(&x) {
        return BG;
    }
    let amount = 80.0 * (1.0 - y as f64 / VIEW as f64);
    let tint = |base: u8, k: f64| base + (amount * k) as u8;
    [tint(BG[0], 0.3), tint(BG[1], 0.5), tint(BG[2], 1.0), 255]
}

fn view(off: usize, glowing: bool) -> Frame {
    let thumb = off * VIEW / PAGE..(off + VIEW) * VIEW / PAGE;
    let mut data = Vec::with_capacity(W * VIEW * 4);
    for y in 0..VIEW {
        for x in 0..W {
            let px = if x >= SCROLLBAR {
                if thumb.contains(&y) { THUMB } else { TRACK }
            } else {
                ink(off + y, x).unwrap_or_else(|| glow(y, x, glowing))
            };
            data.extend_from_slice(&px);
        }
    }
    Frame::new(W, VIEW, data).unwrap()
}

fn px(f: &Frame, y: usize, x: usize) -> [u8; 4] {
    let i = (y * f.width + x) * 4;
    [f.data[i], f.data[i + 1], f.data[i + 2], f.data[i + 3]]
}

fn offsets() -> Vec<usize> {
    (0..=PAGE - VIEW).step_by(STEP).collect()
}

/// Every pixel of the page's ink in its place, the plain background between the margins as it is, and no ink where the page has none.
fn assert_whole_page(image: &Frame, skip: impl Fn(usize, usize) -> bool) {
    let last = offsets().last().copied().unwrap();
    assert_eq!(image.height, last + VIEW, "every view placed once");
    for y in 0..image.height {
        for x in (0..SCROLLBAR).filter(|&x| !skip(y, x)) {
            let got = px(image, y, x);
            match ink(y, x) {
                Some(want) => assert_eq!(got, want, "ink at page row {y}, column {x}"),
                None if (LEFT_GLOW..RIGHT_GLOW).contains(&x) => {
                    assert_eq!(got, BG, "background at page row {y}, column {x}");
                }
                None => assert!(got[0] < INK_RED, "stray ink at page row {y}, column {x}"),
            }
        }
    }
}

// A heading or picture reaching into a margin that looked still in another pair of frames is still page content, and the nav bar that scrolled away at once never becomes a stripe down the margins.
#[test]
fn content_reaching_into_still_looking_margins_is_kept_whole() {
    let mut st = Stitcher::new();
    for (i, off) in offsets().into_iter().enumerate() {
        let r = st.push(&view(off, true));
        assert!(matches!(r, Push::Appended(_)), "frame {i} at {off}: {r:?}");
    }
    assert_whole_page(&st.finish().unwrap(), |_, _| false);
}

const VIDEO_ROWS: std::ops::Range<usize> = 330..530;
const VIDEO_COLS: std::ops::Range<usize> = 150..350;

fn with_video(off: usize, grab: usize, glowing: bool) -> Frame {
    let mut f = view(off, glowing);
    for row in VIDEO_ROWS.filter(|r| (off..off + VIEW).contains(r)) {
        for x in VIDEO_COLS {
            let i = ((row - off) * W + x) * 4;
            let v = |k| (hash(grab * 1_000_003 + row * W + x, k) % 256) as u8;
            f.data[i..i + 4].copy_from_slice(&[v(1), v(2), v(3), 255]);
        }
    }
    f
}

fn stitch_with_video(glowing: bool) {
    let mut st = Stitcher::new();
    assert_eq!(st.push(&with_video(0, 0, glowing)), Push::Appended(VIEW));
    st.push(&with_video(0, 1, glowing));
    for (i, off) in offsets().into_iter().enumerate().skip(1) {
        let r = st.push(&with_video(off, 1 + i, glowing));
        assert!(matches!(r, Push::Appended(_)), "frame {i} at {off}: {r:?}");
    }
    assert_whole_page(&st.finish().unwrap(), |y, x| {
        VIDEO_ROWS.contains(&y) && VIDEO_COLS.contains(&x)
    });
}

// A video playing on a mostly dark page has more changing edges than the page has still ones.
#[test]
fn a_video_playing_in_the_page_does_not_stop_the_stitch() {
    stitch_with_video(false);
}

// Margins taken for still panels would leave the match only the middle, where the video plays.
#[test]
fn a_video_playing_between_glowing_margins_does_not_stop_the_stitch() {
    stitch_with_video(true);
}

const BLOCK_ROWS: std::ops::Range<usize> = 50..250;

/// A 200 by 200 animation that stays on the same screen rows while the page scrolls under it, with a new picture on every grab.
fn with_animation(off: usize, grab: usize) -> Frame {
    let mut f = view(off, false);
    for y in BLOCK_ROWS {
        for x in VIDEO_COLS {
            let i = (y * W + x) * 4;
            let v = |k| (hash(grab * 1_000_003 + y * W + x, k) % 256) as u8;
            f.data[i..i + 4].copy_from_slice(&[v(1), v(2), v(3), 255]);
        }
    }
    f
}

// The animation covers two thirds of the rows, so leaving its rows out of the match would leave too little; its columns are left out instead.
#[test]
fn an_animation_fixed_on_screen_over_most_rows_does_not_stop_the_stitch() {
    let mut st = Stitcher::new();
    assert_eq!(st.push(&with_animation(0, 0)), Push::Appended(VIEW));
    st.push(&with_animation(0, 1));
    for (i, off) in offsets().into_iter().enumerate().skip(1) {
        let r = st.push(&with_animation(off, 1 + i));
        assert_eq!(r, Push::Appended(STEP), "frame {i} at {off}");
    }
    assert_whole_page(&st.finish().unwrap(), |_, x| VIDEO_COLS.contains(&x));
}

const WIDGET_ROWS: std::ops::Range<usize> = 20..60;
const WIDGET_COLS: std::ops::Range<usize> = 60..110;

/// A button pinned near the top of the screen, over the columns where headings start.
fn with_widget(off: usize) -> Frame {
    let mut f = view(off, false);
    for y in WIDGET_ROWS {
        for x in WIDGET_COLS {
            let i = (y * W + x) * 4;
            let stripe = ((y - WIDGET_ROWS.start) / 8).is_multiple_of(2);
            let px = if stripe {
                [220, 90, 90, 255]
            } else {
                [160, 60, 60, 255]
            };
            f.data[i..i + 4].copy_from_slice(&px);
        }
    }
    f
}

// The pinned button makes its columns look still, but headings pass through some of them, so those keep the page.
#[test]
fn headings_passing_a_pinned_button_are_kept_whole() {
    let mut st = Stitcher::new();
    for (i, off) in offsets().into_iter().enumerate() {
        let r = st.push(&with_widget(off));
        assert!(matches!(r, Push::Appended(_)), "frame {i} at {off}: {r:?}");
    }
    assert_whole_page(&st.finish().unwrap(), |y, x| {
        WIDGET_ROWS.contains(&y) && WIDGET_COLS.contains(&x)
    });
}
