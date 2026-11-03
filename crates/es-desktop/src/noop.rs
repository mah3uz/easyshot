use crate::sound::Sound;
use crate::{
    Notification, NotificationId, Notifier, RecordingTrayState, Result, Shortcuts, Tray, TrayModel,
};
use es_protocol::{Route, ShortcutEntry};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct RecordingTray {
    menus: Mutex<Vec<TrayModel>>,
    states: Mutex<Vec<Option<RecordingTrayState>>>,
    agent: Mutex<Vec<bool>>,
    tx: async_channel::Sender<Route>,
    rx: async_channel::Receiver<Route>,
}

impl Default for RecordingTray {
    fn default() -> Self {
        let (tx, rx) = async_channel::unbounded();
        RecordingTray {
            menus: Mutex::default(),
            states: Mutex::default(),
            agent: Mutex::default(),
            tx,
            rx,
        }
    }
}

impl RecordingTray {
    pub fn menus(&self) -> Vec<TrayModel> {
        lock(&self.menus).clone()
    }

    pub fn recording_states(&self) -> Vec<Option<RecordingTrayState>> {
        lock(&self.states).clone()
    }

    pub fn agent_states(&self) -> Vec<bool> {
        lock(&self.agent).clone()
    }

    pub fn activate(&self, route: Route) {
        let _ = self.tx.try_send(route);
    }
}

impl Tray for RecordingTray {
    fn set_menu(&self, m: TrayModel) {
        lock(&self.menus).push(m);
    }

    fn set_recording(&self, s: Option<RecordingTrayState>) {
        lock(&self.states).push(s);
    }

    fn set_agent(&self, active: bool) {
        lock(&self.agent).push(active);
    }

    fn activations(&self) -> async_channel::Receiver<Route> {
        self.rx.clone()
    }
}

pub struct RecordingNotifier {
    sent: Mutex<Vec<Notification>>,
    next: AtomicU32,
    supports_actions: bool,
    tx: async_channel::Sender<(NotificationId, String)>,
    rx: async_channel::Receiver<(NotificationId, String)>,
}

impl RecordingNotifier {
    pub fn new(supports_actions: bool) -> Self {
        let (tx, rx) = async_channel::unbounded();
        RecordingNotifier {
            sent: Mutex::default(),
            next: AtomicU32::new(1),
            supports_actions,
            tx,
            rx,
        }
    }

    pub fn sent(&self) -> Vec<Notification> {
        lock(&self.sent).clone()
    }

    pub fn invoke(&self, id: NotificationId, action: &str) {
        let _ = self.tx.try_send((id, action.to_owned()));
    }
}

impl Default for RecordingNotifier {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Notifier for RecordingNotifier {
    fn notify(&self, n: Notification) -> Result<NotificationId> {
        lock(&self.sent).push(n);
        Ok(NotificationId(self.next.fetch_add(1, Ordering::Relaxed)))
    }

    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        self.rx.clone()
    }

    fn supports_actions(&self) -> bool {
        self.supports_actions
    }
}

pub struct RecordingShortcuts {
    registered: Mutex<Vec<Vec<&'static str>>>,
    user_binds_required: bool,
    tx: async_channel::Sender<Route>,
    rx: async_channel::Receiver<Route>,
}

impl RecordingShortcuts {
    pub fn new(user_binds_required: bool) -> Self {
        let (tx, rx) = async_channel::unbounded();
        RecordingShortcuts {
            registered: Mutex::default(),
            user_binds_required,
            tx,
            rx,
        }
    }

    pub fn registered(&self) -> Vec<Vec<&'static str>> {
        lock(&self.registered).clone()
    }

    pub fn activate(&self, route: Route) {
        let _ = self.tx.try_send(route);
    }
}

impl Default for RecordingShortcuts {
    fn default() -> Self {
        Self::new(true)
    }
}

impl Shortcuts for RecordingShortcuts {
    fn register(&self, defs: &[ShortcutEntry]) -> Result<()> {
        lock(&self.registered).push(defs.iter().map(|d| d.id).collect());
        Ok(())
    }

    fn activations(&self) -> async_channel::Receiver<Route> {
        self.rx.clone()
    }

    fn user_binds_required(&self) -> bool {
        self.user_binds_required
    }
}

#[derive(Default)]
pub struct RecordingSound {
    shutters: AtomicUsize,
}

impl RecordingSound {
    pub fn shutters(&self) -> usize {
        self.shutters.load(Ordering::Relaxed)
    }
}

impl Sound for RecordingSound {
    fn shutter(&self) {
        self.shutters.fetch_add(1, Ordering::Relaxed);
    }
}
