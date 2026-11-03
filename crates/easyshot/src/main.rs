mod cli;
mod complete;
#[cfg(feature = "gui")]
mod daemon;
mod doctor;
mod doctor_tools;
mod run;
#[cfg(test)]
mod run_tests;

use clap::{CommandFactory, FromArgMatches};
use cli::{Cli, Request};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FEATURES: &[(&str, bool)] = &[
    ("gui", cfg!(feature = "gui")),
    ("hyprland", cfg!(feature = "hyprland")),
    ("wlroots", cfg!(feature = "wlroots")),
    ("niri", cfg!(feature = "niri")),
    ("player", cfg!(feature = "player")),
    ("portal-recorder", cfg!(feature = "portal-recorder")),
    ("sound", cfg!(feature = "sound")),
    ("evdev-input", cfg!(feature = "evdev-input")),
];

#[cfg(feature = "gui")]
fn enabled_features() -> Vec<&'static str> {
    FEATURES
        .iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect()
}

fn long_version() -> &'static str {
    let enabled: Vec<_> = FEATURES
        .iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect();
    Box::leak(
        format!(
            "{}\nfeatures: {}",
            env!("EASYSHOT_VERSION"),
            enabled.join(", ")
        )
        .into_boxed_str(),
    )
}

fn print_registration(shell: clap_complete::Shell) -> std::io::Result<()> {
    let shells = clap_complete::env::Shells::builtins();
    let completer = shells
        .completer(&shell.to_string())
        .ok_or_else(|| std::io::Error::other("no completer for this shell"))?;
    let mut script = Vec::new();
    completer.write_registration("COMPLETE", "easyshot", "easyshot", "easyshot", &mut script)?;
    let script =
        complete::adapt_registration(&shell.to_string(), &String::from_utf8_lossy(&script));
    std::io::Write::write_all(&mut std::io::stdout(), script.as_bytes())
}

fn main() -> ExitCode {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
    let matches = Cli::command()
        .version(env!("EASYSHOT_VERSION"))
        .long_version(long_version())
        .get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };
    let request = match cli.request() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("easyshot: {e}");
            return ExitCode::from(run::EXIT_ERROR as u8);
        }
    };
    if request == Request::Daemon {
        return daemon_main();
    }
    if let Request::Completions(shell) = request {
        return match print_registration(shell) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("easyshot: {e}");
                ExitCode::from(run::EXIT_ERROR as u8)
            }
        };
    }
    let (cancel_tx, cancel_rx) = async_channel::bounded::<()>(1);
    let _ = ctrlc::set_handler(move || {
        let _ = cancel_tx.try_send(());
    });
    let env = run::Env {
        endpoint: es_desktop::dbus::Endpoint::session(),
        spawn_daemon: cfg!(feature = "gui"),
        wait: Duration::from_secs(3),
        standalone_report: Box::new(|| {
            let probe = es_backend::Probe::gather()?;
            let config = es_store::Paths::from_xdg().map(|p| p.config_file())?;
            Ok(doctor::standalone_report(
                &probe,
                &doctor_tools::SystemToolRunner,
                &config,
            ))
        }),
        log_dir: es_store::Paths::from_xdg().ok().map(|p| p.log_dir()),
        bundle_dir: std::env::current_dir().unwrap_or_default(),
        now: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    let cancel = async move {
        let _ = cancel_rx.recv().await;
    };
    let code = run::execute(
        request,
        &env,
        cancel,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    ExitCode::from(code as u8)
}

#[cfg(feature = "gui")]
fn daemon_main() -> ExitCode {
    match daemon::run(daemon::DaemonArgs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("easyshot: {e:#}");
            ExitCode::from(run::EXIT_ERROR as u8)
        }
    }
}

#[cfg(not(feature = "gui"))]
fn daemon_main() -> ExitCode {
    eprintln!("easyshot: this build has no GUI; rebuild with the `gui` feature to run the daemon");
    ExitCode::from(run::EXIT_ERROR as u8)
}
