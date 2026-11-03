use clap_complete::CompletionCandidate;
use es_protocol::Route;
use std::ffi::OsStr;
use std::path::Path;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

pub fn open_target(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(current) = current.to_str() else {
        return Vec::new();
    };
    let mut out: Vec<_> = Route::url_routes()
        .filter(|url| url.starts_with(current))
        .map(CompletionCandidate::new)
        .collect();
    if !current.contains("://") {
        out.extend(
            image_paths(current)
                .into_iter()
                .map(CompletionCandidate::new),
        );
    }
    out
}

const BASH_WORDS: &str = r#"    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
"#;

const BASH_WORDS_JOINED: &str = r#"    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        words[COMP_CWORD]="$2"
    fi
    local _line=${COMP_LINE:0:COMP_POINT}
    local _token=${_line##*[[:space:]]}
    local _trim=
    if [[ $_token == *:* && $COMP_WORDBREAKS == *:* ]]; then
        IFS=$' \t\n' read -ra words <<< "$_line"
        _CLAP_COMPLETE_INDEX=$(( ${#words[@]} - 1 ))
        _trim=${_token%"${_token##*:}"}
    fi
"#;

const BASH_END: &str = "        compopt -o nospace\n    fi\n}";

const BASH_END_TRIMMED: &str = "        compopt -o nospace\n    fi\n    if [[ -n $_trim && -n ${COMPREPLY-} ]]; then\n        COMPREPLY=( \"${COMPREPLY[@]#\"$_trim\"}\" )\n    fi\n}";

/// bash splits words at `:`, which would cut `easyshot://` links in two; zsh's autoloaded
/// `_easyshot` only registers the completer unless it also runs it once.
pub fn adapt_registration(shell: &str, script: &str) -> String {
    match shell {
        "bash" => script
            .replace(BASH_WORDS, BASH_WORDS_JOINED)
            .replace(BASH_END, BASH_END_TRIMMED),
        "zsh" => format!(
            "{}\nif [[ $funcstack[1] == _easyshot ]]; then\n    _clap_dynamic_completer_easyshot \"$@\"\nfi\n",
            script.trim_end()
        ),
        _ => script.to_owned(),
    }
}

fn image_paths(current: &str) -> Vec<String> {
    let (dir, prefix) = match current.rfind('/') {
        Some(i) => current.split_at(i + 1),
        None => ("", current),
    };
    let read = if dir.is_empty() { "." } else { dir };
    let Ok(entries) = std::fs::read_dir(read) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(prefix) || (name.starts_with('.') && !prefix.starts_with('.')) {
                return None;
            }
            let path = entry.path();
            if path.is_dir() {
                Some(format!("{dir}{name}/"))
            } else if is_image(&path) {
                Some(format!("{dir}{name}"))
            } else {
                None
            }
        })
        .collect();
    found.sort();
    found
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|e| IMAGE_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(current: &str) -> Vec<String> {
        open_target(OsStr::new(current))
            .iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn open_offers_every_url_route() {
        assert_eq!(values("easyshot://").len(), Route::url_routes().count());
        assert_eq!(
            values("easyshot://cap"),
            [
                "easyshot://capture/region",
                "easyshot://capture/fullscreen",
                "easyshot://capture/window",
                "easyshot://capture/scroll",
            ]
        );
    }

    #[test]
    fn open_offers_only_images_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        for f in ["a.png", "b.JPG", "c.jpeg", "d.txt", ".hidden.png"] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let base = format!("{}/", dir.path().display());
        let got = values(&base);
        let names: Vec<_> = got.iter().map(|v| v.strip_prefix(&base).unwrap()).collect();
        assert_eq!(names, ["a.png", "b.JPG", "c.jpeg", "sub/"]);
    }
}
