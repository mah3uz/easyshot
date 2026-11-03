use crate::bus::{block, connect, protocol};
use crate::dbus::Endpoint;
use crate::{Error, Notification, NotificationId, Notifier, Result, Urgency};
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use zbus::zvariant::Value;

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

/// The key a notification server sends when the notification itself is clicked.
pub const DEFAULT_ACTION: &str = "default";

pub struct DesktopNotifier {
    proxy: NotificationsProxy<'static>,
    ours: Arc<Mutex<HashSet<u32>>>,
    actions: async_channel::Receiver<(NotificationId, String)>,
    supports_actions: bool,
    _conn: zbus::Connection,
}

impl DesktopNotifier {
    pub async fn connect(endpoint: &Endpoint) -> Result<Self> {
        let conn = connect(endpoint).await?;
        let proxy = NotificationsProxy::new(&conn).await.map_err(protocol)?;
        let supports_actions = proxy
            .get_capabilities()
            .await
            .map_err(protocol)?
            .iter()
            .any(|c| c == "actions");
        let mut signals = proxy.receive_action_invoked().await.map_err(protocol)?;
        let ours: Arc<Mutex<HashSet<u32>>> = Arc::default();
        let (tx, actions) = async_channel::unbounded();
        let seen = ours.clone();
        conn.executor()
            .spawn(
                async move {
                    while let Some(signal) = signals.next().await {
                        let Ok(args) = signal.args() else { continue };
                        let mine = seen.lock().is_ok_and(|s| s.contains(&args.id));
                        if mine {
                            let _ = tx
                                .send((NotificationId(args.id), args.action_key.to_owned()))
                                .await;
                        }
                    }
                },
                "es-notify-actions",
            )
            .detach();
        Ok(DesktopNotifier {
            proxy,
            ours,
            actions,
            supports_actions,
            _conn: conn,
        })
    }
}

impl Notifier for DesktopNotifier {
    fn notify(&self, n: Notification) -> Result<NotificationId> {
        let urgency: u8 = match n.urgency {
            Urgency::Low => 0,
            Urgency::Normal => 1,
            Urgency::Critical => 2,
        };
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        hints.insert("urgency", Value::U8(urgency));
        hints.insert("desktop-entry", Value::from(es_protocol::ICON_NAME));
        if let Some(image) = &n.image {
            hints.insert(
                "image-path",
                Value::from(image.to_string_lossy().into_owned()),
            );
        }
        let flat: Vec<&str> = if self.supports_actions {
            n.actions
                .iter()
                .flat_map(|(id, label)| [id.as_str(), label.as_str()])
                .collect()
        } else {
            Vec::new()
        };
        let timeout = n
            .timeout
            .map_or(-1, |d| i32::try_from(d.as_millis()).unwrap_or(i32::MAX));
        let icon = n
            .icon
            .as_ref()
            .map_or_else(String::new, |p| p.to_string_lossy().into_owned());
        let id = block(self.proxy.notify(
            "Easyshot", 0, &icon, &n.title, &n.body, &flat, hints, timeout,
        ))
        .map_err(protocol)?;
        self.ours
            .lock()
            .map_err(|_| Error::Protocol("notification state poisoned".into()))?
            .insert(id);
        Ok(NotificationId(id))
    }

    fn actions(&self) -> async_channel::Receiver<(NotificationId, String)> {
        self.actions.clone()
    }

    fn supports_actions(&self) -> bool {
        self.supports_actions
    }
}
