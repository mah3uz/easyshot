use crate::cli::Cli;
use crate::doctor;
use crate::run::{EXIT_CANCELLED, EXIT_ERROR, EXIT_NO_DAEMON, EXIT_OK, Env, execute};
use clap::Parser;
use es_desktop::dbus::{Callbacks, Endpoint, Incoming, Service};
use es_protocol::{JobStatus, Route};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

const FAKE_DOCTOR: &str = r#"{"compositor":{"name":"fake","version":null},"providers":[],"caps":[],"missing_tools":[],"errors":[]}"#;

pub(crate) struct PrivateBus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl PrivateBus {
    pub(crate) fn start() -> PrivateBus {
        // No service directories: a test must never activate a real desktop service.
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<busconfig><type>session</type><listen>unix:tmpdir={}</listen><policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>"#,
                dir.path().display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon is installed");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        PrivateBus {
            child,
            address: address.trim().to_owned(),
            _dir: dir,
        }
    }

    fn endpoint(&self) -> Endpoint {
        Endpoint::session().on_address(&self.address)
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct NoTools;

impl crate::doctor_tools::ToolRunner for NoTools {
    fn run(&self, _: &str, _: &[&str]) -> Option<String> {
        None
    }
}

pub(crate) fn env(bus: &PrivateBus, dir: &Path) -> Env {
    Env {
        endpoint: bus.endpoint(),
        spawn_daemon: false,
        wait: Duration::from_millis(300),
        standalone_report: Box::new(|| {
            Ok(doctor::standalone_report(
                &es_backend::Probe::default(),
                &NoTools,
                Path::new("easyshot/config.toml"),
                crate::doctor_tools::input(es_model::recording::InputAccess::Denied),
            ))
        }),
        log_dir: None,
        bundle_dir: dir.to_owned(),
        now: 1_700_000_000,
        styled: false,
    }
}

pub(crate) fn serve(
    bus: &PrivateBus,
    jobs: usize,
    status: JobStatus,
    result: &'static str,
) -> Server {
    let endpoint = bus.endpoint();
    let (ready_tx, ready_rx) = mpsc::channel();
    let (stop, stopped) = mpsc::channel::<()>();
    let handle = std::thread::spawn(move || {
        futures::executor::block_on(async {
            let callbacks = Callbacks {
                doctor: Arc::new(|| Box::pin(async { FAKE_DOCTOR.to_owned() })),
                dump: Arc::new(|| Box::pin(async { String::new() })),
            };
            let svc = Service::start(endpoint, callbacks).await.unwrap();
            let incoming = svc.incoming();
            ready_tx.send(()).unwrap();
            let mut seen = Vec::new();
            for _ in 0..jobs {
                let Incoming::Command { job, route, .. } = incoming.recv().await.unwrap() else {
                    panic!("expected a command");
                };
                seen.push(route);
                svc.finish(job, status, result).await.unwrap();
            }
            let _ = stopped.recv();
            seen
        })
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    Server { handle, stop }
}

pub(crate) struct Server {
    handle: JoinHandle<Vec<Route>>,
    stop: mpsc::Sender<()>,
}

impl Server {
    pub(crate) fn finish(self) -> Vec<Route> {
        let _ = self.stop.send(());
        self.handle.join().unwrap()
    }
}

pub(crate) fn run_cli(args: &[&str], env: &Env) -> (i32, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("easyshot").chain(args.iter().copied())).unwrap();
    let request = cli.request().unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = execute(request, env, futures::future::pending(), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn each_subcommand_sends_its_route_to_the_daemon() {
    let table: &[(&[&str], Route)] = &[
        (&[], Route::Home),
        (&["capture", "region"], Route::CaptureRegion),
        (
            &["capture", "region", "--annotate"],
            Route::CaptureRegionAnnotate,
        ),
        (&["capture", "scroll"], Route::CaptureScroll),
        (&["ocr", "--single-line"], Route::OcrSingleLine),
        (&["record", "stop"], Route::RecordStop),
        (&["open", "easyshot://color-picker"], Route::ColorPicker),
        (&["settings"], Route::Settings(None)),
        (&["deck", "close-all"], Route::DeckCloseAll),
        (&["quit"], Route::Quit),
    ];
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, table.len(), JobStatus::Ok, "done");
    let env = env(&bus, dir.path());
    for (args, _) in table {
        let (code, out, _) = run_cli(args, &env);
        assert_eq!((code, out.as_str()), (EXIT_OK, "done\n"), "{args:?}");
    }
    let seen = server.finish();
    let expected: Vec<Route> = table.iter().map(|(_, r)| r.clone()).collect();
    assert_eq!(seen, expected);
}

#[test]
fn a_png_path_reaches_the_daemon_canonicalised() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("a.png");
    std::fs::write(&png, [0x89, b'P', b'N', b'G', 0]).unwrap();
    let server = serve(&bus, 1, JobStatus::Ok, "");
    let (code, ..) = run_cli(&["open", png.to_str().unwrap()], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(
        server.finish(),
        vec![Route::Open(png.canonicalize().unwrap())]
    );
}

#[test]
fn job_failure_and_cancellation_set_the_exit_code() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let env = env(&bus, dir.path());
    let server = serve(&bus, 1, JobStatus::Error, "no screen");
    let (code, _, err) = run_cli(&["capture", "fullscreen"], &env);
    assert_eq!(code, EXIT_ERROR);
    assert!(err.contains("no screen"), "{err}");
    server.finish();

    let bus = PrivateBus::start();
    let env = self::env(&bus, dir.path());
    let server = serve(&bus, 1, JobStatus::Cancelled, "");
    let (code, ..) = run_cli(&["record"], &env);
    assert_eq!(code, EXIT_CANCELLED);
    server.finish();
}

#[test]
fn no_daemon_is_a_clear_error_with_its_own_exit_code() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = run_cli(&["capture", "region"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_NO_DAEMON);
    assert!(out.is_empty());
    assert!(
        err.contains("not running") && err.contains("easyshot daemon"),
        "{err}"
    );
}

#[test]
fn a_missing_session_bus_is_also_daemon_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let bus = PrivateBus::start();
    let mut env = env(&bus, dir.path());
    env.endpoint = Endpoint::session().on_address("unix:path=/nonexistent/easyshot-bus");
    let (code, _, err) = run_cli(&["gallery"], &env);
    assert_eq!(code, EXIT_NO_DAEMON);
    assert!(err.contains("session bus"), "{err}");
}

fn catalogue(id: &str) -> String {
    crate::strings::loader().get(id)
}

#[test]
fn the_plain_cli_error_lines_come_from_the_catalogue() {
    let dir = tempfile::tempdir().unwrap();
    let bus = PrivateBus::start();
    let (_, _, err) = run_cli(&["gallery"], &env(&bus, dir.path()));
    assert_eq!(
        err,
        format!("easyshot: {}\n", catalogue("cli-daemon-not-running"))
    );

    let mut off = env(&bus, dir.path());
    off.endpoint = Endpoint::session().on_address("unix:path=/nonexistent/easyshot-bus");
    let (_, _, err) = run_cli(&["gallery"], &off);
    assert!(
        err.starts_with("easyshot: cannot talk to the session bus: "),
        "{err}"
    );

    let server = serve(&bus, 1, JobStatus::Cancelled, "");
    let (_, _, err) = run_cli(&["record"], &env(&bus, dir.path()));
    assert_eq!(err, format!("easyshot: {}\n", catalogue("cli-cancelled")));
    server.finish();

    let mut probe = env(&bus, dir.path());
    probe.standalone_report = Box::new(|| Err(anyhow::anyhow!("boom")));
    let (_, _, err) = run_cli(&["doctor"], &probe);
    assert!(
        err.starts_with("easyshot: the standalone probe failed: boom"),
        "{err}"
    );
}

#[test]
fn doctor_prefers_the_running_daemon() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 0, JobStatus::Ok, "");
    let (code, out, _) = run_cli(&["doctor", "--json"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, format!("{}\n", doctor::pretty(FAKE_DOCTOR)));
    assert!(!out.contains("standalone"));
    server.finish();
}

