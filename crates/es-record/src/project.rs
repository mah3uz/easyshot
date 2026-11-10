use crate::{Error, Result};
use es_model::recording::{RecordingCapture, RecordingInput, RecordingProject};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use time::OffsetDateTime;
use time::macros::format_description;

pub use es_store::recording::{CAPTURE, INPUT, PROJECT};

pub const FOLDER_SUFFIX: &str = "easyshotrec";
pub const VIDEO: &str = "screen.mkv";
pub const POSTER: &str = "poster.jpg";

pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output>;
}

pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<Output> {
        Command::new(program)
            .args(args)
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .stdin(Stdio::null())
            .output()
    }
}

pub(crate) fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub fn folder_name(now: OffsetDateTime) -> String {
    let stamp = now
        .format(format_description!(
            "[year]-[month]-[day]-[hour][minute][second]"
        ))
        .unwrap_or_default();
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("EasyShot_{stamp}_{}.{FOLDER_SUFFIX}", &id[..6])
}

pub struct ProjectFolder {
    dir: PathBuf,
}

impl ProjectFolder {
    pub fn create(recordings_dir: &Path, now: OffsetDateTime) -> Result<Self> {
        let dir = recordings_dir.join(folder_name(now));
        std::fs::create_dir_all(&dir).map_err(|e| io_err(&dir, e))?;
        Ok(Self { dir })
    }

    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn video_path(&self) -> PathBuf {
        self.dir.join(VIDEO)
    }

    pub fn has_capture(&self) -> bool {
        self.dir.join(CAPTURE).is_file()
    }

    pub fn write_input(&self, input: &RecordingInput) -> Result<()> {
        Ok(es_store::recording::write(&self.dir, input)?)
    }

    pub fn write_project(&self, project: &RecordingProject) -> Result<()> {
        Ok(es_store::recording::write(&self.dir, project)?)
    }

    pub fn write_capture(&self, capture: &RecordingCapture) -> Result<()> {
        Ok(es_store::recording::write(&self.dir, capture)?)
    }

    /// `capture.json` goes last: it is the marker that the recording finished.
    pub fn finish(
        &self,
        capture: &RecordingCapture,
        project: &RecordingProject,
        input: &RecordingInput,
        runner: &dyn CommandRunner,
    ) -> Result<()> {
        self.write_input(input)?;
        self.write_project(project)?;
        if let Err(e) = make_poster(runner, &self.video_path(), capture.duration_secs) {
            tracing::warn!(error = %e, "poster frame failed");
        }
        self.write_capture(capture)
    }
}

pub fn poster_time(duration_secs: f64) -> f64 {
    (duration_secs / 2.0).clamp(0.0, 1.0)
}

pub fn make_poster(
    runner: &dyn CommandRunner,
    video: &Path,
    duration_secs: f64,
) -> Result<PathBuf> {
    let out = video.with_file_name(POSTER);
    let args: Vec<String> = [
        "-y",
        "-v",
        "error",
        "-threads",
        "2",
        "-ss",
        &format!("{:.3}", poster_time(duration_secs)),
        "-i",
        &video.to_string_lossy(),
        "-frames:v",
        "1",
        "-q:v",
        "3",
        &out.to_string_lossy(),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let res = runner.run("ffmpeg", &args).map_err(|e| io_err(video, e))?;
    if !res.status.success() || !out.is_file() {
        return Err(Error::Protocol(format!(
            "ffmpeg poster failed: {}",
            String::from_utf8_lossy(&res.stderr).trim()
        )));
    }
    Ok(out)
}
