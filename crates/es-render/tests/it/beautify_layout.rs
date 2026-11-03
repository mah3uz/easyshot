#![allow(clippy::unwrap_used)]

use es_model::{Alignment, AspectRatio, BackgroundColor, BackgroundSettings, BackgroundStyle};
use es_render::beautify::layout::layout;

fn staged() -> BackgroundSettings {
    BackgroundSettings {
        style: BackgroundStyle::Solid(BackgroundColor::default()),
        ..BackgroundSettings::default()
    }
}

const ALIGNMENTS: [Alignment; 9] = [
    Alignment::TopLeading,
    Alignment::Top,
    Alignment::TopTrailing,
    Alignment::Leading,
    Alignment::Center,
    Alignment::Trailing,
    Alignment::BottomLeading,
    Alignment::Bottom,
    Alignment::BottomTrailing,
];

#[test]
fn without_a_background_the_canvas_is_the_image() {
    let l = layout(640.0, 480.0, &BackgroundSettings::default());
    assert_eq!((l.canvas_width, l.canvas_height), (640.0, 480.0));
    assert_eq!(l.image, l.card);
    assert_eq!(l.image_radii, [0.0; 4]);
}

#[test]
fn padding_is_a_fraction_of_the_shortest_edge_on_every_side() {
    let s = BackgroundSettings {
        padding: 0.1,
        ..staged()
    };
    let l = layout(1000.0, 500.0, &s);
    assert_eq!(l.padding, 50.0);
    assert_eq!((l.canvas_width, l.canvas_height), (1100.0, 600.0));
    assert_eq!((l.image.x0, l.image.y0), (50.0, 50.0));
}

#[test]
fn every_aspect_ratio_expands_the_canvas_without_cropping_the_image() {
    let ratios = [
        (AspectRatio::Square, 1.0),
        (AspectRatio::FourThree, 4.0 / 3.0),
        (AspectRatio::ThreeTwo, 3.0 / 2.0),
        (AspectRatio::SixteenNine, 16.0 / 9.0),
    ];
    for (aspect, ratio) in ratios {
        for (w, h) in [(1000.0, 500.0), (500.0, 1000.0), (800.0, 800.0)] {
            let s = BackgroundSettings {
                aspect_ratio: aspect,
                ..staged()
            };
            let l = layout(w, h, &s);
            let no_ratio = layout(w, h, &staged());
            assert!((l.canvas_width / l.canvas_height - ratio).abs() < 1e-9);
            assert!(l.canvas_width >= no_ratio.canvas_width);
            assert!(l.canvas_height >= no_ratio.canvas_height);
            assert!((l.image.width() - w).abs() < 1e-6 && (l.image.height() - h).abs() < 1e-6);
            assert!(l.image.x0 >= 0.0 && l.image.y0 >= 0.0);
            assert!(l.image.x1 <= l.canvas_width && l.image.y1 <= l.canvas_height);
        }
    }
}

#[test]
fn a_wide_image_grows_taller_for_a_square_canvas_and_stays_centred() {
    let s = BackgroundSettings {
        aspect_ratio: AspectRatio::Square,
        padding: 0.0,
        ..staged()
    };
    let l = layout(1000.0, 500.0, &s);
    assert_eq!((l.canvas_width, l.canvas_height), (1000.0, 1000.0));
    assert_eq!(l.image.y0, 250.0);
}

#[test]
fn stuck_edges_get_no_padding_and_the_image_touches_them() {
    for a in ALIGNMENTS {
        let s = BackgroundSettings {
            alignment: a,
            padding: 0.1,
            ..staged()
        };
        let l = layout(1000.0, 500.0, &s);
        let (w, h) = (l.canvas_width, l.canvas_height);
        assert!(l.image.x0 >= 0.0 && l.image.x1 <= w && l.image.y0 >= 0.0 && l.image.y1 <= h);
        let expect_w = 1000.0 + if a.x_factor() == 0.5 { 100.0 } else { 50.0 };
        let expect_h = 500.0 + if a.y_factor() == 0.5 { 100.0 } else { 50.0 };
        assert_eq!((w, h), (expect_w, expect_h), "{a:?}");
        if a.sticks_to_top() {
            assert_eq!(l.image.y0, 0.0);
        }
        if a.sticks_to_bottom() {
            assert_eq!(l.image.y1, h);
        }
        if a.sticks_to_leading() {
            assert_eq!(l.image.x0, 0.0);
        }
        if a.sticks_to_trailing() {
            assert_eq!(l.image.x1, w);
        }
    }
}

#[test]
fn corner_radius_scales_with_the_image_and_is_square_on_stuck_edges() {
    let s = staged();
    let small = layout(400.0, 300.0, &s);
    let big = layout(800.0, 600.0, &s);
    assert_eq!(small.image_radii, [0.018 * 300.0; 4]);
    assert_eq!(big.image_radii[0], 2.0 * small.image_radii[0]);

    let s = BackgroundSettings {
        alignment: Alignment::TopLeading,
        ..staged()
    };
    let l = layout(400.0, 300.0, &s);
    assert_eq!(l.image_radii, [0.0, 0.0, 0.018 * 300.0, 0.0]);
}

#[test]
fn a_border_grows_the_card_and_rounds_the_ring_concentrically() {
    let mut s = staged();
    s.border.is_enabled = true;
    let l = layout(1000.0, 500.0, &s);
    let b = 0.012 * 500.0;
    assert_eq!(l.border_width, b);
    assert_eq!(l.card.width(), 1000.0 + 2.0 * b);
    assert_eq!(l.image.x0 - l.card.x0, b);
    assert_eq!(l.card_radii[0], l.image_radii[0] + b);
}

#[test]
fn a_border_alone_adds_only_the_ring() {
    let mut s = BackgroundSettings::default();
    s.border.is_enabled = true;
    let l = layout(1000.0, 500.0, &s);
    assert_eq!(l.padding, 0.0);
    assert_eq!(l.canvas_width, 1000.0 + 2.0 * 6.0);
}
