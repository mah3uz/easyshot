use super::*;
use crate::agent::skill_install::{Clients, install, uninstall};
use es_store::skills::{assess, record, write_set};
use std::fs;

const OLD: &[(&str, &str)] = &[
    ("SKILL.md", "an old skill"),
    ("references/commands.md", "old commands"),
];

struct Setup {
    _root: tempfile::TempDir,
    home: PathBuf,
    record: PathBuf,
}

fn setup() -> Setup {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let record = root.path().join("state/skill-installs.json");
    Setup {
        home,
        record,
        _root: root,
    }
}

fn recorded(s: &Setup) -> Vec<PathBuf> {
    assess(&s.record, &FILES)
        .unwrap()
        .into_iter()
        .map(|(d, _)| d)
        .collect()
}

fn read(dir: &Path) -> String {
    fs::read_to_string(dir.join("SKILL.md")).unwrap()
}

#[test]
fn an_install_is_recorded_so_an_upgrade_can_find_it() {
    let s = setup();
    install(&s.home, Clients::All, false, Some(&s.record)).unwrap();
    assert_eq!(recorded(&s), vec![shared_dir(&s.home)]);
}

#[test]
fn a_claude_only_copy_is_recorded_and_a_link_is_not() {
    let s = setup();
    install(&s.home, Clients::Claude, false, Some(&s.record)).unwrap();
    assert_eq!(recorded(&s), vec![s.home.join(".claude/skills/easyshot")]);
    let linked = setup();
    install(&linked.home, Clients::All, false, Some(&linked.record)).unwrap();
    assert!(!recorded(&linked).contains(&linked.home.join(".claude/skills/easyshot")));
}

#[test]
fn uninstalling_forgets_the_entry() {
    let s = setup();
    install(&s.home, Clients::All, false, Some(&s.record)).unwrap();
    uninstall(&s.home, Clients::All, Some(&s.record)).unwrap();
    assert!(recorded(&s).is_empty());
}

#[test]
fn start_up_replaces_a_copy_from_an_older_easyshot() {
    let s = setup();
    let dir = shared_dir(&s.home);
    write_set(&dir, OLD).unwrap();
    record(&s.record, &dir, OLD).unwrap();
    refresh_installs(&s.record);
    assert_eq!(read(&dir), FILES[0].1);
    assert!(notes(&s.record, Some(&s.home)).is_empty());
}

#[test]
fn start_up_keeps_the_users_edits_and_says_so() {
    let s = setup();
    let dir = shared_dir(&s.home);
    write_set(&dir, OLD).unwrap();
    record(&s.record, &dir, OLD).unwrap();
    fs::write(dir.join("SKILL.md"), "my own rules").unwrap();
    refresh_installs(&s.record);
    assert_eq!(read(&dir), "my own rules");
    let found = notes(&s.record, Some(&s.home));
    assert_eq!(
        found,
        vec![SkillNote {
            dir: dir.clone(),
            kind: NoteKind::Edited
        }]
    );
    assert!(
        found[0]
            .line()
            .contains("easyshot agent skill install --force")
    );
}

#[test]
fn a_recorded_copy_not_yet_refreshed_is_reported_out_of_date() {
    let s = setup();
    let dir = shared_dir(&s.home);
    write_set(&dir, OLD).unwrap();
    record(&s.record, &dir, OLD).unwrap();
    let found = notes(&s.record, Some(&s.home));
    assert_eq!(found[0].kind, NoteKind::OutOfDate);
    assert!(found[0].line().contains("is out of date"));
}

#[test]
fn an_unrecorded_install_from_before_recording_is_reported_but_never_replaced() {
    let s = setup();
    let dir = shared_dir(&s.home);
    write_set(&dir, OLD).unwrap();
    refresh_installs(&s.record);
    assert_eq!(read(&dir), "an old skill");
    let found = notes(&s.record, Some(&s.home));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, NoteKind::OutOfDate);
    assert!(found[0].fix().contains("easyshot agent skill install"));
}

#[test]
fn a_current_install_and_no_install_say_nothing() {
    let s = setup();
    assert!(notes(&s.record, Some(&s.home)).is_empty());
    install(&s.home, Clients::All, false, Some(&s.record)).unwrap();
    assert!(notes(&s.record, Some(&s.home)).is_empty());
}

#[test]
fn the_json_report_carries_the_state_and_the_fix() {
    let note = SkillNote {
        dir: PathBuf::from("/x"),
        kind: NoteKind::Edited,
    };
    let body = with_notes(json!({"ok": true}), &[note]);
    assert_eq!(body["skill"][0]["state"], "edited");
    assert_eq!(
        body["skill"][0]["fix"],
        "easyshot agent skill install --force"
    );
    assert_eq!(with_notes(json!({"ok": true}), &[]), json!({"ok": true}));
}

#[test]
fn doctor_text_names_the_folder_and_the_fix() {
    let notes = [
        SkillNote {
            dir: PathBuf::from("/a"),
            kind: NoteKind::Edited,
        },
        SkillNote {
            dir: PathBuf::from("/b"),
            kind: NoteKind::OutOfDate,
        },
    ];
    let report = crate::doctor::with_skill(r#"{"caps":[]}"#, &notes);
    let text = crate::doctor::render_text(&report);
    assert!(text.contains("/a was edited"), "{text}");
    assert!(
        text.contains("easyshot agent skill install --force"),
        "{text}"
    );
    assert!(text.contains("/b is out of date"), "{text}");
    let clean = crate::doctor::render_text(&crate::doctor::with_skill(r#"{"caps":[]}"#, &[]));
    assert!(!clean.contains("Agent skill"), "{clean}");
}

#[test]
fn agent_status_lists_a_stale_install_in_text_and_json() {
    use es_protocol::agent::{AgentReply, ReplyKind};
    let note = SkillNote {
        dir: PathBuf::from("/a"),
        kind: NoteKind::Edited,
    };
    let reply = AgentReply::ok(ReplyKind::Access);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    crate::agent::run::print(
        false,
        &reply,
        std::slice::from_ref(&note),
        &mut out,
        &mut err,
    );
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("skill\t/a was edited")
    );
    let mut out = Vec::new();
    crate::agent::run::print(true, &reply, &[note], &mut out, &mut err);
    let body: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(body["skill"][0]["state"], "edited");
}
