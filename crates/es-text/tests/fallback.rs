#![allow(clippy::unwrap_used)]

use es_text::{BUNDLED, FontFamily, FontSet, Outline, TextEngine, TextStyle};

const BENGALI: &[u8] = include_bytes!("../../../tests/fixtures/fonts/FallbackBengali.ttf");
const WORD: &str = "বাংলা";

fn with_bengali() -> TextEngine {
    let fonts = BUNDLED.iter().map(|b| b.to_vec()).chain([BENGALI.to_vec()]);
    TextEngine::new(FontSet::from_data(fonts))
}

fn style(family: FontFamily, bold: bool, italic: bool) -> TextStyle {
    TextStyle {
        family,
        bold,
        italic,
        ..TextStyle::default()
    }
}

fn outline(e: &mut TextEngine, text: &str, style: &TextStyle) -> Outline {
    let layout = e.measure(text, style, None);
    (*e.outline(&layout)).clone()
}

#[test]
fn a_script_the_chosen_family_lacks_draws_from_an_installed_font_that_has_it() {
    let mut e = with_bengali();
    let own = outline(
        &mut e,
        WORD,
        &style(FontFamily::System("Fallback Bengali".into()), false, false),
    );
    assert!(!own.path.elements().is_empty());
    for family in [
        FontFamily::Pro,
        FontFamily::Compact,
        FontFamily::Rounded,
        FontFamily::System("Not Installed Here".into()),
    ] {
        let s = style(family.clone(), false, false);
        assert!(
            !e.measure(WORD, &s, None).has_missing_glyphs(),
            "{family:?}"
        );
        assert_eq!(outline(&mut e, WORD, &s), own, "{family:?}");
    }
}

#[test]
fn bold_and_italic_still_fall_back_to_a_font_with_only_a_regular_face() {
    let mut e = with_bengali();
    for (bold, italic) in [(true, false), (false, true), (true, true)] {
        let layout = e.measure(WORD, &style(FontFamily::Pro, bold, italic), None);
        assert!(!layout.has_missing_glyphs(), "bold {bold} italic {italic}");
    }
}

#[test]
fn latin_and_another_script_share_a_line_without_boxes() {
    let mut e = with_bengali();
    let s = style(FontFamily::Pro, false, false);
    let mixed = e.measure(&format!("Hi {WORD}"), &s, None);
    assert!(!mixed.has_missing_glyphs());
    let parts = e.measure("Hi ", &s, None).glyphs.len() + e.measure(WORD, &s, None).glyphs.len();
    assert_eq!(mixed.glyphs.len(), parts);
}

#[test]
fn a_character_no_font_has_is_reported_missing() {
    let mut e = TextEngine::bundled();
    let s = style(FontFamily::Pro, false, false);
    assert!(e.measure(WORD, &s, None).has_missing_glyphs());
    assert!(!e.measure("Hello", &s, None).has_missing_glyphs());
}
