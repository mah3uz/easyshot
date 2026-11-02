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

struct PrivateBus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl PrivateBus {
    fn start() -> PrivateBus {
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

fn env(bus: &PrivateBus, dir: &Path) -> Env {
    Env {
        endpoint: bus.endpoint(),
        spawn_daemon: false,
        wait: Duration::from_millis(300),
        standalone_report: Box::new(|| {
            Ok(doctor::standalone_report(&es_backend::Probe::default()))
        }),
        log_dir: None,
        bundle_dir: dir.to_owned(),
        now: 1_700_000_000,
    }
}

fn serve(bus: &PrivateBus, jobs: usize, status: JobStatus, result: &'static str) -> Server {
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
                let Incoming::Command { job, route } = incoming.recv().await.unwrap() else {
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

struct Server {
    handle: JoinHandle<Vec<Route>>,
    stop: mpsc::Sender<()>,
}

impl Server {
    fn finish(self) -> Vec<Route> {
        let _ = self.stop.send(());
        self.handle.join().unwrap()
    }
}

fn run_cli(args: &[&str], env: &Env) -> (i32, String, String) {
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
fn doctor_text_names_the_chosen_and_rejected_providers() {
    let bus = PrivateBus::start();
    let dir = tempfile::tempdir().unwrap();
    let (code, out, _) = run_cli(&["doctor"], &env(&bus, dir.path()));
    assert_eq!(code, EXIT_OK);
    assert!(out.contains("Source: standalone"), "{out}");
    assert!(
        out.contains("Providers:") && out.contains("Portals:"),
        "{out}"
    );
    assert!(out.contains("Missing tools:"), "{out}");
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
