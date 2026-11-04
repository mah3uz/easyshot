use crate::mute::{AudioSource, Mutes, Shape, sources};
use crate::process::{Clock, Helper, SystemClock, Timing, run_blocking, which};
use crate::project::CommandRunner;
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    probed: Arc<Mutex<Option<Probed>>>,
    audio: Option<Arc<dyn CommandRunner>>,
    split: Arc<AtomicBool>,
}

type Layout = (String, es_geom::Rect, (u32, u32));
type Probed = (Vec<Layout>, RegionUnits);

fn layout(outputs: &[OutputInfo]) -> Vec<Layout> {
    outputs
        .iter()
        .map(|o| (o.id.0.clone(), o.logical, o.physical))
        .collect()
}

// Off every monitor, so the recorder lists its monitors and exits without recording.
const PROBE_ARGS: [&str; 6] = ["-w", "1x1+1000000+1000000", "-c", "mkv", "-o", "/dev/null"];

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
            probed: Arc::default(),
            audio: None,
            split: Arc::default(),
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

    /// Runs `pactl` to find and mute the recorder's own sound streams; without it nothing is muted.
    pub fn with_audio_runner(mut self, runner: Arc<dyn CommandRunner>) -> Self {
        self.audio = Some(runner);
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

    /// Runs the recorder against a region off every monitor so it prints its monitor list, once per output layout.
    pub fn probe_units(&self, outputs: &[OutputInfo]) -> RegionUnits {
        let units = if !needs_unit_probe(outputs) {
            RegionUnits::Logical
        } else {
            let now = layout(outputs);
            let cached = self.probed.lock().ok().and_then(|p| {
                p.as_ref()
                    .filter(|(l, _)| *l == now)
                    .map(|(_, u)| u.clone())
            });
            match cached {
                Some(units) => units,
                None => {
                    let units = self.run_probe(outputs);
                    if let Ok(mut p) = self.probed.lock() {
                        *p = Some((now, units.clone()));
                    }
                    units
                }
            }
        };
        self.set_units(units.clone());
        units
    }

    fn run_probe(&self, outputs: &[OutputInfo]) -> RegionUnits {
        let out = std::process::Command::new(&self.program)
            .args(PROBE_ARGS)
            .envs(&self.env)
            .stdin(std::process::Stdio::null())
            .output();
        let Ok(out) = out else {
            return RegionUnits::Logical;
        };
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        decide_units(&parse_monitor_list(&text), outputs)
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
        "" | "default" | "default_input" => "default_input".into(),
        m if m.starts_with("device:") || m == "default_output" => m.into(),
        m => format!("device:{m}"),
    }
}

pub fn early_exit(code: Option<i32>, status: &str, stderr: &str) -> Error {
    match code {
        Some(50) => Error::AudioDeviceMissing,
        Some(51) => Error::AreaOffMonitor,
        _ => Error::Protocol(format!(
            "gpu-screen-recorder exited early ({status}): {stderr}"
        )),
    }
}

