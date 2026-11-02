use crate::process::{Clock, Helper, SystemClock, Timing, which};
use crate::units::{RegionUnits, decide_units, needs_unit_probe, parse_monitor_list};
use crate::{
    Codec, Container, Error, Quality, RecordSource, RecordSpec, Recorder, RecorderEvent,
    RecorderProbe, RecordingHandle, RecordingOutput, Result,
};
use es_model::platform::OutputInfo;
use futures::future::BoxFuture;
use rustix::process::Signal;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SOCKET_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct GsrRecorder {
    portal: bool,
    program: PathBuf,
    kms_server: Option<PathBuf>,
    trust_kms: bool,
    ipc_dir: PathBuf,
    timing: Timing,
    clock: Arc<dyn Clock>,
    env: HashMap<String, String>,
    units: Arc<Mutex<RegionUnits>>,
}

impl GsrRecorder {
    fn new(portal: bool) -> Self {
        Self {
            portal,
            program: "gpu-screen-recorder".into(),
            kms_server: None,
            trust_kms: false,
            ipc_dir: std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir),
            timing: Timing::default(),
            clock: Arc::new(SystemClock::default()),
            env: HashMap::new(),
            units: Arc::default(),
        }
    }

    pub fn native() -> Self {
        Self::new(false)
    }

    pub fn portal() -> Self {
        Self::new(true)
    }

    pub fn with_program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    pub fn with_kms_server(mut self, path: impl Into<PathBuf>) -> Self {
        self.kms_server = Some(path.into());
        self
    }

    pub fn trust_kms(mut self) -> Self {
        self.trust_kms = true;
        self
    }

    pub fn with_ipc_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.ipc_dir = dir.into();
        self
    }

    pub fn with_timing(mut self, timing: Timing) -> Self {
        self.timing = timing;
        self
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn set_units(&self, units: RegionUnits) {
        if let Ok(mut u) = self.units.lock() {
            *u = units;
        }
    }

    pub fn version(&self) -> Option<String> {
        let out = std::process::Command::new(&self.program)
            .arg("--version")
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        (!text.is_empty()).then_some(text)
    }

    /// Runs the recorder against a 1x1 region so it prints its monitor list; the caller decides when.
    pub fn probe_units(&self, outputs: &[OutputInfo]) -> RegionUnits {
        if !needs_unit_probe(outputs) {
            return RegionUnits::Logical;
        }
        let out = std::process::Command::new(&self.program)
            .args(["-w", "1x1+0+0", "-o", "/dev/null"])
            .output();
        let Ok(out) = out else {
            return RegionUnits::Logical;
        };
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        let units = decide_units(&parse_monitor_list(&text), outputs);
        self.set_units(units.clone());
        units
    }

    fn kms_problem(&self) -> Option<String> {
        let Some(server) = self.kms_server.clone().or_else(|| which("gsr-kms-server")) else {
            return Some("gsr-kms-server not found".into());
        };
        let setuid = std::fs::metadata(&server)
            .map(|m| m.permissions().mode() & 0o4000 != 0)
            .unwrap_or(false);
        let mut buf = [0u8; 64];
        let capable = rustix::fs::getxattr(&server, "security.capability", &mut buf).is_ok();
        (!setuid && !capable).then(|| "gsr-kms-server lacks cap_sys_admin".into())
    }
}

fn codec_name(c: Codec) -> &'static str {
    match c {
        Codec::H264 => "h264",
        Codec::H265 => "hevc",
        Codec::Av1 => "av1",
        Codec::Vp9 => "vp9",
    }
}

pub(crate) fn container_ext(c: Container) -> &'static str {
    match c {
        Container::Mkv => "mkv",
        Container::Mp4 => "mp4",
        Container::Webm => "webm",
    }
}

fn quality_name(q: Quality) -> &'static str {
    match q {
        Quality::Medium => "medium",
        Quality::High => "high",
        Quality::VeryHigh => "very_high",
        Quality::Ultra => "ultra",
    }
}

fn mic_arg(mic: &str) -> String {
    match mic {
        "" | "default_input" => "default_input".into(),
        m if m.starts_with("device:") || m == "default_output" => m.into(),
        m => format!("device:{m}"),
    }
}

