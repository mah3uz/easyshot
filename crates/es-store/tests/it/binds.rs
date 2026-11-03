#![allow(clippy::unwrap_used)]
use es_store::Paths;

#[test]
fn the_binds_file_lives_in_the_config_dir_and_is_replaced_whole() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::rooted(dir.path());
    let file = es_store::binds::write(&paths, "binds.conf", "one\n").unwrap();
    assert_eq!(file, paths.config_dir().join("binds.conf"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\n");

    es_store::binds::write(&paths, "binds.conf", "two\n").unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "two\n");
    let names: Vec<_> = std::fs::read_dir(paths.config_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        names,
        [std::ffi::OsString::from("binds.conf")],
        "no temp file is left beside it"
    );
}
