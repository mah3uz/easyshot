use super::keys::ModState;
use es_model::platform::KeymapText;
use std::collections::HashMap;
use xkbcommon::xkb::{self, KeyDirection, Keysym};

const EVDEV_OFFSET: u32 = 8;

/// A compiled XKB keymap. It is not `Send`: it lives on the thread that reads the devices.
#[derive(Clone)]
pub struct Keymap {
    map: xkb::Keymap,
}

impl Keymap {
    /// `None` for text that does not compile; a trailing NUL, as the compositor sends it, is fine.
    pub fn from_text(text: &str) -> Option<Self> {
        let text = text.trim_end_matches('\0');
        if text.contains('\0') {
            return None;
        }
        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        xkb::Keymap::new_from_string(
            &ctx,
            text.to_owned(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .map(|map| Keymap { map })
    }
}

fn special_name(sym: Keysym) -> Option<&'static str> {
    use xkb::keysyms as k;
    Some(match sym.raw() {
        k::KEY_Escape => "Esc",
        k::KEY_BackSpace => "Backspace",
        k::KEY_Tab | k::KEY_ISO_Left_Tab => "Tab",
        k::KEY_Return | k::KEY_KP_Enter => "Enter",
        k::KEY_Home => "Home",
        k::KEY_Up => "Up",
        k::KEY_Prior => "PageUp",
        k::KEY_Left => "Left",
        k::KEY_Right => "Right",
        k::KEY_End => "End",
        k::KEY_Down => "Down",
        k::KEY_Next => "PageDown",
        k::KEY_Insert => "Insert",
        k::KEY_Delete => "Delete",
        k::KEY_F1 => "F1",
        k::KEY_F2 => "F2",
        k::KEY_F3 => "F3",
        k::KEY_F4 => "F4",
        k::KEY_F5 => "F5",
        k::KEY_F6 => "F6",
        k::KEY_F7 => "F7",
        k::KEY_F8 => "F8",
        k::KEY_F9 => "F9",
        k::KEY_F10 => "F10",
        k::KEY_F11 => "F11",
        k::KEY_F12 => "F12",
        _ => return None,
    })
}

/// Dead keys, Compose, group and level switches and input-method keys are part of typing.
fn composes_text(sym: Keysym) -> bool {
    let raw = sym.raw();
    sym.is_modifier_key()
        || raw == xkb::keysyms::KEY_NoSymbol
        || (0xfe00..=0xfeff).contains(&raw)
        || (0xff20..=0xff3f).contains(&raw)
}

struct XkbKeyboard {
    state: xkb::State,
    unicode_entry: bool,
}

impl XkbKeyboard {
    fn active(&self, name: &str) -> bool {
        self.state
            .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
    }

    fn chord_mod(&self, kc: xkb::Keycode, name: &str) -> bool {
        let idx = self.state.get_keymap().mod_get_index(name);
        self.active(name) && !self.state.mod_index_is_consumed(kc, idx)
    }

    fn classify(&mut self, kc: xkb::Keycode) -> Option<(Vec<String>, String)> {
        let sym = self.state.key_get_one_sym(kc);
        let ctrl_shift = self.active(xkb::MOD_NAME_CTRL) && self.active(xkb::MOD_NAME_SHIFT);
        if ctrl_shift && matches!(sym.key_char(), Some('u' | 'U')) {
            self.unicode_entry = true;
        }
        let special = special_name(sym);
        if (special.is_none() && composes_text(sym))
            || self.state.key_get_utf8(kc).chars().any(|c| !c.is_control())
        {
            return None;
        }
        let chord = [xkb::MOD_NAME_CTRL, xkb::MOD_NAME_ALT, xkb::MOD_NAME_LOGO]
            .iter()
            .any(|m| self.chord_mod(kc, m));
        if chord && self.unicode_entry {
            return None;
        }
        let label = match (special, sym.key_char()) {
            (Some(name), _) => name.to_owned(),
            _ if !chord => return None,
            (None, Some(' ')) => "Space".to_owned(),
            (None, Some(c)) => c.to_uppercase().collect(),
            (None, None) => xkb::keysym_get_name(sym),
        };
        let mods = [
            (xkb::MOD_NAME_CTRL, "⌃"),
            (xkb::MOD_NAME_ALT, "⌥"),
            (xkb::MOD_NAME_SHIFT, "⇧"),
            (xkb::MOD_NAME_LOGO, "⌘"),
        ]
        .into_iter()
        .filter(|(m, _)| self.active(m))
        .map(|(_, s)| s.to_owned())
        .collect();
        Some((mods, label))
    }

    fn feed(&mut self, code: u16, down: bool) -> Option<(Vec<String>, String)> {
        let kc = xkb::Keycode::new(u32::from(code) + EVDEV_OFFSET);
        let out = if down { self.classify(kc) } else { None };
        let dir = if down {
            KeyDirection::Down
        } else {
            KeyDirection::Up
        };
        self.state.update_key(kc, dir);
        if !self.active(xkb::MOD_NAME_CTRL) && !self.active(xkb::MOD_NAME_SHIFT) {
            self.unicode_entry = false;
        }
        out
    }
}

enum Keyboard {
    Xkb(XkbKeyboard),
    Codes(ModState),
}

/// Turns key transitions into stored keystrokes: only chords that type no text, and special keys.
/// Every device keeps its own state, as the compositor does for keyboards it does not group.
#[derive(Default)]
pub struct KeyFilter {
    serial: Option<u64>,
    keymap: Option<Keymap>,
    keyboards: HashMap<u32, Keyboard>,
}

impl KeyFilter {
    pub fn new(keymap: Option<Keymap>) -> Self {
        KeyFilter {
            keymap,
            ..KeyFilter::default()
        }
    }

    /// Follows the compositor's keymap; a changed or missing one starts every keyboard afresh.
    pub fn follow(&mut self, current: Option<&KeymapText>) {
        let serial = current.map(|k| k.serial);
        if serial == self.serial {
            return;
        }
        self.serial = serial;
        self.keymap = current.and_then(|k| Keymap::from_text(&k.text));
        if current.is_some() && self.keymap.is_none() {
            tracing::warn!("the compositor's keymap does not compile; storing special keys only");
        }
        self.keyboards.clear();
    }

    /// Forgets what every keyboard holds, for when reading stops.
    pub fn forget(&mut self) {
        self.keyboards.clear();
    }

    pub fn feed(&mut self, device: u32, code: u16, down: bool) -> Option<(Vec<String>, String)> {
        let keymap = self.keymap.as_ref();
        let kb = self
            .keyboards
            .entry(device)
            .or_insert_with(|| match keymap {
                Some(k) => Keyboard::Xkb(XkbKeyboard {
                    state: xkb::State::new(&k.map),
                    unicode_entry: false,
                }),
                None => Keyboard::Codes(ModState::default()),
            });
        match kb {
            Keyboard::Xkb(x) => x.feed(code, down),
            Keyboard::Codes(m) => m.feed(code, down),
        }
    }
}
