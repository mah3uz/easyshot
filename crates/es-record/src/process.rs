use crate::{Error, Result};
use rustix::process::{Pid, Signal, kill_process};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
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

pub struct Helper {
    pid: u32,
    shared: Arc<Shared>,
    stdout: Arc<Mutex<Vec<String>>>,
    stderr: Arc<Mutex<Vec<String>>>,
    threads: Vec<JoinHandle<()>>,
}

fn pump(r: impl std::io::Read + Send + 'static, sink: Arc<Mutex<Vec<String>>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(r).lines().map_while(std::result::Result::ok) {
            if let Ok(mut v) = sink.lock() {
                v.push(line);
            }
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
    /// The spawning thread stays alive until the child exits: the death signal is tied to it.
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
        let stdout = Arc::new(Mutex::new(Vec::new()));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let (sh, so, se) = (shared.clone(), stdout.clone(), stderr.clone());
        std::thread::spawn(move || {
            let mut cmd = if which("setpriv").is_some() {
                let mut c = Command::new("setpriv");
                c.args(["--pdeathsig", "INT", "--"]).arg(&program);
                c
            } else {
                Command::new(&program)
            };
            cmd.args(&args)
                .envs(&env)
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
            let mut threads = Vec::new();
            if let Some(o) = child.stdout.take() {
                threads.push(pump(o, so));
            }
            if let Some(e) = child.stderr.take() {
                threads.push(pump(e, se));
            }
            let _ = tx.send(Ok((child.id(), threads)));
            let status = child.wait();
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

    pub fn exited(&self) -> Option<ExitStatus> {
        self.shared.status.lock().ok().and_then(|g| *g)
    }

    pub fn signal(&self, sig: Signal) {
        let guard = self.shared.status.lock();
        if guard.map(|g| g.is_some()).unwrap_or(true) {
            return;
        }
        if let Some(pid) = Pid::from_raw(self.pid as i32) {
            let _ = kill_process(pid, sig);
        }
    }

    pub fn stdout_lines(&self) -> Vec<String> {
        self.stdout.lock().map(|v| v.clone()).unwrap_or_default()
    }

    pub fn stderr_tail(&self) -> String {
        let v = self.stderr.lock().map(|v| v.clone()).unwrap_or_default();
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
