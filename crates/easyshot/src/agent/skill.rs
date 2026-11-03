use super::skill_install::{self as install, Clients, Entry, FILES, Scope};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug)]
pub enum SkillCommand {
    /// Install the easyshot skill for Claude Code, Codex, Gemini CLI and Cursor
    Install(InstallArgs),
    /// Remove the files and link that install made
    Uninstall(Where),
    /// Print the skill's SKILL.md
    Show,
}

#[derive(Args, Debug)]
pub struct Where {
    /// For your user, in your home folder (the default)
    #[arg(long, conflicts_with = "project")]
    user: bool,
    /// For the project in the current folder
    #[arg(long)]
    project: bool,
    /// Claude Code only: a copy in .claude/skills
    #[arg(long, conflicts_with_all = ["agents", "all"])]
    claude: bool,
    /// The shared .agents/skills folder only, read by Codex, Gemini CLI and Cursor
    #[arg(long, conflicts_with = "all")]
    agents: bool,
    /// The shared folder and a link to it for Claude Code (the default)
    #[arg(long)]
    all: bool,
}

#[derive(Args, Debug)]
pub struct InstallArgs {
    #[command(flatten)]
    at: Where,
    /// Replace what is at the Claude Code path when easyshot did not put it there
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillAction {
    Install {
        scope: Scope,
        clients: Clients,
        force: bool,
    },
    Uninstall {
        scope: Scope,
        clients: Clients,
    },
    Show,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCall {
    pub action: SkillAction,
    pub json: bool,
}

impl Where {
    fn scope(&self) -> Scope {
        if self.project {
            Scope::Project
        } else {
            Scope::User
        }
    }

    fn clients(&self) -> Clients {
        if self.claude {
            Clients::Claude
        } else if self.agents {
            Clients::Agents
        } else {
            Clients::All
        }
    }
}

impl SkillCommand {
    pub fn call(&self, json: bool) -> SkillCall {
        let action = match self {
            SkillCommand::Install(a) => SkillAction::Install {
                scope: a.at.scope(),
                clients: a.at.clients(),
                force: a.force,
            },
            SkillCommand::Uninstall(w) => SkillAction::Uninstall {
                scope: w.scope(),
                clients: w.clients(),
            },
            SkillCommand::Show => SkillAction::Show,
        };
        SkillCall { action, json }
    }
}

pub fn run_skill(call: &SkillCall, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let cwd = std::env::current_dir().ok();
    execute(call, home.as_deref(), cwd.as_deref(), out, err)
}

pub(super) fn execute(
    call: &SkillCall,
    home: Option<&Path>,
    cwd: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let result = match &call.action {
        SkillAction::Show => Ok(("show", Vec::new())),
        SkillAction::Install {
            scope,
            clients,
            force,
        } => base(*scope, home, cwd)
            .and_then(|b| install::install(b, *clients, *force))
            .map(|e| ("install", e)),
        SkillAction::Uninstall { scope, clients } => base(*scope, home, cwd)
            .and_then(|b| install::uninstall(b, *clients))
            .map(|e| ("uninstall", e)),
    };
    match result {
        Ok((action, entries)) => {
            print(call.json, action, &entries, out);
            0
        }
        Err(e) => {
            if call.json {
                let body = json!({"ok": false, "kind": "skill",
                    "error": {"code": "failed", "message": format!("{e:#}")}});
                let _ = writeln!(out, "{body}");
            } else {
                let _ = writeln!(err, "easyshot: {e:#}");
            }
            1
        }
    }
}

fn base<'a>(
    scope: Scope,
    home: Option<&'a Path>,
    cwd: Option<&'a Path>,
) -> anyhow::Result<&'a Path> {
    match scope {
        Scope::User => home.ok_or_else(|| anyhow::anyhow!("HOME is not set")),
        Scope::Project => cwd.ok_or_else(|| anyhow::anyhow!("the current folder is unknown")),
    }
}

fn print(json: bool, action: &str, entries: &[Entry], out: &mut dyn Write) {
    let skill = FILES[0].1;
    if json {
        let body: Value = if action == "show" {
            json!({"ok": true, "kind": "skill", "action": action, "text": skill})
        } else {
            let list: Vec<_> = entries.iter().map(Entry::to_json).collect();
            json!({"ok": true, "kind": "skill", "action": action, "entries": list})
        };
        let _ = writeln!(out, "{body}");
    } else if action == "show" {
        let _ = write!(out, "{skill}");
    } else {
        for e in entries {
            let _ = writeln!(out, "{}\t{}\t{}", e.status, e.what, e.path.display());
        }
    }
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
