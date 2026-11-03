#[derive(Debug, Default, Clone, Copy)]
pub struct ModState {
    ctrl: u8,
    alt: u8,
    alt_gr: u8,
    shift: u8,
    meta: u8,
}

enum Modifier {
    Ctrl,
    Alt,
    AltGr,
    Shift,
    Meta,
}

fn modifier_of(code: u16) -> Option<Modifier> {
    match code {
        29 | 97 => Some(Modifier::Ctrl),
        56 => Some(Modifier::Alt),
        100 => Some(Modifier::AltGr),
        42 | 54 => Some(Modifier::Shift),
        125 | 126 => Some(Modifier::Meta),
        _ => None,
    }
}

fn is_modifier(code: u16) -> bool {
    modifier_of(code).is_some()
}

fn special_name(code: u16) -> Option<&'static str> {
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

fn plain_name(code: u16) -> String {
    let s = match code {
        2..=10 => return char::from(b'1' + (code - 2) as u8).to_string(),
        11 => "0",
        12 => "-",
        13 => "=",
        16..=25 => return char::from(b"QWERTYUIOP"[usize::from(code - 16)]).to_string(),
        26 => "[",
        27 => "]",
        30..=38 => return char::from(b"ASDFGHJKL"[usize::from(code - 30)]).to_string(),
        39 => ";",
        40 => "'",
        41 => "`",
        43 => "\\",
        44..=50 => return char::from(b"ZXCVBNM"[usize::from(code - 44)]).to_string(),
        51 => ",",
        52 => ".",
        53 => "/",
        57 => "Space",
        _ => return format!("Key{code}"),
    };
    s.to_owned()
}

impl ModState {
    fn symbols(&self, with_shift: bool) -> Vec<String> {
        let mut m = Vec::new();
        if self.ctrl > 0 {
            m.push("⌃".to_owned());
        }
        if self.alt > 0 || self.alt_gr > 0 {
            m.push("⌥".to_owned());
        }
        if with_shift && self.shift > 0 {
            m.push("⇧".to_owned());
        }
        if self.meta > 0 {
            m.push("⌘".to_owned());
        }
        m
    }

    /// Feeds one key transition; returns a keystroke only for a chord (Ctrl, left Alt or Meta
    /// held) or a special key. Plain typing yields nothing. Right Alt never makes a chord because
    /// on many layouts it is AltGr, which types characters.
    pub fn feed(&mut self, code: u16, down: bool) -> Option<(Vec<String>, String)> {
        if let Some(m) = modifier_of(code) {
            let n = match m {
                Modifier::Ctrl => &mut self.ctrl,
                Modifier::Alt => &mut self.alt,
                Modifier::AltGr => &mut self.alt_gr,
                Modifier::Shift => &mut self.shift,
                Modifier::Meta => &mut self.meta,
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
        let chord = self.ctrl > 0 || self.alt > 0 || self.meta > 0;
        if let Some(name) = special_name(code) {
            return Some((self.symbols(true), name.to_owned()));
        }
        chord.then(|| (self.symbols(true), plain_name(code)))
    }
}

/// The modifiers one device reported down. A release that never arrives (the device was
/// unplugged, or another program grabbed it) would leave a chord held and turn plain typing into
/// recorded chords, so the source checks these against the device and releases what is up.
#[derive(Debug, Default, Clone)]
pub struct HeldModifiers(Vec<u16>);

impl HeldModifiers {
    pub fn note(&mut self, code: u16, down: bool) {
        if !is_modifier(code) {
            return;
        }
        self.0.retain(|&c| c != code);
        if down {
            self.0.push(code);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Releases for every noted modifier that `still_down` says is up.
    pub fn lost(&mut self, still_down: impl Fn(u16) -> bool, t_us: u64) -> Vec<super::RawEvent> {
        let mut out = Vec::new();
        self.0.retain(|&code| {
            let held = still_down(code);
            if !held {
                out.push(super::RawEvent::Key {
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
