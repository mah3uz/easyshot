use crate::backend::ProviderReport;
use crate::desktop;
use crate::probe::{PORTAL_NAME, PortalIfaces, Probe, portal_ifaces, session_bus};
use crate::registry::{Registry, select};
use es_desktop::{Appearance, FilePicker, Shortcuts};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError, RwLock, Weak};

/// The providers served by the desktop portal, which can start, stop or restart while the daemon runs.
pub(crate) struct Portal {
    files: RwLock<Option<Arc<dyn FilePicker>>>,
    appearance: RwLock<Option<Arc<dyn Appearance>>>,
    shortcuts: RwLock<Arc<dyn Shortcuts>>,
    report: RwLock<ProviderReport>,
    changed: async_channel::Sender<()>,
    changes: async_channel::Receiver<()>,
    probe: Probe,
    reg: Registry,
    following: AtomicBool,
}

pub(crate) struct Chosen {
    pub files: Option<Arc<dyn FilePicker>>,
    pub appearance: Option<Arc<dyn Appearance>>,
    pub shortcuts: Arc<dyn Shortcuts>,
}

pub(crate) fn select_shortcuts(
    probe: &Probe,
    reg: &Registry,
    report: &mut ProviderReport,
) -> Arc<dyn Shortcuts> {
    select("shortcuts", reg.shortcuts, probe, report)
        .map_or_else(|| Arc::new(desktop::UserBinds::new()) as _, Arc::from)
}

pub(crate) fn portals_report(p: &PortalIfaces) -> serde_json::Map<String, serde_json::Value> {
    [
        ("FileChooser", p.file_chooser),
        ("Settings", p.settings),
        ("GlobalShortcuts", p.global_shortcuts),
        ("ScreenCast", p.screencast),
        ("Screenshot", p.screenshot),
    ]
    .into_iter()
    .map(|(name, on)| (name.to_owned(), serde_json::json!(on)))
    .collect()
}

impl Portal {
    pub fn new(chosen: Chosen, report: ProviderReport, probe: &Probe, reg: &Registry) -> Self {
        let (changed, changes) = async_channel::unbounded();
        Portal {
            files: RwLock::new(chosen.files),
            appearance: RwLock::new(chosen.appearance),
            shortcuts: RwLock::new(chosen.shortcuts),
            report: RwLock::new(report),
            changed,
            changes,
            probe: probe.clone(),
            reg: *reg,
            following: AtomicBool::new(false),
        }
    }

    pub fn files(&self) -> Option<Arc<dyn FilePicker>> {
        self.files
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn appearance(&self) -> Option<Arc<dyn Appearance>> {
        self.appearance
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn shortcuts(&self) -> Arc<dyn Shortcuts> {
        self.shortcuts
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[cfg(feature = "test-hooks")]
    pub fn set_shortcuts(&self, shortcuts: Arc<dyn Shortcuts>) {
        *self
            .shortcuts
            .write()
            .unwrap_or_else(PoisonError::into_inner) = shortcuts;
    }

    pub fn report(&self) -> ProviderReport {
        self.report
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn chosen(&self, capability: &str) -> Option<&'static str> {
        self.report
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .chosen(capability)
    }

    pub fn changes(&self) -> async_channel::Receiver<()> {
        self.changes.clone()
    }

    fn reselect(&self, probe: &Probe) {
        let mut found = ProviderReport::default();
        let shortcuts = select_shortcuts(probe, &self.reg, &mut found);
        let files = select("files", self.reg.files, probe, &mut found).map(Arc::from);
        let appearance =
            select("appearance", self.reg.appearance, probe, &mut found).map(Arc::from);
        *self.files.write().unwrap_or_else(PoisonError::into_inner) = files;
        *self
            .appearance
            .write()
            .unwrap_or_else(PoisonError::into_inner) = appearance;
        *self
            .shortcuts
            .write()
            .unwrap_or_else(PoisonError::into_inner) = shortcuts;
        {
            let mut report = self.report.write().unwrap_or_else(PoisonError::into_inner);
            for entry in found.entries {
                if let Some(slot) = report
                    .entries
                    .iter_mut()
                    .find(|e| e.capability == entry.capability)
                {
                    *slot = entry;
                }
            }
            report.portals = portals_report(&probe.portals);
        }
        let _ = self.changed.try_send(());
    }
}

/// Starts following the portal's bus name, once; test mode never does.
pub(crate) fn follow(portal: &Arc<Portal>) {
    #[cfg(feature = "test-hooks")]
    if portal.probe.test.is_some() {
        return;
    }
    if portal.following.swap(true, Ordering::AcqRel) {
        return;
    }
    let weak = Arc::downgrade(portal);
    let probe = portal.probe.clone();
    let spawned = std::thread::Builder::new()
        .name("es-portal-watch".into())
        .spawn(move || {
            if let Err(e) = watch(&weak, probe) {
                tracing::warn!("portal watch stopped: {e}");
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("cannot follow the portal: {e}");
    }
}

fn watch(portal: &Weak<Portal>, mut probe: Probe) -> zbus::Result<()> {
    let conn = session_bus(&probe.env)?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
    let owners = bus.receive_name_owner_changed_with_args(&[(0, PORTAL_NAME)])?;
    // Asked after subscribing, so an owner that arrived since the probe is not missed.
    let owner = bus
        .get_name_owner(zbus::names::BusName::try_from(PORTAL_NAME)?)
        .ok()
        .map(|o| o.to_string());
    if !update(portal, &mut probe, &conn, owner.as_deref()) {
        return Ok(());
    }
    for change in owners {
        let owner = change
            .args()
            .ok()
            .and_then(|a| a.new_owner.as_ref().map(ToString::to_string));
        if !update(portal, &mut probe, &conn, owner.as_deref()) {
            break;
        }
    }
    Ok(())
}

fn update(
    portal: &Weak<Portal>,
    probe: &mut Probe,
    conn: &zbus::blocking::Connection,
    owner: Option<&str>,
) -> bool {
    let Some(portal) = portal.upgrade() else {
        return false;
    };
    let found = match owner {
        Some(owner) => portal_ifaces(conn, owner).unwrap_or_else(|e| {
            tracing::warn!("portal introspection: {e}");
            PortalIfaces::default()
        }),
        None => PortalIfaces::default(),
    };
    if found != probe.portals {
        tracing::info!(portals = ?found, "portal changed");
        probe.portals = found;
        portal.reselect(probe);
    }
    true
}
