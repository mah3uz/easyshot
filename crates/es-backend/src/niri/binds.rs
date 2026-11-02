use crate::probe::Env;
use crate::traits::Trigger;
use es_protocol::Route;
use std::path::PathBuf;

pub(crate) fn config_path(env: &Env) -> Option<PathBuf> {
    if let Some(p) = env.var("NIRI_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = match env.var("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env.var("HOME")?).join(".config"),
    };
    Some(base.join("niri").join("config.kdl"))
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

pub(crate) fn snippet(binds: &[(Trigger, Route)]) -> String {
    let mut out = String::new();
    // A route with arguments has no fixed command line to bind.
    for (trigger, route) in binds.iter().filter(|(_, r)| r.args().0.is_empty()) {
        let mut spawn = quote("easyshot");
        for word in route.as_str().split('/') {
            spawn.push(' ');
            spawn.push_str(&quote(word));
        }
        out.push_str(&format!("{} {{ spawn {spawn}; }}\n", trigger.0));
    }
    out
}

/// `(opening, closing)` braces outside strings, up to a `//` comment.
fn braces(line: &str) -> (usize, usize) {
    let (mut open, mut close, mut in_str, mut escaped) = (0, 0, false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '/' if !in_str && chars.peek() == Some(&'/') => break,
            '{' if !in_str => open += 1,
            '}' if !in_str => close += 1,
            _ => {}
        }
    }
    (open, close)
}

/// Binds in top-level `binds` blocks; includes and `/* */` comments are not followed.
pub(crate) fn parse_config(config: &str) -> Vec<(Trigger, String)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_binds = false;
    let mut pending: Option<(String, String)> = None;
    let mut skipping = false;
    for raw in config.lines() {
        let line = raw.trim();
        let (open, close) = braces(line);
        let start = depth;
        depth = (depth + open).saturating_sub(close);
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if start == 0 {
            in_binds = line.starts_with("binds") && open > 0;
            continue;
        }
        if !in_binds {
            continue;
        }
        if start == 1 {
            skipping = line.starts_with("/-") || line.starts_with('}');
            if !skipping {
                let key = line.split_whitespace().next().unwrap_or_default();
                let body = line.split_once('{').map_or("", |(_, b)| b);
                pending = Some((key.to_owned(), body.to_owned()));
            }
        } else if let Some((_, body)) = pending.as_mut().filter(|_| !skipping) {
            body.push(' ');
            body.push_str(line);
        }
        if depth <= 1 {
            if let Some((key, body)) = pending.take() {
                let action = body
                    .trim()
                    .trim_end_matches('}')
                    .trim()
                    .trim_end_matches(';');
                out.push((Trigger(key), action.trim().to_owned()));
            }
            if depth == 0 {
                in_binds = false;
            }
        }
    }
    out
}
