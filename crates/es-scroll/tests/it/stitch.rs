#![allow(clippy::unwrap_used)]

use es_scroll::{Exclusions, Frame, Push, Stitcher, shift};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 24) as u8
    }
}

fn noise_page(width: usize, height: usize, seed: u64) -> Frame {
    let mut rng = Rng(seed);
    let mut data = Vec::with_capacity(width * height * 4);
    for _ in 0..width * height {
        data.extend_from_slice(&[rng.next(), rng.next(), rng.next(), 255]);
    }
    Frame::new(width, height, data).unwrap()
}

fn window(page: &Frame, off: usize, h: usize) -> Frame {
    let s = page.width * 4;
    Frame::new(page.width, h, page.data[off * s..(off + h) * s].to_vec()).unwrap()
}

fn stamp_header(frame: &mut Frame, header: &Frame) {
    let n = header.data.len();
    frame.data[..n].copy_from_slice(&header.data);
}

fn paint_cols(frame: &mut Frame, from: usize, mut px: impl FnMut(usize) -> [u8; 4]) {
    for y in 0..frame.height {
        for x in from..frame.width {
            let i = (y * frame.width + x) * 4;
            frame.data[i..i + 4].copy_from_slice(&px(y));
        }
    }
}

fn left_cols(frame: &Frame, cols: usize) -> Vec<u8> {
    (0..frame.height)
        .flat_map(|y| frame.data[y * frame.width * 4..y * frame.width * 4 + cols * 4].to_vec())
        .collect()
}

fn push_all(st: &mut Stitcher, page: &Frame, h: usize, offs: &[usize]) {
    for &o in offs {
        let r = st.push(&window(page, o, h));
        assert!(matches!(r, Push::Appended(_)), "offset {o}: {r:?}");
    }
}

#[test]
fn windows_stitch_back_to_the_page_exactly() {
    let page = noise_page(200, 1400, 1);
    let mut st = Stitcher::new();
    let offs = [0, 120, 260, 400, 530, 700, 900];
    push_all(&mut st, &page, 300, &offs);
    assert_eq!(st.height(), 1200);
    assert_eq!(st.finish().unwrap(), window(&page, 0, 1200));
}

#[test]
fn fixed_header_appears_once() {
    let page = noise_page(200, 1400, 2);
    let header = noise_page(200, 40, 99);
    let frame = |off: usize| {
        let mut f = window(&page, off, 300);
        stamp_header(&mut f, &header);
        f
    };
    let mut st = Stitcher::new();
    for off in [0, 100, 230, 380, 500] {
        assert!(matches!(st.push(&frame(off)), Push::Appended(_)));
    }
    let mut expected = window(&page, 0, 800);
    stamp_header(&mut expected, &header);
    assert_eq!(st.height(), 800);
    assert_eq!(st.finish().unwrap(), expected);
}

#[test]
fn header_exclusion_is_what_makes_headed_frames_match() {
    let page = noise_page(200, 800, 3);
    let header = noise_page(200, 120, 98);
    let stamped = |off| {
        let mut f = window(&page, off, 300);
        stamp_header(&mut f, &header);
        f
    };
    let (a, b) = (stamped(0), stamped(100));
    assert_eq!(
        shift(
            &a,
            &b,
            Exclusions {
                top: 120,
                bottom: 0,
                right: 0
            }
        ),
        Some(100)
    );
    assert_ne!(shift(&a, &b, Exclusions::default()), Some(100));
}

#[test]
fn scrollbar_columns_are_ignored_once_detected() {
    let page = noise_page(160, 1400, 4);
    let thumb = |off: usize| {
        move |y: usize| {
            let top = 80 + off / 4;
            if (top..top + 80).contains(&y) {
                [20, 20, 20, 255]
            } else {
                [220, 220, 220, 255]
            }
        }
    };
    let mut rng = Rng(77);
    let mut st = Stitcher::new();
    for (i, off) in [0, 120, 250, 380, 520, 660].into_iter().enumerate() {
        let mut f = window(&page, off, 300);
        if i < 2 {
            paint_cols(&mut f, 144, thumb(off));
        } else {
            paint_cols(&mut f, 144, |_| [rng.next(), rng.next(), rng.next(), 255]);
        }
        assert!(matches!(st.push(&f), Push::Appended(_)), "frame {i}");
    }
    let out = st.finish().unwrap();
    assert_eq!(out.height, 960);
    assert_eq!(left_cols(&out, 144), left_cols(&window(&page, 0, 960), 144));
}