#[test]
fn doctor_falls_back_to_a_standalone_probe_and_matches_the_snapshot() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run_cli(&["doctor", "--json"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/doctor.json");
    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        std::fs::write(&path, &out).unwrap();
    }
    assert_eq!(out, std::fs::read_to_string(&path).unwrap());
}

#[test]
fn doctor_raw_keeps_the_full_text_and_the_summary_leaves_it_out() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let (code, raw, _) = run_cli(&["doctor", "--raw"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    for want in [
        "Source: standalone probe (the daemon is not running)",
        "Providers:",
        "    rejected ext-image-copy: not detected",
        "Portals:",
        "Missing tools:\n  gpu-screen-recorder (package: gpu-screen-recorder)\n  wf-recorder (package: wf-recorder)",
    ] {
        assert!(raw.contains(want), "{want} missing from\n{raw}");
    }
    let (code, summary, _) = run_cli(&["doctor"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    assert!(summary.contains("What works"), "{summary}");
    assert!(summary.contains("Missing tools"), "{summary}");
    for gone in ["rejected", "Portals", "Wayland globals"] {
        assert!(!summary.contains(gone), "{gone} in\n{summary}");
    }
}

#[test]
fn doctor_raw_text_is_unchanged() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let server = serve(&bus, 0, JobStatus::Ok, "");
    let (code, out, _) = run_cli(&["doctor", "--raw"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    assert_eq!(out, "Compositor: fake \nCapabilities: \n\nProviders:\n");
    server.finish();
}

#[test]
fn doctor_bundle_holds_the_report_and_the_logs_only() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    std::fs::write(logs.join("easyshot.log"), "line\n").unwrap();
    std::fs::create_dir(logs.join("sub")).unwrap();
    let mut env = env(&bus, dir.path());
    env.log_dir = Some(logs);
    let (code, _, err) = run_cli(&["doctor", "--bundle"], &env);
    assert_eq!(code, EXIT_OK);
    let tar = dir.path().join("easyshot-doctor-1700000000.tar");
    assert!(err.contains(tar.to_str().unwrap()), "{err}");
    let list = Command::new("tar").arg("-tf").arg(&tar).output().unwrap();
    let names = String::from_utf8(list.stdout).unwrap();
    assert_eq!(
        names.lines().collect::<Vec<_>>(),
        ["doctor.json", "log/easyshot.log"]
    );
}

#[test]
fn the_cli_path_links_no_gui_library() {
    let out = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "easyshot",
            "--no-default-features",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--offline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8(out.stdout).unwrap();
    let gui: Vec<_> = tree
        .lines()
        .map(|l| l.split(' ').next().unwrap_or(""))
        .filter(|n| {
            n.starts_with("gpui")
                || ["es-ui", "es-app", "es-editor", "es-overlays", "es-shell"].contains(n)
        })
        .collect();
    assert!(gui.is_empty(), "{gui:?}");
    assert!(tree.contains("es-desktop"));
}

#[test]
fn the_default_build_reaches_gpui_only_through_the_gui_feature() {
    let out = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "easyshot",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--offline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .lines()
            .any(|l| l.starts_with("gpui-pre "))
    );
}

struct Installed;

impl crate::doctor_tools::ToolRunner for Installed {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        Some(
            match (program, args.first().copied()) {
                ("gpu-screen-recorder", _) => "5.6.0\n",
                ("ffmpeg", Some("-version")) => "ffmpeg version 7.1 Copyright\nbuilt with gcc\n",
                ("ffmpeg", _) => "Encoders:\n V....D libx264 H.264\n V....D libvpx-vp9 VP9\n",
                ("tesseract", Some("--version")) => "tesseract 5.5.0\n leptonica-1.85\n",
                ("tesseract", _) => "List of available languages (2):\neng\ndeu\n",
                _ => return None,
            }
            .to_owned(),
        )
    }
}

#[test]
fn the_standalone_report_names_services_tool_versions_and_the_config() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config, "[general\n").unwrap();
    let mut probe = es_backend::Probe::default();
    probe
        .dbus
        .names
        .insert("org.freedesktop.Notifications".into());
    let input = crate::doctor_tools::input(es_model::recording::InputAccess::Denied);
    let raw = doctor::standalone_report(&probe, &Installed, &config, input);
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(v["services"]["notification_server"], true);
    assert_eq!(v["services"]["tray_watcher"], false);
    assert_eq!(v["tools"]["gpu-screen-recorder"], "5.6.0");
    assert_eq!(v["tools"]["wf-recorder"], serde_json::Value::Null);
    assert_eq!(
        v["tools"]["ffmpeg"]["version"],
        "ffmpeg version 7.1 Copyright"
    );
    assert_eq!(
        v["tools"]["ffmpeg"]["encoders"],
        serde_json::json!(["libx264", "libvpx-vp9"])
    );
    assert_eq!(
        v["tools"]["tesseract"]["languages"],
        serde_json::json!(["eng", "deu"])
    );
    assert_eq!(v["config"]["status"], "error");
    assert_eq!(v["input"]["status"], "denied");
    assert_eq!(v["input"]["fix"], crate::doctor_tools::INPUT_FIX);
    let text = doctor::render_text(&raw);
    for want in [
        "tray watcher: absent",
        "notification server: present",
        "wf-recorder: not found",
        "encoders: libx264, libvpx-vp9",
        "languages: eng, deu",
        "(error:",
        "Clicks and keys in recordings: cannot read the keyboard and mouse",
        "Fix: sudo usermod -aG input $USER, then log out and in",
        "never typed text",
    ] {
        assert!(text.contains(want), "{want} missing from\n{text}");
    }
}

#[cfg(feature = "evdev-input")]
#[test]
fn doctor_and_the_settings_row_agree_when_the_group_is_added_but_not_active() {
    use es_model::recording::InputAccess;
    let dir = tempfile::tempdir().unwrap();
    let groups = dir.path().join("group");
    std::fs::write(&groups, "wheel:x:998:ann\ninput:x:97:bob,ann\n").unwrap();
    let nodes = dir.path().join("dev");
    std::fs::create_dir(&nodes).unwrap();
    let node = nodes.join("event0");
    std::fs::write(&node, b"").unwrap();
    std::fs::set_permissions(&node, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();

    let access = es_record::input::access_for(&nodes, &groups, Some("ann"));
    assert_eq!(access, InputAccess::GroupPending);
    let v = crate::doctor_tools::input(access);
    assert_eq!(v["status"], "group_pending");
    assert_eq!(v["fix"], crate::doctor_tools::INPUT_PENDING_FIX);
    let report = serde_json::json!({"input": v}).to_string();
    assert!(doctor::render_text(&report).contains("added to the group, log out and in"));
    let other = es_record::input::access_for(&nodes, &groups, Some("carol"));
    assert_eq!(other, InputAccess::Denied, "a user not in the group");
}
