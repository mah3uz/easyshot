use crate::{Error, Result};
use futures::future::BoxFuture;
use rustix::process::{Pid, Signal, kill_process, kill_process_group};
use std::collections::{HashMap, VecDeque};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub trait Clock: Send + Sync {
    fn now(&self) -> Duration;
    fn sleep(&self, d: Duration);
    fn monotonic_us(&self) -> u64;
}

pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }

    fn monotonic_us(&self) -> u64 {
        let t = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        (t.tv_sec as u64) * 1_000_000 + (t.tv_nsec as u64) / 1000
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub ipc_grace: Duration,
    pub kill_after: Duration,
    pub poll: Duration,
    pub watchdog: Duration,
    pub ipc_wait: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            ipc_grace: Duration::from_secs(5),
            kill_after: Duration::from_secs(10),
            poll: Duration::from_millis(50),
            watchdog: Duration::from_secs(10),
            ipc_wait: Duration::from_secs(3),
        }
    }
}

#[derive(Default)]
struct Shared {
    status: Mutex<Option<ExitStatus>>,
    exited: Condvar,
}

const KEPT_LINES: usize = 50;
const MAX_LINE: usize = 4096;

// A death signal is dropped on exec of a file-capability binary; the watcher outlives us instead.
const WATCHER: &str = "trap '' INT; if ! read -r _; then kill -INT 0; sleep 10; kill -KILL 0; fi";

type Lines = Arc<Mutex<VecDeque<String>>>;

pub struct Helper {
    pid: u32,
    shared: Arc<Shared>,
    stdout: Lines,
    stderr: Lines,
    threads: Vec<JoinHandle<()>>,
}

fn snapshot(lines: &Lines) -> Vec<String> {
    lines
        .lock()
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default()
}

fn push_line(sink: &Lines, line: &[u8]) {
    if let Ok(mut v) = sink.lock() {
        if v.len() == KEPT_LINES {
            v.pop_front();
        }
        v.push_back(String::from_utf8_lossy(line).into_owned());
    }
}

fn pump(r: impl std::io::Read + Send + 'static, sink: Lines) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(r);
        let mut line = Vec::new();
        while let Ok(buf) = reader.fill_buf()
            && !buf.is_empty()
        {
            let (chunk, ended) = match buf.iter().position(|&b| b == b'\n') {
                Some(i) => (&buf[..i], Some(i + 1)),
                None => (buf, None),
            };
            let room = MAX_LINE.saturating_sub(line.len());
            line.extend_from_slice(&chunk[..chunk.len().min(room)]);
            let used = ended.unwrap_or(buf.len());
            reader.consume(used);
            if ended.is_some() {
                push_line(&sink, &line);
                line.clear();
            }
        }
        if !line.is_empty() {
            push_line(&sink, &line);
        }
    })
}

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

