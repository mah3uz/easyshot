use clap_complete::CompletionCandidate;
use es_desktop::dbus::{Client, Endpoint};
use es_protocol::agent::{AgentReply, WindowEntry};
use es_protocol::{AgentRoute, JobStatus, Route};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

const WINDOW_LIST_TIMEOUT: Duration = Duration::from_millis(1500);

const OPEN_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "mp4", "webm", "mkv", "mov", "gif"];

pub fn open_target(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let mut out: Vec<_> = Route::url_routes()
        .filter(|url| url.starts_with(current))
        .map(CompletionCandidate::new)
        .collect();
    if !current.contains("://") {
        out.extend(
            image_paths(current)
                .into_iter()
                .map(CompletionCandidate::new),
        );
    }
    out
}

pub fn share_target(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    image_paths(current)
        .into_iter()
        .map(CompletionCandidate::new)
        .collect()
}

pub fn window_ids(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    window_candidates(
        current,
        listed_windows(crate::endpoint(), WINDOW_LIST_TIMEOUT),
    )
}

fn window_candidates(current: &str, windows: Vec<WindowEntry>) -> Vec<CompletionCandidate> {
    windows
        .into_iter()
        .filter(|w| w.id.starts_with(current))
        .map(|w| {
            let help = w.title.or(w.app_id).map(|t| t.replace(['\t', '\n'], " "));
            CompletionCandidate::new(w.id).help(help.map(Into::into))
        })
        .collect()
}

fn listed_windows(endpoint: Endpoint, timeout: Duration) -> Vec<WindowEntry> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(futures::executor::block_on(running_daemon_windows(
            &endpoint,
        )));
    });
    rx.recv_timeout(timeout).unwrap_or_default()
}

/// Completion must neither start the daemon nor raise a consent dialog, so it lists windows only
/// when a daemon is already up and the access settings already allow it without asking.
async fn running_daemon_windows(endpoint: &Endpoint) -> Vec<WindowEntry> {
    let Ok(client) = Client::connect_running(endpoint).await else {
        return Vec::new();
    };
    let status = AgentRoute::Manage(es_protocol::agent::ManageRoute::Status);
    let Some(status) = reply(&client, &status).await.and_then(|r| r.access) else {
        return Vec::new();
    };
    if !status.enabled || !(status.consent == "always-allow" || status.grant.is_some()) {
        return Vec::new();
    }
    reply(&client, &AgentRoute::ListWindows)
        .await
        .and_then(|r| r.windows)
        .unwrap_or_default()
}

async fn reply(client: &Client, route: &AgentRoute) -> Option<AgentReply> {
    let outcome = client
        .run(
            route.route_name(),
            &route.args(),
            futures::future::pending(),
        )
        .await
        .ok()?;
    if outcome.status != JobStatus::Ok {
        return None;
    }
    AgentReply::from_json(&outcome.result).ok().filter(|r| r.ok)
}

const BASH_WORDS: &str = r#"    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
"#;

