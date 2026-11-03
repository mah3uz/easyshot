use es_scroll::{Exclusions, Frame, Push, Stitcher, shift};

const W: usize = 320;
const LINE: usize = 30;

/// Numbered lines as a terminal shows them: the same width of ink on every line, only the digits move.
fn numbered(lines: usize) -> Frame {
    let mut data = Vec::with_capacity(W * lines * LINE * 4);
    for y in 0..lines * LINE {
        let (n, r) = (y / LINE, y % LINE);
        let tens = 20 + 10 * ((n / 10) % 10);
        let units = 160 + 10 * (n % 10);
        for x in 0..W {
            let ink = (8..22).contains(&r)
                && ((tens..tens + 4).contains(&x) || (units..units + 4).contains(&x));
            data.extend_from_slice(if ink {
                &[230, 230, 230, 255]
            } else {
                &[200, 110, 50, 255]
            });
        }
    }
    Frame::new(W, lines * LINE, data).expect("frame")
}

fn window(page: &Frame, off: usize, h: usize) -> Frame {
    let row = page.width * 4;
    Frame::new(
        page.width,
        h,
        page.data[off * row..(off + h) * row].to_vec(),
    )
    .expect("window")
}

#[test]
fn lines_that_differ_only_in_a_digit_register_at_the_true_shift() {
    let page = numbered(60);
    let (a, b) = (window(&page, 0, 720), window(&page, 90, 720));
    assert_eq!(shift(&a, &b, Exclusions::default()), Some(90));
}

#[test]
fn a_terminal_scrolled_three_lines_at_a_time_stitches_back_exactly() {
    let page = numbered(60);
    let mut st = Stitcher::new();
    for off in (0..=page.height - 720).step_by(90) {
        assert!(matches!(
            st.push(&window(&page, off, 720)),
            Push::Appended(_)
        ));
    }
    let image = st.finish().expect("an image");
    assert_eq!(image.height, 720 + 90 * ((page.height - 720) / 90));
    assert!(image == window(&page, 0, image.height));
}
