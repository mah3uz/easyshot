#[derive(Debug, Default, Clone, Copy)]
pub struct ModState {
    ctrl: u8,
    alt: u8,
    shift: u8,
    meta: u8,
}

enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Meta,
}

fn modifier_of(code: u16) -> Option<Modifier> {
    match code {
        29 | 97 => Some(Modifier::Ctrl),
        56 | 100 => Some(Modifier::Alt),
        42 | 54 => Some(Modifier::Shift),
        125 | 126 => Some(Modifier::Meta),
        _ => None,
    }
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
        if self.alt > 0 {
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

    /// Feeds one key transition; returns a keystroke only for a chord (Ctrl, Alt or Meta held)
    /// or a special key. Plain typing yields nothing.
    pub fn feed(&mut self, code: u16, down: bool) -> Option<(Vec<String>, String)> {
        if let Some(m) = modifier_of(code) {
            let n = match m {
                Modifier::Ctrl => &mut self.ctrl,
                Modifier::Alt => &mut self.alt,
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
