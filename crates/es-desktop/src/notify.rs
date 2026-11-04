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
}

/// The key a notification server sends when the notification itself is clicked.
pub const DEFAULT_ACTION: &str = "default";

const NAME: &str = "org.freedesktop.Notifications";

type Capabilities = Arc<Mutex<Option<bool>>>;

pub struct DesktopNotifier {
    proxy: NotificationsProxy<'static>,
    ours: Arc<Mutex<HashSet<u32>>>,
    actions: async_channel::Receiver<(NotificationId, String)>,
    supports_actions: Capabilities,
    _conn: zbus::Connection,
}

async fn fetch_actions(proxy: &NotificationsProxy<'_>, known: &Capabilities) -> Option<bool> {
    let found = proxy
        .get_capabilities()
        .await
        .ok()
        .map(|caps| caps.iter().any(|c| c == "actions"));
    if let Ok(mut slot) = known.lock() {
        *slot = found;
    }
    found
}

impl DesktopNotifier {
    /// Sends nothing to the notification name: a call to an unowned name would start whichever
    /// server D-Bus can activate and keep the user's own server from claiming it.
    pub async fn connect(endpoint: &Endpoint) -> Result<Self> {
        let conn = connect(endpoint).await?;
        let proxy = NotificationsProxy::builder(&conn)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .map_err(protocol)?;
        let known: Capabilities = Arc::default();
        let bus = zbus::fdo::DBusProxy::new(&conn).await.map_err(protocol)?;
        let mut owners = bus
            .receive_name_owner_changed_with_args(&[(0, NAME)])
            .await
            .map_err(protocol)?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface(NAME)
            .map_err(protocol)?
            .member("ActionInvoked")
            .map_err(protocol)?
            .build();
        let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, None)
            .await
            .map_err(protocol)?;
        if bus
            .name_has_owner(NAME.try_into().map_err(protocol)?)
            .await
            .map_err(protocol)?
        {
            fetch_actions(&proxy, &known).await;
        }
        let ours: Arc<Mutex<HashSet<u32>>> = Arc::default();
        let (tx, actions) = async_channel::unbounded();
        let seen = ours.clone();
        conn.executor()
            .spawn(
                async move {
                    while let Some(Ok(message)) = signals.next().await {
                        let Ok((id, key)) = message.body().deserialize::<(u32, String)>() else {
                            continue;
                        };
                        if seen.lock().is_ok_and(|s| s.contains(&id)) {
                            let _ = tx.send((NotificationId(id), key)).await;
                        }
                    }
                },
                "es-notify-actions",
            )
            .detach();
        let watch = (proxy.clone(), known.clone());
        conn.executor()
            .spawn(
                async move {
                    let (proxy, known) = watch;
                    while let Some(change) = owners.next().await {
                        let owned = change.args().is_ok_and(|a| a.new_owner.is_some());
                        if owned {
                            fetch_actions(&proxy, &known).await;
                        } else if let Ok(mut slot) = known.lock() {
                            *slot = None;
                        }
                    }
                },
                "es-notify-owner",
            )
            .detach();
        Ok(DesktopNotifier {
            proxy,
            ours,
            actions,
            supports_actions: known,
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
        let known = self.supports_actions.lock().ok().and_then(|k| *k);
        let actions = match known {
            Some(known) => known,
            None => block(fetch_actions(&self.proxy, &self.supports_actions)).unwrap_or(false),
        };
        let flat: Vec<&str> = if actions {
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
            .lock()
            .is_ok_and(|k| k.unwrap_or(false))
    }
}
