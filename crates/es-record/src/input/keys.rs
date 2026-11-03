/// Modifier keys by evdev code, for when no keymap is known. It only labels special keys: without
/// the keymap nothing says what a held key types, so no letter, digit or punctuation is stored.
#[derive(Debug, Default, Clone, Copy)]
pub struct ModState {
    ctrl: u8,
    alt: u8,
    shift: u8,
    meta: u8,
}

fn modifier_of(code: u16) -> Option<usize> {
    match code {
        29 | 97 => Some(0),
        56 | 100 => Some(1),
        42 | 54 => Some(2),
        125 | 126 => Some(3),
        _ => None,
    }
}

pub(super) fn special_name(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "Esc",
        14 => "Backspace",
        15 => "Tab",
        28 | 96 => "Enter",
        102 => "Home",
        103 => "Up",
        104 => "PageUp",
        105 => "Left",
        106 => "Right",
        107 => "End",
        108 => "Down",
        109 => "PageDown",
        110 => "Insert",
        111 => "Delete",
        59 => "F1",
        60 => "F2",
        61 => "F3",
        62 => "F4",
        63 => "F5",
        64 => "F6",
        65 => "F7",
        66 => "F8",
        67 => "F9",
        68 => "F10",
        87 => "F11",
        88 => "F12",
        _ => return None,
    })
}

impl ModState {
    fn symbols(&self) -> Vec<String> {
        [
            (self.ctrl, "⌃"),
            (self.alt, "⌥"),
            (self.shift, "⇧"),
            (self.meta, "⌘"),
        ]
        .into_iter()
        .filter(|&(n, _)| n > 0)
        .map(|(_, s)| s.to_owned())
        .collect()
    }

    /// Feeds one key transition; returns a keystroke only for a special key.
    pub fn feed(&mut self, code: u16, down: bool) -> Option<(Vec<String>, String)> {
        if let Some(i) = modifier_of(code) {
            let n = match i {
                0 => &mut self.ctrl,
                1 => &mut self.alt,
                2 => &mut self.shift,
                _ => &mut self.meta,
            };
            *n = if down {
                n.saturating_add(1)
            } else {
                n.saturating_sub(1)
            };
            return None;
        }
        if !down {
            return None;
        }
        special_name(code).map(|name| (self.symbols(), name.to_owned()))
    }
}

/// The keys one device reported down. A release that never arrives (the device was unplugged, or
/// another program grabbed it) would leave a modifier held and turn plain typing into chords, so
/// the source checks these against the device and releases what is up.
#[derive(Debug, Default, Clone)]
pub struct HeldKeys(Vec<u16>);

impl HeldKeys {
    pub fn note(&mut self, code: u16, down: bool) {
        self.0.retain(|&c| c != code);
        if down {
            self.0.push(code);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Releases for every noted key that `still_down` says is up.
    pub fn lost(
        &mut self,
        device: u32,
        still_down: impl Fn(u16) -> bool,
        t_us: u64,
    ) -> Vec<super::RawEvent> {
        let mut out = Vec::new();
        self.0.retain(|&code| {
            let held = still_down(code);
            if !held {
                out.push(super::RawEvent::Key {
                    device,
                    code,
                    down: false,
                    t_us,
                });
            }
            held
        });
        out
    }
}
