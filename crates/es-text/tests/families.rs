#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use es_text::{FontFamily, FontSet, TextEngine, TextStyle};

const NUNITO: &[u8] = include_bytes!("../../../assets/fonts/Nunito[wght].ttf");
const INTER_TIGHT: &[u8] = include_bytes!("../../../assets/fonts/InterTight-Regular.ttf");

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// A bundled font under another name, so the family exists only in the test's font folder.
fn renamed_dir(test: &str, font: &[u8], from: &str, to: &str) -> PathBuf {
    let mut data = font.to_vec();
    for (from, to) in [
        (from.as_bytes().to_vec(), to.as_bytes().to_vec()),
        (utf16(from), utf16(to)),
    ] {
        let mut i = 0;
        while let Some(p) = data[i..].windows(from.len()).position(|w| w == from) {
            data[i + p..i + p + from.len()].copy_from_slice(&to);
            i += p + from.len();
        }
    }
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("es-text-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{to}.ttf")), data).unwrap();
    dir
}

fn pebble_dir(test: &str) -> PathBuf {
    renamed_dir(test, NUNITO, "Nunito", "Pebble")
}

fn outline(e: &mut TextEngine, family: FontFamily) -> es_text::Outline {
    let style = TextStyle {
        family,
        size: 48.0,
        bold: false,
        ..TextStyle::default()
    };
    let layout = e.measure("Hello, World", &style, None);
    (*e.outline(&layout)).clone()
}

#[test]
fn an_installed_family_draws_its_own_faces() {
    let mut e = TextEngine::new(FontSet::bundled().with_dir(&pebble_dir("own")));
    let pebble = FontFamily::System("Pebble".into());
    assert!(e.has_family(&pebble));
    assert_eq!(e.installed_families(), ["Pebble".to_owned()]);
    let drawn = outline(&mut e, pebble);
    assert_eq!(drawn, outline(&mut e, FontFamily::Rounded));
    assert_ne!(drawn, outline(&mut e, FontFamily::Pro));
}

#[test]
fn a_missing_family_draws_in_the_default_bundled_family_and_says_so() {
    let fallback = renamed_dir("missing", INTER_TIGHT, "Inter Tight", "DejaVu Sans");
    let mut e = TextEngine::new(FontSet::bundled().with_dir(&fallback));
    let gone = FontFamily::System("Not Installed Here".into());
    assert!(!e.has_family(&gone));
    assert!(e.has_family(&FontFamily::Compact));
    assert_eq!(outline(&mut e, gone), outline(&mut e, FontFamily::Pro));
}

#[test]
fn the_bundled_set_lists_no_installed_families() {
    let e = TextEngine::bundled();
    assert!(e.installed_families().is_empty());
    assert!(!e.has_family(&FontFamily::System("Pebble".into())));
}
