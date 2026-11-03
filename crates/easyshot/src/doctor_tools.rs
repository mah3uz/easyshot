use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Stdio};

const TRAY_WATCHER: &str = "org.kde.StatusNotifierWatcher";
const NOTIFICATION_SERVER: &str = "org.freedesktop.Notifications";
const STRIPPED_ENV: [&str; 3] = ["WAYLAND_DISPLAY", "DISPLAY", "DBUS_SESSION_BUS_ADDRESS"];
const ENCODERS: [&str; 8] = [
    "libx264",
    "libx265",
    "libvpx-vp9",
    "libsvtav1",
    "h264_vaapi",
    "hevc_vaapi",
    "h264_nvenc",
    "hevc_nvenc",
];

/// Runs a probe command and returns what it printed; `None` when it cannot run.
pub trait ToolRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;
}

pub struct SystemToolRunner;

impl SystemToolRunner {
    /// A probe must reach neither the user's display nor their bus.
    fn command(program: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        for name in STRIPPED_ENV {
            cmd.env_remove(name);
        }
        cmd
    }
}

impl ToolRunner for SystemToolRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let out = Self::command(program, args).output().ok()?;
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        Some(text)
    }
}

fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

fn version(runner: &dyn ToolRunner, program: &str, flag: &str) -> Value {
    runner
        .run(program, &[flag])
        .and_then(|t| first_line(&t))
        .map_or(Value::Null, Value::from)
}

fn encoders(listing: &str) -> Vec<&'static str> {
    ENCODERS
        .into_iter()
        .filter(|name| {
            listing
                .lines()
                .any(|l| l.split_whitespace().nth(1) == Some(name))
        })
        .collect()
}

fn languages(listing: &str) -> Vec<String> {
    listing
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn tools(runner: &dyn ToolRunner) -> Value {
    let ffmpeg_encoders = runner
        .run("ffmpeg", &["-hide_banner", "-encoders"])
        .map_or_else(Vec::new, |t| encoders(&t));
    let langs = runner
        .run("tesseract", &["--list-langs"])
        .map_or_else(Vec::new, |t| languages(&t));
    json!({
        "gpu-screen-recorder": version(runner, "gpu-screen-recorder", "--version"),
        "wf-recorder": version(runner, "wf-recorder", "--version"),
        "ffmpeg": {
            "version": version(runner, "ffmpeg", "-version"),
            "encoders": ffmpeg_encoders,
        },
        "tesseract": {
            "version": version(runner, "tesseract", "--version"),
            "languages": langs,
        },
    })
}

pub fn services(names: &es_backend::DbusNames) -> Value {
    json!({
        "tray_watcher": names.has(TRAY_WATCHER),
        "notification_server": names.has(NOTIFICATION_SERVER),
    })
}

pub const INPUT_FIX: &str = "sudo usermod -aG input $USER, then log out and back in";

/// Whether recordings can log clicks and shortcuts, and the fix when they cannot.
pub fn input(access: es_model::recording::InputAccess) -> Value {
    let mut v = json!({
        "status": access,
        "records": "clicks and shortcuts, never plain typing, into input.json beside each recording, for click effects and key badges",
    });
    if access == es_model::recording::InputAccess::Denied {
        v["fix"] = json!(INPUT_FIX);
    }
    v
}

/// The input access this process has, read from the device permissions.
pub fn input_here() -> Value {
    input(es_record::input::access(Path::new(
        es_record::input::DEV_INPUT,
    )))
}

/// A missing or newer-version file still opens; a file that does not parse falls back to defaults.
pub fn config_status(path: &Path) -> Value {
    let shown = path.display().to_string();
    if !path.exists() {
        return json!({"path": shown, "status": "missing"});
    }
    match es_store::ConfigFile::load(path) {
        Ok(file) => match (file.error(), file.newer_version()) {
            (Some(e), _) => json!({"path": shown, "status": "error", "detail": e.to_string()}),
            (None, Some(v)) => json!({"path": shown, "status": "newer", "detail": v}),
            (None, None) => json!({"path": shown, "status": "ok"}),
        },
        Err(e) => json!({"path": shown, "status": "error", "detail": e.to_string()}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probes_never_see_the_display_or_the_bus() {
        let cmd = SystemToolRunner::command("ffmpeg", &["-version"]);
        let removed: Vec<_> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        for name in STRIPPED_ENV {
            assert!(removed.iter().any(|r| r == name), "{name} not stripped");
        }
    }

    #[test]
    fn only_listed_encoders_that_exist_are_reported() {
        let listing = " V....D libx264              libx264 H.264\n V....D libx265  x\n V..... h264_nvenc NVENC\n V..... notlisted y\n";
        assert_eq!(encoders(listing), ["libx264", "libx265", "h264_nvenc"]);
    }

    #[test]
    fn the_language_list_drops_its_header() {
        let langs = languages("List of available languages in \"/x\" (2):\neng\nosd\n");
        assert_eq!(langs, ["eng", "osd"]);
    }

    #[test]
    fn a_broken_config_is_reported_not_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("easyshot.toml");
        assert_eq!(config_status(&path)["status"], "missing");
        std::fs::write(&path, "[general\n").unwrap();
        assert_eq!(config_status(&path)["status"], "error");
        std::fs::write(&path, "").unwrap();
        assert_eq!(config_status(&path)["status"], "ok");
    }
}
