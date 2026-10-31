use crate::error::BackendError;

#[derive(Debug, Default)]
pub struct WlGlobals {}
#[derive(Debug, Default)]
pub struct DbusNames {}
#[derive(Debug, Default)]
pub struct PortalIfaces {}
#[derive(Debug, Default)]
pub struct Env {}
#[derive(Debug, Default)]
pub struct Tools {}
#[derive(Debug, Default)]
pub struct TestConfig {}

pub struct Probe<'a> {
    pub globals: &'a WlGlobals,
    pub dbus: &'a DbusNames,
    pub portals: &'a PortalIfaces,
    pub env: &'a Env,
    pub tools: &'a Tools,
    pub test: Option<&'a TestConfig>,
}

pub struct Provider<T: ?Sized> {
    pub name: &'static str,
    /// Priority; higher wins, `None` means not applicable.
    pub probe: fn(&Probe) -> Option<u8>,
    pub build: fn(&Probe) -> std::result::Result<Box<T>, BackendError>,
}
