#![allow(clippy::unwrap_used, clippy::expect_used)]

const CATALOGUE: &str = include_str!("../../../../i18n/en-US/es-shell.ftl");

/// Messages that take no arguments, so a `{` left in their text is an unresolved reference.
fn plain_ids() -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut current: Option<(&str, bool)> = None;
    for line in CATALOGUE.lines().chain(["end = x"]) {
        let id = line.split_once('=').map(|(id, _)| id.trim()).filter(|id| {
            !line.starts_with([' ', '#', '.', '-', '*', '}'])
                && !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        });
        if let Some(id) = id {
            if let Some((prev, true)) = current {
                out.push(prev);
            }
            current = Some((id, true));
        }
        if (line.contains('$') || line.trim_start().starts_with('['))
            && let Some((_, plain)) = current.as_mut()
        {
            *plain = false;
        }
    }
    out
}

#[test]
fn no_message_shows_an_unresolved_reference() {
    let loader = es_shell::strings::loader();
    let ids = plain_ids();
    assert!(ids.len() > 200, "the scan found only {}", ids.len());
    let broken: Vec<String> = ids
        .into_iter()
        .map(|id| (id, loader.get(id)))
        .filter(|(_, text)| text.contains('{'))
        .map(|(id, text)| format!("{id}: {text}"))
        .collect();
    assert!(broken.is_empty(), "{broken:#?}");
}

#[test]
fn the_settings_sidebar_group_is_the_app_name() {
    assert_eq!(
        es_shell::strings::loader().get("settings-nav-group-app"),
        "easyshot"
    );
}
