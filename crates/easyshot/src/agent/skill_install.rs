use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

pub const FILES: [(&str, &str); 2] = [
    (
        "SKILL.md",
        include_str!("../../../../assets/agent-skill/easyshot/SKILL.md"),
    ),
    (
        "references/commands.md",
        include_str!("../../../../assets/agent-skill/easyshot/references/commands.md"),
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clients {
    Claude,
    Agents,
    All,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub what: &'static str,
    pub status: &'static str,
}

impl Entry {
    pub fn to_json(&self) -> Value {
        json!({"path": self.path, "what": self.what, "status": self.status})
    }
}

fn shared_dir(base: &Path) -> PathBuf {
    base.join(".agents/skills/easyshot")
}

fn claude_dir(base: &Path) -> PathBuf {
    base.join(".claude/skills/easyshot")
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn same(path: &Path, text: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes == text.as_bytes())
}

fn write_files(dir: &Path) -> Result<&'static str> {
    let existed = dir.exists();
    let mut changed = false;
    for (name, text) in FILES {
        let path = dir.join(name);
        if same(&path, text) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("cannot create {parent:?}"))?;
        }
        fs::write(&path, text).with_context(|| format!("cannot write {path:?}"))?;
        changed = true;
    }
    Ok(match (existed, changed) {
        (false, _) => "created",
        (true, true) => "updated",
        (true, false) => "unchanged",
    })
}

fn remove_files(dir: &Path) -> Result<&'static str> {
    if !dir.is_dir() {
        return Ok("absent");
    }
    let mut kept = false;
    for (name, text) in FILES {
        let path = dir.join(name);
        if !exists(&path) {
            continue;
        }
        if same(&path, text) {
            fs::remove_file(&path).with_context(|| format!("cannot remove {path:?}"))?;
        } else {
            kept = true;
        }
    }
    let _ = fs::remove_dir(dir.join("references"));
    let _ = fs::remove_dir(dir);
    Ok(if kept { "kept" } else { "removed" })
}

fn ours(dir: &Path) -> bool {
    dir.is_dir() && FILES.iter().all(|(name, text)| same(&dir.join(name), text))
}

fn clear(path: &Path) -> Result<()> {
    if path.is_dir() && !is_link(path) {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("cannot remove {path:?}"))
}

fn link(at: &Path, to: &Path) -> Result<&'static str> {
    if is_link(at) && fs::read_link(at).is_ok_and(|t| t == to) {
        return Ok("unchanged");
    }
    let replaced = exists(at);
    if replaced {
        clear(at)?;
    }
    if let Some(parent) = at.parent() {
        fs::create_dir_all(parent).with_context(|| format!("cannot create {parent:?}"))?;
    }
    symlink(to, at).with_context(|| format!("cannot link {at:?}"))?;
    Ok(if replaced { "updated" } else { "created" })
}

fn copy_to_claude(dir: &Path, force: bool) -> Result<&'static str> {
    if is_link(dir) {
        clear(dir)?;
    } else if exists(dir) && !ours(dir) && !force {
        bail!("{dir:?} exists and is not the easyshot skill; pass --force to overwrite its files");
    }
    write_files(dir)
}

pub fn install(base: &Path, clients: Clients, force: bool) -> Result<Vec<Entry>> {
    let (shared, claude) = (shared_dir(base), claude_dir(base));
    if clients == Clients::All && exists(&claude) && !is_link(&claude) && !force {
        bail!("{claude:?} exists and is not a link; pass --force to replace it");
    }
    let mut entries = Vec::new();
    if clients != Clients::Claude {
        entries.push(Entry {
            status: write_files(&shared)?,
            path: shared.clone(),
            what: "files",
        });
    }
    match clients {
        Clients::Agents => {}
        Clients::All => entries.push(Entry {
            status: link(&claude, &shared)?,
            path: claude,
            what: "link",
        }),
        Clients::Claude => entries.push(Entry {
            status: copy_to_claude(&claude, force)?,
            path: claude,
            what: "files",
        }),
    }
    Ok(entries)
}

pub fn uninstall(base: &Path, clients: Clients) -> Result<Vec<Entry>> {
    let (shared, claude) = (shared_dir(base), claude_dir(base));
    let mut entries = Vec::new();
    if clients != Clients::Claude {
        entries.push(Entry {
            status: remove_files(&shared)?,
            path: shared.clone(),
            what: "files",
        });
    }
    if clients != Clients::Agents {
        let entry = if is_link(&claude) {
            let ours = fs::read_link(&claude).is_ok_and(|t| t == shared);
            if ours {
                clear(&claude)?;
            }
            Entry {
                status: if ours { "removed" } else { "kept" },
                path: claude,
                what: "link",
            }
        } else {
            Entry {
                status: remove_files(&claude)?,
                path: claude,
                what: "files",
            }
        };
        entries.push(entry);
    }
    Ok(entries)
}
