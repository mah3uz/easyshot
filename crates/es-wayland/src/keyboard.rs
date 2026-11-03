use crate::worker::State;
use es_model::platform::KeymapText;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex};
use wayland_client::protocol::wl_keyboard::{self, KeymapFormat, WlKeyboard};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};

const MAX_KEYMAP: u32 = 16 << 20;

pub(crate) type SharedKeymap = Arc<Mutex<Option<KeymapText>>>;

/// The seat's keyboard, held only for the keymap: this connection has no surfaces, so the
/// compositor never sends it keys.
pub(crate) struct Keyboard {
    pub shared: SharedKeymap,
    keyboard: Option<WlKeyboard>,
    serial: u64,
}

impl Keyboard {
    pub fn new(shared: SharedKeymap) -> Self {
        Keyboard {
            shared,
            keyboard: None,
            serial: 0,
        }
    }

    pub fn set_capability(&mut self, present: bool, seat: &WlSeat, qh: &QueueHandle<State>) {
        match (present, self.keyboard.take()) {
            (true, None) => self.keyboard = Some(seat.get_keyboard(qh, ())),
            (true, kept) => self.keyboard = kept,
            (false, Some(old)) => {
                if old.version() >= 3 {
                    old.release();
                }
                self.publish(None);
            }
            (false, None) => {}
        }
    }

    fn publish(&mut self, text: Option<String>) {
        let mut shared = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let same = match (shared.as_ref(), text.as_deref()) {
            (Some(old), Some(new)) => *old.text == *new,
            (None, None) => true,
            _ => false,
        };
        if same {
            return;
        }
        self.serial += 1;
        *shared = text.map(|t| KeymapText {
            serial: self.serial,
            text: t.into(),
        });
    }
}

fn read_keymap(fd: &OwnedFd, size: u32) -> Option<String> {
    if size == 0 || size > MAX_KEYMAP {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let mut done = 0;
    while done < buf.len() {
        // pread, not read: compositors share one descriptor, and its offset, among clients.
        match rustix::io::pread(fd, &mut buf[done..], done as u64) {
            Ok(0) | Err(_) => return None,
            Ok(n) => done += n,
        }
    }
    let text = String::from_utf8(buf).ok()?;
    Some(text.trim_end_matches('\0').to_owned())
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format, fd, size } = event {
            let text = match format {
                WEnum::Value(KeymapFormat::XkbV1) => read_keymap(&fd, size),
                _ => None,
            };
            if text.is_none() {
                tracing::warn!("the compositor's keymap could not be read");
            }
            state.keyboard.publish(text);
        }
    }
}
