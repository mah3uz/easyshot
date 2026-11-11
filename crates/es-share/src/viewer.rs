use std::collections::HashMap;

/// The share page; the service fills the same file for its own shares.
pub const PAGE: &str = include_str!("../../../services/share/viewer/viewer.html");

const OPEN: &str = "<!--if:";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FillError {
    #[error("no value for {{{{{0}}}}}")]
    MissingValue(String),
    #[error("{0} is not an https URL")]
    NotHttps(String),
    #[error("block {0} has no end marker")]
    Unclosed(String),
}

pub fn fill(
    template: &str,
    values: &HashMap<&str, &str>,
    blocks: &[&str],
) -> Result<String, FillError> {
    if let Some((name, _)) = values
        .iter()
        .find(|(name, value)| name.ends_with("_url") && !value.starts_with("https://"))
    {
        return Err(FillError::NotHttps((*name).to_owned()));
    }
    substitute(&resolve_blocks(template, blocks)?, values)
}

fn resolve_blocks(template: &str, blocks: &[&str]) -> Result<String, FillError> {
    let mut text = template.to_owned();
    let mut from = 0;
    while let Some(found) = text[from..].find(OPEN) {
        let start = from + found;
        let name_at = start + OPEN.len();
        let Some(name_len) = name_then(&text[name_at..], "-->", |c| c.is_ascii_lowercase()) else {
            from = name_at;
            continue;
        };
        let name = text[name_at..name_at + name_len].to_owned();
        let open_end = name_at + name_len + "-->".len();
        let end_marker = format!("<!--end:{name}-->");
        let close = text[open_end..]
            .find(&end_marker)
            .map(|at| open_end + at)
            .ok_or_else(|| FillError::Unclosed(name.clone()))?;
        if blocks.contains(&name.as_str()) {
            text.replace_range(close..close + end_marker.len(), "");
            text.replace_range(start..open_end, "");
        } else {
            text.replace_range(start..close + end_marker.len(), "");
        }
        from = start;
    }
    Ok(text)
}

fn substitute(text: &str, values: &HashMap<&str, &str>) -> Result<String, FillError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("{{") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match name_then(after, "}}", |c| c.is_ascii_lowercase() || c == '_') {
            Some(len) => {
                let name = &after[..len];
                let value = values
                    .get(name)
                    .ok_or_else(|| FillError::MissingValue(name.to_owned()))?;
                escape_into(&mut out, value);
                rest = &after[len + 2..];
            }
            None => {
                out.push_str("{{");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn name_then(s: &str, end: &str, allowed: impl Fn(char) -> bool) -> Option<usize> {
    let len = s.find(|c: char| !allowed(c)).unwrap_or(s.len());
    (len > 0 && s[len..].starts_with(end)).then_some(len)
}

fn escape_into(out: &mut String, value: &str) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests;
