use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const REQUEST_TIMEOUT: Duration = Duration::from_millis(50);

pub const REQUEST_SOCKET: &str = ".socket.sock";
pub const EVENT_SOCKET: &str = ".socket2.sock";

/// One short-lived connection per request: Hyprland stalls while a `.socket.sock` client stays open.
#[derive(Debug, Clone)]
pub struct Ipc {
    dir: PathBuf,
    timeout: Duration,
}

impl Ipc {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_timeout(dir, REQUEST_TIMEOUT)
    }

    pub fn with_timeout(dir: impl Into<PathBuf>, timeout: Duration) -> Self {
        Ipc {
            dir: dir.into(),
            timeout,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn request(&self, command: &str) -> Result<Vec<u8>> {
        let deadline = Instant::now() + self.timeout;
        let path = self.dir.join(REQUEST_SOCKET);
        let io = |source: std::io::Error| match source.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => Error::Timeout,
            _ => Error::Io {
                path: path.clone(),
                source,
            },
        };
        let mut stream = UnixStream::connect(&path).map_err(io)?;
        stream.set_write_timeout(Some(self.timeout)).map_err(io)?;
        stream.write_all(command.as_bytes()).map_err(io)?;
        let mut reply = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::Timeout);
            }
            stream.set_read_timeout(Some(left)).map_err(io)?;
            match stream.read(&mut buf) {
                Ok(0) => return Ok(reply),
                Ok(n) => reply.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(io(e)),
            }
        }
    }

    pub fn json<T: DeserializeOwned>(&self, command: &str) -> Result<T> {
        let reply = self.request(&format!("j/{command}"))?;
        serde_json::from_slice(&reply)
            .map_err(|e| Error::Protocol(format!("hyprland {command}: {e}")))
    }

    pub fn eval(&self, lua: &str) -> Result<()> {
        let reply = self.request(&format!("eval {lua}"))?;
        let reply = String::from_utf8_lossy(&reply);
        match reply.trim() {
            "ok" => Ok(()),
            other => Err(Error::Protocol(format!("hyprland eval: {other}"))),
        }
    }

    pub fn has_socket(&self) -> bool {
        self.dir.join(REQUEST_SOCKET).exists()
    }

    pub fn connect_events(&self) -> Result<UnixStream> {
        let path = self.dir.join(EVENT_SOCKET);
        UnixStream::connect(&path).map_err(|source| Error::Io { path, source })
    }
}
