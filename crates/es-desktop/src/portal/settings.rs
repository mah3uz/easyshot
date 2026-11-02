use crate::bus::connect;
use crate::dbus::Endpoint;
use crate::{Appearance, AppearanceEvent, ColorScheme, Result};
use ashpd::desktop::settings::{ColorScheme as PortalScheme, Settings};
use futures::StreamExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const ANIMATIONS_NAMESPACE: &str = "org.gnome.desktop.interface";
const ANIMATIONS_KEY: &str = "enable-animations";
const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";
const SCHEME_KEY: &str = "color-scheme";

pub struct PortalAppearance {
    dark: Arc<AtomicBool>,
    reduce_motion: Arc<AtomicBool>,
    changes: async_channel::Receiver<AppearanceEvent>,
    _conn: zbus::Connection,
}

fn scheme(s: PortalScheme) -> ColorScheme {
    match s {
        PortalScheme::PreferLight => ColorScheme::Light,
        PortalScheme::PreferDark | PortalScheme::NoPreference => ColorScheme::Dark,
    }
}

impl PortalAppearance {
    pub async fn start(endpoint: &Endpoint) -> Result<Self> {
        let conn = connect(endpoint).await?;
        let dark = Arc::new(AtomicBool::new(true));
        let reduce_motion = Arc::new(AtomicBool::new(false));
        let (tx, changes) = async_channel::unbounded();
        if let Ok(settings) = Settings::with_connection(conn.clone()).await {
            if let Ok(s) = settings.color_scheme().await {
                dark.store(scheme(s) == ColorScheme::Dark, Ordering::Relaxed);
            }
            if let Ok(enabled) = settings
                .read::<bool>(ANIMATIONS_NAMESPACE, ANIMATIONS_KEY)
                .await
            {
                reduce_motion.store(!enabled, Ordering::Relaxed);
            }
            let schemes = settings
                .receive_setting_changed_with_args::<PortalScheme>(APPEARANCE_NAMESPACE, SCHEME_KEY)
                .await;
            let animations = settings
                .receive_setting_changed_with_args::<bool>(ANIMATIONS_NAMESPACE, ANIMATIONS_KEY)
                .await;
            if let (Ok(schemes), Ok(animations)) = (schemes, animations) {
                let (dark, motion) = (dark.clone(), reduce_motion.clone());
                let schemes = schemes.filter_map(|s| async move {
                    s.ok().map(|s| AppearanceEvent::ColorScheme(scheme(s)))
                });
                let animations = animations.filter_map(|b| async move {
                    b.ok()
                        .map(|enabled| AppearanceEvent::ReduceMotion(!enabled))
                });
                let mut events = Box::pin(futures::stream::select(schemes, animations));
                conn.executor()
                    .spawn(
                        async move {
                            while let Some(event) = events.next().await {
                                match event {
                                    AppearanceEvent::ColorScheme(c) => {
                                        dark.store(c == ColorScheme::Dark, Ordering::Relaxed);
                                    }
                                    AppearanceEvent::ReduceMotion(r) => {
                                        motion.store(r, Ordering::Relaxed);
                                    }
                                }
                                let _ = tx.send(event).await;
                            }
                        },
                        "es-appearance",
                    )
                    .detach();
            }
        }
        Ok(PortalAppearance {
            dark,
            reduce_motion,
            changes,
            _conn: conn,
        })
    }
}

impl Appearance for PortalAppearance {
    fn color_scheme(&self) -> ColorScheme {
        if self.dark.load(Ordering::Relaxed) {
            ColorScheme::Dark
        } else {
            ColorScheme::Light
        }
    }

    fn reduce_motion(&self) -> bool {
        self.reduce_motion.load(Ordering::Relaxed)
    }

    fn changes(&self) -> async_channel::Receiver<AppearanceEvent> {
        self.changes.clone()
    }
}