pub fn gsr_args(
    spec: &RecordSpec,
    portal: bool,
    units: &RegionUnits,
    ipc: &Path,
    out: &Path,
) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| s.to_string()));
    if portal {
        push(&["-w", "portal", "-restore-portal-session", "yes"]);
    } else {
        let target = match &spec.source {
            RecordSource::Output(id) => id.0.clone(),
            RecordSource::Region { output, logical } => units.region_target(output, *logical),
            RecordSource::Window(w) => match (&w.output, w.logical) {
                (Some(o), Some(r)) => units.region_target(o, r),
                _ => String::new(),
            },
        };
        push(&["-w", &target]);
    }
    push(&["-f", &spec.fps.to_string()]);
    push(&["-k", codec_name(spec.codec)]);
    push(&["-c", container_ext(spec.container)]);
    push(&["-q", quality_name(spec.quality)]);
    if spec.system_audio {
        push(&["-a", "default_output"]);
    }
    if let Some(m) = &spec.mic {
        push(&["-a", &mic_arg(m)]);
    }
    push(&["-cursor", if spec.cursor { "yes" } else { "no" }]);
    push(&["-ipc", &ipc.to_string_lossy()]);
    push(&["-write-first-frame-ts", "yes"]);
    push(&["-o", &out.to_string_lossy()]);
    a
}