const BASH_WORDS_JOINED: &str = r#"    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
    local _line=${COMP_LINE:0:COMP_POINT}
    local _token=${_line##*[[:space:]]}
    local _trim=
    if [[ $_token == *:* && $COMP_WORDBREAKS == *:* ]]; then
        IFS=$' \t\n' read -ra words <<< "$_line"
        _CLAP_COMPLETE_INDEX=$(( ${#words[@]} - 1 ))
        _trim=${_token%"${_token##*:}"}
    fi
"#;

const BASH_END: &str = "        compopt -o nospace\n    fi\n}";

const BASH_END_TRIMMED: &str = "        compopt -o nospace\n    fi\n    if [[ -n $_trim && -n ${COMPREPLY-} ]]; then\n        COMPREPLY=( \"${COMPREPLY[@]#\"$_trim\"}\" )\n    fi\n}";

/// bash splits words at `:`, which would cut `easyshot://` links in two; zsh's autoloaded
/// `_easyshot` only registers the completer unless it also runs it once.
pub fn adapt_registration(shell: &str, script: &str) -> String {
    match shell {
        "bash" => script
            .replace(BASH_WORDS, BASH_WORDS_JOINED)
            .replace(BASH_END, BASH_END_TRIMMED),
        "zsh" => format!(
            "{}\nif [[ $funcstack[1] == _easyshot ]]; then\n    _clap_dynamic_completer_easyshot \"$@\"\nfi\n",
            script.trim_end()
        ),
        _ => script.to_owned(),
    }
}

fn image_paths(current: &str) -> Vec<String> {
    let (dir, prefix) = match current.rfind('/') {
        Some(i) => current.split_at(i + 1),
        None => ("", current),
    };
    let read = if dir.is_empty() { "." } else { dir };
    let Ok(entries) = std::fs::read_dir(read) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(prefix) || (name.starts_with('.') && !prefix.starts_with('.')) {
                return None;
            }
            let path = entry.path();
            if path.is_dir() {
                Some(format!("{dir}{name}/"))
            } else if is_openable(&path) {
                Some(format!("{dir}{name}"))
            } else {
                None
            }
        })
        .collect();
    found.sort();
    found
}

fn is_openable(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| OPEN_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::run_tests::{PrivateBus, env, serve};
    use clap::CommandFactory;
    use clap_complete::engine::ArgValueCompleter;
    use es_protocol::agent::ManageRoute;

    const ALLOWED: &str = r#"{"ok":true,"kind":"windows","access":{"enabled":true,"consent":"once-per-session","grant":{"span":"hour"},"allowed_outputs":[],"allowed_windows":[],"redact_secrets":true,"recent_actions":0},"windows":[{"id":"4","app_id":"foot","title":"notes"},{"id":"42","app_id":"kitty"},{"id":"7","title":"a\tb"}]}"#;
    const ASK_FIRST: &str = r#"{"ok":true,"kind":"windows","access":{"enabled":true,"consent":"once-per-session","allowed_outputs":[],"allowed_windows":[],"redact_secrets":true,"recent_actions":0},"windows":[{"id":"4"}]}"#;
    const OFF: &str = r#"{"ok":true,"kind":"windows","access":{"enabled":false,"consent":"always-allow","allowed_outputs":[],"allowed_windows":[],"redact_secrets":true,"recent_actions":0},"windows":[{"id":"4"}]}"#;

    fn offered(result: &'static str, jobs: usize) -> (Vec<WindowEntry>, Vec<Route>) {
        let bus = PrivateBus::start();
        let dir = tempfile::tempdir().unwrap();
        let server = serve(&bus, jobs, JobStatus::Ok, result);
        let windows = listed_windows(env(&bus, dir.path()).endpoint, Duration::from_secs(5));
        (windows, server.finish())
    }

    fn pairs(windows: Vec<WindowEntry>, current: &str) -> Vec<(String, String)> {
        window_candidates(current, windows)
            .iter()
            .map(|c| {
                (
                    c.get_value().to_string_lossy().into_owned(),
                    c.get_help().map(|h| h.to_string()).unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn windows_offer_their_ids_described_by_title_then_app_id() {
        let (windows, routes) = offered(ALLOWED, 2);
        assert_eq!(
            routes,
            [
                Route::Agent(AgentRoute::Manage(ManageRoute::Status)),
                Route::Agent(AgentRoute::ListWindows),
            ]
        );
        let all = |c: &str| pairs(windows.clone(), c);
        assert_eq!(
            all(""),
            [("4", "notes"), ("42", "kitty"), ("7", "a b")].map(|(a, b)| (a.into(), b.into()))
        );
        assert_eq!(all("4").len(), 2);
        assert!(all("9").is_empty());
    }

    #[test]
    fn completion_asks_the_daemon_nothing_that_would_raise_a_consent_dialog() {
        for result in [ASK_FIRST, OFF] {
            let bus = PrivateBus::start();
            let dir = tempfile::tempdir().unwrap();
            let server = serve(&bus, 2, JobStatus::Ok, result);
            let endpoint = || env(&bus, dir.path()).endpoint;
            let timeout = Duration::from_secs(5);
            assert!(listed_windows(endpoint(), timeout).is_empty());
            assert!(listed_windows(endpoint(), timeout).is_empty());
            assert_eq!(
                server.finish(),
                vec![Route::Agent(AgentRoute::Manage(ManageRoute::Status)); 2]
            );
        }
    }

    #[test]
    fn no_daemon_offers_nothing_and_does_not_start_one() {
        let bus = PrivateBus::start();
        let dir = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let windows = listed_windows(env(&bus, dir.path()).endpoint, Duration::from_secs(5));
        assert!(windows.is_empty());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn every_window_flag_completes_from_the_daemon() {
        let cmd = Cli::command();
        let agent = cmd.find_subcommand("agent").unwrap();
        let record = agent
            .find_subcommand("record")
            .unwrap()
            .find_subcommand("start")
            .unwrap();
        for sub in [
            agent.find_subcommand("capture").unwrap(),
            agent.find_subcommand("ocr").unwrap(),
            record,
        ] {
            let arg = sub
                .get_arguments()
                .find(|a| a.get_id() == "window")
                .unwrap();
            assert!(
                arg.get::<ArgValueCompleter>().is_some(),
                "{}",
                sub.get_name()
            );
        }
    }

    #[test]
    fn registration_scripts_ask_the_binary_so_the_window_flag_completes_dynamically() {
        for shell in ["bash", "zsh", "fish"] {
            let mut script = Vec::new();
            clap_complete::env::Shells::builtins()
                .completer(shell)
                .unwrap()
                .write_registration("COMPLETE", "easyshot", "easyshot", "easyshot", &mut script)
                .unwrap();
            let script = adapt_registration(shell, &String::from_utf8(script).unwrap());
            assert!(script.contains("COMPLETE"), "{shell}: {script}");
            assert!(script.contains("easyshot"), "{shell}");
        }
    }

    fn values(current: &str) -> Vec<String> {
        open_target(OsStr::new(current))
            .iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn open_offers_every_url_route() {
        assert_eq!(values("easyshot://").len(), Route::url_routes().count());
        assert_eq!(
            values("easyshot://cap"),
            [
                "easyshot://capture/region",
                "easyshot://capture/fullscreen",
                "easyshot://capture/window",
                "easyshot://capture/scroll",
            ]
        );
    }

    #[test]
    fn open_offers_only_images_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        for f in ["a.png", "b.JPG", "c.jpeg", "d.txt", ".hidden.png"] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let base = format!("{}/", dir.path().display());
        let got = values(&base);
        let names: Vec<_> = got.iter().map(|v| v.strip_prefix(&base).unwrap()).collect();
        assert_eq!(names, ["a.png", "b.JPG", "c.jpeg", "sub/"]);
    }
}