#[test]
fn noisy_right_columns_match_only_when_excluded() {
    let page = noise_page(100, 700, 5);
    let mut rng = Rng(5);
    let mut noisy = |off| {
        let mut f = window(&page, off, 300);
        paint_cols(&mut f, 80, |_| [rng.next(), rng.next(), rng.next(), 255]);
        f
    };
    let (a, b) = (noisy(0), noisy(110));
    assert_eq!(
        shift(
            &a,
            &b,
            Exclusions {
                top: 0,
                bottom: 0,
                right: 20
            }
        ),
        Some(110)
    );
    assert_eq!(shift(&a, &b, Exclusions::default()), None);
}

#[test]
fn identical_frames_are_duplicates() {
    let page = noise_page(120, 600, 6);
    let mut st = Stitcher::new();
    let f = window(&page, 0, 300);
    assert_eq!(st.push(&f), Push::Appended(300));
    assert_eq!(st.push(&f), Push::Duplicate);
    assert_eq!(st.height(), 300);
}

#[test]
fn unrelated_frame_has_no_overlap() {
    let mut st = Stitcher::new();
    st.push(&window(&noise_page(120, 600, 7), 0, 300));
    let other = window(&noise_page(120, 600, 8), 0, 300);
    assert_eq!(st.push(&other), Push::NoOverlap);
    assert_eq!(st.height(), 300);
}

#[test]
fn different_frame_size_has_no_overlap() {
    let page = noise_page(120, 600, 9);
    let mut st = Stitcher::new();
    st.push(&window(&page, 0, 300));
    assert_eq!(st.push(&window(&page, 0, 200)), Push::NoOverlap);
}

#[test]
fn height_is_capped() {
    let page = noise_page(100, 1400, 10);
    let mut st = Stitcher::with_max_height(700);
    push_all(&mut st, &page, 300, &[0, 150, 300]);
    assert_eq!(st.push(&window(&page, 450, 300)), Push::Full);
    assert_eq!(st.push(&window(&page, 600, 300)), Push::Full);
    assert_eq!(st.finish().unwrap(), window(&page, 0, 700));
}

#[test]
fn flat_pages_never_match() {
    let flat = |v: u8| Frame::new(100, 300, vec![v; 100 * 300 * 4]).unwrap();
    assert_eq!(shift(&flat(10), &flat(200), Exclusions::default()), None);
    let mut st = Stitcher::new();
    st.push(&flat(10));
    assert_eq!(st.push(&flat(200)), Push::NoOverlap);
}

#[test]
fn small_scrolls_wait_unless_final() {
    let page = noise_page(120, 600, 11);
    let mut st = Stitcher::new();
    st.push(&window(&page, 0, 300));
    assert_eq!(st.push(&window(&page, 10, 300)), Push::Duplicate);
    assert_eq!(st.push_final(&window(&page, 10, 300)), Push::Appended(10));
    assert_eq!(st.finish().unwrap(), window(&page, 0, 310));
}

#[test]
fn banded_document_scrolled_to_its_end() {
    let (w, rows) = (160, 600);
    let mut rng = Rng(12);
    let mut data = Vec::new();
    for y in 0..rows {
        let red = ((y / 30) * 37 + 17) % 200 + 20;
        for x in 0..w {
            let text = if x > 35 && x < 120 && rng.next() > 200 {
                255
            } else {
                0
            };
            data.extend_from_slice(&[76, 51, (red as u8).max(text), 255]);
        }
    }
    let page = Frame::new(w, rows, data).unwrap();
    let mut st = Stitcher::new();
    push_all(&mut st, &page, 300, &[0, 100, 200, 300]);
    assert_eq!(st.push(&window(&page, 300, 300)), Push::Duplicate);
    assert_eq!(st.finish().unwrap(), page);
}

fn animate(frame: &mut Frame, rows: std::ops::Range<usize>, seed: u64) {
    let mut rng = Rng(seed);
    for y in rows {
        let shade = rng.next();
        for x in 0..frame.width {
            let i = (y * frame.width + x) * 4;
            let v = shade.wrapping_add((x * 3) as u8);
            frame.data[i..i + 4].copy_from_slice(&[v, 255 - v, v / 2, 255]);
        }
    }
}

// A web page's animation stays on the same screen rows while the page scrolls under it.
#[test]
fn an_animation_fixed_on_screen_does_not_stop_the_page_from_stitching() {
    let page = noise_page(200, 1400, 9);
    let mut st = Stitcher::new();
    let steps = [
        (0, Push::Appended(200)),
        (0, Push::Duplicate),
        (60, Push::Appended(60)),
        (130, Push::Appended(70)),
        (190, Push::Appended(60)),
    ];
    for (i, (off, want)) in steps.into_iter().enumerate() {
        let mut f = window(&page, off, 200);
        animate(&mut f, 50..110, 100 + i as u64);
        assert_eq!(st.push(&f), want, "step {i}, offset {off}");
    }
    assert_eq!(st.height(), 390);
    let canvas = st.image().unwrap();
    let s = page.width * 4;
    assert!(
        canvas.data[300 * s..390 * s] == page.data[300 * s..390 * s]
            && canvas.data[..50 * s] == page.data[..50 * s],
        "the rows clear of the animation are the page's own"
    );
}

