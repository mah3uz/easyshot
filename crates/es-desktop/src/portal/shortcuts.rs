use super::portal_error;
use crate::bus::{Labels, block, connect};
use crate::dbus::Endpoint;
use crate::{Error, Result, Shortcuts};
use ashpd::desktop::Session;
use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use es_protocol::{Modifiers, Route, Scope, ShortcutEntry, Target, Trigger};
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

type PortalSession = Session<GlobalShortcuts>;

pub struct PortalShortcuts {
    portal: Option<GlobalShortcuts>,
    labels: Labels,
    session: Mutex<Option<PortalSession>>,
    bound: Arc<AtomicBool>,
    tx: async_channel::Sender<Route>,
    activations: async_channel::Receiver<Route>,
    conn: Option<zbus::Connection>,
}

pub fn portal_trigger(trigger: &str) -> Option<String> {
    let t = Trigger::parse(trigger).ok()?;
    let mut parts = Vec::new();
    for (flag, name) in [
        (Modifiers::CTRL, "CTRL"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::SUPER, "LOGO"),
    ] {
        if t.modifiers.contains(flag) {
            parts.push(name.to_owned());
        }
    }
    parts.push(t.key);
    Some(parts.join("+"))
}

impl PortalShortcuts {
    pub async fn start(endpoint: &Endpoint, labels: Labels) -> Self {
        let (tx, activations) = async_channel::unbounded();
        let conn = connect(endpoint).await.ok();
        let portal = match &conn {
            Some(c) => GlobalShortcuts::with_connection(c.clone()).await.ok(),
            None => None,
        };
        PortalShortcuts {
            portal,
            labels,
            session: Mutex::new(None),
            bound: Arc::new(AtomicBool::new(false)),
            tx,
            activations,
            conn,
        }
    }

    async fn bind(&self, portal: &GlobalShortcuts, defs: &[ShortcutEntry]) -> Result<()> {
        let session = portal
            .create_session(Default::default())
            .await
            .map_err(portal_error)?;
        let shortcuts: Vec<NewShortcut> = defs
            .iter()
            .filter(|d| d.scope == Scope::Global)
            .map(|d| {
                let trigger = d.default_trigger.and_then(portal_trigger);
                NewShortcut::new(d.id, (self.labels)(d.title_id))
                    .preferred_trigger(trigger.as_deref())
            })
            .collect();
        let bound = portal
            .bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default())
            .await
            .map_err(portal_error)?
            .response()
            .map_err(portal_error)?;
        let has_trigger = bound
            .shortcuts()
            .iter()
            .any(|s| !s.trigger_description().is_empty());
        self.bound.store(has_trigger, Ordering::Relaxed);

        let routes: Vec<(&'static str, Route)> = defs
            .iter()
            .filter_map(|d| match &d.target {
                Target::Route(r) if d.scope == Scope::Global => Some((d.id, r.clone())),
                _ => None,
            })
            .collect();
        let mut activated = portal.receive_activated().await.map_err(portal_error)?;
        let tx = self.tx.clone();
        if let Some(conn) = &self.conn {
            conn.executor()
                .spawn(
                    async move {
                        while let Some(a) = activated.next().await {
                            if let Some((_, route)) =
                                routes.iter().find(|(id, _)| *id == a.shortcut_id())
                            {
                                let _ = tx.send(route.clone()).await;
                            }
                        }
                    },
                    "es-shortcuts",
                )
                .detach();
        }
        *self
            .session
            .lock()
            .map_err(|_| Error::Protocol("shortcut session poisoned".into()))? = Some(session);
        Ok(())
    }
}

impl Shortcuts for PortalShortcuts {
    fn register(&self, defs: &[ShortcutEntry]) -> Result<()> {
        let Some(portal) = &self.portal else {
            return Err(Error::Unsupported);
        };
        block(self.bind(portal, defs))
    }

    fn activations(&self) -> async_channel::Receiver<Route> {
        self.activations.clone()
    }

    fn user_binds_required(&self) -> bool {
        !self.bound.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::portal_trigger;

    #[test]
    fn triggers_use_the_portal_spelling() {
        assert_eq!(
            portal_trigger("Super+Shift+4").as_deref(),
            Some("SHIFT+LOGO+4")
        );
        assert_eq!(portal_trigger("not a trigger"), None);
    }
}
