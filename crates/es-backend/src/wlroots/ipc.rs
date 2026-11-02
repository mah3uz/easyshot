use crate::error::{Error, Result};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const MAGIC: &[u8; 6] = b"i3-ipc";
const HEADER: usize = MAGIC.len() + 8;
const MAX_PAYLOAD: usize = 64 << 20;
pub(crate) const TIMEOUT: Duration = Duration::from_millis(50);

pub(crate) const RUN_COMMAND: u32 = 0;
pub(crate) const SUBSCRIBE: u32 = 2;
pub(crate) const GET_OUTPUTS: u32 = 3;
pub(crate) const GET_TREE: u32 = 4;
pub(crate) const GET_CONFIG: u32 = 9;
pub(crate) const EVENT_BIT: u32 = 1 << 31;

pub(crate) fn encode(kind: u32, payload: &[u8]) -> Result<Vec<u8>> {
    let len = u32::try_from(payload.len())
        .map_err(|_| Error::Protocol("i3-ipc payload too large".into()))?;
    let mut msg = Vec::with_capacity(HEADER + payload.len());
    msg.extend_from_slice(MAGIC);
    msg.extend_from_slice(&len.to_ne_bytes());
    msg.extend_from_slice(&kind.to_ne_bytes());
    msg.extend_from_slice(payload);
    Ok(msg)
}

/// Returns `(payload length, message type)`.
pub(crate) fn decode_header(h: &[u8; HEADER]) -> Result<(usize, u32)> {
    if &h[..MAGIC.len()] != MAGIC {
        return Err(Error::Protocol("bad i3-ipc magic".into()));
    }
    let word = |at: usize| u32::from_ne_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]]);
    let len = word(MAGIC.len()) as usize;
    if len > MAX_PAYLOAD {
        return Err(Error::Protocol(format!("i3-ipc payload of {len} bytes")));
    }
    Ok((len, word(MAGIC.len() + 4)))
}

#[derive(Debug, Clone)]
pub(crate) struct Ipc {
    pub(crate) path: PathBuf,
}

impl Ipc {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Ipc { path: path.into() }
    }

    fn io(&self, source: std::io::Error) -> Error {
        io_error(&self.path, source)
    }

    fn connect(&self) -> Result<UnixStream> {
        let s = UnixStream::connect(&self.path).map_err(|e| self.io(e))?;
        s.set_write_timeout(Some(TIMEOUT)).map_err(|e| self.io(e))?;
        Ok(s)
    }

    pub(crate) fn request(&self, kind: u32, payload: &[u8]) -> Result<Vec<u8>> {
        let deadline = Instant::now() + TIMEOUT;
        let mut s = self.connect()?;
        send(&mut s, &self.path, kind, payload)?;
        let (got, reply) = read_message(&mut s, &self.path, Some(deadline))?;
        if got != kind {
            return Err(Error::Protocol(format!(
                "i3-ipc reply type {got} to request {kind}"
            )));
        }
        Ok(reply)
    }

    pub(crate) fn json<T: serde::de::DeserializeOwned>(
        &self,
        kind: u32,
        payload: &[u8],
    ) -> Result<T> {
        parse(&self.request(kind, payload)?)
    }

    /// The stream carries only events once this returns.
    pub(crate) fn subscribe(&self, events: &[&str]) -> Result<UnixStream> {
        let deadline = Instant::now() + TIMEOUT;
        let mut s = self.connect()?;
        let body = serde_json::to_vec(events).map_err(|e| Error::Protocol(e.to_string()))?;
        send(&mut s, &self.path, SUBSCRIBE, &body)?;
        let (got, reply) = read_message(&mut s, &self.path, Some(deadline))?;
        #[derive(serde::Deserialize)]
        struct Ack {
            success: bool,
        }
        if got != SUBSCRIBE || !parse::<Ack>(&reply)?.success {
            return Err(Error::Protocol("sway refused the subscription".into()));
        }
        s.set_read_timeout(None).map_err(|e| self.io(e))?;
        Ok(s)
    }
}

pub(crate) fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_slice(bytes).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

fn send(s: &mut UnixStream, path: &Path, kind: u32, payload: &[u8]) -> Result<()> {
    s.write_all(&encode(kind, payload)?)
        .map_err(|e| match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => Error::Timeout,
            _ => io_error(path, e),
        })
}

/// With no deadline the read blocks until a whole message or the socket closes.
pub(crate) fn read_message(
    s: &mut UnixStream,
    path: &Path,
    deadline: Option<Instant>,
) -> Result<(u32, Vec<u8>)> {
    let mut header = [0u8; HEADER];
    read_exact_by(s, path, &mut header, deadline)?;
    let (len, kind) = decode_header(&header)?;
    let mut payload = vec![0u8; len];
    read_exact_by(s, path, &mut payload, deadline)?;
    Ok((kind, payload))
}

fn read_exact_by(
    s: &mut UnixStream,
    path: &Path,
    buf: &mut [u8],
    deadline: Option<Instant>,
) -> Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        if let Some(deadline) = deadline {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Error::Timeout);
            }
            s.set_read_timeout(Some(left))
                .map_err(|e| io_error(path, e))?;
        }
        match s.read(&mut buf[filled..]) {
            Ok(0) => return Err(Error::Protocol("sway closed the IPC socket".into())),
            Ok(n) => filled += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(Error::Timeout);
            }
            Err(e) => return Err(io_error(path, e)),
        }
    }
    Ok(())
}
