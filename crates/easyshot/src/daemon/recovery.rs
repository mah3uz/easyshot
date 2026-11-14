use es_app::deck::{Deck, DeckItem};
use es_editor::session::journal::{self, Recovered};
use es_store::Paths;
use gpui::{App, Entity};
use std::path::Path;
use time::OffsetDateTime;

pub fn reopen_recovered(wiring: &super::Wiring, recovered: Vec<Recovered>, cx: &mut App) {
    let mut opened = 0;
    for found in recovered {
        match wiring.recover(&found.entry.display, found.path, cx) {
            Ok(()) => opened += 1,
            Err(e) => tracing::warn!(error = %e, "recovered edits not reopened"),
        }
    }
    if opened > 0 {
        es_app::toast::show_toast(
            es_app::toast::Toast::new("toast-recovered-edits", es_app::toast::IconName::Info),
            cx,
        );
    }
}

/// Restores deck cards, finishes interrupted recordings and collects editor journals for reopening.
pub fn sweep(paths: &Paths) {
    let record = paths.skill_installs_file();
    std::thread::spawn(move || crate::agent::refresh_installs(&record));
    if let Err(e) = es_store::temp::purge(paths, std::time::SystemTime::now()) {
        tracing::warn!(error = %e, "temporary files not purged");
    }
    if let Err(e) = es_store::agent_captures::clear(paths) {
        tracing::warn!(error = %e, "agent captures not cleared");
    }
}

pub fn run(
    paths: &Paths,
    deck: &Entity<Deck>,
    recordings: &super::recording::Recordings,
    cx: &mut App,
) -> Vec<Recovered> {
    match es_store::Deck::new(paths).restore(OffsetDateTime::now_utc()) {
        Ok(items) => deck.update(cx, |deck, cx| {
            for manifest in &items {
                deck.show(DeckItem::staged(manifest), cx);
            }
        }),
        Err(e) => tracing::warn!(error = %e, "deck staging not restored"),
    }
    let recordings_dir = paths.recordings_dir();
    let found = cx.background_executor().spawn(async move {
        match es_record::recovery::recover(&recordings_dir, &es_record::project::SystemRunner) {
            Ok(report) => {
                tracing::info!(
                    recovered = report.recovered.len(),
                    unplayable = report.unplayable.len(),
                    "recording recovery"
                );
                report.recovered
            }
            Err(e) => {
                tracing::warn!(error = %e, "recording recovery");
                Vec::new()
            }
        }
    });
    let shown = recordings.clone();
    cx.spawn(async move |cx| {
        let found = found.await;
        cx.update(|cx| shown.show_recovered(&found, cx));
    })
    .detach();
    sweep_orphans(paths);
    journal::recover(paths)
}

fn session_of_staged(file: &Path) -> Option<String> {
    let name = file.file_name()?.to_str()?;
    let (session, _) = name.split_once(".base-")?;
    Some(session.to_owned())
}

/// A staged base whose session has no journal on disk at all can never be reopened. A journal
/// that exists but cannot be read keeps its files.
fn sweep_orphans(paths: &Paths) {
    let Ok(dir) = std::fs::read_dir(paths.recovery_dir()) else {
        return;
    };
    for path in dir.filter_map(|e| e.ok().map(|e| e.path())) {
        let Some(session) = session_of_staged(&path) else {
            continue;
        };
        let staged = journal::staged_files(paths, &session);
        if !staged.contains(&path) || journal::journal_path(paths, &session).exists() {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::info!(path = %path.display(), "orphaned editor base removed"),
            Err(e) => tracing::warn!(error = %e, "orphaned editor base not removed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bases_without_any_journal_are_swept() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        let recovery = paths.recovery_dir();
        std::fs::create_dir_all(&recovery).unwrap();
        let id = "0b8f0e3c-9a52-4d43-a8e2-1e7f1f0b9c11";
        let orphan = recovery.join(format!("lost.base-{id}.png"));
        let kept = recovery.join(format!("open.base-{id}.png"));
        let unrelated = recovery.join("notes.png");
        for file in [&orphan, &kept, &unrelated] {
            std::fs::write(file, b"png").unwrap();
        }
        std::fs::write(recovery.join("open.json"), b"unreadable").unwrap();
        sweep_orphans(&paths);
        assert!(!orphan.exists());
        assert!(kept.exists(), "an unreadable journal still owns its base");
        assert!(unrelated.exists());
    }
}
