mod capture;
mod clipboard;
mod globals;
mod input;
mod keyboard;
mod output;
mod shm;
#[cfg(test)]
mod sway_tests;
mod toplevel;
mod worker;

pub use globals::WlGlobals;
pub use input::CursorSession;
pub use toplevel::Toplevel;
pub use worker::Connection;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot connect to the Wayland display: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    #[error("Wayland protocol error: {0}")]
    Protocol(String),
    #[error("the compositor does not support {0}")]
    Unsupported(&'static str),
    #[error("timed out waiting for the compositor")]
    Timeout,
    #[error("no output named {0}")]
    UnknownOutput(String),
    #[error("no window with identifier {0}")]
    UnknownToplevel(String),
    #[error("not an absolute path: {}", .0.display())]
    NotAbsolute(std::path::PathBuf),
    #[error("the point is not on any output")]
    OffScreen,
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("the Wayland worker has stopped")]
    Disconnected,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rustix::io::Errno> for Error {
    fn from(e: rustix::io::Errno) -> Self {
        Error::Io(e.into())
    }
}