fn ipc_call(sock: &Path, name: &str, data: Option<bool>, expect_reply: bool) -> Result<()> {
    let io = |source| Error::Io {
        path: sock.to_path_buf(),
        source,
    };
    let mut s = UnixStream::connect(sock).map_err(io)?;
    let t = Some(Duration::from_secs(2));
    s.set_read_timeout(t).map_err(io)?;
    s.set_write_timeout(t).map_err(io)?;
    let msg = match data {
        Some(d) => format!(r#"{{"id":1,"name":"{name}","data":{d}}}"#),
        None => format!(r#"{{"id":1,"name":"{name}"}}"#),
    };
    writeln!(s, "{msg}").map_err(io)?;
    if expect_reply {
        let mut line = String::new();
        BufReader::new(&s).read_line(&mut line).map_err(io)?;
        if !line.contains("\"ok\"") {
            return Err(Error::Protocol(format!("ipc {name}: {}", line.trim())));
        }
    }
    Ok(())
}

pub(crate) fn read_first_frame_ts(out: &Path) -> Option<u64> {
    let ts = PathBuf::from(format!("{}.ts", out.display()));
    let text = std::fs::read_to_string(ts).ok()?;
    text.lines().nth(1)?.split_whitespace().next()?.parse().ok()
}

impl Recorder for GsrRecorder {
    fn name(&self) -> &'static str {
        if self.portal {
            "gsr-portal"
        } else {
            "gsr-native"
        }
    }

    fn probe(&self) -> RecorderProbe {
        let problem = if which(&self.program).is_none() {
            Some("gpu-screen-recorder not found".to_owned())
        } else if !self.portal && !self.trust_kms {
            self.kms_problem()
        } else {
            None
        };
        RecorderProbe {
            available: problem.is_none(),
            reason: problem,
        }
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        std::fs::create_dir_all(&spec.dir).map_err(|source| Error::Io {
            path: spec.dir.clone(),
            source,
        })?;
        let out = spec
            .dir
            .join(format!("screen.{}", container_ext(spec.container)));
        let sock = self.ipc_dir.join(format!(
            "easyshot-gsr-{}-{}.sock",
            std::process::id(),
            SOCKET_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let units = self.units.lock().map(|u| u.clone()).unwrap_or_default();
        let args = gsr_args(&spec, self.portal, &units, &sock, &out);

        let (tx, rx) = async_channel::unbounded();
        let stopping = Arc::new(AtomicBool::new(false));
        let on_exit: Box<dyn FnOnce(std::process::ExitStatus) + Send> = {
            let (tx, stopping) = (tx.clone(), stopping.clone());
            Box::new(move |st| {
                if !stopping.load(Ordering::SeqCst) {
                    let _ = tx.try_send(RecorderEvent::Failed(format!("recorder exited: {st}")));
                }
            })
        };
        let begun = self.clock.monotonic_us();
        let mut helper = Helper::spawn(&self.program, &args, &self.env, Some(on_exit))?;

        let t0 = self.clock.now();
        let limit = if self.portal {
            self.timing.watchdog
        } else {
            self.timing.ipc_wait
        };
        let ts_file = PathBuf::from(format!("{}.ts", out.display()));
        loop {
            if let Some(st) = helper.exited() {
                return Err(Error::Protocol(format!(
                    "gpu-screen-recorder exited early ({st}): {}",
                    helper.stderr_tail()
                )));
            }
            let ready = if self.portal {
                ts_file.exists()
            } else {
                sock.exists()
            };
            if ready {
                break;
            }
            if self.clock.now().saturating_sub(t0) >= limit {
                if self.portal {
                    helper.stop(&*self.clock, &self.timing, false);
                    return Err(Error::Timeout);
                }
                break;
            }
            self.clock.sleep(self.timing.poll);
        }
        let first = read_first_frame_ts(&out).unwrap_or(begun);
        let _ = tx.try_send(RecorderEvent::Started);
        Ok(Box::new(GsrHandle {
            helper,
            sock,
            out,
            dir: spec.dir,
            clock: self.clock.clone(),
            timing: self.timing,
            tx,
            rx,
            paused: false,
            stopping,
            seg_start: first,
            segments: Vec::new(),
        }))
    }
}

struct GsrHandle {
    helper: Helper,
    sock: PathBuf,
    out: PathBuf,
    dir: PathBuf,
    clock: Arc<dyn Clock>,
    timing: Timing,
    tx: async_channel::Sender<RecorderEvent>,
    rx: async_channel::Receiver<RecorderEvent>,
    paused: bool,
    stopping: Arc<AtomicBool>,
    seg_start: u64,
    segments: Vec<(u64, u64)>,
}

impl GsrHandle {
    fn set_paused(&mut self, want: bool) -> Result<()> {
        if self.paused == want {
            return Ok(());
        }
        if ipc_call(&self.sock, "set-paused", Some(want), true).is_err() {
            if self.helper.exited().is_some() {
                return Err(Error::Protocol("recorder is not running".into()));
            }
            self.helper.signal(Signal::USR2);
        }
        let now = self.clock.monotonic_us();
        if want {
            self.segments.push((self.seg_start, now));
        } else {
            self.seg_start = now;
        }
        self.paused = want;
        let _ = self.tx.try_send(if want {
            RecorderEvent::Paused
        } else {
            RecorderEvent::Resumed
        });
        Ok(())
    }

    fn shut_down(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        let graceful = ipc_call(&self.sock, "stop", None, false).is_ok();
        self.helper.stop(&*self.clock, &self.timing, graceful);
        let _ = std::fs::remove_file(&self.sock);
    }
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

impl RecordingHandle for GsrHandle {
    fn pause(&mut self) -> Result<()> {
        self.set_paused(true)
    }

    fn resume(&mut self) -> Result<()> {
        self.set_paused(false)
    }

    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
        run_blocking(move || {
            let end = self.clock.monotonic_us();
            self.shut_down();
            if !self.paused {
                let seg = (self.seg_start, end);
                self.segments.push(seg);
            }
            let first = read_first_frame_ts(&self.out).unwrap_or(self.seg_start);
            let reported = self
                .helper
                .stdout_lines()
                .into_iter()
                .rev()
                .map(|l| PathBuf::from(l.trim()))
                .find(|p| p.is_file());
            let file = match reported {
                Some(p) => p,
                None if self.out.is_file() => self.out.clone(),
                None => return Err(Error::NotFound(self.out.display().to_string())),
            };
            Ok(RecordingOutput {
                file,
                first_frame_monotonic_us: first,
                segments: std::mem::take(&mut self.segments),
            })
        })
    }

    fn discard(mut self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        run_blocking(move || {
            self.shut_down();
            std::fs::remove_dir_all(&self.dir).map_err(|source| Error::Io {
                path: self.dir.clone(),
                source,
            })
        })
    }

    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.rx.clone()
    }
}