pub fn gsr_args(
    spec: &RecordSpec,
    portal: bool,
    units: &RegionUnits,
    split: bool,
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
    let audio: Vec<String> = match (spec.system_audio, &spec.mic) {
        (true, Some(m)) if split => vec!["default_output".into(), mic_arg(m)],
        (true, Some(m)) => vec![format!("default_output|{}", mic_arg(m))],
        (true, None) => vec!["default_output".into()],
        (false, Some(m)) => vec![mic_arg(m)],
        (false, None) => vec![],
    };
    for source in &audio {
        push(&["-a", source]);
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
    fn prepare(&self, spec: &RecordSpec, outputs: &[OutputInfo]) {
        if !matches!(spec.source, RecordSource::Output(_)) {
            self.probe_units(outputs);
        }
    }

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
        let ipc = tempfile::Builder::new()
            .prefix("easyshot-gsr-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(&self.ipc_dir)
            .map_err(|source| Error::Io {
                path: self.ipc_dir.clone(),
                source,
            })?;
        let sock = ipc.path().join("gsr.sock");
        let (tx, rx) = async_channel::unbounded();
        let both = spec.system_audio && spec.mic.is_some();
        let mut split = both && self.split.load(Ordering::SeqCst);
        let begun = self.clock.monotonic_us();
        let mut run = self.launch(&spec, split, &sock, &out, &tx)?;
        let mutes = self
            .audio
            .clone()
            .map(|runner| Mutes::new(runner, sources(spec.system_audio, spec.mic.is_some())));
        if both && !split && mutes.as_ref().is_some_and(|m| self.merged(m, &run.0)) {
            tracing::info!("the recorder mixes its sound into one stream: recording two tracks");
            self.split.store(true, Ordering::SeqCst);
            run.1.store(true, Ordering::SeqCst);
            let graceful = ipc_call(&sock, "stop", None, false).is_ok();
            run.0.stop(&*self.clock, &self.timing, graceful);
            for stale in [
                sock.clone(),
                out.clone(),
                PathBuf::from(format!("{}.ts", out.display())),
            ] {
                let _ = std::fs::remove_file(stale);
            }
            split = true;
            run = self.launch(&spec, split, &sock, &out, &tx)?;
        }
        let (helper, stopping) = run;
        let first = read_first_frame_ts(&out).unwrap_or(begun);
        let _ = tx.try_send(RecorderEvent::Started);
        Ok(Box::new(GsrHandle {
            helper,
            _ipc: ipc,
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
            mutes,
            split,
        }))
    }
}

impl GsrRecorder {
    fn launch(
        &self,
        spec: &RecordSpec,
        split: bool,
        sock: &Path,
        out: &Path,
        tx: &async_channel::Sender<RecorderEvent>,
    ) -> Result<(Helper, Arc<AtomicBool>)> {
        let units = self.units.lock().map(|u| u.clone()).unwrap_or_default();
        let args = gsr_args(spec, self.portal, &units, split, sock, out);
        let stopping = Arc::new(AtomicBool::new(false));
        let on_exit: Box<dyn FnOnce(std::process::ExitStatus) + Send> = {
            let (tx, stopping) = (tx.clone(), stopping.clone());
            Box::new(move |st| {
                if !stopping.load(Ordering::SeqCst) {
                    let _ = tx.try_send(RecorderEvent::Failed(format!("recorder exited: {st}")));
                }
            })
        };
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
                return Err(early_exit(
                    st.code(),
                    &st.to_string(),
                    &helper.stderr_tail(),
                ));
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
        Ok((helper, stopping))
    }

    /// True when the sound server shows fewer capture streams than the recording has sources.
    fn merged(&self, mutes: &Mutes, helper: &Helper) -> bool {
        let t0 = self.clock.now();
        loop {
            let found = match mutes.streams(helper.pid()) {
                Ok(found) => found.len(),
                Err(e) => {
                    tracing::warn!(error = %e, "the recorder's sound streams");
                    return false;
                }
            };
            if Shape::of(found, 2) == Shape::PerSource
                || self.clock.now().saturating_sub(t0) >= self.timing.ipc_wait
            {
                return Shape::of(found, 2) == Shape::Merged;
            }
            self.clock.sleep(self.timing.poll);
        }
    }
}

struct GsrHandle {
    helper: Helper,
    _ipc: tempfile::TempDir,
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
    mutes: Option<Mutes>,
    split: bool,
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

impl RecordingHandle for GsrHandle {
    fn pause(&mut self) -> Result<()> {
        self.set_paused(true)
    }

    fn resume(&mut self) -> Result<()> {
        self.set_paused(false)
    }

    fn set_muted(&mut self, source: AudioSource, muted: bool) -> Result<()> {
        let mutes = self.mutes.as_ref().ok_or(Error::Unsupported)?;
        mutes.set(self.helper.pid(), source, muted)
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
                split_audio: self.split,
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
