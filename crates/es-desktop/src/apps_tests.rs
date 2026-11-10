use super::*;

fn dir_with(file: &str, text: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    put(dir.path(), &format!("applications/{file}"), text);
    dir
}

fn put(root: &Path, file: &str, text: &str) -> PathBuf {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, text).expect("file");
    path
}

fn themes(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| (*n).to_owned()).collect()
}

#[test]
fn the_name_comes_from_the_desktop_entry_section_only() {
    let dir = dir_with(
        "org.mozilla.firefox.desktop",
        "[Desktop Action new-window]\nName=New Window\n[Desktop Entry]\nName[de]=Feuerfuchs\nName=Firefox\n",
    );
    let dirs = [dir.path().to_path_buf()];
    assert_eq!(
        app_name_in(&dirs, "org.mozilla.firefox").as_deref(),
        Some("Firefox")
    );
    assert_eq!(app_name_in(&dirs, "org.example.missing"), None);
}

#[test]
fn an_app_id_never_reaches_outside_the_applications_folder() {
    let dir = dir_with("x.desktop", "[Desktop Entry]\nName=X\n");
    let deeper = dir.path().join("deeper");
    std::fs::create_dir_all(deeper.join("applications")).expect("nested dir");
    let dirs = [deeper];
    assert_eq!(app_name_in(&dirs, "../../applications/x"), None);
}

#[test]
fn an_app_whose_id_is_not_its_file_name_is_found_by_its_window_class() {
    let dir = dir_with(
        "visual-studio-code.desktop",
        "[Desktop Entry]\nName=Visual Studio Code\nIcon=vscode\nStartupWMClass=Code\n",
    );
    let dirs = [dir.path().to_path_buf()];
    assert_eq!(
        app_name_in(&dirs, "code").as_deref(),
        Some("Visual Studio Code"),
        "the class matches without regard to case"
    );
}

#[test]
fn a_reverse_domain_id_without_an_entry_reads_as_a_cleaned_up_name() {
    for (id, name) in [
        ("com.microsoft.vscode-sdjfhsk", "Vscode"),
        ("com.example.Slack-x8f2k1", "Slack"),
        (
            "chrome-fmgjjmmmlfnkbppncabfkddbjimcfncm-Default",
            "Chrome Default",
        ),
        ("org.mozilla.firefox", "Firefox"),
        ("google-chrome", "Google Chrome"),
        ("foot", "Foot"),
        ("", ""),
    ] {
        assert_eq!(fallback_name(id), name, "{id}");
    }
    let label = app_label_in(&[], &themes(&["hicolor"]), "com.microsoft.vscode-sdjfhsk");
    assert_eq!(
        label,
        AppLabel {
            name: "Vscode".into(),
            icon: None
        }
    );
}

#[test]
fn the_icon_is_the_scalable_one_then_the_nearest_size_above_32() {
    let dir = dir_with("app.desktop", "[Desktop Entry]\nName=App\nIcon=app\n");
    let root = dir.path();
    for size in ["16x16", "32x32", "48x48", "256x256"] {
        put(root, &format!("icons/hicolor/{size}/apps/app.png"), "png");
    }
    let dirs = [root.to_path_buf()];
    let icon = |dirs: &[PathBuf]| app_label_in(dirs, &themes(&["hicolor"]), "app").icon;
    assert_eq!(
        icon(&dirs),
        Some(root.join("icons/hicolor/48x48/apps/app.png")),
        "48 stays sharp on a 2x screen, where 32 would be enlarged"
    );

    let svg = put(root, "icons/hicolor/scalable/apps/app.svg", "<svg/>");
    assert_eq!(icon(&dirs), Some(svg));

    let small = tempfile::tempdir().expect("temp dir");
    put(
        small.path(),
        "applications/app.desktop",
        "[Desktop Entry]\nIcon=app\n",
    );
    for size in ["16x16", "24x24"] {
        put(
            small.path(),
            &format!("icons/hicolor/{size}/apps/app.png"),
            "png",
        );
    }
    assert_eq!(
        icon(&[small.path().to_path_buf()]),
        Some(small.path().join("icons/hicolor/24x24/apps/app.png"))
    );
}

#[test]
fn the_users_theme_wins_then_hicolor_then_pixmaps() {
    let dir = dir_with("app.desktop", "[Desktop Entry]\nName=App\nIcon=app\n");
    let root = dir.path();
    let dirs = [root.to_path_buf()];
    let icon = || app_label_in(&dirs, &themes(&["Papirus", "hicolor"]), "app").icon;

    let pixmap = put(root, "pixmaps/app.png", "png");
    assert_eq!(icon(), Some(pixmap));
    let hicolor = put(root, "icons/hicolor/48x48/apps/app.png", "png");
    assert_eq!(icon(), Some(hicolor));
    let user = put(root, "icons/Papirus/64x64/apps/app.svg", "<svg/>");
    assert_eq!(icon(), Some(user));
}

#[test]
fn an_absolute_icon_is_used_as_is_and_a_name_never_leaves_the_icon_folders() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let png = put(root, "elsewhere/logo.png", "png");
    let xpm = put(root, "elsewhere/old.xpm", "xpm");
    put(root, "secret.png", "png");
    put(
        root,
        "applications/a.desktop",
        &format!("[Desktop Entry]\nIcon={}\n", png.display()),
    );
    put(
        root,
        "applications/b.desktop",
        &format!("[Desktop Entry]\nIcon={}\n", xpm.display()),
    );
    put(
        root,
        "applications/c.desktop",
        "[Desktop Entry]\nIcon=../../secret\n",
    );
    put(
        root,
        "applications/d.desktop",
        "[Desktop Entry]\nIcon=.hidden\n",
    );
    put(root, "icons/hicolor/48x48/apps/.hidden.png", "png");
    let dirs = [root.to_path_buf()];
    let icon = |id| app_label_in(&dirs, &themes(&["hicolor"]), id).icon;
    assert_eq!(icon("a"), Some(png));
    assert_eq!(icon("b"), None, "the toolkit cannot draw XPM");
    assert_eq!(icon("c"), None);
    assert_eq!(icon("d"), None);
    assert_eq!(icon("missing"), None);
}
