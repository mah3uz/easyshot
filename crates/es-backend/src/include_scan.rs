use crate::error::{Error, Result};
use crate::probe::Env;
use std::path::{Path, PathBuf};

/// How one compositor config language writes an include of a file.
pub(crate) struct Rule {
    pub keyword: &'static str,
    pub comment: &'static str,
    /// The path a line includes, when it includes one.
    pub reference: fn(&str) -> Option<String>,
    /// Appended to a referenced path that has no extension.
    pub suffix: &'static str,
    /// Module names such as `a.b` name the file `a/b`.
    pub dotted: bool,
}

fn quoted(s: &str) -> Option<String> {
    let start = s.find(['"', '\''])?;
    let quote = s[start..].chars().next()?;
    let rest = &s[start + 1..];
    Some(rest[..rest.find(quote)?].to_owned())
}

pub(crate) const LUA: Rule = Rule {
    keyword: "require",
    comment: "--",
    reference: |line| quoted(&line[line.find("require")?..]),
    suffix: ".lua",
    dotted: true,
};

pub(crate) const HYPRLANG: Rule = Rule {
    keyword: "source",
    comment: "#",
    reference: |line| {
        let value = line
            .trim()
            .strip_prefix("source")?
            .trim()
            .strip_prefix('=')?;
        Some(value.split('#').next()?.trim().to_owned())
    },
    suffix: "",
    dotted: false,
};

pub(crate) const SWAY: Rule = Rule {
    keyword: "include",
    comment: "#",
    reference: |line| Some(line.trim().strip_prefix("include")?.trim().to_owned()),
    suffix: "",
    dotted: false,
};

pub(crate) const NIRI: Rule = Rule {
    keyword: "include",
    comment: "//",
    reference: quoted,
    suffix: "",
    dotted: false,
};

fn include_lines<'a>(text: &'a str, rule: &'a Rule) -> impl Iterator<Item = &'a str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with(rule.comment) && l.contains(rule.keyword))
}

fn mentions(text: &str, rule: &Rule, target: &str) -> bool {
    include_lines(text, rule).any(|l| l.contains(target))
}

fn resolve(reference: &str, base: &Path, home: Option<&Path>, rule: &Rule) -> Option<PathBuf> {
    let explicit = ["/", "./", "../", "~/"]
        .iter()
        .any(|p| reference.starts_with(p));
    let name = if rule.dotted && !explicit {
        reference.replace('.', "/")
    } else {
        reference.to_owned()
    };
    let mut path = match name.strip_prefix("~/") {
        Some(rest) => home?.join(rest),
        None => base.join(&name),
    };
    if !rule.suffix.is_empty() && path.extension().is_none() {
        let mut os = path.into_os_string();
        os.push(rule.suffix);
        path = PathBuf::from(os);
    }
    Some(path)
}

/// True when the main config, or a file it includes directly, includes the binds file in `dir`.
pub(crate) fn scan(main: &Path, home: Option<&Path>, rule: &Rule, dir: &Path) -> Result<bool> {
    let target = format!(
        "{}/binds",
        dir.file_name().and_then(|n| n.to_str()).unwrap_or_default()
    );
    let text = std::fs::read_to_string(main).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => Error::NotFound(main.display().to_string()),
        _ => Error::Io {
            path: main.to_path_buf(),
            source,
        },
    })?;
    if mentions(&text, rule, &target) {
        return Ok(true);
    }
    let base = main.parent().unwrap_or_else(|| Path::new("."));
    Ok(include_lines(&text, rule)
        .filter_map(rule.reference)
        .filter_map(|r| resolve(&r, base, home, rule))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .any(|t| mentions(&t, rule, &target)))
}

pub(crate) fn config_home(env: &Env) -> Option<PathBuf> {
    match env.var("XDG_CONFIG_HOME") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => Some(PathBuf::from(env.var("HOME")?).join(".config")),
    }
}

pub(crate) fn home(env: &Env) -> Option<PathBuf> {
    env.var("HOME").map(PathBuf::from)
}
