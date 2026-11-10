#![allow(clippy::unwrap_used)]

use es_scroll::{Frame, Push, Stitcher};

const W: usize = 320;
const VIEW: usize = 300;
const LINE: usize = 20;
const LINES: usize = 60;
const MINIMAP: usize = 250;
const CODE_BG: [u8; 4] = [30, 30, 30, 255];
const INK: [u8; 4] = [200, 200, 120, 255];
const INK_AS_BRIGHT: [u8; 4] = [123, 200, 149, 255];
const MAP_BG: [u8; 4] = [45, 45, 45, 255];
const SLIDER: [u8; 4] = [80, 80, 80, 255];
const MAP_INK: [u8; 4] = [150, 150, 90, 255];
const OFFSETS: [usize; 9] = [0, 110, 220, 330, 440, 550, 660, 770, 880];

// Five kinds of line repeat, as a run of similar statements does, so the page looks the same one block of lines further on.
const TOKENS: [&[(usize, usize)]; 5] = [
    &[(16, 40), (64, 80), (150, 30)],
    &[(32, 60), (100, 100)],
    &[(32, 24), (64, 40), (110, 24), (140, 60)],
    &[(48, 120)],
    &[(16, 10)],
];

fn inked(line: usize, x: usize) -> bool {
    line < LINES
        && TOKENS[line % 5]
            .iter()
            .any(|&(s, l)| (s..s + l).contains(&x))
}

/// `coloured` draws every other block of lines in a syntax colour as bright as the first, so the blocks differ in their pixels but not in their brightness.
fn code_px(page_row: usize, x: usize, coloured: bool) -> [u8; 4] {
    let (line, r) = (page_row / LINE, page_row % LINE);
    if (6..14).contains(&r) && inked(line, x) {
        if coloured && (line / TOKENS.len()) % 2 == 1 {
            INK_AS_BRIGHT
        } else {
            INK
        }
    } else {
        CODE_BG
    }
}

/// The minimap scrolls an eighth as fast as the code and a slider marks the lines in view.
fn map_px(off: usize, y: usize, x: usize) -> [u8; 4] {
    let m = y + off / 8;
    let slider = (off / 4..off / 4 + 75).contains(&y);
    if m.is_multiple_of(2) && x >= MINIMAP + 4 && inked(m / 2, (x - MINIMAP - 4) * 4) {
        MAP_INK
    } else if slider {
        SLIDER
    } else {
        MAP_BG
    }
}

fn view(off: usize, coloured: bool) -> Frame {
    let mut data = Vec::with_capacity(W * VIEW * 4);
    for y in 0..VIEW {
        for x in 0..W {
            data.extend_from_slice(&if x < MINIMAP {
                code_px(off + y, x, coloured)
            } else {
                map_px(off, y, x)
            });
        }
    }
    Frame::new(W, VIEW, data).unwrap()
}

fn px(f: &Frame, y: usize, x: usize) -> [u8; 4] {
    let i = (y * f.width + x) * 4;
    [f.data[i], f.data[i + 1], f.data[i + 2], f.data[i + 3]]
}

fn stitch(coloured: bool, prior: bool) -> Frame {
    let mut st = Stitcher::new();
    for (i, &off) in OFFSETS.iter().enumerate() {
        let f = view(off, coloured);
        let r = match i {
            0 => st.push(&f),
            _ if prior => st.push_near(&f, off - OFFSETS[i - 1]),
            _ => st.push(&f),
        };
        assert_eq!(
            r,
            if i == 0 {
                Push::Appended(VIEW)
            } else {
                Push::Appended(110)
            },
            "frame {i} at {off}"
        );
    }
    st.finish().unwrap()
}

// Code repeats one block of lines further on, so only the distance the scroll was sent can tell the true shift from one a block shorter.
#[test]
fn repeating_code_lines_register_at_the_shift_the_wheel_sent() {
    assert_code(&stitch(false, true), false);
}

// Without a prior, as when the user scrolls by hand, only the colours tell a block of lines from the same shapes a block further on.
#[test]
fn coloured_code_lines_register_by_their_pixels_when_no_scroll_was_sent() {
    assert_code(&stitch(true, false), true);
}

fn assert_code(image: &Frame, coloured: bool) {
    let total = OFFSETS[OFFSETS.len() - 1] + VIEW;
    assert_eq!(
        image.height, total,
        "every frame placed once, none overlapping"
    );
    for y in 0..total {
        for x in 0..MINIMAP {
            let want = code_px(y, x, coloured);
            assert_eq!(px(image, y, x), want, "code at row {y}, column {x}");
        }
    }
}

// A minimap shows where the view is, not the page; copied with each frame it would run down the whole image.
#[test]
fn a_wide_minimap_is_kept_from_the_first_view_and_never_repeats() {
    let image = stitch(false, true);
    let first = view(0, false);
    for x in MINIMAP..W {
        for y in 0..VIEW {
            assert_eq!(px(&image, y, x), px(&first, y, x), "first view, row {y}");
        }
        for y in VIEW..image.height {
            assert_eq!(
                px(&image, y, x),
                MAP_BG,
                "below the first view, row {y}, column {x}"
            );
        }
    }
}
