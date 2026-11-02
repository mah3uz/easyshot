use crate::datefmt;
use es_model::State;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

pub const DEFAULT_TEMPLATE: &str = "easyshot_{date}-{time}";
const MAX_NAME_CHARS: usize = 200;
const MAX_UNIQUE_SUFFIX: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Screenshot,
    Recording,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Screenshot => "Screenshot",
            Kind::Recording => "Recording",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub date: OffsetDateTime,
    pub kind: Kind,
    pub counter: u32,
}

impl Context {
    pub fn new(date: OffsetDateTime) -> Self {
        Self {
            date,
            kind: Kind::Screenshot,
            counter: 1,
        }
    }
}

const HEX: &str = "0123456789abcdef";
const BASE62: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";

fn random_string(alphabet: &str, count: usize) -> String {
    let alphabet: Vec<char> = alphabet.chars().collect();
    let mut out = String::with_capacity(count);
    while out.len() < count {
        // Bytes 6 and 8 of a v4 UUID carry fixed version and variant bits.
        for (i, b) in Uuid::new_v4().as_bytes().iter().enumerate() {
            if i != 6 && i != 8 && out.len() < count {
                out.push(alphabet[usize::from(*b) % alphabet.len()]);
            }
        }
    }
    out
}

fn length(arg: Option<&str>, fallback: usize) -> usize {
    arg.and_then(|a| a.trim().parse::<i64>().ok())
        .map_or(fallback, |n| n.clamp(1, 32) as usize)
}

fn value(token: &str, ctx: &Context, ext: &str) -> Option<String> {
    let (name, arg) = match token.split_once(':') {
        Some((n, a)) => (n, Some(a)),
        None => (token, None),
    };
    Some(match name.trim().to_lowercase().as_str() {
        "date" => datefmt::format(ctx.date, arg.unwrap_or("yyyy-MM-dd")),
        "time" => datefmt::format(ctx.date, arg.unwrap_or("HH-mm-ss")),
        "hex" => random_string(HEX, length(arg, 8)),
        "base62" => random_string(BASE62, length(arg, 6)),
        "digits" => random_string(DIGITS, length(arg, 4)),
        "uuid" => Uuid::new_v4().to_string(),
        "counter" => format!("{:0width$}", ctx.counter, width = length(arg, 1)),
        "kind" => ctx.kind.label().into(),
        "ext" => ext.into(),
        _ => return None,
    })
}

/// Substitutes tokens; an unknown or unclosed brace is copied through as typed.
pub fn render(template: &str, ctx: &Context, ext: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let token = after
            .find('}')
            .and_then(|close| value(&after[..close], ctx, ext).map(|v| (v, close)));
        match token {
            Some((v, close)) => {
                out.push_str(&v);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

pub fn token_names(template: &str) -> Vec<String> {
    let probe = Context::new(OffsetDateTime::UNIX_EPOCH);
    let mut names = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if value(&after[..close], &probe, "png").is_some() => {
                let body = &after[..close];
                let name = body.split(':').next().unwrap_or(body);
                names.push(name.trim().to_lowercase());
                rest = &after[close + 1..];
            }
            _ => rest = after,
        }
    }
    names
}

pub fn uses_counter(template: &str) -> bool {
    token_names(template).iter().any(|n| n == "counter")
}

fn sanitize(name: &str) -> String {
    const ILLEGAL: &str = "/\\:*?\"<>|";
    let cleaned: String = name
        .chars()
        .filter(|c| !ILLEGAL.contains(*c) && !c.is_control())
        .collect();
    let trimmed = cleaned.trim_start_matches(['.', ' ']);
    let cut: String = trimmed.chars().take(MAX_NAME_CHARS).collect();
    cut.trim_matches(['.', ' ']).to_owned()
}

/// The finished file name. An empty result falls back to the default template.
pub fn file_name(template: &str, ext: &str, ctx: &Context) -> String {
    let rendered = sanitize(&render(template, ctx, ""));
    let base = if rendered.is_empty() {
        sanitize(&render(DEFAULT_TEMPLATE, ctx, ""))
    } else {
        rendered
    };
    let ext = sanitize(ext);
    if ext.is_empty()
        || base
            .to_lowercase()
            .ends_with(&format!(".{}", ext.to_lowercase()))
    {
        return base;
    }
    format!("{base}.{ext}")
}

/// Names one new capture. Spends one `{counter}` number from `state`, and only when the template uses it.
pub fn next_file_name(
    template: &str,
    ext: &str,
    kind: Kind,
    date: OffsetDateTime,
    state: &mut State,
) -> String {
    let counter = state.counter.max(1);
    let name = file_name(
        template,
        ext,
        &Context {
            date,
            kind,
            counter,
        },
    );
    if uses_counter(template) {
        state.counter = counter.saturating_add(1);
    }
    name
}

/// `file_name` in `dir`, or the first free `name<sep>1.ext`. `is_taken` lets a caller reserve companion files.
pub fn unique_path(
    file_name: &str,
    dir: &Path,
    separator: &str,
    is_taken: impl Fn(&Path) -> bool,
) -> PathBuf {
    let original = dir.join(file_name);
    if !is_taken(&original) {
        return original;
    }
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let ext = path
        .extension()
        .map_or_else(String::new, |e| format!(".{}", e.to_string_lossy()));
    for index in 1..=MAX_UNIQUE_SUFFIX {
        let candidate = dir.join(format!("{stem}{separator}{index}{ext}"));
        if !is_taken(&candidate) {
            return candidate;
        }
    }
    dir.join(format!("{stem}{separator}{}{ext}", Uuid::new_v4()))
}

pub fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}