impl Helper {
    pub fn spawn(
        program: &Path,
        args: &[String],
        env: &HashMap<String, String>,
        on_exit: Option<Box<dyn FnOnce(ExitStatus) + Send>>,
    ) -> Result<Helper> {
        let (tx, rx) = std::sync::mpsc::channel();
        let program = program.to_path_buf();
        let args = args.to_vec();
        let env = env.clone();
        let shared = Arc::new(Shared::default());
        let stdout = Lines::default();
        let stderr = Lines::default();
        let (sh, so, se) = (shared.clone(), stdout.clone(), stderr.clone());
        std::thread::spawn(move || {
            let mut cmd = Command::new(&program);
            cmd.args(&args)
                .envs(&env)
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(io_err(&program, e)));
                    return;
                }
            };
            let watcher = Command::new("/bin/sh")
                .args(["-c", WATCHER])
                .process_group(child.id() as i32)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            let mut watcher = match watcher {
                Ok(w) => w,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = tx.send(Err(io_err(Path::new("/bin/sh"), e)));
                    return;
                }
            };
            let lifeline = watcher.stdin.take();
            let mut threads = Vec::new();
            if let Some(o) = child.stdout.take() {
                threads.push(pump(o, so));
            }
            if let Some(e) = child.stderr.take() {
                threads.push(pump(e, se));
            }
            let _ = tx.send(Ok((child.id(), threads)));
            let status = child.wait();
            if let Some(mut l) = lifeline {
                let _ = l.write_all(b"\n");
            }
            let _ = watcher.wait();
            if let Ok(s) = status {
                if let Ok(mut g) = sh.status.lock() {
                    *g = Some(s);
                }
                sh.exited.notify_all();
                if let Some(f) = on_exit {
                    f(s);
                }
            }
        });
        let (pid, threads) = rx
            .recv()
            .map_err(|_| Error::Protocol("spawn thread died".into()))??;
        Ok(Helper {
            pid,
            shared,
            stdout,
            stderr,
            threads,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn exited(&self) -> Option<ExitStatus> {
        self.shared.status.lock().ok().and_then(|g| *g)
    }

    /// SIGKILL goes to the whole group so the watcher dies with the helper.
    pub fn signal(&self, sig: Signal) {
        let guard = self.shared.status.lock();
        if guard.map(|g| g.is_some()).unwrap_or(true) {
            return;
        }
        if let Some(pid) = Pid::from_raw(self.pid as i32) {
            let _ = if sig == Signal::KILL {
                kill_process_group(pid, sig)
            } else {
                kill_process(pid, sig)
            };
        }
    }

    pub fn stdout_lines(&self) -> Vec<String> {
        snapshot(&self.stdout)
    }

    pub fn stderr_tail(&self) -> String {
        let v = snapshot(&self.stderr);
        v[v.len().saturating_sub(5)..].join("\n")
    }

    /// With `graceful` the caller has just asked the program to stop on its own; without it
    /// the ladder starts at SIGINT. Always returns with the child reaped.
    pub fn stop(
        &mut self,
        clock: &dyn Clock,
        timing: &Timing,
        graceful: bool,
    ) -> Option<ExitStatus> {
        let start = clock.now();
        let mut interrupted = !graceful;
        if interrupted {
            self.signal(Signal::INT);
        }
        let mut killed = false;
        while self.exited().is_none() {
            let elapsed = clock.now().saturating_sub(start);
            if !killed && elapsed >= timing.kill_after {
                self.signal(Signal::KILL);
                killed = true;
            } else if !interrupted && elapsed >= timing.ipc_grace {
                self.signal(Signal::INT);
                interrupted = true;
            }
            if killed {
                self.wait_real(Duration::from_millis(20));
            } else {
                clock.sleep(timing.poll);
            }
        }
        self.join_readers();
        self.exited()
    }

    fn wait_real(&self, d: Duration) {
        if let Ok(g) = self.shared.status.lock() {
            let _ = self.shared.exited.wait_timeout_while(g, d, |s| s.is_none());
        }
    }

    fn join_readers(&mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.signal(Signal::KILL);
    }
}

pub fn which(program: impl AsRef<OsStr>) -> Option<std::path::PathBuf> {
    let p = Path::new(program.as_ref());
    if p.components().count() > 1 {
        return p.is_file().then(|| p.to_path_buf());
    }
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(|d| Path::new(d).join(p))
        .find(|c| c.is_file())
}

pub(crate) fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> BoxFuture<'static, Result<T>> {
    let (tx, rx) = futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    Box::pin(async move {
        rx.await
            .unwrap_or_else(|_| Err(Error::Protocol("stop thread died".into())))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_keeps_the_last_lines_each_cut_to_a_bound() {
        let mut data = Vec::new();
        for i in 0..200 {
            data.extend_from_slice(format!("line {i}\n").as_bytes());
        }
        data.extend(std::iter::repeat_n(b'\r', 1 << 20));
        let sink = Lines::default();
        pump(std::io::Cursor::new(data), sink.clone())
            .join()
            .expect("pump");
        let lines = snapshot(&sink);
        assert_eq!(lines.len(), KEPT_LINES);
        assert_eq!(lines[0], "line 151");
        assert_eq!(lines[KEPT_LINES - 1].len(), MAX_LINE);
    }
}
