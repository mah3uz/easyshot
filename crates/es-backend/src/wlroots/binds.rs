use crate::traits::Trigger;
use es_protocol::Route;

pub(crate) const EDITOR_RULE: &str = r#"for_window [app_id="dev.easyshot.editor"] "floating enable, resize set 1180 780, move position center""#;

pub(crate) fn snippet(binds: &[(Trigger, Route)]) -> String {
    let mut out = String::new();
    // A route with arguments has no fixed command line to bind.
    for (trigger, route) in binds.iter().filter(|(_, r)| r.args().0.is_empty()) {
        let command = route.command_args().join(" ");
        out.push_str(&format!("bindsym {} exec easyshot {command}\n", trigger.0));
    }
    out
}

/// Binds of the default mode; variables such as `$mod` are left as written.
pub(crate) fn parse_config(config: &str) -> Vec<(Trigger, String)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut bind_block: Option<usize> = None;
    for raw in config.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "}" {
            if bind_block == Some(depth) {
                bind_block = None;
            }
            depth = depth.saturating_sub(1);
            continue;
        }
        if bind_block.is_some() {
            push_bind(line, &mut out);
            continue;
        }
        let mut words = line.split_whitespace();
        let first = words.next().unwrap_or_default();
        let opens = line.ends_with('{');
        if depth == 0 && matches!(first, "bindsym" | "bindcode") {
            let rest = skip_flags(&line[first.len()..]);
            if opens && rest.trim() == "{" {
                depth += 1;
                bind_block = Some(depth);
            } else {
                push_bind(rest, &mut out);
            }
        } else if opens {
            depth += 1;
        }
    }
    out
}

fn skip_flags(mut s: &str) -> &str {
    loop {
        s = s.trim_start();
        match s.strip_prefix("--") {
            Some(flag) => s = flag.split_once(char::is_whitespace).map_or("", |(_, r)| r),
            None => return s,
        }
    }
}

fn push_bind(line: &str, out: &mut Vec<(Trigger, String)>) {
    let line = skip_flags(line);
    if let Some((key, command)) = line.split_once(char::is_whitespace) {
        out.push((Trigger(key.to_owned()), command.trim().to_owned()));
    }
}
