#![allow(clippy::unwrap_used)]

use es_record::input::{KeyFilter, Keymap};
use xkbcommon::xkb;

const ESC: u16 = 1;
const KEY_1: u16 = 2;
const KEY_2: u16 = 3;
const KEY_7: u16 = 8;
const KEY_0: u16 = 11;
const BACKSPACE: u16 = 14;
const TAB: u16 = 15;
const KEY_Q: u16 = 16;
const KEY_E: u16 = 18;
const KEY_T: u16 = 20;
const KEY_U: u16 = 22;
const KEY_P: u16 = 25;
const ENTER: u16 = 28;
const CTRL: u16 = 29;
const KEY_A: u16 = 30;
const KEY_D: u16 = 32;
const KEY_J: u16 = 36;
const KEY_L: u16 = 38;
const SHIFT: u16 = 42;
const KEY_C: u16 = 46;
const COMMA: u16 = 51;
const ALT: u16 = 56;
const SPACE: u16 = 57;
const F5: u16 = 63;
const KP_7: u16 = 71;
const RIGHT_CTRL: u16 = 97;
const RIGHT_ALT: u16 = 100;
const UP: u16 = 103;
const SUPER: u16 = 125;

/// Built from the system's XKB rules, as text with the trailing NUL the compositor sends.
fn keymap_text(layout: &str, options: &str) -> String {
    let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let map = xkb::Keymap::new_from_names(
        &ctx,
        "evdev",
        "pc105",
        layout,
        "",
        Some(options.to_owned()),
        xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .unwrap_or_else(|| panic!("xkeyboard-config cannot build {layout} {options}"));
    let mut text = map.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
    text.push('\0');
    text
}

pub fn us_text() -> String {
    keymap_text("us", "")
}

fn keymap(layout: &str, options: &str) -> Keymap {
    Keymap::from_text(&keymap_text(layout, options)).unwrap()
}

fn tap(code: u16) -> [(u16, bool); 2] {
    [(code, true), (code, false)]
}

fn chord(mods: &[u16], codes: &[u16]) -> Vec<(u16, bool)> {
    let mut seq: Vec<_> = mods.iter().map(|&m| (m, true)).collect();
    for &c in codes {
        seq.extend(tap(c));
    }
    seq.extend(mods.iter().rev().map(|&m| (m, false)));
    seq
}

fn stored(filter: &mut KeyFilter, seq: &[(u16, bool)]) -> Vec<String> {
    seq.iter()
        .filter_map(|&(code, down)| filter.feed(0, code, down))
        .map(|(mods, key)| format!("{}{key}", mods.concat()))
        .collect()
}

fn us() -> KeyFilter {
    KeyFilter::new(Some(keymap("us", "")))
}

#[test]
fn plain_and_shifted_typing_is_never_stored() {
    let mut seq = Vec::new();
    for code in [KEY_P, KEY_A, KEY_1, COMMA, SPACE, KP_7] {
        seq.extend(tap(code));
    }
    seq.extend(chord(&[SHIFT], &[KEY_Q, KEY_1]));
    assert_eq!(stored(&mut us(), &seq), Vec::<String>::new());
    let mut de = KeyFilter::new(Some(keymap("de", "")));
    let altgr = chord(&[RIGHT_ALT], &[KEY_Q, KEY_E, KEY_7]);
    assert_eq!(stored(&mut de, &altgr), Vec::<String>::new());
}

#[test]
fn characters_typed_with_a_level_three_key_held_are_never_stored() {
    let password = [KEY_Q, KEY_E, KEY_7, KEY_0];
    for (option, key) in [
        ("lv3:lalt_switch", ALT),
        ("lv3:switch", RIGHT_CTRL),
        ("lv3:lwin_switch", SUPER),
    ] {
        let mut f = KeyFilter::new(Some(keymap("de", option)));
        assert_eq!(
            stored(&mut f, &chord(&[key], &password)),
            Vec::<String>::new(),
            "{option}"
        );
    }
}

#[test]
fn a_second_layout_typed_with_a_group_switch_held_is_never_stored() {
    let cyrillic = [KEY_J, KEY_D, KEY_C, KEY_Q];
    for (option, key) in [
        ("grp:lwin_switch", SUPER),
        ("grp:rctrl_switch", RIGHT_CTRL),
        ("grp:lalt_switch", ALT),
    ] {
        let mut f = KeyFilter::new(Some(keymap("us,ru", option)));
        assert_eq!(
            stored(&mut f, &chord(&[key], &cyrillic)),
            Vec::<String>::new(),
            "{option}"
        );
    }
}

#[test]
fn a_remapped_ctrl_key_typed_in_rollover_is_never_stored() {
    let mut f = KeyFilter::new(Some(keymap("us", "ctrl:swapcaps")));
    let rollover = [(CTRL, true), (KEY_P, true), (CTRL, false), (KEY_P, false)];
    assert_eq!(stored(&mut f, &rollover), Vec::<String>::new());
    let mut f = KeyFilter::new(Some(keymap("us", "compose:rctrl")));
    assert_eq!(
        stored(&mut f, &chord(&[RIGHT_CTRL], &[KEY_E])),
        Vec::<String>::new()
    );
}

#[test]
fn alt_or_super_with_a_letter_types_text_and_is_not_stored() {
    let mut seq = chord(&[ALT], &[KEY_Q]);
    seq.extend(chord(&[SUPER], &[KEY_L]));
    assert_eq!(stored(&mut us(), &seq), Vec::<String>::new());
}

#[test]
fn shortcuts_that_type_no_text_and_special_keys_are_stored() {
    let mut seq = chord(&[CTRL], &[KEY_C]);
    seq.extend(chord(&[ALT], &[TAB]));
    seq.extend(chord(&[SUPER], &[UP]));
    seq.extend(chord(&[CTRL, SHIFT], &[KEY_T]));
    seq.extend(chord(&[CTRL], &[SPACE]));
    for code in [ENTER, ESC, UP, F5, BACKSPACE] {
        seq.extend(tap(code));
    }
    seq.extend(chord(&[SHIFT], &[TAB]));
    assert_eq!(
        stored(&mut us(), &seq),
        [
            "⌃C",
            "⌥Tab",
            "⌘Up",
            "⌃⇧T",
            "⌃Space",
            "Enter",
            "Esc",
            "Up",
            "F5",
            "Backspace",
            "⇧Tab"
        ]
    );
}

#[test]
fn without_a_keymap_only_special_keys_are_stored() {
    let mut f = KeyFilter::default();
    let mut seq = chord(&[CTRL], &[KEY_C, KEY_1, COMMA, SPACE]);
    seq.extend(chord(&[SUPER], &[KEY_L]));
    seq.extend(chord(&[CTRL], &[TAB]));
    seq.extend(tap(ENTER));
    assert_eq!(stored(&mut f, &seq), ["⌃Tab", "Enter"]);
}

#[test]
fn a_keymap_that_does_not_compile_stores_like_no_keymap() {
    use es_model::platform::KeymapText;
    let mut f = KeyFilter::default();
    let bad = KeymapText {
        serial: 1,
        text: "xkb_keymap { garbage".into(),
    };
    f.follow(Some(&bad));
    assert_eq!(
        stored(&mut f, &chord(&[CTRL], &[KEY_C])),
        Vec::<String>::new()
    );
}

#[test]
fn a_new_keymap_takes_over_and_a_lost_one_falls_back_to_special_keys() {
    use es_model::platform::KeymapText;
    let text = |layout: &str| keymap_text(layout, "");
    let mut f = KeyFilter::default();
    let ctrl_c = chord(&[CTRL], &[KEY_C]);
    f.follow(Some(&KeymapText {
        serial: 1,
        text: text("us").into(),
    }));
    assert_eq!(stored(&mut f, &ctrl_c), ["⌃C"]);
    f.follow(Some(&KeymapText {
        serial: 2,
        text: text("ru").into(),
    }));
    assert_eq!(
        stored(&mut f, &ctrl_c),
        Vec::<String>::new(),
        "Ctrl+С is text"
    );
    f.follow(None);
    assert_eq!(stored(&mut f, &ctrl_c), Vec::<String>::new());
}

#[test]
fn unicode_entry_with_ctrl_shift_held_is_never_stored() {
    let mut f = us();
    let entry = chord(&[CTRL, SHIFT], &[KEY_U, KEY_2, KEY_0, KEY_A, KEY_C]);
    assert_eq!(stored(&mut f, &entry), Vec::<String>::new());
    let mut held_ctrl = vec![(CTRL, true), (SHIFT, true)];
    held_ctrl.extend(tap(KEY_U));
    held_ctrl.push((SHIFT, false));
    held_ctrl.extend(tap(KEY_2));
    held_ctrl.push((CTRL, false));
    assert_eq!(stored(&mut f, &held_ctrl), Vec::<String>::new());
    assert_eq!(stored(&mut f, &chord(&[CTRL], &[KEY_C])), ["⌃C"]);
}

#[test]
fn a_modifier_held_on_one_keyboard_does_not_make_a_chord_on_another() {
    let mut f = us();
    assert_eq!(f.feed(1, CTRL, true), None);
    assert_eq!(f.feed(2, KEY_C, true), None);
    assert_eq!(f.feed(1, KEY_C, true), Some((vec!["⌃".into()], "C".into())));
}

#[test]
fn forgetting_releases_every_held_key() {
    let mut f = us();
    f.feed(0, CTRL, true);
    f.forget();
    assert_eq!(f.feed(0, KEY_C, true), None);
}
