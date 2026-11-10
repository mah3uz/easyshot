use crate::gsr::container_ext;
use crate::mute::{Mutes, sources};
use crate::process::{Clock, Helper, SystemClock, Timing, run_blocking, which};
use crate::{
    AudioSource, Codec, CommandRunner, Error, RecordSource, RecordSpec, Recorder, RecorderEvent,
    RecorderProbe, RecordingHandle, RecordingOutput, Result,
};
use es_geom::Rect;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoder {
    pub name: &'static str,
    pub vaapi: bool,
}

pub fn pick_encoder(encoders: &str, codec: Codec, vaapi_device: bool) -> Result<Encoder> {
    let has = |n: &str| encoders.split_whitespace().any(|t| t == n);
    let candidates: &[(&'static str, bool)] = match codec {
        Codec::H264 => &[
            ("h264_vaapi", true),
            ("libx264", false),
            ("libvpx-vp9", false),
        ],
        Codec::H265 => &[("hevc_vaapi", true), ("libx265", false)],
        Codec::Vp9 => &[("libvpx-vp9", false)],
        Codec::Av1 => &[
            ("av1_vaapi", true),
            ("libsvtav1", false),
            ("libaom-av1", false),
        ],
    };
    candidates
        .iter()
        .find(|(n, vaapi)| has(n) && (!vaapi || vaapi_device))
        .map(|&(name, vaapi)| Encoder { name, vaapi })
        .ok_or(Error::Unsupported)
}

fn region_arg(r: Rect) -> String {
    format!(
        "{},{} {}x{}",
        r.x0.round(),
        r.y0.round(),
        r.width().round(),
        r.height().round()
    )
}

pub fn wf_args(
    spec: &RecordSpec,
    encoder: &Encoder,
    device: Option<&Path>,
    system_device: Option<&str>,
    file: &Path,
) -> Result<Vec<String>> {
    let mut a = vec![
        "-f".to_owned(),
        file.to_string_lossy().into_owned(),
        "-r".into(),
        spec.fps.to_string(),
        "-c".into(),
        encoder.name.into(),
    ];
    if let (true, Some(d)) = (encoder.vaapi, device) {
        a.extend(["-d".into(), d.to_string_lossy().into_owned()]);
    }
    match (&spec.mic, spec.system_audio) {
        (Some(_), true) => return Err(Error::Unsupported),
        (Some(m), false) if m.is_empty() || m == "default_input" => a.push("--audio".into()),
        (Some(m), false) => a.push(format!("--audio={m}")),
        (None, true) => a.push(format!(
            "--audio={}",
            system_device.ok_or(Error::Unsupported)?
        )),
        (None, false) => {}
    }
    match &spec.source {
        RecordSource::Output(id) => a.extend(["-o".into(), id.0.clone()]),
        RecordSource::Region { logical, .. } => a.extend(["-g".into(), region_arg(*logical)]),
        RecordSource::Window(w) => {
            let r = w.logical.ok_or(Error::Unsupported)?;
            a.extend(["-g".into(), region_arg(r)]);
        }
    }
    Ok(a)
}

pub fn concat_list(segments: &[PathBuf]) -> String {
    segments
        .iter()
        .map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''")))
        .collect()
}

pub struct WfRecorder {
    program: PathBuf,
    ffmpeg: PathBuf,
    encoders: String,
    vaapi_device: Option<PathBuf>,
    system_device: Option<String>,
    timing: Timing,
    clock: Arc<dyn Clock>,
    env: HashMap<String, String>,
    audio: Option<Arc<dyn CommandRunner>>,
}

impl WfRecorder {
    /// `encoders` is the text of `ffmpeg -encoders`; `vaapi_device` is a render node known to work.
    pub fn new(encoders: String, vaapi_device: Option<PathBuf>) -> Self {
        Self {
            program: "wf-recorder".into(),
            ffmpeg: "ffmpeg".into(),
            encoders,
            vaapi_device,
            system_device: None,
            timing: Timing::default(),
            clock: Arc::new(SystemClock::default()),
            env: HashMap::new(),
            audio: None,
        }
    }

    pub fn with_programs(mut self, wf: impl Into<PathBuf>, ffmpeg: impl Into<PathBuf>) -> Self {
        self.program = wf.into();
        self.ffmpeg = ffmpeg.into();
        self
    }

    pub fn with_system_audio_device(mut self, monitor: impl Into<String>) -> Self {
        self.system_device = Some(monitor.into());
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

    /// Runs `pactl` to find and mute the recorder's own sound stream; without it nothing is muted.
    pub fn with_audio_runner(mut self, runner: Arc<dyn CommandRunner>) -> Self {
        self.audio = Some(runner);
        self
    }

    pub fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

impl Recorder for WfRecorder {
    fn name(&self) -> &'static str {
        "wf-recorder"
    }

    fn can_record_system_audio(&self, with_mic: bool) -> bool {
        !with_mic && self.system_device.is_some()
    }

    fn probe(&self) -> RecorderProbe {
        let problem = if which(&self.program).is_none() {
            Some("wf-recorder not found")
        } else if which(&self.ffmpeg).is_none() {
            Some("ffmpeg not found")
        } else if pick_encoder(&self.encoders, Codec::H264, self.vaapi_device.is_some()).is_err() {
            Some("ffmpeg has no usable H.264 or VP9 encoder")
        } else {
            None
        };
        RecorderProbe {
            available: problem.is_none(),
            reason: problem.map(str::to_owned),
        }
    }

    fn start(&self, spec: RecordSpec) -> Result<Box<dyn RecordingHandle>> {
        std::fs::create_dir_all(&spec.dir).map_err(|source| Error::Io {
            path: spec.dir.clone(),
            source,
        })?;
        let encoder = pick_encoder(&self.encoders, spec.codec, self.vaapi_device.is_some())?;
        wf_args(
            &spec,
            &encoder,
            self.vaapi_device.as_deref(),
            self.system_device.as_deref(),
            Path::new("probe"),
        )?;
        let (spec_system, spec_mic) = (spec.system_audio, spec.mic.is_some());
        let (tx, rx) = async_channel::unbounded();
        let mut h = WfHandle {
            program: self.program.clone(),
            ffmpeg: self.ffmpeg.clone(),
            env: self.env.clone(),
            vaapi_device: self.vaapi_device.clone(),
            system_device: self.system_device.clone(),
            encoder,
            spec,
            clock: self.clock.clone(),
            timing: self.timing,
            mutes: self
                .audio
                .clone()
                .map(|r| Mutes::new(r, sources(spec_system, spec_mic))),
            muted: Vec::new(),
            tx,
            rx,
            current: None,
            files: Vec::new(),
            expected_exit: Arc::new(AtomicBool::new(false)),
            seg_start: 0,
            first: 0,
            segments: Vec::new(),
        };
        h.first = h.clock.monotonic_us();
        h.spawn_segment()?;
        h.unmute_new();
        let _ = h.tx.try_send(RecorderEvent::Started);
        Ok(Box::new(h))
    }
}

struct WfHandle {
    program: PathBuf,
    ffmpeg: PathBuf,
    env: HashMap<String, String>,
    vaapi_device: Option<PathBuf>,
    system_device: Option<String>,
    encoder: Encoder,
    spec: RecordSpec,
    clock: Arc<dyn Clock>,
    timing: Timing,
    mutes: Option<Mutes>,
    muted: Vec<AudioSource>,
    tx: async_channel::Sender<RecorderEvent>,
    rx: async_channel::Receiver<RecorderEvent>,
    current: Option<Helper>,
    files: Vec<PathBuf>,
    expected_exit: Arc<AtomicBool>,
    seg_start: u64,
    first: u64,
    segments: Vec<(u64, u64)>,
}

impl WfHandle {
    fn spawn_segment(&mut self) -> Result<()> {
        let ext = container_ext(self.spec.container);
        let file = self
            .spec
            .dir
            .join(format!("segment-{:03}.{ext}", self.files.len()));
        let args = wf_args(
            &self.spec,
            &self.encoder,
            self.vaapi_device.as_deref(),
            self.system_device.as_deref(),
            &file,
        )?;
        self.expected_exit.store(false, Ordering::SeqCst);
        let on_exit: Box<dyn FnOnce(std::process::ExitStatus) + Send> = {
            let (tx, expected) = (self.tx.clone(), self.expected_exit.clone());
            Box::new(move |st| {
                if !expected.load(Ordering::SeqCst) {
                    let _ = tx.try_send(RecorderEvent::Failed(format!("recorder exited: {st}")));
                }
            })
        };
        let helper = Helper::spawn(&self.program, &args, &self.env, Some(on_exit))?;
        self.clock.sleep(self.timing.poll);
        if let Some(st) = helper.exited() {
            return Err(Error::Protocol(format!(
                "wf-recorder exited early ({st}): {}",
                helper.stderr_tail()
            )));
        }
        self.seg_start = self.clock.monotonic_us();
        self.files.push(file);
        self.current = Some(helper);
        Ok(())
    }

    /// A new wf-recorder process opens a new stream, so the mutes are applied to it again.
    fn remute(&self) -> Result<()> {
        let (Some(mutes), Some(helper)) = (&self.mutes, &self.current) else {
            return Ok(());
        };
        for source in &self.muted {
            let mut tries = 0;
            while let Err(e) = mutes.set(helper.pid(), *source, true) {
                tries += 1;
                if tries == 5 {
                    return Err(e);
                }
                self.clock.sleep(self.timing.poll);
            }
        }
        Ok(())
    }

    fn unmute_new(&self) {
        if let (Some(mutes), Some(helper)) = (&self.mutes, &self.current) {
            mutes.unmute_when_open(helper.pid(), &*self.clock, self.timing.poll);
        }
    }

    fn release_mutes(&self) {
        if let (Some(mutes), Some(helper)) = (&self.mutes, &self.current) {
            mutes.unmute_all(helper.pid());
        }
    }

    fn end_segment(&mut self) {
        if let Some(mut h) = self.current.take() {
            self.expected_exit.store(true, Ordering::SeqCst);
            let end = self.clock.monotonic_us();
            h.stop(&*self.clock, &self.timing, false);
            self.segments.push((self.seg_start, end));
        }
    }

    fn join(&mut self) -> Result<PathBuf> {
        let ext = container_ext(self.spec.container);
        let target = self.spec.dir.join(format!("screen.{ext}"));
        let have: Vec<PathBuf> = self
            .files
            .iter()
            .filter(|f| std::fs::metadata(f).map(|m| m.len() > 0).unwrap_or(false))
            .cloned()
            .collect();
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| Error::Io { path, source }
        };
        match have.as_slice() {
            [] => return Err(Error::NotFound(target.display().to_string())),
            [one] => std::fs::rename(one, &target).map_err(io(one))?,
            many => {
                let list = self.spec.dir.join("segments.txt");
                std::fs::write(&list, concat_list(many)).map_err(io(&list))?;
                let st = std::process::Command::new(&self.ffmpeg)
                    .envs(&self.env)
                    .args(["-y", "-f", "concat", "-safe", "0", "-i"])
                    .arg(&list)
                    .args(["-c", "copy"])
                    .arg(&target)
                    .stdin(std::process::Stdio::null())
                    .output()
                    .map_err(io(&self.ffmpeg))?;
                let _ = std::fs::remove_file(&list);
                if !st.status.success() || !target.is_file() {
                    return Err(Error::Protocol(format!(
                        "ffmpeg concat failed: {}",
                        String::from_utf8_lossy(&st.stderr).trim()
                    )));
                }
                for f in many {
                    let _ = std::fs::remove_file(f);
                }
            }
        }
        Ok(target)
    }
}

impl RecordingHandle for WfHandle {
    fn pause(&mut self) -> Result<()> {
        if self.current.is_none() {
            return Ok(());
        }
        self.end_segment();
        let _ = self.tx.try_send(RecorderEvent::Paused);
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if self.current.is_some() {
            return Ok(());
        }
        self.spawn_segment()?;
        self.unmute_new();
        let _ = self.tx.try_send(RecorderEvent::Resumed);
        self.remute()
    }

    fn set_muted(&mut self, source: AudioSource, muted: bool) -> Result<()> {
        let mutes = self.mutes.as_ref().ok_or(Error::Unsupported)?;
        if let Some(helper) = &self.current {
            mutes.set(helper.pid(), source, muted)?;
        }
        self.muted.retain(|s| *s != source);
        if muted {
            self.muted.push(source);
        }
        Ok(())
    }

    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<RecordingOutput>> {
        run_blocking(move || {
            self.release_mutes();
            self.end_segment();
            let file = self.join()?;
            Ok(RecordingOutput {
                file,
                first_frame_monotonic_us: self.first,
                segments: std::mem::take(&mut self.segments),
            })
        })
    }

    fn discard(mut self: Box<Self>) -> BoxFuture<'static, Result<()>> {
        run_blocking(move || {
            self.release_mutes();
            self.end_segment();
            std::fs::remove_dir_all(&self.spec.dir).map_err(|source| Error::Io {
                path: self.spec.dir.clone(),
                source,
            })
        })
    }

    fn events(&self) -> async_channel::Receiver<RecorderEvent> {
        self.rx.clone()
    }
}
