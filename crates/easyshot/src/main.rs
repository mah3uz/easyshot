mod agent;
mod cli;
mod complete;
#[cfg(feature = "gui")]
mod daemon;
mod doctor;
mod doctor_summary;
mod doctor_tools;
mod hints;
mod run;
#[cfg(test)]
mod run_tests;
mod strings;

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
    let matches = match Cli::command()
        .version(env!("EASYSHOT_VERSION"))
        .long_version(long_version())
        .try_get_matches()
    {
        Ok(m) => m,
        Err(e) => {
            let argv: Vec<_> = std::env::args_os().collect();
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            match agent::usage_error(&argv, &e, &mut out, &mut err) {
                Some(code) => return ExitCode::from(code as u8),
                None => e.exit(),
            }
        }
    };
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(e) => e.exit(),
    };
    let request = match cli.request() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{}", strings::line(e));
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
                eprintln!("{}", strings::line(e));
                ExitCode::from(run::EXIT_ERROR as u8)
            }
        };
    }
    let (cancel_tx, cancel_rx) = async_channel::bounded::<()>(1);
    let _ = ctrlc::set_handler(move || {
        let _ = cancel_tx.try_send(());
    });
    let env = run::Env {
        endpoint: endpoint(),
        spawn_daemon: cfg!(feature = "gui") && !test_mode(),
        wait: Duration::from_secs(3),
        standalone_report: Box::new(|| {
            let probe = es_backend::Probe::gather()?;
            let config = es_store::Paths::from_xdg().map(|p| p.config_file())?;
            Ok(doctor::standalone_report(
                &probe,
                &doctor_tools::SystemToolRunner,
                &config,
                doctor_tools::input_here(),
            ))
        }),
        skill_notes: Box::new(|| {
            let record = es_store::Paths::from_xdg().map(|p| p.skill_installs_file());
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
            record.map_or_else(|_| Vec::new(), |r| agent::skill_notes(&r, home.as_deref()))
        }),
        log_dir: es_store::Paths::from_xdg().ok().map(|p| p.log_dir()),
        bundle_dir: std::env::current_dir().unwrap_or_default(),
        now: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        styled: std::io::IsTerminal::is_terminal(&std::io::stdout())
            && std::env::var_os("NO_COLOR").is_none(),
        progress: std::io::IsTerminal::is_terminal(&std::io::stderr()),
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

fn test_mode() -> bool {
    cfg!(feature = "test-hooks") && std::env::var("EASYSHOT_TEST").as_deref() == Ok("1")
}

/// A test daemon answers on its own bus name, so a test-mode command never reaches the user's.
fn endpoint() -> es_desktop::dbus::Endpoint {
    #[cfg(feature = "test-hooks")]
    if test_mode() {
        return es_desktop::dbus::Endpoint::test();
    }
    es_desktop::dbus::Endpoint::session()
}

#[cfg(feature = "gui")]
fn daemon_main() -> ExitCode {
    match daemon::run(daemon::DaemonArgs) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", strings::line(format!("{e:#}")));
            ExitCode::from(run::EXIT_ERROR as u8)
        }
    }
}

#[cfg(not(feature = "gui"))]
fn daemon_main() -> ExitCode {
    eprintln!("{}", strings::line(strings::fl!("cli-no-gui")));
    ExitCode::from(run::EXIT_ERROR as u8)
}
