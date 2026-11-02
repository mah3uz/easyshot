use crate::atomic::write_atomic;
use crate::document::{self, sidecar_path};
use crate::error::{Result, io};
use crate::library::base_path;
use crate::naming::{exists, unique_path};
use crate::paths::Paths;
use es_model::AnnotationDocument;
use std::path::{Path, PathBuf};

pub struct Commit<'a> {
    /// The image being edited. Outside the library, the result is imported into it.
    pub display: &'a Path,
    /// The untouched pixels the editor renders from; the display image itself on a first edit.
    pub base: &'a Path,
    pub rendered_png: &'a [u8],
    pub document: AnnotationDocument,
}

pub struct Reopened {
    pub display: PathBuf,
    /// What the editor renders from: the base when a document and base exist, else the image.
    pub source: PathBuf,
    pub document: Option<AnnotationDocument>,
    pub read_only: bool,
}

fn copy_atomic(from: &Path, to: &Path) -> Result<()> {
    write_atomic(to, &std::fs::read(from).map_err(io(from))?)
}

fn library_target(paths: &Paths, name: &str) -> PathBuf {
    unique_path(name, &paths.library_dir(), "-", |p| {
        exists(p) || exists(&base_path(p)) || exists(&sidecar_path(p))
    })
}

fn write_sidecar(display: &Path, base_dest: &Path, mut document: AnnotationDocument) -> Result<()> {
    document.base_image_file_name = base_dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    document::create(display, document).save()
}

/// Adds a new capture held in memory to the library under a free name. With `edit`, the raw pixels
/// and the document come along so the editor reopens the look; without it the image stands alone.
pub fn commit_new(
    paths: &Paths,
    name: &str,
    rendered_png: &[u8],
    edit: Option<(&[u8], AnnotationDocument)>,
) -> Result<PathBuf> {
    let display = library_target(paths, name);
    if let Some((base_png, document)) = edit {
        let base_dest = base_path(&display);
        write_atomic(&base_dest, base_png)?;
        write_atomic(&display, rendered_png)?;
        write_sidecar(&display, &base_dest, document)?;
    } else {
        write_atomic(&display, rendered_png)?;
    }
    Ok(display)
}

/// Writes base, flattened image, then sidecar, each atomically; returns the image path in the library.
pub fn commit(paths: &Paths, commit: Commit<'_>) -> Result<PathBuf> {
    let library = paths.library_dir();
    let display = if commit.display.starts_with(&library) {
        commit.display.to_path_buf()
    } else {
        let name = commit
            .display
            .with_extension("png")
            .file_name()
            .map_or_else(
                || "capture.png".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
        library_target(paths, &name)
    };
    let base_dest = base_path(&display);
    if commit.base != base_dest {
        if commit.base == commit.display {
            if !base_dest.exists() && commit.display.exists() {
                copy_atomic(commit.display, &base_dest)?;
            }
        } else {
            copy_atomic(commit.base, &base_dest)?;
        }
    }
    write_atomic(&display, commit.rendered_png)?;
    write_sidecar(&display, &base_dest, commit.document)?;
    Ok(display)
}

/// A plain capture has no sidecar and comes back with no document.
pub fn reopen(image: &Path) -> Reopened {
    let sidecar = document::open(image);
    let read_only = sidecar
        .as_ref()
        .is_some_and(|s| s.newer_version().is_some());
    let document = sidecar.map(|s| s.value);
    let base = document
        .as_ref()
        .map(|d| {
            Path::new(&d.base_image_file_name)
                .file_name()
                .map_or_else(|| base_path(image), |n| image.with_file_name(n))
        })
        .filter(|b| b.exists());
    Reopened {
        display: image.to_path_buf(),
        source: base.unwrap_or_else(|| image.to_path_buf()),
        document,
        read_only,
    }
}
