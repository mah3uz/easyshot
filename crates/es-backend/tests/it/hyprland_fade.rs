#![cfg(feature = "hyprland")]

use es_backend::hyprland::{Animation, layer_close, parse_fade};
use std::path::Path;
use std::time::Duration;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hyprland")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn set(name: &str, enabled: bool, speed: f64) -> Animation {
    Animation {
        name: name.into(),
        overridden: true,
        enabled,
        speed,
    }
}

fn inherited(name: &str) -> Animation {
    Animation {
        name: name.into(),
        overridden: false,
        enabled: true,
        speed: 0.0,
    }
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

// Measured on this config: the closed bar was still drawn 235 ms after it closed and gone by 270 ms.
#[test]
fn a_closed_layer_lingers_for_the_longer_of_its_fade_and_its_pop() {
    let got = parse_fade(
        &fixture("getoption-animations.json"),
        &fixture("animations.json"),
    );
    assert_eq!(
        got.map(ms).ok(),
        Some(270),
        "fadeLayersOut 2.7 beats layersOut 2.4"
    );
}

#[test]
fn an_unset_leaf_takes_the_nearest_parent_that_is_set() {
    let list = [
        inherited("layersOut"),
        inherited("layers"),
        inherited("fadeLayersOut"),
        set("fadeLayers", true, 1.5),
        set("fade", true, 9.0),
        set("global", true, 8.0),
    ];
    assert_eq!(
        ms(layer_close(true, &list)),
        800,
        "layersOut falls back to global"
    );
    let list = [
        inherited("layersOut"),
        set("layers", true, 1.0),
        set("fadeLayers", true, 3.0),
    ];
    assert_eq!(
        ms(layer_close(true, &list)),
        300,
        "fadeLayersOut stops at fadeLayers"
    );
}

#[test]
fn nothing_lingers_when_the_compositor_does_not_animate_layers() {
    let list = [
        set("layersOut", false, 2.4),
        set("fadeLayersOut", false, 2.7),
        set("global", true, 8.0),
    ];
    assert_eq!(layer_close(true, &list), Duration::ZERO);
    let list = [set("layersOut", true, 2.4), set("global", true, 8.0)];
    assert_eq!(
        layer_close(false, &list),
        Duration::ZERO,
        "animations:enabled off"
    );
    let off = br#"{"option": "animations:enabled", "int": 0, "set": true }"#;
    assert_eq!(
        parse_fade(off, &fixture("animations.json")).ok(),
        Some(Duration::ZERO)
    );
}

#[test]
fn a_disabled_fade_leaves_the_pop_to_wait_for() {
    let list = [
        set("layersOut", true, 2.4),
        set("fadeLayersOut", false, 2.7),
    ];
    assert_eq!(ms(layer_close(true, &list)), 240);
}

#[test]
fn a_reply_that_is_not_json_is_an_error_not_a_zero_wait() {
    assert!(parse_fade(b"unknown request", &fixture("animations.json")).is_err());
    assert!(parse_fade(&fixture("getoption-animations.json"), b"ok").is_err());
}
