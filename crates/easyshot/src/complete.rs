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
