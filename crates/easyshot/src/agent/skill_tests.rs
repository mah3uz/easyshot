use super::*;
use crate::agent::skill_install::FILES;
use crate::cli::{Cli, Request};
use clap::{CommandFactory, FromArgMatches, Parser};
use std::collections::BTreeSet;
use std::fs;

fn skill_md() -> &'static str {
    FILES[0].1
}

fn frontmatter(text: &str) -> Vec<(String, String)> {
    let rest = text.strip_prefix("---\n").expect("starts with ---");
    let (head, _) = rest.split_once("\n---\n").expect("closing ---");
    head.lines()
        .map(|l| {
            let (k, v) = l.split_once(':').expect("key: value");
            (k.trim().to_owned(), v.trim().to_owned())
        })
        .collect()
}

fn field(fields: &[(String, String)], name: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| k == name)
        .unwrap_or_else(|| panic!("frontmatter has no {name}"))
        .1
        .clone()
}

#[test]
fn frontmatter_meets_the_skill_spec() {
    let fields = frontmatter(skill_md());
    let name = field(&fields, "name");
    assert_eq!(name, "easyshot", "must equal the skill's directory name");
    assert!(name.len() <= 64);
    assert!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !name.starts_with('-')
            && !name.ends_with('-')
            && !name.contains("--")
    );
    let description = field(&fields, "description");
    assert!(!description.is_empty() && description.len() <= 1024);
    for (key, _) in &fields {
        assert!(
            [
                "name",
                "description",
                "license",
                "compatibility",
                "metadata",
                "allowed-tools"
            ]
            .contains(&key.as_str()),
            "{key} is not a portable skill field"
        );
    }
    assert!(skill_md().lines().count() < 500);
}

fn words(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

fn commands() -> Vec<Vec<String>> {
    let mut found = Vec::new();
    for (_, text) in FILES {
        let mut fenced = false;
        for line in text.lines() {
            if line.starts_with("```") {
                fenced = !fenced;
            } else if fenced && line.starts_with("easyshot ") {
                found.push(words(line));
            }
        }
    }
    found
}

#[test]
fn every_command_in_the_skill_parses_with_the_real_cli() {
    let commands = commands();
    assert!(commands.len() >= 20, "found {}", commands.len());
    for argv in &commands {
        let matches = Cli::command()
            .try_get_matches_from(argv)
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"));
        let cli = Cli::from_arg_matches(&matches).expect("matches build a Cli");
        cli.request().unwrap_or_else(|e| panic!("{argv:?}: {e}"));
    }
}

#[test]
fn the_skill_teaches_every_agent_action() {
    let joined: BTreeSet<String> = commands().iter().map(|a| a[1..].join(" ")).collect();
    for action in [
        "agent status",
        "agent list outputs",
        "agent list windows",
        "agent capture",
        "agent ocr",
        "agent record start",
        "agent record stop",
        "agent record status",
        "agent record discard",
        "agent export",
        "agent annotate",
    ] {
        assert!(
            joined.iter().any(|c| c.starts_with(action)),
            "no example of {action}"
        );
    }
}

fn agent_flags(cmd: &clap::Command, into: &mut BTreeSet<String>) {
    for arg in cmd.get_arguments() {
        if let Some(long) = arg.get_long() {
            into.insert(format!("--{long}"));
        }
    }
    for sub in cmd.get_subcommands() {
        agent_flags(sub, into);
    }
}

#[test]
fn every_flag_the_skill_mentions_exists_in_the_agent_cli() {
    let cli = Cli::command();
    let agent = cli.find_subcommand("agent").expect("agent command");
    let mut known = BTreeSet::new();
    agent_flags(agent, &mut known);
    for (name, text) in FILES {
        for token in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            if token.starts_with("--") && !token.trim_matches('-').is_empty() && token != "--help" {
                assert!(
                    known.contains(token),
                    "{name} mentions unknown flag {token}"
                );
            }
        }
    }
}

