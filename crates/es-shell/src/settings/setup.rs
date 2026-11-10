use super::binds::{compositor_trigger, normalise, runs, trigger_string};
use es_backend::{Backend, BindHelper, Trigger as CompositorTrigger};
use es_protocol::{CATALOGUE, Route, ShortcutEntry, Target, Trigger};
use es_store::paths::Paths;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Actions offered keys on first run, each with the keys to try in order.
const SUGGESTIONS: [(&str, &[&str]); 4] = [
    (
        "capture-region",
        &["PrintScreen", "Super+Shift+S", "Super+Alt+S"],
    ),
    (
        "capture-fullscreen",
        &["Shift+PrintScreen", "Super+Shift+3", "Super+Alt+F"],
    ),
    ("capture-window", &["Alt+PrintScreen", "Super+Shift+W"]),
    ("capture-bar", &["Ctrl+PrintScreen", "Super+Shift+A"]),
];

pub(crate) fn route_of(entry: &ShortcutEntry) -> Option<&Route> {
    match &entry.target {
        Target::Route(route) if route.args().0.is_empty() => Some(route),
        _ => None,
    }
}

pub(crate) fn entry(id: &str) -> Option<&'static ShortcutEntry> {
    CATALOGUE.iter().find(|e| e.id == id)
}

/// One key per action from its candidates, skipping keys the compositor already binds to
/// something else or that an earlier suggestion took. Actions that have a key are left alone.
pub fn suggest(
    binds: &dyn BindHelper,
    existing: &[(CompositorTrigger, String)],
    have: &BTreeMap<String, String>,
) -> Vec<(&'static ShortcutEntry, Option<Trigger>)> {
    let syntax = binds.syntax();
    let mut taken: Vec<_> = have
        .values()
        .filter_map(|v| Trigger::parse(v).ok())
        .map(|t| normalise(&compositor_trigger(&t, syntax).0))
        .collect();
    let mut out = Vec::new();
    for (id, candidates) in SUGGESTIONS {
        let Some(entry) = entry(id) else { continue };
        let Some(route) = route_of(entry) else {
            continue;
        };
        if have.contains_key(id) {
            continue;
        }
        let free = candidates
            .iter()
            .filter_map(|c| Trigger::parse(c).ok())
            .find(|t| {
                let key = normalise(&compositor_trigger(t, syntax).0);
                !taken.contains(&key)
                    && !existing
                        .iter()
                        .any(|(bound, command)| normalise(&bound.0) == key && !runs(command, route))
            });
        if let Some(t) = &free {
            taken.push(normalise(&compositor_trigger(t, syntax).0));
        }
        out.push((entry, free));
    }
    out
}

pub fn global_binds(
    binds: &dyn BindHelper,
    globals: &BTreeMap<String, String>,
) -> Vec<(CompositorTrigger, Route)> {
    let syntax = binds.syntax();
    CATALOGUE
        .iter()
        .filter_map(|e| {
            let trigger = Trigger::parse(globals.get(e.id)?).ok()?;
            Some((compositor_trigger(&trigger, syntax), route_of(e)?.clone()))
        })
        .collect()
}

pub fn write_binds_file(
    binds: &dyn BindHelper,
    paths: &Paths,
    globals: &BTreeMap<String, String>,
) -> es_store::Result<PathBuf> {
    let text = binds.file_text(&global_binds(binds, globals));
    es_store::binds::write(paths, binds.file_name(), &text)
}

pub fn ensure_binds_file(
    binds: &dyn BindHelper,
    paths: &Paths,
    globals: &BTreeMap<String, String>,
) -> es_store::Result<()> {
    if paths.binds_file(binds.file_name()).exists() {
        return Ok(());
    }
    write_binds_file(binds, paths, globals).map(|_| ())
}

/// Rewrites an existing binds file whose text differs from what this build writes for the same
/// keys. A file with a command easyshot cannot map back to a route is hand-edited and left alone.
pub fn refresh_binds_file(binds: &dyn BindHelper, paths: &Paths) -> es_store::Result<bool> {
    let Some(text) = es_store::binds::read(paths, binds.file_name()) else {
        return Ok(false);
    };
    let routes = Route::all();
    let mapped: Option<Vec<(CompositorTrigger, Route)>> = binds
        .file_binds(&text)
        .into_iter()
        .map(|(trigger, command)| {
            let route = routes.iter().find(|r| runs(&command, r))?;
            Some((trigger, route.clone()))
        })
        .collect();
    let Some(mapped) = mapped.filter(|m| !m.is_empty()) else {
        tracing::info!("binds file left as it is: it holds commands easyshot did not write");
        return Ok(false);
    };
    let fresh = binds.file_text(&mapped);
    if fresh == text {
        return Ok(false);
    }
    es_store::binds::write(paths, binds.file_name(), &fresh)?;
    tracing::info!("binds file brought up to date");
    Ok(true)
}

