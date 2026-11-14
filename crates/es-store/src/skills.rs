use crate::atomic::replace_atomic;
use crate::error::{Error, Result, io};
use crate::json::JsonFile;
use es_model::{FormatKind, SkillInstall, SkillInstalls};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub type Files<'a> = &'a [(&'a str, &'a str)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    Created,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removed {
    Absent,
    Kept,
    Removed,
}

/// What a recorded folder holds now, against the skill this build carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Current,
    /// Unedited since easyshot wrote it, and different from this build's skill.
    Outdated,
    /// Changed by the user, so never overwritten.
    Edited,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refresh {
    Replaced,
    Current,
    Edited,
    Gone,
}

fn hash_parts<'a>(parts: impl Iterator<Item = (&'a str, &'a [u8])>) -> String {
    let mut sha = Sha256::new();
    for (name, bytes) in parts {
        sha.update((name.len() as u64).to_le_bytes());
        sha.update(name.as_bytes());
        sha.update((bytes.len() as u64).to_le_bytes());
        sha.update(bytes);
    }
    sha.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hash_set(files: Files) -> String {
    hash_parts(files.iter().map(|(n, t)| (*n, t.as_bytes())))
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

fn through_link(dir: &Path, path: &Path) -> bool {
    path.ancestors().take_while(|a| *a != dir).any(is_link)
}

/// None when a file is missing or sits behind a link, so a copy that is not plainly ours never matches.
fn disk_hash(dir: &Path, files: Files) -> Option<String> {
    let mut read = Vec::with_capacity(files.len());
    for (name, _) in files {
        let path = dir.join(name);
        if through_link(dir, &path) {
            return None;
        }
        read.push((*name, fs::read(&path).ok()?));
    }
    Some(hash_parts(read.iter().map(|(n, b)| (*n, b.as_slice()))))
}

pub fn is_current(dir: &Path, files: Files) -> bool {
    dir.is_dir() && disk_hash(dir, files).is_some_and(|h| h == hash_set(files))
}

fn same(path: &Path, text: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes == text.as_bytes())
}

pub fn write_set(dir: &Path, files: Files) -> Result<Written> {
    let existed = dir.exists();
    let mut changed = false;
    for (name, text) in files {
        let path = dir.join(name);
        if same(&path, text) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io(parent))?;
        }
        replace_atomic(&path, text.as_bytes())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).map_err(io(&path))?;
        changed = true;
    }
    Ok(match (existed, changed) {
        (false, _) => Written::Created,
        (true, true) => Written::Updated,
        (true, false) => Written::Unchanged,
    })
}

/// Removes the files that still match; a file the user changed stays, and so does its folder.
pub fn remove_set(dir: &Path, files: Files) -> Result<Removed> {
    if !dir.is_dir() {
        return Ok(Removed::Absent);
    }
    let mut kept = false;
    for (name, text) in files {
        let path = dir.join(name);
        if fs::symlink_metadata(&path).is_err() {
            continue;
        }
        if same(&path, text) {
            fs::remove_file(&path).map_err(io(&path))?;
        } else {
            kept = true;
        }
    }
    let _ = fs::remove_dir(dir.join("references"));
    let _ = fs::remove_dir(dir);
    Ok(if kept {
        Removed::Kept
    } else {
        Removed::Removed
    })
}

fn open(file: &Path) -> Result<JsonFile<SkillInstalls>> {
    JsonFile::open(file, FormatKind::SkillInstalls)
}

fn open_writable(file: &Path) -> Result<JsonFile<SkillInstalls>> {
    let opened = open(file)?;
    match opened.newer_version() {
        Some(version) => Err(Error::NewerVersion { version }),
        None => Ok(opened),
    }
}

pub fn record(file: &Path, dir: &Path, files: Files) -> Result<()> {
    let mut opened = open_writable(file)?;
    let entry = SkillInstall {
        dir: dir.to_path_buf(),
        hash: hash_set(files),
        stale: false,
    };
    match opened.value.installs.iter_mut().find(|i| i.dir == dir) {
        Some(slot) => *slot = entry,
        None => opened.value.installs.push(entry),
    }
    opened.save()
}

pub fn forget(file: &Path, dir: &Path) -> Result<()> {
    let mut opened = open_writable(file)?;
    let before = opened.value.installs.len();
    opened.value.installs.retain(|i| i.dir != dir);
    if opened.value.installs.len() == before {
        return Ok(());
    }
    opened.save()
}