#[test]
fn an_animation_alone_is_not_a_scroll() {
    let page = noise_page(200, 400, 10);
    let mut st = Stitcher::new();
    for i in 0..3 {
        let mut f = window(&page, 0, 200);
        animate(&mut f, 50..110, 200 + i);
        let r = st.push(&f);
        assert!(
            matches!(r, Push::Appended(200) | Push::Duplicate),
            "frame {i}: {r:?}"
        );
    }
    assert_eq!(st.height(), 200);
}

fn animate_box(
    frame: &mut Frame,
    rows: std::ops::Range<usize>,
    cols: std::ops::Range<usize>,
    seed: u64,
) {
    let mut rng = Rng(seed);
    for y in rows {
        for x in cols.clone() {
            let i = (y * frame.width + x) * 4;
            frame.data[i..i + 4].copy_from_slice(&[rng.next(), rng.next(), rng.next(), 255]);
        }
    }
}

// A box that moves on screen can cover most rows of the area at once, so leaving whole rows out of the match is not enough.
#[test]
fn a_box_animating_over_most_rows_does_not_stop_the_page_from_stitching() {
    let page = noise_page(200, 1400, 11);
    let mut st = Stitcher::new();
    let steps = [
        (0, Push::Appended(200)),
        (60, Push::Appended(60)),
        (130, Push::Appended(70)),
        (220, Push::Appended(90)),
    ];
    for (i, (off, want)) in steps.into_iter().enumerate() {
        let mut f = window(&page, off, 200);
        let left = 40 + 10 * i;
        animate_box(&mut f, 20..180, left..left + 70, 300 + i as u64);
        assert_eq!(st.push(&f), want, "step {i}, offset {off}");
    }
    assert_eq!(st.height(), 420);
    let canvas = st.image().unwrap();
    assert_eq!(
        left_cols(canvas, 30),
        left_cols(&window(&page, 0, 420), 30),
        "the columns clear of the box are the page's own"
    );
}

#[test]
fn shift_robust_finds_the_shift_where_most_edges_reappear() {
    let page = noise_page(200, 800, 12);
    let (mut a, mut b) = (window(&page, 0, 200), window(&page, 80, 200));
    animate_box(&mut a, 20..180, 60..140, 1);
    animate_box(&mut b, 20..180, 80..160, 2);
    assert_eq!(
        shift(&a, &b, Exclusions::default()),
        None,
        "the whole-area match fails"
    );
    assert_eq!(
        es_scroll::shift_robust(&a, &b, Exclusions::default()),
        Some(80)
    );
    let other = noise_page(200, 200, 13);
    assert_eq!(
        es_scroll::shift_robust(&a, &other, Exclusions::default()),
        None,
        "unrelated frames share no edges"
    );
}

const OFFS: [usize; 6] = [0, 120, 250, 380, 520, 660];

// The thumb sits at a different row in each frame, so copying the strip down the image would repeat it.
#[test]
fn a_moving_scrollbar_thumb_is_cropped_off_the_saved_image() {
    let page = noise_page(160, 1400, 4);
    let mut st = Stitcher::new();
    for (i, off) in OFFS.into_iter().enumerate() {
        let mut f = window(&page, off, 300);
        paint_cols(&mut f, 144, |y| {
            let top = 40 + off / 4;
            if (top..top + 60).contains(&y) {
                [20, 20, 20, 255]
            } else {
                [220, 220, 220, 255]
            }
        });
        assert!(matches!(st.push(&f), Push::Appended(_)), "frame {i}");
    }
    let out = st.finish().unwrap();
    assert_eq!((out.width, out.height), (144, 960));
    assert_eq!(left_cols(&out, 144), left_cols(&window(&page, 0, 960), 144));
}

#[test]
fn a_static_side_strip_is_kept_as_the_first_view_showed_it() {
    let page = noise_page(160, 1400, 4);
    let strip = |y: usize| [(y * 7 % 251) as u8, 90, (y % 13) as u8 * 10, 255];
    let mut st = Stitcher::new();
    for off in OFFS {
        let mut f = window(&page, off, 300);
        paint_cols(&mut f, 136, strip);
        st.push(&f);
    }
    let out = st.finish().unwrap();
    assert_eq!(out.width, 160);
    let at = (10 * 160 + 140) * 4;
    assert_eq!(out.data[at..at + 4], strip(10));
}

#[test]
fn a_page_without_a_scrollbar_keeps_its_width() {
    let page = noise_page(160, 1400, 4);
    let mut st = Stitcher::new();
    push_all(&mut st, &page, 300, &OFFS);
    let out = st.finish().unwrap();
    assert_eq!((out.width, out.height), (160, 960));
}