fn run(call: &SkillCall, home: &Path, cwd: &Path) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = execute(call, Some(home), Some(cwd), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn install_call(scope: Scope, clients: Clients, force: bool) -> SkillCall {
    SkillCall {
        action: SkillAction::Install {
            scope,
            clients,
            force,
        },
        json: true,
    }
}

fn uninstall_call(scope: Scope, clients: Clients) -> SkillCall {
    SkillCall {
        action: SkillAction::Uninstall { scope, clients },
        json: true,
    }
}

fn dirs() -> (tempfile::TempDir, tempfile::TempDir) {
    (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap())
}

#[test]
fn install_writes_the_files_and_links_claude_to_them() {
    let (home, cwd) = dirs();
    let (code, out, _) = run(
        &install_call(Scope::User, Clients::All, false),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0, "{out}");
    let shared = home.path().join(".agents/skills/easyshot");
    for (name, text) in FILES {
        assert_eq!(fs::read_to_string(shared.join(name)).unwrap(), text);
    }
    let claude = home.path().join(".claude/skills/easyshot");
    assert_eq!(fs::read_link(&claude).unwrap(), shared);
    assert!(claude.join("SKILL.md").is_file());
    let body: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(body["kind"], "skill");
    assert_eq!(body["entries"][1]["what"], "link");
    assert!(cwd.path().read_dir().unwrap().next().is_none());
}

#[test]
fn project_scope_uses_the_current_folder_not_home() {
    let (home, cwd) = dirs();
    let (code, ..) = run(
        &install_call(Scope::Project, Clients::All, false),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0);
    assert!(
        cwd.path()
            .join(".agents/skills/easyshot/SKILL.md")
            .is_file()
    );
    assert!(is_link(&cwd.path().join(".claude/skills/easyshot")));
    assert!(home.path().read_dir().unwrap().next().is_none());
}

fn is_link(p: &Path) -> bool {
    fs::symlink_metadata(p).unwrap().file_type().is_symlink()
}

#[test]
fn installing_twice_changes_nothing_the_second_time() {
    let (home, cwd) = dirs();
    let call = || install_call(Scope::User, Clients::All, false);
    run(&call(), home.path(), cwd.path());
    let (code, out, _) = run(&call(), home.path(), cwd.path());
    assert_eq!(code, 0);
    let body: Value = serde_json::from_str(&out).unwrap();
    for entry in body["entries"].as_array().unwrap() {
        assert_eq!(entry["status"], "unchanged", "{entry}");
    }
}

#[test]
fn a_stale_installed_file_is_brought_up_to_date() {
    let (home, cwd) = dirs();
    let call = install_call(Scope::User, Clients::Agents, false);
    run(&call, home.path(), cwd.path());
    let file = home.path().join(".agents/skills/easyshot/SKILL.md");
    fs::write(&file, "old").unwrap();
    let (_, out, _) = run(&call, home.path(), cwd.path());
    assert!(out.contains("updated"), "{out}");
    assert_eq!(fs::read_to_string(file).unwrap(), FILES[0].1);
}

#[test]
fn a_real_folder_at_the_claude_path_is_refused_without_force() {
    let (home, cwd) = dirs();
    let claude = home.path().join(".claude/skills/easyshot");
    fs::create_dir_all(&claude).unwrap();
    fs::write(claude.join("mine.txt"), "keep").unwrap();
    let (code, out, _) = run(
        &install_call(Scope::User, Clients::All, false),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 1);
    let body: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(body["ok"], false);
    assert!(claude.join("mine.txt").is_file());
    assert!(!is_link(&claude));
    assert!(
        !home.path().join(".agents").exists(),
        "a refusal writes nothing"
    );

    let (code, ..) = run(
        &install_call(Scope::User, Clients::All, true),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0);
    assert!(is_link(&claude));
}

#[test]
fn claude_only_copies_the_files_and_refuses_a_foreign_folder() {
    let (home, cwd) = dirs();
    let call = |force| install_call(Scope::User, Clients::Claude, force);
    let (code, ..) = run(&call(false), home.path(), cwd.path());
    assert_eq!(code, 0);
    let claude = home.path().join(".claude/skills/easyshot");
    assert!(!is_link(&claude) && claude.join("SKILL.md").is_file());
    assert!(!home.path().join(".agents").exists());
    assert_eq!(run(&call(false), home.path(), cwd.path()).0, 0);

    fs::write(claude.join("SKILL.md"), "someone else's").unwrap();
    assert_eq!(run(&call(false), home.path(), cwd.path()).0, 1);
    assert_eq!(run(&call(true), home.path(), cwd.path()).0, 0);
}

#[test]
fn uninstall_removes_only_what_install_made() {
    let (home, cwd) = dirs();
    run(
        &install_call(Scope::User, Clients::All, false),
        home.path(),
        cwd.path(),
    );
    let shared = home.path().join(".agents/skills/easyshot");
    fs::write(shared.join("notes.md"), "mine").unwrap();
    let other = home.path().join(".agents/skills/other");
    fs::create_dir_all(&other).unwrap();

    let (code, out, _) = run(
        &uninstall_call(Scope::User, Clients::All),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0, "{out}");
    assert!(!exists(&home.path().join(".claude/skills/easyshot")));
    assert!(!shared.join("SKILL.md").exists());
    assert!(
        shared.join("notes.md").is_file(),
        "a file we did not write stays"
    );
    assert!(other.is_dir());
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

#[test]
fn uninstall_keeps_an_edited_file_and_a_link_to_somewhere_else() {
    let (home, cwd) = dirs();
    run(
        &install_call(Scope::User, Clients::Agents, false),
        home.path(),
        cwd.path(),
    );
    let shared = home.path().join(".agents/skills/easyshot");
    fs::write(shared.join("SKILL.md"), "edited by the user").unwrap();
    let elsewhere = home.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let claude = home.path().join(".claude/skills/easyshot");
    fs::create_dir_all(claude.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &claude).unwrap();

    let (code, out, _) = run(
        &uninstall_call(Scope::User, Clients::All),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(shared.join("SKILL.md")).unwrap(),
        "edited by the user"
    );
    assert!(is_link(&claude), "{out}");
    assert!(!shared.join("references/commands.md").exists());
}

#[test]
fn uninstall_without_an_install_reports_absent() {
    let (home, cwd) = dirs();
    let (code, out, _) = run(
        &uninstall_call(Scope::User, Clients::Agents),
        home.path(),
        cwd.path(),
    );
    assert_eq!(code, 0);
    assert!(out.contains("absent"), "{out}");
}

#[test]
fn show_prints_the_skill_with_no_home_needed() {
    let call = |json| SkillCall {
        action: SkillAction::Show,
        json,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(execute(&call(false), None, None, &mut out, &mut err), 0);
    assert_eq!(String::from_utf8(out).unwrap(), skill_md());
    let mut out = Vec::new();
    execute(&call(true), None, None, &mut out, &mut err);
    let body: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(body["text"], skill_md());
}

#[test]
fn skill_commands_never_become_daemon_calls() {
    let parse = |args: &[&str]| Cli::try_parse_from(args).unwrap().request().unwrap();
    assert!(matches!(
        parse(&[
            "easyshot",
            "agent",
            "skill",
            "install",
            "--project",
            "--agents",
            "--json"
        ]),
        Request::Skill(SkillCall {
            action: SkillAction::Install {
                scope: Scope::Project,
                clients: Clients::Agents,
                force: false
            },
            json: true
        })
    ));
    assert!(matches!(
        parse(&["easyshot", "ai", "skill", "install"]),
        Request::Skill(SkillCall {
            action: SkillAction::Install {
                scope: Scope::User,
                clients: Clients::All,
                ..
            },
            ..
        })
    ));
    let both = Cli::command().try_get_matches_from([
        "easyshot",
        "agent",
        "skill",
        "install",
        "--user",
        "--project",
    ]);
    assert!(both.is_err());
}
