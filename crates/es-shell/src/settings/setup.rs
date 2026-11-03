use super::binds::{compositor_trigger, normalise, runs, trigger_string};
use es_backend::{BindHelper, Trigger as CompositorTrigger};
use es_protocol::{CATALOGUE, Route, ShortcutEntry, Target, Trigger};
use es_store::paths::Paths;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
