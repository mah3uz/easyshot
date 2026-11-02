#![allow(clippy::unwrap_used)]
use es_model::AnnotationDocument;
use es_store::document::sidecar_path;
use es_store::library::base_path;
use es_store::{Commit, History, Paths, commit, reopen};
use std::path::Path;

fn setup() -> (tempfile::TempDir, Paths) {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    paths.ensure_all().unwrap();
    (dir, paths)
}

fn png_bytes(w: u32, h: u32, shade: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([shade, shade, shade, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn doc(tag: &str) -> AnnotationDocument {
    AnnotationDocument {
        base_image_file_name: tag.into(),
        ..AnnotationDocument::default()
    }
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

#[test]
fn first_commit_of_an_outside_capture_imports_it_and_keeps_the_raw_pixels_as_base() {
    let (dir, paths) = setup();
    let raw = dir.path().join("raw.png");
    std::fs::write(&raw, png_bytes(10, 8, 7)).unwrap();
    let rendered = png_bytes(12, 10, 200);

    let image = commit(
        &paths,
        Commit {
            display: &raw,
            base: &raw,
            rendered_png: &rendered,
            document: doc("ignored"),
        },
    )
    .unwrap();

    assert!(image.starts_with(paths.library_dir()));
    assert_eq!(read(&image), rendered);
    assert_eq!(read(&base_path(&image)), png_bytes(10, 8, 7));
    assert!(sidecar_path(&image).exists());
    assert_eq!(
        read(&raw),
        png_bytes(10, 8, 7),
        "the original is left alone"
    );
}

#[test]
fn commit_then_reopen_round_trips_document_and_base() {
    let (dir, paths) = setup();
    let raw = dir.path().join("raw.png");
    std::fs::write(&raw, png_bytes(10, 8, 7)).unwrap();
    let image = commit(
        &paths,
        Commit {
            display: &raw,
            base: &raw,
            rendered_png: &png_bytes(10, 8, 99),
            document: doc("x"),
        },
    )
    .unwrap();

    let opened = reopen(&image);
    assert_eq!(opened.display, image);
    assert_eq!(opened.source, base_path(&image));
    let document = opened.document.unwrap();
    assert_eq!(
        document.base_image_file_name,
        base_path(&image).file_name().unwrap().to_str().unwrap()
    );
    assert!(!opened.read_only);
}

#[test]
fn a_second_commit_updates_in_place_and_keeps_the_original_base() {
    let (dir, paths) = setup();
    let raw = dir.path().join("raw.png");
    std::fs::write(&raw, png_bytes(10, 8, 7)).unwrap();
    let first = commit(
        &paths,
        Commit {
            display: &raw,
            base: &raw,
            rendered_png: &png_bytes(10, 8, 100),
            document: doc(""),
        },
    )
    .unwrap();
    let base = base_path(&first);
    let second = commit(
        &paths,
        Commit {
            display: &first,
            base: &base,
            rendered_png: &png_bytes(10, 8, 150),
            document: doc(""),
        },
    )
    .unwrap();
    assert_eq!(second, first);
    assert_eq!(read(&second), png_bytes(10, 8, 150));
    assert_eq!(read(&base), png_bytes(10, 8, 7));
    assert_eq!(std::fs::read_dir(paths.library_dir()).unwrap().count(), 3);
}

#[test]
fn a_cropped_base_replaces_the_stored_base() {
    let (dir, paths) = setup();
    let raw = dir.path().join("raw.png");
    std::fs::write(&raw, png_bytes(10, 8, 7)).unwrap();
    let first = commit(
        &paths,
        Commit {
            display: &raw,
            base: &raw,
            rendered_png: &png_bytes(10, 8, 100),
            document: doc(""),
        },
    )
    .unwrap();
    let cropped = dir.path().join("cropped.png");
    std::fs::write(&cropped, png_bytes(4, 4, 33)).unwrap();
    commit(
        &paths,
        Commit {
            display: &first,
            base: &cropped,
            rendered_png: &png_bytes(4, 4, 120),
            document: doc(""),
        },
    )
    .unwrap();
    assert_eq!(read(&base_path(&first)), png_bytes(4, 4, 33));
}

#[test]
fn importing_never_overwrites_an_existing_library_capture() {
    let (dir, paths) = setup();
    let raw = dir.path().join("same.png");
    std::fs::write(&raw, png_bytes(2, 2, 1)).unwrap();
    let make = |shade| {
        commit(
            &paths,
            Commit {
                display: &raw,
                base: &raw,
                rendered_png: &png_bytes(2, 2, shade),
                document: doc(""),
            },
        )
        .unwrap()
    };
    let a = make(10);
    let b = make(20);
    assert_ne!(a, b);
    assert_eq!(read(&a), png_bytes(2, 2, 10));
}

#[test]
fn a_plain_capture_reopens_without_a_sidecar() {
    let (_d, paths) = setup();
    let image = paths.library_dir().join("plain.png");
    std::fs::write(&image, png_bytes(3, 3, 5)).unwrap();
    let opened = reopen(&image);
    assert!(opened.document.is_none());
    assert_eq!(opened.source, image);
}

#[test]
fn a_document_whose_base_is_missing_renders_from_the_image() {
    let (_d, paths) = setup();
    let image = paths.library_dir().join("shot.png");
    std::fs::write(&image, png_bytes(3, 3, 5)).unwrap();
    es_store::document::create(&image, doc("shot.base.png"))
        .save()
        .unwrap();
    let opened = reopen(&image);
    assert!(opened.document.is_some());
    assert_eq!(opened.source, image);
}

#[test]
fn a_sidecar_cannot_point_the_base_outside_the_image_directory() {
    let (_d, paths) = setup();
    let image = paths.library_dir().join("shot.png");
    std::fs::write(&image, png_bytes(3, 3, 5)).unwrap();
    es_store::document::create(&image, doc("../../../etc/passwd"))
        .save()
        .unwrap();
    let opened = reopen(&image);
    assert_eq!(opened.source, image);
}

#[test]
fn marking_a_commit_flags_the_history_entry_and_adds_unknown_ones() {
    let (dir, paths) = setup();
    let raw = dir.path().join("raw.png");
    std::fs::write(&raw, png_bytes(10, 8, 7)).unwrap();
    let mut history = History::open(&paths, 100).unwrap();
    let image = commit(
        &paths,
        Commit {
            display: &raw,
            base: &raw,
            rendered_png: &png_bytes(12, 10, 1),
            document: doc(""),
        },
    )
    .unwrap();
    history.mark_annotated(&image).unwrap();
    let entry = history.by_file(&image).unwrap();
    assert!(entry.has_annotations);
    assert_eq!(entry.pixel_size, (12, 10));

    commit(
        &paths,
        Commit {
            display: &image,
            base: &base_path(&image),
            rendered_png: &png_bytes(20, 20, 1),
            document: doc(""),
        },
    )
    .unwrap();
    history.mark_annotated(&image).unwrap();
    assert_eq!(history.by_file(&image).unwrap().pixel_size, (20, 20));
    assert_eq!(history.entries().len(), 1);
}