/// The backend's bind helper, after bringing an existing binds file up to date.
pub fn refreshed_binds(backend: Arc<Backend>, paths: &Paths) -> Option<Arc<dyn BindHelper>> {
    let binds = super::binds::from_backend(backend)?;
    if let Err(e) = refresh_binds_file(binds.as_ref(), paths) {
        tracing::warn!(error = %e, "binds file not brought up to date");
    }
    Some(binds)
}

pub fn globals_with(
    have: &BTreeMap<String, String>,
    chosen: &[(&ShortcutEntry, Option<Trigger>)],
) -> BTreeMap<String, String> {
    let mut out = have.clone();
    for (entry, trigger) in chosen {
        if let Some(t) = trigger {
            out.insert(entry.id.to_owned(), trigger_string(t));
        }
    }
    out
}

/// The config directory as written in the compositor config: `~` stands for the home directory.
pub fn display_dir(paths: &Paths) -> String {
    display_under(
        paths.config_dir(),
        std::env::var_os("HOME").map(PathBuf::from).as_deref(),
    )
}

fn display_under(dir: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| dir.strip_prefix(h).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => dir.display().to_string(),
    }
}

pub fn include_line(binds: &dyn BindHelper, paths: &Paths) -> String {
    binds.include_line(&display_dir(paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_backend::BindSyntax;

    struct Fake;

    impl BindHelper for Fake {
        fn syntax(&self) -> BindSyntax {
            BindSyntax::Sway
        }
        fn snippet(&self, _: &[(CompositorTrigger, Route)]) -> String {
            String::new()
        }
        fn existing_binds(&self) -> es_backend::Result<Vec<(CompositorTrigger, String)>> {
            Ok(Vec::new())
        }
        fn file_name(&self) -> &'static str {
            "binds.conf"
        }
        fn file_text(&self, binds: &[(CompositorTrigger, Route)]) -> String {
            let mut out = String::from("# current\n");
            for (t, r) in binds {
                out += &format!("{} easyshot {}\n", t.0, r.command_args().join(" "));
            }
            out
        }
        fn file_binds(&self, text: &str) -> Vec<(CompositorTrigger, String)> {
            text.lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.split_once(' '))
                .map(|(t, c)| (CompositorTrigger(t.into()), c.into()))
                .collect()
        }
        fn include_line(&self, _: &str) -> String {
            String::new()
        }
        fn is_included(&self, _: &Path) -> es_backend::Result<bool> {
            Ok(false)
        }
    }

    fn with_file(text: Option<&str>) -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = Paths::rooted(dir.path());
        if let Some(text) = text {
            std::fs::create_dir_all(paths.config_dir()).expect("config dir");
            std::fs::write(paths.binds_file("binds.conf"), text).expect("write");
        }
        (dir, paths)
    }

    fn on_disk(paths: &Paths) -> String {
        std::fs::read_to_string(paths.binds_file("binds.conf")).expect("read")
    }

    #[test]
    fn an_older_file_is_rewritten_with_the_same_keys() {
        let (_dir, paths) = with_file(Some("Mod4+s easyshot capture region\n"));
        assert!(refresh_binds_file(&Fake, &paths).expect("refresh"));
        assert_eq!(
            on_disk(&paths),
            "# current\nMod4+s easyshot capture region\n"
        );
    }

    #[test]
    fn a_file_with_a_foreign_command_is_left_byte_for_byte() {
        let text = "Mod4+s easyshot capture region\nMod4+x easyshot frobnicate\n";
        let (_dir, paths) = with_file(Some(text));
        assert!(!refresh_binds_file(&Fake, &paths).expect("refresh"));
        assert_eq!(on_disk(&paths), text);
    }

    #[test]
    fn a_current_file_is_not_written_again() {
        let text = "# current\nMod4+s easyshot capture region\n";
        let (_dir, paths) = with_file(Some(text));
        assert!(!refresh_binds_file(&Fake, &paths).expect("refresh"));
    }

    #[test]
    fn no_file_is_never_created() {
        let (_dir, paths) = with_file(None);
        assert!(!refresh_binds_file(&Fake, &paths).expect("refresh"));
        assert!(!paths.binds_file("binds.conf").exists());
    }

    #[test]
    fn the_home_directory_is_written_as_a_tilde() {
        let home = Some(Path::new("/home/ada"));
        assert_eq!(
            display_under(Path::new("/home/ada/.config/easyshot"), home),
            "~/.config/easyshot"
        );
        assert_eq!(
            display_under(Path::new("/srv/cfg/easyshot"), home),
            "/srv/cfg/easyshot"
        );
    }
}
