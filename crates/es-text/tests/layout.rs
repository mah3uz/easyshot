use std::sync::Arc;

use es_text::{FontFamily, TextAlign, TextEngine, TextStyle};
use kurbo::Shape;

const FAMILIES: [FontFamily; 3] = [FontFamily::Pro, FontFamily::Compact, FontFamily::Rounded];

fn style(family: FontFamily, size: f64, bold: bool) -> TextStyle {
    TextStyle {
        family,
        size,
        bold,
        ..TextStyle::default()
    }
}

#[test]
fn line_height_is_135_percent_of_the_font_size() {
    let mut e = TextEngine::bundled();
    for family in FAMILIES {
        for size in [12.0, 24.0, 48.0] {
            let l = e.measure("x", &style(family, size, false), None);
            assert_eq!(l.lines[0].height, size * 1.35);
            assert_eq!(l.size.height, (size * 1.35).ceil());
        }
    }
}

#[test]
fn empty_text_keeps_one_unrounded_line_and_minimum_width() {
    let mut e = TextEngine::bundled();
    let l = e.measure("", &style(FontFamily::Pro, 48.0, true), None);
    assert_eq!((l.size.width, l.size.height), (16.0, 48.0 * 1.35));
    assert_eq!(l.lines.len(), 1);
}

#[test]
fn auto_width_is_ceil_of_widest_line_plus_one() {
    let mut e = TextEngine::bundled();
    let l = e.measure(
        "Hello, World\nhi",
        &style(FontFamily::Pro, 48.0, false),
        None,
    );
    let widest = l.lines.iter().map(|x| x.width).fold(0.0, f64::max);
    assert_eq!(l.size.width, widest.ceil() + 1.0);
    assert_eq!(l.size.height, (2.0 * 48.0 * 1.35_f64).ceil());
}

#[test]
fn golden_auto_widths() {
    let mut e = TextEngine::bundled();
    let golden = [
        (FontFamily::Pro, 12.0, false, 71.0),
        (FontFamily::Pro, 48.0, true, 289.0),
        (FontFamily::Compact, 24.0, false, 125.0),
        (FontFamily::Compact, 48.0, true, 270.0),
        (FontFamily::Rounded, 12.0, true, 73.0),
        (FontFamily::Rounded, 48.0, false, 277.0),
    ];
    for (family, size, bold, width) in golden {
        let l = e.measure("Hello, World", &style(family, size, bold), None);
        assert_eq!(
            (family, size, bold, l.size.width),
            (family, size, bold, width)
        );
    }
}

#[test]
fn wrapped_width_is_floor_of_the_box_and_text_flows_into_lines() {
    let mut e = TextEngine::bundled();
    let text = "Hello world this is wrapped text";
    let l = e.measure(text, &style(FontFamily::Pro, 48.0, false), Some(200.7));
    assert_eq!(l.size.width, 200.0);
    assert_eq!(l.lines.len(), 5);
    assert_eq!(l.size.height, (5.0 * 48.0 * 1.35_f64).ceil());
    assert!(l.lines.iter().all(|x| x.width <= 200.0));
    let narrow = e.measure(text, &style(FontFamily::Pro, 12.0, false), Some(3.0));
    assert_eq!(narrow.size.width, 16.0);
}

#[test]
fn bold_is_wider_than_regular_in_every_family() {
    let mut e = TextEngine::bundled();
    for family in FAMILIES {
        let regular = e.measure("Hello, World", &style(family, 48.0, false), None);
        let bold = e.measure("Hello, World", &style(family, 48.0, true), None);
        assert!(bold.size.width > regular.size.width, "{family:?}");
        let (ro, bo) = (e.outline(&regular), e.outline(&bold));
        assert_ne!(ro.path, bo.path, "{family:?}");
    }
}

#[test]
fn italic_changes_the_glyph_shapes() {
    let mut e = TextEngine::bundled();
    for family in FAMILIES {
        let upright = e.measure("Hxl", &style(family, 48.0, false), None);
        let mut s = style(family, 48.0, false);
        s.italic = true;
        let slanted = e.measure("Hxl", &s, None);
        assert_ne!(
            e.outline(&upright).path,
            e.outline(&slanted).path,
            "{family:?}"
        );
    }
}

