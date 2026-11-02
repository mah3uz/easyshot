use crate::error::BackendError;
pub use es_wayland::WlGlobals;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "test-hooks")]
use es_model::platform::OutputId;

const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
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

#[derive(Debug, Clone, Default)]
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

pub const GST_ELEMENTS: &[&str] = &[
    "pipewiresrc",
    "vah264enc",
    "nvh264enc",
    "x264enc",
    "openh264enc",
    "vp9enc",
    "matroskamux",
];

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub gsr: bool,
    pub wf_recorder: bool,
    pub tesseract: bool,
    pub ffmpeg: bool,
    pub gst_elements: BTreeSet<String>,
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
        let gst_elements = match find_on_path(path, "gst-inspect-1.0") {
            Some(inspect) => GST_ELEMENTS
                .iter()
                .filter(|e| gst_has(&inspect, e))
                .map(|e| (*e).to_owned())
                .collect(),
            None => BTreeSet::new(),
        };
        Tools {
            gsr: on_path("gpu-screen-recorder"),
            wf_recorder: on_path("wf-recorder"),
            tesseract: on_path("tesseract"),
            ffmpeg: on_path("ffmpeg"),
            gst_elements,
        }
    }

    pub fn has(&self, tool: &str) -> bool {
        match tool {
            "gpu-screen-recorder" => self.gsr,
            "wf-recorder" => self.wf_recorder,
            "tesseract" => self.tesseract,
            "ffmpeg" => self.ffmpeg,
            other => self.gst_elements.contains(other),
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

fn gst_has(inspect: &Path, element: &str) -> bool {
    std::process::Command::new(inspect)
        .args(["--exists", element])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub output: OutputId,
    /// The clipboard sink writes under `<dir>/clipboard/`.
    pub dir: PathBuf,
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
        }))
    }
}

#[derive(Default)]
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
    // Introspecting an activatable but stopped portal would start it and can stall startup.
    let portals = if owned.contains(PORTAL_NAME) {
        let xml = zbus::blocking::fdo::IntrospectableProxy::builder(&conn)
            .destination(PORTAL_NAME)?
            .path(PORTAL_PATH)?
            .build()?
            .introspect()?;
        PortalIfaces::from_introspection(&xml)
    } else {
        PortalIfaces::default()
    };
    Ok((DbusNames { names }, portals))
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
