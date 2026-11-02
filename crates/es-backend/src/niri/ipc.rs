use crate::error::{Error, Result};
use niri_ipc::{Reply, Request, Response};
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub(crate) const TIMEOUT: Duration = Duration::from_millis(50);
const MAX_LINE: usize = 64 << 20;

#[derive(Debug, Clone)]
pub(crate) struct Ipc {
    pub(crate) path: PathBuf,
}

fn io_error(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

pub(crate) fn parse<T: serde::de::DeserializeOwned>(line: &[u8]) -> Result<T> {
    serde_json::from_slice(line).map_err(|e| Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    })
}

impl Ipc {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Ipc { path: path.into() }
    }

    fn open(
        &self,
        request: &Request,
        deadline: Instant,
    ) -> Result<(BufReader<UnixStream>, Response)> {
        let mut s = UnixStream::connect(&self.path).map_err(|e| io_error(&self.path, e))?;
        s.set_write_timeout(Some(TIMEOUT))
            .map_err(|e| io_error(&self.path, e))?;
        let mut line = serde_json::to_vec(request).map_err(|e| Error::Protocol(e.to_string()))?;
        line.push(b'\n');
        s.write_all(&line).map_err(|e| match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => Error::Timeout,
            _ => io_error(&self.path, e),
        })?;
        let mut r = BufReader::new(s);
        let reply = read_line_by(&mut r, &self.path, deadline)?;
        match parse::<Reply>(&reply)? {
            Ok(response) => Ok((r, response)),
            Err(message) => Err(Error::Protocol(format!("niri: {message}"))),
        }
    }

    pub(crate) fn request(&self, request: &Request) -> Result<Response> {
        Ok(self.open(request, Instant::now() + TIMEOUT)?.1)
    }

    /// One event per line follows; the reader blocks until the socket closes.
    pub(crate) fn event_stream(&self) -> Result<(UnixStream, BufReader<UnixStream>)> {
        let (r, response) = self.open(&Request::EventStream, Instant::now() + TIMEOUT)?;
        if !matches!(response, Response::Handled) {
            return Err(Error::Protocol(
                "niri did not start the event stream".into(),
            ));
        }
        r.get_ref()
            .set_read_timeout(None)
            .map_err(|e| io_error(&self.path, e))?;
        let handle = r
            .get_ref()
            .try_clone()
            .map_err(|e| io_error(&self.path, e))?;
        Ok((handle, r))
    }
}

pub(crate) fn unexpected(what: &str) -> Error {
    Error::Protocol(format!("niri answered {what} with another response"))
}

fn read_line_by(r: &mut BufReader<UnixStream>, path: &Path, deadline: Instant) -> Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(Error::Timeout);
        }
        r.get_ref()
            .set_read_timeout(Some(left))
            .map_err(|e| io_error(path, e))?;
        let chunk = match r.fill_buf() {
            Ok([]) => return Err(Error::Protocol("niri closed the IPC socket".into())),
            Ok(chunk) => chunk,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Err(Error::Timeout);
            }
            Err(e) => return Err(io_error(path, e)),
        };
        let (take, done) = match chunk.iter().position(|b| *b == b'\n') {
            Some(i) => (i + 1, true),
            None => (chunk.len(), false),
        };
        line.extend_from_slice(&chunk[..take]);
        r.consume(take);
        if done {
            line.pop();
            return Ok(line);
        }
        if line.len() > MAX_LINE {
            return Err(Error::Protocol("niri reply too long".into()));
        }
    }
}

pub(crate) fn next_line(reader: &mut BufReader<UnixStream>, buf: &mut Vec<u8>) -> Result<bool> {
    buf.clear();
    let n = reader
        .by_ref()
        .take(MAX_LINE as u64)
        .read_until(b'\n', buf)
        .map_err(|e| Error::Protocol(format!("niri event stream: {e}")))?;
    Ok(n > 0)
}
