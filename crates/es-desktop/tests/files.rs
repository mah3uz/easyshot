mod dbus_support;
mod desktop_support;

use dbus_support::run;
use desktop_support::{QuietBus, isolated, serve_on};
use es_desktop::{autostart_file, move_to_trash, set_autostart_in, show_items, spawn_reaped};
use std::path::Path;

struct FileManager {
    tx: async_channel::Sender<(Vec<String>, String)>,
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager {
    async fn show_items(&self, uris: Vec<String>, startup_id: String) {
        let _ = self.tx.send((uris, startup_id)).await;
    }
}

#[test]
fn show_items_sends_an_encoded_file_uri_to_the_file_manager() {
    let bus = QuietBus::start();
    let (tx, rx) = async_channel::unbounded();
    let _server = run(serve_on(
        &bus.address,
        "org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        FileManager { tx },
    ));
    run(show_items(
        &bus.endpoint(),
        Path::new("/home/a b/Shots/100%.png"),
    ))
    .expect("show items");
    let (uris, _) = run(rx.recv()).expect("call");
    assert_eq!(uris, ["file:///home/a%20b/Shots/100%25.png"]);
}

#[test]
fn show_items_fails_when_no_file_manager_owns_the_name() {
    let bus = QuietBus::start();
    assert!(run(show_items(&bus.endpoint(), Path::new("/tmp/x.png"))).is_err());
}

#[test]
fn trash_moves_the_file_out_of_its_folder_into_the_trash() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("trash-test");
    let data = root.join("data");
    let data_arg = data.to_str().expect("utf-8 path").to_owned();
    let file = root.join("shot-to-trash.png");
    isolated(
        "trash_moves_the_file_out_of_its_folder_into_the_trash",
        &[("XDG_DATA_HOME", &data_arg)],
        || {
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&data).expect("data dir");
            std::fs::write(&file, b"png").expect("write");
            move_to_trash(&file).expect("trash");
            assert!(!file.exists());
            assert!(data.join("Trash/files/shot-to-trash.png").exists());
        },
    );
}

#[test]
fn trashing_a_missing_file_is_not_found() {
    let missing = Path::new(env!("CARGO_TARGET_TMPDIR")).join("never-existed.png");
    let result = move_to_trash(&missing);
    assert!(
        matches!(result, Err(es_desktop::Error::NotFound(_))),
        "{result:?}"
    );
}

#[test]
fn autostart_writes_the_desktop_entry_and_removes_it() {
    let home = tempfile::tempdir().expect("temp dir");
    set_autostart_in(home.path(), true, "easyshot daemon").expect("enable");
    let entry = std::fs::read_to_string(autostart_file(home.path())).expect("entry");
    assert!(entry.contains("Exec=easyshot daemon\n"));
    assert!(entry.starts_with("[Desktop Entry]\n"));

    set_autostart_in(home.path(), false, "easyshot daemon").expect("disable");
    assert!(!autostart_file(home.path()).exists());
    set_autostart_in(home.path(), false, "easyshot daemon").expect("disabling twice is fine");
}

#[test]
fn a_detached_program_is_reaped_when_it_exits() {
    let pid = spawn_reaped(&mut std::process::Command::new("true")).expect("spawn");
    let proc_dir = format!("/proc/{pid}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while Path::new(&proc_dir).exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the child is still a zombie"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