#[test]
fn underline_sits_below_the_baseline_on_every_nonempty_line() {
    let mut e = TextEngine::bundled();
    let mut s = style(FontFamily::Pro, 48.0, false);
    s.underline = true;
    let l = e.measure("Hi\n\nthere", &s, None);
    let o = e.outline(&l);
    assert_eq!(o.underlines.len(), 2);
    for (rect, line) in o.underlines.iter().zip([&l.lines[0], &l.lines[2]]) {
        assert!(rect.y0 > line.baseline);
        assert!(rect.y1 <= line.top + line.height);
        assert_eq!((rect.x0, rect.width()), (line.x, line.width));
        assert!(rect.height() >= 1.0);
    }
    s.underline = false;
    let plain = e.measure("Hi\n\nthere", &s, None);
    assert!(e.outline(&plain).underlines.is_empty());
}

#[test]
fn outlines_are_not_empty_and_stay_inside_the_measured_box() {
    let mut e = TextEngine::bundled();
    for family in FAMILIES {
        for align in [TextAlign::Start, TextAlign::Middle, TextAlign::End] {
            let mut s = style(family, 36.0, true);
            s.align = align;
            for wrap in [None, Some(180.0)] {
                let l = e.measure("Quick brown fox\njumps over", &s, wrap);
                let b = e.outline(&l).path.bounding_box();
                assert!(b.area() > 0.0);
                assert!(b.x0 >= -3.6 && b.y0 >= 0.0, "{family:?} {align:?} {b:?}");
                assert!(
                    b.x1 <= l.size.width + 3.6 && b.y1 <= l.size.height,
                    "{family:?} {align:?} {b:?}"
                );
            }
        }
    }
}

#[test]
fn alignment_positions_lines_inside_the_box() {
    let mut e = TextEngine::bundled();
    let mut s = style(FontFamily::Pro, 24.0, false);
    let text = "wide line here\nhi";
    let mut at = |e: &mut TextEngine, a| {
        s.align = a;
        e.measure(text, &s, None)
    };
    let start = at(&mut e, TextAlign::Start);
    let middle = at(&mut e, TextAlign::Middle);
    let end = at(&mut e, TextAlign::End);
    assert_eq!(start.size.width, end.size.width);
    assert_eq!(start.lines[1].x, 0.0);
    let w = start.size.width;
    assert_eq!(middle.lines[1].x, (w - middle.lines[1].width) / 2.0);
    assert_eq!(end.lines[1].x + end.lines[1].width, w);
}

#[test]
fn repeated_measure_hits_the_cache_and_returns_the_same_layout() {
    let mut e = TextEngine::bundled();
    let s = style(FontFamily::Compact, 30.0, false);
    let a = e.measure("cached", &s, None);
    let b = e.measure("cached", &s, None);
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(e.cached_layouts(), 1);
    let o1 = e.outline(&a);
    let o2 = e.outline(&b);
    assert!(Arc::ptr_eq(&o1, &o2));
    let c = e.measure("cached", &s, Some(100.0));
    assert!(!Arc::ptr_eq(&a, &c));
}

#[test]
fn cache_evicts_the_least_recently_used_layout() {
    let mut e = TextEngine::bundled();
    let s = style(FontFamily::Pro, 12.0, false);
    let first = e.measure("0", &s, None);
    for i in 1..512 {
        e.measure(&i.to_string(), &s, None);
    }
    e.measure("0", &s, None);
    e.measure("512", &s, None);
    assert_eq!(e.cached_layouts(), 512);
    assert!(Arc::ptr_eq(&first, &e.measure("0", &s, None)));
}

#[test]
fn caret_and_hit_test_round_trip() {
    let mut e = TextEngine::bundled();
    let l = e.measure("abc\ndef", &style(FontFamily::Pro, 24.0, false), None);
    for i in [0, 1, 2, 3, 4, 5, 6, 7] {
        let (line, x) = l.caret_x(i);
        let y = l.lines[line].top + 1.0;
        assert_eq!(l.hit_test(x, y), i, "index {i}");
    }
    assert_eq!(l.caret_x(4).0, 1);
    assert_eq!(l.caret_x(4).1, l.lines[1].x);
}
