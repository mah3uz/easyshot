#![allow(clippy::unwrap_used)]
use std::path::Path;
use std::process::Command;

const SHELLS: [&str; 3] = ["bash", "zsh", "fish"];

fn candidates(shell: &str, cwd: &Path, words: &[&str]) -> Vec<String> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_easyshot"));
    cmd.current_dir(cwd)
        .env_clear()
        .env("COMPLETE", shell)
        .args(["--", "easyshot"])
        .args(words);
    if shell != "fish" {
        cmd.env("_CLAP_IFS", "\n")
            .env("_CLAP_COMPLETE_INDEX", words.len().to_string());
    }
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "{shell} {words:?}");
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| value_of(shell, l))
        .collect()
}

fn value_of(shell: &str, line: &str) -> String {
    match shell {
        "fish" => line.split('\t').next().unwrap().to_owned(),
        "bash" => line.to_owned(),
        _ => {
            let mut value = String::new();
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => value.extend(chars.next()),
                    ':' => break,
                    c => value.push(c),
                }
            }
            value
        }
    }
}

fn assert_offers(shell: &str, cwd: &Path, words: &[&str], expected: &[&str], absent: &[&str]) {
    let got = candidates(shell, cwd, words);
    for want in expected {
        assert!(
            got.iter().any(|g| g == want),
            "{shell} {words:?}: {want} in {got:?}"
        );
    }
    for no in absent {
        assert!(
            !got.iter().any(|g| g == no),
            "{shell} {words:?}: {no} in {got:?}"
        );
    }
}

#[test]
fn settings_offers_every_page() {
    let pages: Vec<_> = es_protocol::SettingsPage::ALL
        .iter()
        .map(|p| p.as_str())
        .collect();
    for shell in SHELLS {
        assert_offers(shell, Path::new("/"), &["settings", ""], &pages, &[]);
    }
}

#[test]
fn capture_offers_modes_and_region_offers_its_flags() {
    for shell in SHELLS {
        assert_offers(
            shell,
            Path::new("/"),
            &["capture", ""],
            &[
                "region",
                "fullscreen",
                "window",
                "previous",
                "timed",
                "scroll",
            ],
            &[],
        );
        assert_offers(
            shell,
            Path::new("/"),
            &["capture", "region", "--"],
            &["--copy", "--save", "--annotate", "--pin"],
            &[],
        );
    }
}

#[test]
fn record_deck_and_doctor_complete_their_values() {
    for shell in SHELLS {
        let cwd = Path::new("/");
        assert_offers(
            shell,
            cwd,
            &["record", ""],
            &["area", "stop", "pause", "restart", "discard"],
            &[],
        );
        assert_offers(
            shell,
            cwd,
            &["deck", ""],
            &["toggle", "save-all", "close-all"],
            &[],
        );
        assert_offers(
            shell,
            cwd,
            &["doctor", "--"],
            &["--json", "--raw", "--bundle"],
            &[],
        );
        assert_offers(
            shell,
            cwd,
            &["completions", ""],
            &["bash", "zsh", "fish"],
            &[],
        );
    }
}

#[test]
fn open_offers_url_routes_and_only_image_files() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["shot.png", "photo.jpg", "notes.txt"] {
        std::fs::write(dir.path().join(f), b"x").unwrap();
    }
    for shell in SHELLS {
        assert_offers(
            shell,
            dir.path(),
            &["open", ""],
            &[
                "easyshot://capture/region",
                "easyshot://settings",
                "shot.png",
                "photo.jpg",
            ],
            &["notes.txt"],
        );
    }
}

#[test]
fn the_script_calls_back_into_the_binary() {
    for shell in SHELLS {
        let out = Command::new(env!("CARGO_BIN_EXE_easyshot"))
            .args(["completions", shell])
            .env_clear()
            .output()
            .unwrap();
        assert!(out.status.success(), "{shell}");
        let script = String::from_utf8(out.stdout).unwrap();
        assert!(
            script.contains(&format!("COMPLETE=\"{shell}\""))
                || script.contains(&format!("COMPLETE={shell}")),
            "{shell}"
        );
    }
}

fn bash_reply(line: &str, words: &[&str], cword: usize, cur: &str) -> Vec<String> {
    let bin = Path::new(env!("CARGO_BIN_EXE_easyshot")).parent().unwrap();
    let registration = Command::new(env!("CARGO_BIN_EXE_easyshot"))
        .args(["completions", "bash"])
        .env_clear()
        .output()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("reg.bash");
    std::fs::write(&script, registration.stdout).unwrap();
    let words_src: String = words.iter().map(|w| format!("'{w}' ")).collect();
    let program = format!(
        "source {}; COMP_WORDBREAKS=$' \\t\\n:='; COMP_LINE='{line}'; COMP_POINT=${{#COMP_LINE}}; \
         COMP_WORDS=({words_src}); COMP_CWORD={cword}; COMP_TYPE=9; \
         _clap_complete_easyshot easyshot '{cur}' x; printf '%s\\n' \"${{COMPREPLY[@]}}\"",
        script.display()
    );
    let out = Command::new("bash")
        .env_clear()
        .env("PATH", format!("{}:/usr/bin", bin.display()))
        .args(["-c", &program])
        .output()
        .unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn bash_completes_links_across_the_colon() {
    let got = bash_reply(
        "easyshot open easyshot://cap",
        &["easyshot", "open", "easyshot", ":", "//cap"],
        4,
        "//cap",
    );
    assert_eq!(
        got,
        [
            "//capture/region",
            "//capture/fullscreen",
            "//capture/window",
            "//capture/scroll"
        ]
    );
}

#[test]
fn bash_without_a_colon_is_left_alone() {
    let got = bash_reply("easyshot setti", &["easyshot", "setti"], 1, "setti");
    assert_eq!(got, ["settings"]);
}

#[test]
fn zsh_registration_runs_the_completer_when_autoloaded() {
    let out = Command::new(env!("CARGO_BIN_EXE_easyshot"))
        .args(["completions", "zsh"])
        .env_clear()
        .output()
        .unwrap();
    let script = String::from_utf8(out.stdout).unwrap();
    assert!(script.contains("$funcstack[1] == _easyshot"));
}

#[test]
fn the_top_level_offers_agent_and_ai_but_none_of_their_subcommands() {
    for shell in SHELLS {
        let cwd = Path::new("/");
        assert_offers(
            shell,
            cwd,
            &[""],
            &["agent", "ai", "capture"],
            &["list", "outputs", "windows"],
        );
        assert_offers(shell, cwd, &["--"], &[], &["--json"]);
        for namespace in ["agent", "ai"] {
            assert_offers(
                shell,
                cwd,
                &[namespace, ""],
                &["list", "capture"],
                &["outputs"],
            );
            assert_offers(
                shell,
                cwd,
                &[namespace, "list", ""],
                &["outputs", "windows"],
                &[],
            );
            assert_offers(
                shell,
                cwd,
                &[namespace, "list", "outputs", "--"],
                &["--json"],
                &[],
            );
        }
    }
}
