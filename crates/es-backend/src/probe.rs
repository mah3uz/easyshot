use crate::error::BackendError;
pub use es_wayland::WlGlobals;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "test-hooks")]
use es_model::platform::OutputId;

pub(crate) const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

#[derive(Debug, Clone, Default)]
pub struct DbusNames {
    /// Owned and activatable names.
    pub names: BTreeSet<String>,
}

impl DbusNames {
    pub fn has(&self, name: &str) -> bool {
        self.names.contains(name)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortalIfaces {
    pub screenshot: bool,
    pub screencast: bool,
    pub global_shortcuts: bool,
    pub file_chooser: bool,
    pub settings: bool,
}

impl PortalIfaces {
    fn from_introspection(xml: &str) -> Self {
        let ifaces: BTreeSet<&str> = xml
            .split("<interface name=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .collect();
        let has = |short: &str| ifaces.contains(format!("org.freedesktop.portal.{short}").as_str());
        PortalIfaces {
            screenshot: has("Screenshot"),
            screencast: has("ScreenCast"),
            global_shortcuts: has("GlobalShortcuts"),
            file_chooser: has("FileChooser"),
            settings: has("Settings"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Env {
    pub vars: BTreeMap<String, String>,
}

impl Env {
    pub fn from_process() -> Self {
        Self::from_vars(
            std::env::vars_os()
                .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?))),
        )
    }

    pub fn from_vars<K: Into<String>, V: Into<String>>(
        vars: impl IntoIterator<Item = (K, V)>,
    ) -> Self {
        Env {
            vars: vars
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    pub fn var(&self, key: &str) -> Option<&str> {
        self.vars
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
    }

    pub fn current_desktop(&self) -> Option<&str> {
        self.var("XDG_CURRENT_DESKTOP")
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub gsr: bool,
    pub wf_recorder: bool,
    pub tesseract: bool,
    pub ffmpeg: bool,
}

/// `(tool, package)`: what a missing tool is called and what provides it.
pub const TOOL_PACKAGES: &[(&str, &str)] = &[
    ("gpu-screen-recorder", "gpu-screen-recorder"),
    ("wf-recorder", "wf-recorder"),
    ("tesseract", "tesseract"),
    ("ffmpeg", "ffmpeg"),
];

impl Tools {
    pub fn gather(env: &Env) -> Self {
        let path = env.var("PATH").unwrap_or_default();
        let on_path = |name: &str| find_on_path(path, name).is_some();
        Tools {
            gsr: on_path("gpu-screen-recorder"),
            wf_recorder: on_path("wf-recorder"),
            tesseract: on_path("tesseract"),
            ffmpeg: on_path("ffmpeg"),
        }
    }

    pub fn has(&self, tool: &str) -> bool {
        match tool {
            "gpu-screen-recorder" => self.gsr,
            "wf-recorder" => self.wf_recorder,
            "tesseract" => self.tesseract,
            "ffmpeg" => self.ffmpeg,
            _ => false,
        }
    }
}

fn find_on_path(path: &str, name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|p| {
            std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub output: OutputId,
    /// The clipboard sink writes under `<dir>/clipboard/`.
    pub dir: PathBuf,
    /// Only a harness that owns its whole compositor sets `EASYSHOT_TEST_VIRTUAL_INPUT=1`.
    pub virtual_input: bool,
}

#[cfg(feature = "test-hooks")]
impl TestConfig {
    pub fn from_env(env: &Env) -> Result<Option<Self>, BackendError> {
        if env.var("EASYSHOT_TEST") != Some("1") {
            return Ok(None);
        }
        let output = env
            .var("EASYSHOT_TEST_OUTPUT")
            .ok_or(BackendError::NoTestOutput)?;
        let dir = env
            .var("EASYSHOT_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("easyshot-test"));
        Ok(Some(TestConfig {
            output: OutputId(output.to_owned()),
            dir,
            virtual_input: env.var("EASYSHOT_TEST_VIRTUAL_INPUT") == Some("1"),
        }))
    }
}

#[derive(Clone, Default)]
pub struct Probe {
    pub globals: WlGlobals,
    pub dbus: DbusNames,
    pub portals: PortalIfaces,
    pub env: Env,
    pub tools: Tools,
    #[cfg(feature = "test-hooks")]
    pub test: Option<TestConfig>,
    /// Shared by every Wayland provider.
    pub wayland: Option<Arc<es_wayland::Connection>>,
    /// Why a source could not be read, for doctor.
    pub errors: Vec<String>,
    /// Turns catalogue ids into the tray's and the shortcuts portal's text; without it they show the ids.
    pub labels: Option<es_desktop::Labels>,
    pub tray_icon: Vec<es_desktop::IconPixmap>,
    /// The daemon keeps a tray ready for a panel that starts after it; other commands do not.
    pub await_tray: bool,
}

impl Probe {
    /// Fails only in test mode without a test output.
    pub fn gather() -> Result<Probe, BackendError> {
        let env = Env::from_process();
        #[cfg(feature = "test-hooks")]
        let test = TestConfig::from_env(&env)?;
        let mut errors = Vec::new();
        let wayland = match es_wayland::Connection::connect() {
            Ok(c) => Some(Arc::new(c)),
            Err(e) => {
                errors.push(format!("wayland: {e}"));
                None
            }
        };
        let globals = wayland
            .as_ref()
            .map(|c| c.globals().clone())
            .unwrap_or_default();
        let (dbus, portals) = match gather_dbus() {
            Ok(found) => found,
            Err(e) => {
                errors.push(format!("d-bus: {e}"));
                Default::default()
            }
        };
        let tools = Tools::gather(&env);
        for e in &errors {
            tracing::warn!("probe: {e}");
        }
        Ok(Probe {
            globals,
            dbus,
            portals,
            env,
            tools,
            #[cfg(feature = "test-hooks")]
            test,
            wayland,
            errors,
            labels: None,
            tray_icon: es_desktop::tray_icons(),
            await_tray: false,
        })
    }
}

fn gather_dbus() -> zbus::Result<(DbusNames, PortalIfaces)> {
    let conn = zbus::blocking::Connection::session()?;
    let bus = zbus::blocking::fdo::DBusProxy::new(&conn)?;
    let owned: BTreeSet<String> = bus
        .list_names()?
        .into_iter()
        .map(|n| n.to_string())
        .collect();
    let mut names = owned.clone();
    names.extend(
        bus.list_activatable_names()?
            .into_iter()
            .map(|n| n.to_string()),
    );
    let portals = match bus.get_name_owner(zbus::names::BusName::try_from(PORTAL_NAME)?) {
        Ok(owner) => portal_ifaces(&conn, owner.as_str())?,
        Err(_) => PortalIfaces::default(),
    };
    Ok((DbusNames { names }, portals))
}

// A unique name is never activated; the well-known name of a stopped portal would start it.
pub(crate) fn portal_ifaces(
    conn: &zbus::blocking::Connection,
    owner: &str,
) -> zbus::Result<PortalIfaces> {
    let xml = zbus::blocking::fdo::IntrospectableProxy::builder(conn)
        .destination(owner.to_owned())?
        .path(PORTAL_PATH)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()?
        .introspect()?;
    Ok(PortalIfaces::from_introspection(&xml))
}

pub(crate) fn session_bus(env: &Env) -> zbus::Result<zbus::blocking::Connection> {
    match env.var("DBUS_SESSION_BUS_ADDRESS") {
        Some(address) => zbus::blocking::connection::Builder::address(address)?.build(),
        None => zbus::blocking::Connection::session(),
    }
}

pub struct Provider<T: ?Sized> {
    pub name: &'static str,
    /// Priority; higher wins, `None` means not applicable.
    pub probe: fn(&Probe) -> Option<u8>,
    pub build: fn(&Probe) -> std::result::Result<Box<T>, BackendError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_interfaces_come_from_introspection() {
        let xml = r#"<node><interface name="org.freedesktop.portal.Screenshot"></interface>
            <interface name="org.freedesktop.portal.Settings"/></node>"#;
        let p = PortalIfaces::from_introspection(xml);
        assert!(p.screenshot && p.settings);
        assert!(!p.screencast && !p.global_shortcuts && !p.file_chooser);
    }

    #[test]
    fn empty_variables_count_as_unset() {
        let env = Env::from_vars([("A", ""), ("B", "x")]);
        assert_eq!(env.var("A"), None);
        assert_eq!(env.var("B"), Some("x"));
    }
}
