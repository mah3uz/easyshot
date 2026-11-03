use futures::StreamExt as _;
use futures::stream::select;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;

const AUTO_SESSION: &str = "/org/freedesktop/login1/session/auto";

#[zbus::proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait Session {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn active(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn locked_hint(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn get_session(&self, id: &str) -> zbus::Result<OwnedObjectPath>;
}

/// Keeps `open` true only while this user's login session is active and not locked, and false
/// whenever logind cannot say. Returns, closed, when logind or the bus goes away.
pub async fn follow_login_session(conn: &zbus::Connection, open: &AtomicBool) -> zbus::Result<()> {
    open.store(false, Ordering::SeqCst);
    // A user service has no session of its own; "auto" resolves to the user's display session.
    let id = SessionProxy::builder(conn)
        .path(AUTO_SESSION)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?
        .id()
        .await?;
    let path = ManagerProxy::new(conn).await?.get_session(&id).await?;
    let session = SessionProxy::builder(conn).path(path)?.build().await?;
    let active = session.receive_active_changed().await.map(|_| true);
    let locked = session.receive_locked_hint_changed().await.map(|_| true);
    let owner = session
        .inner()
        .receive_owner_changed()
        .await?
        .map(|_| false);
    let mut events = select(active, select(locked, owner));
    loop {
        let is_open = session.active().await? && !session.locked_hint().await.unwrap_or(false);
        open.store(is_open, Ordering::SeqCst);
        if events.next().await != Some(true) {
            open.store(false, Ordering::SeqCst);
            return Ok(());
        }
    }
}

/// Follows the system bus's logind on its own thread for as long as the daemon runs.
pub fn watch_login_session(open: Arc<AtomicBool>) {
    let spawned = std::thread::Builder::new()
        .name("es-login".into())
        .spawn(move || {
            let followed = zbus::block_on(async {
                let conn = zbus::Connection::system().await?;
                follow_login_session(&conn, &open).await
            });
            open.store(false, Ordering::SeqCst);
            if let Err(e) = followed {
                tracing::warn!(error = %e, "no login session; clicks and keys are not recorded");
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "login session watcher");
    }
}
