use bitflags::bitflags;

use crate::{Error, Result};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifiers: u8 {
        const SUPER = 1;
        const CTRL = 1 << 1;
        const ALT = 1 << 2;
        const SHIFT = 1 << 3;
    }
}

const MODIFIER_ORDER: [(Modifiers, &str); 4] = [
    (Modifiers::SUPER, "Super"),
    (Modifiers::CTRL, "Ctrl"),
    (Modifiers::ALT, "Alt"),
    (Modifiers::SHIFT, "Shift"),
];

struct Named {
    name: &'static str,
    label: &'static str,
    spoken: &'static str,
    recording: Option<&'static str>,
}

const fn key(
    name: &'static str,
    label: &'static str,
    spoken: &'static str,
    recording: Option<&'static str>,
) -> Named {
    Named {
        name,
        label,
        spoken,
        recording,
    }
}

static NAMED: &[Named] = &[
    key("Home", "Home", "Home", Some("Home")),
    key("End", "End", "End", Some("End")),
    key("PageUp", "PgUp", "Page Up", Some("PgUp")),
    key("PageDown", "PgDn", "Page Down", Some("PgDn")),
    key("Return", "Enter", "Return", Some("⏎")),
    key("KeypadEnter", "KP Enter", "Keypad Enter", Some("⏎")),
    key("Escape", "Esc", "Escape", Some("esc")),
    key("Tab", "Tab", "Tab", Some("Tab")),
    key("Space", "Space", "Space", Some("space")),
    key("Backspace", "Backspace", "Backspace", Some("Backspace")),
    key("Delete", "Del", "Delete", Some("Del")),
    key("Up", "↑", "Up Arrow", Some("↑")),
    key("Down", "↓", "Down Arrow", Some("↓")),
    key("Left", "←", "Left Arrow", Some("←")),
    key("Right", "→", "Right Arrow", Some("→")),
    key("Eisu", "英数", "Eisu", Some("英数")),
    key("Kana", "かな", "Kana", Some("かな")),
    key("KeypadClear", "Clear", "Keypad Clear", Some("clear")),
    key("Equal", "=", "Equals", None),
    key("Minus", "-", "Minus", None),
    key("Plus", "+", "Plus", None),
    key("KeypadDecimal", "KP .", "Keypad Decimal", None),
    key("KeypadMultiply", "KP *", "Keypad Multiply", None),
    key("KeypadPlus", "KP +", "Keypad Plus", None),
    key("KeypadMinus", "KP -", "Keypad Minus", None),
    key("KeypadDivide", "KP /", "Keypad Divide", None),
    key("KeypadEquals", "KP =", "Keypad Equals", None),
    key("KeypadComma", "KP ,", "Keypad Comma", None),
];

const KEYPAD_DIGITS: [(&str, &str, &str); 10] = [
    ("Keypad0", "KP 0", "Keypad 0"),
    ("Keypad1", "KP 1", "Keypad 1"),
    ("Keypad2", "KP 2", "Keypad 2"),
    ("Keypad3", "KP 3", "Keypad 3"),
    ("Keypad4", "KP 4", "Keypad 4"),
    ("Keypad5", "KP 5", "Keypad 5"),
    ("Keypad6", "KP 6", "Keypad 6"),
    ("Keypad7", "KP 7", "Keypad 7"),
    ("Keypad8", "KP 8", "Keypad 8"),
    ("Keypad9", "KP 9", "Keypad 9"),
];

const MAX_FUNCTION_KEY: u8 = 20;

fn function_key(name: &str) -> Option<u8> {
    let n: u8 = name.strip_prefix('F')?.parse().ok()?;
    ((1..=MAX_FUNCTION_KEY).contains(&n) && name == format!("F{n}")).then_some(n)
}

fn single_char(name: &str) -> Option<char> {
    let mut chars = name.chars();
    let c = chars.next()?;
    (chars.next().is_none() && c.is_ascii_alphanumeric()).then_some(c)
}

fn canonical_key(name: &str) -> Option<String> {
    if let Some(c) = single_char(name) {
        return Some(c.to_ascii_uppercase().to_string());
    }
    if function_key(name).is_some() {
        return Some(name.to_owned());
    }
    let named = NAMED.iter().map(|k| k.name);
    let pad = KEYPAD_DIGITS.iter().map(|k| k.0);
    named
        .chain(pad)
        .find(|n| n.eq_ignore_ascii_case(name))
        .map(str::to_owned)
}

pub fn key_label(key: &str) -> String {
    if single_char(key).is_some() || function_key(key).is_some() {
        return key.to_ascii_uppercase();
    }
    if let Some(k) = NAMED.iter().find(|k| k.name == key) {
        return k.label.to_owned();
    }
    if let Some(k) = KEYPAD_DIGITS.iter().find(|k| k.0 == key) {
        return k.1.to_owned();
    }
    format!("Key {key}")
}

pub fn key_spoken_name(key: &str) -> String {
    if single_char(key).is_some() || function_key(key).is_some() {
        return key.to_ascii_uppercase();
    }
    if let Some(k) = NAMED.iter().find(|k| k.name == key) {
        return k.spoken.to_owned();
    }
    if let Some(k) = KEYPAD_DIGITS.iter().find(|k| k.0 == key) {
        return k.2.to_owned();
    }
    format!("Key {key}")
}

/// What a keystroke shows as on its own, e.g. in a recording; printable keys need a chord and give `None`.
pub fn key_recording_name(key: &str) -> Option<String> {
    if function_key(key).is_some() {
        return Some(key.to_owned());
    }
    NAMED
        .iter()
        .find(|k| k.name == key)
        .and_then(|k| k.recording)
        .map(str::to_owned)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub modifiers: Modifiers,
    pub key: String,
}

