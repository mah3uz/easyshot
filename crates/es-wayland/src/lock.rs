use crate::worker::State;
use protocol::hyprland_lock_notification_v1::{self, HyprlandLockNotificationV1};
use protocol::hyprland_lock_notifier_v1::HyprlandLockNotifierV1;
use std::sync::{Arc, Mutex};
use wayland_client::{Connection, Dispatch, QueueHandle};

#[allow(
    dead_code,
    non_camel_case_types,
    unused_imports,
    missing_docs,
    clippy::all
)]
pub(crate) mod protocol {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-lock-notify-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-lock-notify-v1.xml");
}

/// What the compositor last said about its screen lock, sent on to every subscriber.
#[derive(Default)]
pub(crate) struct LockReport {
    locked: bool,
    gone: bool,
    subscribers: Vec<async_channel::Sender<bool>>,
}

pub(crate) type SharedLock = Arc<Mutex<LockReport>>;

impl LockReport {
    /// The state now, then each change; the channel closes when the connection ends.
    pub fn subscribe(&mut self) -> async_channel::Receiver<bool> {
        let (tx, rx) = async_channel::unbounded();
        let _ = tx.try_send(self.locked);
        if !self.gone {
            self.subscribers.push(tx);
        }
        rx
    }

    fn set(&mut self, locked: bool) {
        self.locked = locked;
        self.subscribers.retain(|tx| tx.try_send(locked).is_ok());
    }

    /// With the connection gone nothing can report an unlock, so the screen counts as locked.
    pub fn end(&mut self) {
        self.set(true);
        self.gone = true;
        self.subscribers.clear();
    }
}

pub(crate) fn bind(
    registry: &wayland_client::protocol::wl_registry::WlRegistry,
    name: u32,
    qh: &QueueHandle<State>,
) {
    let notifier: HyprlandLockNotifierV1 = registry.bind(name, 1, qh, ());
    notifier.get_lock_notification(qh, ());
}

impl Dispatch<HyprlandLockNotifierV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &HyprlandLockNotifierV1,
        _: <HyprlandLockNotifierV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HyprlandLockNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &HyprlandLockNotificationV1,
        event: hyprland_lock_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let locked = match event {
            hyprland_lock_notification_v1::Event::Locked => true,
            hyprland_lock_notification_v1::Event::Unlocked => false,
        };
        state
            .lock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set(locked);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_get_the_state_now_then_each_change_and_a_lock_when_the_connection_ends() {
        let mut report = LockReport::default();
        let early = report.subscribe();
        report.set(true);
        let late = report.subscribe();
        report.set(false);
        report.end();
        let after = report.subscribe();
        let drain = |rx: &async_channel::Receiver<bool>| {
            std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>()
        };
        assert_eq!(drain(&early), [false, true, false, true]);
        assert_eq!(drain(&late), [true, false, true]);
        assert_eq!(drain(&after), [true]);
        assert!(early.is_closed() && after.is_closed());
    }
}
