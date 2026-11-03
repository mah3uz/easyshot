use futures::StreamExt as _;
use futures::stream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedObjectPath;

const AUTO_SESSION: &str = "/org/freedesktop/login1/session/auto";
const RETRY_FIRST: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(60);

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
    #[zbus(signal)]
    fn lock(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn unlock(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn get_session(&self, id: &str) -> zbus::Result<OwnedObjectPath>;
}

enum Change {
    Session,
    LockRequested(bool),
    Screen(Option<bool>),
    Gone,
}

/// Keeps `open` true only while this user's login session is active and not locked, and false
/// whenever logind cannot say. A lock counts while `LockedHint` says so, from a Lock signal until
/// an Unlock, and while `screen` (the compositor's own report, its state first) says locked;
/// `lock_requested` carries an unanswered Lock across calls. Returns, closed, when logind or the
/// bus goes away.
pub async fn follow_login_session(
    conn: &zbus::Connection,
    open: &AtomicBool,
    screen: Option<async_channel::Receiver<bool>>,
    lock_requested: &mut bool,
) -> zbus::Result<()> {
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
    let mut screen_locked = screen.is_some();
    let screen = match screen {
        Some(rx) => rx.map(Some).chain(stream::once(async { None })).boxed(),
        None => stream::pending().boxed(),
    };
    let mut events = stream::select_all([
        session
            .receive_active_changed()
            .await
            .map(|_| Change::Session)
            .boxed(),
        session
            .receive_locked_hint_changed()
            .await
            .map(|_| Change::Session)
            .boxed(),
        session
            .receive_lock()
            .await?
            .map(|_| Change::LockRequested(true))
            .boxed(),
        session
            .receive_unlock()
            .await?
            .map(|_| Change::LockRequested(false))
            .boxed(),
        session
            .inner()
            .receive_owner_changed()
            .await?
            .map(|_| Change::Gone)
            .boxed(),
        screen.map(Change::Screen).boxed(),
    ]);
    loop {
        let is_open = !*lock_requested
            && !screen_locked
            && session.active().await?
            && !session.locked_hint().await?;
        open.store(is_open, Ordering::SeqCst);
        match events.next().await {
            Some(Change::Session) => {}
            Some(Change::LockRequested(requested)) => *lock_requested = requested,
            Some(Change::Screen(Some(locked))) => {
                screen_locked = locked;
                // A locker the compositor reports on never asks logind to unlock.
                *lock_requested &= locked;
            }
            Some(Change::Screen(None)) => screen_locked = true,
            Some(Change::Gone) | None => {
                open.store(false, Ordering::SeqCst);
                return Ok(());
            }
        }
    }
}

/// Follows the system bus's logind on its own thread for as long as the daemon runs, again after
/// it goes away; `screen` subscribes to the compositor's report of its lock, where it has one.
pub fn watch_login_session<S>(open: Arc<AtomicBool>, screen: Option<S>)
where
    S: Fn() -> async_channel::Receiver<bool> + Send + 'static,
{
    let spawned = std::thread::Builder::new()
        .name("es-login".into())
        .spawn(move || {
            let mut lock_requested = false;
            let mut wait = RETRY_FIRST;
            loop {
                let followed = zbus::block_on(async {
                    let conn = zbus::Connection::system().await?;
                    let screen = screen.as_ref().map(|s| s());
                    follow_login_session(&conn, &open, screen, &mut lock_requested).await
                });
                open.store(false, Ordering::SeqCst);
                match followed {
                    Ok(()) => {
                        tracing::warn!("logind went away; clicks and keys are not recorded");
                        wait = RETRY_FIRST;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "no login session; clicks and keys are not recorded");
                    }
                }
                std::thread::sleep(wait);
                wait = (wait * 2).min(RETRY_MAX);
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "login session watcher");
    }
}