impl Trigger {
    pub fn parse(s: &str) -> Result<Trigger> {
        let bad = |reason: &str| Error::InvalidArgs {
            route: s.to_owned(),
            reason: reason.to_owned(),
        };
        let mut parts: Vec<&str> = s.split('+').collect();
        let key_part = parts.pop().unwrap_or_default();
        let mut modifiers = Modifiers::empty();
        for part in parts {
            let m = match part.to_ascii_lowercase().as_str() {
                "super" | "meta" | "win" | "mod4" => Modifiers::SUPER,
                "ctrl" | "control" => Modifiers::CTRL,
                "alt" | "mod1" => Modifiers::ALT,
                "shift" => Modifiers::SHIFT,
                _ => return Err(bad("unknown modifier")),
            };
            if modifiers.intersects(m) {
                return Err(bad("repeated modifier"));
            }
            modifiers |= m;
        }
        let key = canonical_key(key_part).ok_or_else(|| bad("unknown key"))?;
        Ok(Trigger { modifiers, key })
    }

    /// Display text such as `Super+Shift+4`.
    pub fn label(&self) -> String {
        self.join("+", &key_label(&self.key))
    }

    /// Screen-reader text such as `Super, Shift, 4`.
    pub fn spoken(&self) -> String {
        self.join(", ", &key_spoken_name(&self.key))
    }

    fn join(&self, sep: &str, key: &str) -> String {
        let mut out = String::new();
        for (flag, name) in MODIFIER_ORDER {
            if self.modifiers.contains(flag) {
                out.push_str(name);
                out.push_str(sep);
            }
        }
        out + key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_keys_have_a_label_and_a_spoken_name() {
        for (key, label, spoken) in [
            ("Home", "Home", "Home"),
            ("End", "End", "End"),
            ("PageUp", "PgUp", "Page Up"),
            ("PageDown", "PgDn", "Page Down"),
            ("Return", "Enter", "Return"),
            ("Escape", "Esc", "Escape"),
            ("Delete", "Del", "Delete"),
            ("KeypadEnter", "KP Enter", "Keypad Enter"),
            ("Eisu", "英数", "Eisu"),
            ("Kana", "かな", "Kana"),
        ] {
            assert_eq!(key_label(key), label);
            assert_eq!(key_spoken_name(key), spoken);
        }
    }

    #[test]
    fn function_keys_f1_to_f20_read_the_same_everywhere() {
        for n in 1..=20 {
            let k = format!("F{n}");
            assert_eq!(key_label(&k), k);
            assert_eq!(key_recording_name(&k).as_deref(), Some(k.as_str()));
            assert_eq!(Trigger::parse(&k).unwrap().key, k);
        }
        assert!(Trigger::parse("F0").is_err());
        assert!(Trigger::parse("F21").is_err());
        assert!(Trigger::parse("F01").is_err());
    }

    #[test]
    fn digits_label_as_themselves_and_need_a_chord_in_recordings() {
        for d in 0..=9 {
            let k = d.to_string();
            assert_eq!(key_label(&k), k);
            assert_eq!(key_recording_name(&k), None);
        }
    }

    #[test]
    fn keypad_keys_never_fall_back_to_the_generic_label() {
        let pad = (0..=9).map(|d| format!("Keypad{d}")).chain(
            [
                "Decimal", "Multiply", "Plus", "Minus", "Divide", "Equals", "Comma",
            ]
            .map(|s| format!("Keypad{s}")),
        );
        for k in pad {
            assert!(!key_label(&k).starts_with("Key "), "{k}");
            assert!(key_spoken_name(&k).starts_with("Keypad "), "{k}");
            assert_eq!(key_recording_name(&k), None, "{k}");
            assert!(Trigger::parse(&k).is_ok(), "{k}");
        }
    }

    #[test]
    fn recording_names_for_the_common_standalone_keys() {
        assert_eq!(key_recording_name("Return").as_deref(), Some("⏎"));
        assert_eq!(key_recording_name("Escape").as_deref(), Some("esc"));
        assert_eq!(key_recording_name("Space").as_deref(), Some("space"));
        assert_eq!(key_recording_name("KeypadClear").as_deref(), Some("clear"));
        assert_eq!(key_recording_name("A"), None);
    }

    #[test]
    fn unknown_keys_get_a_generic_label() {
        assert_eq!(key_label("Hyper"), "Key Hyper");
        assert_eq!(key_spoken_name("Hyper"), "Key Hyper");
        assert_eq!(key_recording_name("Hyper"), None);
    }

    #[test]
    fn triggers_label_with_linux_modifiers_in_a_fixed_order() {
        let t = Trigger::parse("Super+Shift+4").unwrap();
        assert_eq!(t.label(), "Super+Shift+4");
        assert_eq!(t.spoken(), "Super, Shift, 4");
        let t = Trigger::parse("shift+alt+control+super+o").unwrap();
        assert_eq!(t.label(), "Super+Ctrl+Alt+Shift+O");
        assert_eq!(Trigger::parse("Ctrl+Equal").unwrap().label(), "Ctrl+=");
        assert_eq!(
            Trigger::parse("Meta+1").unwrap(),
            Trigger::parse("Super+1").unwrap()
        );
        assert_eq!(Trigger::parse("Delete").unwrap().label(), "Del");
    }

    #[test]
    fn malformed_triggers_are_rejected() {
        for s in [
            "",
            "+",
            "Super+",
            "Super+Super+4",
            "Hyper+4",
            "Super+Shift",
            "Super+Nope",
            "4+Super",
        ] {
            assert!(Trigger::parse(s).is_err(), "{s:?}");
        }
    }
}