fn classify(install: &SkillInstall, files: Files) -> State {
    let dir = &install.dir;
    if is_link(dir) || !dir.is_dir() {
        return State::Gone;
    }
    match disk_hash(dir, files) {
        Some(h) if h == hash_set(files) => State::Current,
        Some(h) if h == install.hash => State::Outdated,
        _ => State::Edited,
    }
}

/// Read-only: the recorded folders and what each holds. A missing record file is no installs.
pub fn assess(file: &Path, files: Files) -> Result<Vec<(PathBuf, State)>> {
    Ok(open(file)?
        .value
        .installs
        .iter()
        .map(|i| (i.dir.clone(), classify(i, files)))
        .collect())
}

/// Replaces unedited copies with `files`, marks edited ones stale and forgets folders that are gone.
/// Writes only inside recorded folders.
pub fn refresh(file: &Path, files: Files) -> Result<Vec<(PathBuf, Refresh)>> {
    let mut opened = open_writable(file)?;
    let mut report = Vec::new();
    let mut kept = Vec::new();
    for mut install in std::mem::take(&mut opened.value.installs) {
        let outcome = match classify(&install, files) {
            State::Gone => Refresh::Gone,
            State::Current => {
                install.hash = hash_set(files);
                install.stale = false;
                Refresh::Current
            }
            State::Edited => {
                install.stale = true;
                Refresh::Edited
            }
            State::Outdated => match write_set(&install.dir, files) {
                Ok(_) => {
                    install.hash = hash_set(files);
                    install.stale = false;
                    Refresh::Replaced
                }
                Err(e) => {
                    tracing::warn!(error = %e, dir = %install.dir.display(), "skill not refreshed");
                    Refresh::Edited
                }
            },
        };
        report.push((install.dir.clone(), outcome));
        if outcome != Refresh::Gone {
            kept.push(install);
        }
    }
    opened.value.installs = kept;
    opened.save()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: Files = &[
        ("SKILL.md", "old skill"),
        ("references/commands.md", "old refs"),
    ];
    const NEW: Files = &[
        ("SKILL.md", "new skill"),
        ("references/commands.md", "new refs"),
    ];

    struct Fixture {
        _root: tempfile::TempDir,
        file: PathBuf,
        dir: PathBuf,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("state/skill-installs.json");
        let dir = root.path().join("home/.agents/skills/easyshot");
        Fixture {
            file,
            dir,
            _root: root,
        }
    }

    fn installed(f: &Fixture, files: Files) {
        write_set(&f.dir, files).unwrap();
        record(&f.file, &f.dir, files).unwrap();
    }

    fn read(dir: &Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap()
    }

    #[test]
    fn a_recorded_copy_nobody_edited_takes_the_new_skill() {
        let f = fixture();
        installed(&f, OLD);
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report, vec![(f.dir.clone(), Refresh::Replaced)]);
        assert_eq!(read(&f.dir, "SKILL.md"), "new skill");
        assert_eq!(read(&f.dir, "references/commands.md"), "new refs");
        assert_eq!(assess(&f.file, NEW).unwrap()[0].1, State::Current);
        assert!(!open(&f.file).unwrap().value.installs[0].stale);
    }

    #[test]
    fn an_edited_copy_is_left_alone_and_marked_stale() {
        let f = fixture();
        installed(&f, OLD);
        fs::write(f.dir.join("SKILL.md"), "my own notes").unwrap();
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report, vec![(f.dir.clone(), Refresh::Edited)]);
        assert_eq!(read(&f.dir, "SKILL.md"), "my own notes");
        assert_eq!(read(&f.dir, "references/commands.md"), "old refs");
        assert!(open(&f.file).unwrap().value.installs[0].stale);
        assert_eq!(assess(&f.file, NEW).unwrap()[0].1, State::Edited);
    }

    #[test]
    fn a_missing_folder_is_dropped_from_the_record() {
        let f = fixture();
        installed(&f, OLD);
        fs::remove_dir_all(&f.dir).unwrap();
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report, vec![(f.dir.clone(), Refresh::Gone)]);
        assert!(open(&f.file).unwrap().value.installs.is_empty());
    }

    #[test]
    fn a_current_copy_is_not_rewritten() {
        let f = fixture();
        installed(&f, NEW);
        let before = fs::metadata(f.dir.join("SKILL.md"))
            .unwrap()
            .modified()
            .unwrap();
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report, vec![(f.dir.clone(), Refresh::Current)]);
        let after = fs::metadata(f.dir.join("SKILL.md"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn a_link_to_the_recorded_folder_is_refreshed_with_it() {
        let f = fixture();
        installed(&f, OLD);
        let claude = f.dir.parent().unwrap().join("claude-link");
        std::os::unix::fs::symlink(&f.dir, &claude).unwrap();
        refresh(&f.file, NEW).unwrap();
        assert_eq!(read(&claude, "SKILL.md"), "new skill");
    }

    #[test]
    fn a_recorded_folder_turned_into_a_foreign_link_is_never_written_through() {
        let f = fixture();
        installed(&f, OLD);
        let foreign = f.dir.parent().unwrap().join("foreign");
        write_set(&foreign, OLD).unwrap();
        fs::remove_dir_all(&f.dir).unwrap();
        std::os::unix::fs::symlink(&foreign, &f.dir).unwrap();
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report, vec![(f.dir.clone(), Refresh::Gone)]);
        assert_eq!(read(&foreign, "SKILL.md"), "old skill");
    }

    #[test]
    fn a_link_inside_the_folder_make_it_count_as_edited() {
        let f = fixture();
        installed(&f, OLD);
        let foreign = f.dir.parent().unwrap().join("foreign");
        fs::create_dir_all(&foreign).unwrap();
        fs::write(foreign.join("commands.md"), "old refs").unwrap();
        fs::remove_dir_all(f.dir.join("references")).unwrap();
        std::os::unix::fs::symlink(&foreign, f.dir.join("references")).unwrap();
        let report = refresh(&f.file, NEW).unwrap();
        assert_eq!(report[0].1, Refresh::Edited);
        assert_eq!(
            fs::read_to_string(foreign.join("commands.md")).unwrap(),
            "old refs"
        );
    }

    #[test]
    fn forgetting_removes_only_that_entry() {
        let f = fixture();
        installed(&f, OLD);
        let other = f.dir.parent().unwrap().join("other");
        write_set(&other, OLD).unwrap();
        record(&f.file, &other, OLD).unwrap();
        forget(&f.file, &f.dir).unwrap();
        let left = open(&f.file).unwrap().value.installs;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].dir, other);
    }

    #[test]
    fn recording_the_same_folder_twice_keeps_one_entry() {
        let f = fixture();
        installed(&f, OLD);
        installed(&f, NEW);
        assert_eq!(open(&f.file).unwrap().value.installs.len(), 1);
    }

    #[test]
    fn the_record_round_trips_at_version_one() {
        let f = fixture();
        installed(&f, OLD);
        let text = fs::read_to_string(&f.file).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["format"], "easyshot.skill-installs");
        assert_eq!(json["version"], 1);
        assert_eq!(open(&f.file).unwrap().value.installs[0].hash, hash_set(OLD));
    }

    #[test]
    fn a_record_from_a_newer_easyshot_is_never_written() {
        let f = fixture();
        installed(&f, OLD);
        let newer = r#"{"format":"easyshot.skill-installs","version":9,"installs":[],"extra":1}"#;
        fs::write(&f.file, newer).unwrap();
        assert!(matches!(
            record(&f.file, &f.dir, NEW),
            Err(Error::NewerVersion { version: 9 })
        ));
        assert!(matches!(
            refresh(&f.file, NEW),
            Err(Error::NewerVersion { version: 9 })
        ));
        assert!(forget(&f.file, &f.dir).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(), newer);
        assert_eq!(read(&f.dir, "SKILL.md"), "old skill");
    }

    #[test]
    fn a_damaged_record_is_kept_as_it_is() {
        let f = fixture();
        fs::create_dir_all(f.file.parent().unwrap()).unwrap();
        fs::write(&f.file, "{ not json").unwrap();
        assert!(record(&f.file, &f.dir, OLD).is_err());
        assert_eq!(fs::read_to_string(&f.file).unwrap(), "{ not json");
    }

    #[test]
    fn remove_set_keeps_what_the_user_changed() {
        let f = fixture();
        installed(&f, OLD);
        fs::write(f.dir.join("SKILL.md"), "mine").unwrap();
        assert_eq!(remove_set(&f.dir, OLD).unwrap(), Removed::Kept);
        assert_eq!(read(&f.dir, "SKILL.md"), "mine");
        assert!(!f.dir.join("references/commands.md").exists());
    }
}
