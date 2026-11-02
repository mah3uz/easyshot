use es_anno::{BundledText, GlyphOutliner, TextMeasure};
use es_model::shape::{FontFamily, TextProps};
use kurbo::Shape;

fn props(text: &str) -> TextProps {
    TextProps {
        text: text.into(),
        font_size: 32.0,
        ..TextProps::default()
    }
}

#[test]
fn glyphs_sit_inside_the_box_the_editor_hit_tests() {
    let text = BundledText::bundled();
    let p = props("Hello\nworld");
    let size = text.measure(&p);
    let bounds = text.outline(&p).glyphs.bounding_box();
    assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    assert!(bounds.x0 >= -0.5 && bounds.y0 >= -0.5, "{bounds:?}");
    assert!(
        bounds.x1 <= size.width + 0.5 && bounds.y1 <= size.height + 0.5,
        "{bounds:?} in {size:?}"
    );
}

#[test]
fn a_wrap_width_applies_only_when_auto_size_is_off() {
    let text = BundledText::bundled();
    let long = "a sentence long enough to wrap at a narrow width";
    let auto = text.measure(&props(long));
    let wrapped = text.measure(&TextProps {
        auto_size: false,
        w: 120.0,
        ..props(long)
    });
    assert!(wrapped.width <= 120.0 && wrapped.height > auto.height);
    let ignored = text.measure(&TextProps {
        auto_size: true,
        w: 120.0,
        ..props(long)
    });
    assert_eq!(ignored, auto);
}

#[test]
fn font_family_and_weight_reach_the_engine() {
    let text = BundledText::bundled();
    let pro = text.measure(&props("Rounded"));
    let rounded = text.measure(&TextProps {
        font_family: FontFamily::Rounded,
        ..props("Rounded")
    });
    let regular = text.measure(&TextProps {
        is_bold: false,
        ..props("Rounded")
    });
    assert_ne!(pro.width, rounded.width);
    assert_ne!(pro.width, regular.width);
}

#[test]
fn underline_rects_come_back_only_when_asked() {
    let text = BundledText::bundled();
    assert!(text.outline(&props("plain")).underline.is_empty());
    let underlined = TextProps {
        is_underline: true,
        ..props("under")
    };
    assert!(!text.outline(&underlined).underline.is_empty());
}
