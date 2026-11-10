#![allow(clippy::unwrap_used)]
#![cfg(all(feature = "hyprland", feature = "niri", feature = "wlroots"))]

use es_backend::hyprland::{HyprBinds, Ipc};
use es_backend::{BindHelper, BindSyntax, Env, Error, Probe, Trigger, niri, wlroots};
use es_protocol::Route;
use std::path::{Path, PathBuf};

fn binds() -> Vec<(Trigger, Route)> {
    vec![
        (Trigger("SUPER+SHIFT+S".into()), Route::CaptureRegion),
        (Trigger("Print".into()), Route::CaptureFullscreen),
    ]
}

struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Home {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn config_dir(&self) -> PathBuf {
        self.dir.path().join(".config/easyshot")
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(".config").join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn env(&self) -> Env {
        Env::from_vars([
            ("HOME", self.dir.path().to_str().unwrap()),
            (
                "XDG_CONFIG_HOME",
                self.dir.path().join(".config").to_str().unwrap(),
            ),
            (wlroots::SOCKET_ENV, "/nonexistent/sway.sock"),
        ])
    }

    fn hypr(&self, syntax: BindSyntax) -> HyprBinds {
        let file = match syntax {
            BindSyntax::Hyprlang => "hypr/hyprland.conf",
            _ => "hypr/hyprland.lua",
        };
        HyprBinds {
            ipc: Ipc::new("/nonexistent"),
            syntax,
            config: Some(self.dir.path().join(".config").join(file)),
            home: Some(self.dir.path().to_path_buf()),
        }
    }

    fn sway(&self) -> Box<dyn BindHelper> {
        let probe = Probe {
            env: self.env(),
            ..Probe::default()
        };
        (wlroots::BINDS.build)(&probe).ok().unwrap()
    }

    fn niri(&self) -> Box<dyn BindHelper> {
        let probe = Probe {
            env: self.env(),
            ..Probe::default()
        };
        (niri::BINDS.build)(&probe).ok().unwrap()
    }
}

#[test]
fn hyprland_lua_file_is_lua_calls_and_the_line_is_a_guarded_require() {
    let helper = Home::new().hypr(BindSyntax::HyprLua);
    assert_eq!(helper.file_name(), "binds.lua");
    assert_eq!(
        helper.file_text(&binds()),
        "-- easyshot rewrites this file when you change its shortcuts. Edit them in easyshot.\n\
         hl.bind(\"SUPER + SHIFT + S\", hl.dsp.exec_cmd(\"easyshot capture region\"))\n\
         hl.bind(\"Print\", hl.dsp.exec_cmd(\"easyshot capture fullscreen\"))\n\
         hl.window_rule({ name = \"easyshot-preview\", match = { class = \"^dev.easyshot.app.preview$\" }, float = true, center = true })\n"
    );
    assert_eq!(
        helper.include_line("~/.config/easyshot"),
        "pcall(require, \"~/.config/easyshot/binds\")"
    );
}

#[test]
fn hyprlang_file_is_bind_lines_and_the_line_is_a_source() {
    let helper = Home::new().hypr(BindSyntax::Hyprlang);
    assert_eq!(helper.file_name(), "binds.conf");
    assert_eq!(
        helper.file_text(&binds()),
        "# easyshot rewrites this file when you change its shortcuts. Edit them in easyshot.\n\
         bind = SUPER SHIFT, S, exec, easyshot capture region\n\
         bind = , Print, exec, easyshot capture fullscreen\n\
         windowrule {\n\
         \x20   name = easyshot-preview\n\
         \x20   match:class = ^dev.easyshot.app.preview$\n\
         \x20   float = yes\n\
         \x20   center = yes\n\
         }\n"
    );
    assert_eq!(
        helper.include_line("~/.config/easyshot"),
        "source = ~/.config/easyshot/binds.conf"
    );
}

#[test]
fn sway_file_is_bindsym_lines_and_the_line_is_an_include() {
    let helper = Home::new().sway();
    assert_eq!(helper.file_name(), "binds.conf");
    assert_eq!(
        helper.file_text(&binds()),
        "# easyshot rewrites this file when you change its shortcuts. Edit them in easyshot.\n\
         bindsym SUPER+SHIFT+S exec easyshot capture region\n\
         bindsym Print exec easyshot capture fullscreen\n"
    );
    assert_eq!(
        helper.include_line("~/.config/easyshot"),
        "include ~/.config/easyshot/binds.conf"
    );
    assert!(helper.needs_reload(), "sway only reads the file on reload");
}

#[test]
fn niri_file_is_one_binds_block_and_the_line_is_an_optional_include() {
    let helper = Home::new().niri();
    assert_eq!(helper.file_name(), "binds.kdl");
    assert_eq!(
        helper.file_text(&binds()),
        "// easyshot rewrites this file when you change its shortcuts. Edit them in easyshot.\n\
         binds {\n\
         \x20   SUPER+SHIFT+S { spawn \"easyshot\" \"capture\" \"region\"; }\n\
         \x20   Print { spawn \"easyshot\" \"capture\" \"fullscreen\"; }\n\
         }\n"
    );
    assert_eq!(
        helper.include_line("~/.config/easyshot"),
        "include optional=true \"~/.config/easyshot/binds.kdl\""
    );
    assert!(!helper.needs_reload(), "niri watches included files");
}

#[test]
fn an_empty_list_still_gives_a_valid_file() {
    let home = Home::new();
    assert!(home.niri().file_text(&[]).contains("binds {\n}"));
    assert!(!home.sway().file_text(&[]).contains("bindsym"));
}

fn included_everywhere(home: &Home, dir: &Path) -> [(&'static str, Result<bool, Error>); 4] {
    [
        ("lua", home.hypr(BindSyntax::HyprLua).is_included(dir)),
        ("hyprlang", home.hypr(BindSyntax::Hyprlang).is_included(dir)),
        ("sway", home.sway().is_included(dir)),
        ("niri", home.niri().is_included(dir)),
    ]
}

#[test]
fn the_include_is_found_in_the_main_config() {
    let home = Home::new();
    home.write(
        "hypr/hyprland.lua",
        "pcall(require, \"~/.config/easyshot/binds\")\n",
    );
    home.write(
        "hypr/hyprland.conf",
        "source = ~/.config/easyshot/binds.conf\n",
    );
    home.write(
        "sway/config",
        "set $mod Mod4\ninclude ~/.config/easyshot/binds.conf\n",
    );
    home.write(
        "niri/config.kdl",
        "include optional=true \"~/.config/easyshot/binds.kdl\"\n",
    );
    for (name, found) in included_everywhere(&home, &home.config_dir()) {
        assert!(found.unwrap(), "{name}");
    }
}

#[test]
fn a_config_without_the_line_or_with_it_commented_out_is_not_included() {
    let home = Home::new();
    home.write(
        "hypr/hyprland.lua",
        "-- pcall(require, \"~/.config/easyshot/binds\")\nrequire(\"other\")\n",
    );
    home.write(
        "hypr/hyprland.conf",
        "# source = ~/.config/easyshot/binds.conf\n",
    );
    home.write(
        "sway/config",
        "# include ~/.config/easyshot/binds.conf\nset $mod Mod4\n",
    );
    home.write(
        "niri/config.kdl",
        "// include optional=true \"~/.config/easyshot/binds.kdl\"\nbinds { }\n",
    );
    for (name, found) in included_everywhere(&home, &home.config_dir()) {
        assert!(!found.unwrap(), "{name}");
    }
}

#[test]
fn the_include_is_found_one_level_down_in_a_sourced_file() {
    let home = Home::new();
    home.write("hypr/hyprland.lua", "require(\"keys\")\n");
    home.write(
        "hypr/keys.lua",
        "pcall(require, \"~/.config/easyshot/binds\")\n",
    );
    home.write("hypr/hyprland.conf", "source = ~/.config/hypr/keys.conf\n");
    home.write("hypr/keys.conf", "source = ~/.config/easyshot/binds.conf\n");
    home.write("sway/config", "include ~/.config/sway/config.d/keys\n");
    home.write(
        "sway/config.d/keys",
        "include ~/.config/easyshot/binds.conf\n",
    );
    home.write("niri/config.kdl", "include \"keys.kdl\"\n");
    home.write(
        "niri/keys.kdl",
        "include optional=true \"~/.config/easyshot/binds.kdl\"\n",
    );
    for (name, found) in included_everywhere(&home, &home.config_dir()) {
        assert!(found.unwrap(), "{name}");
    }
}

#[test]
fn an_include_two_levels_down_is_not_followed() {
    let home = Home::new();
    home.write("niri/config.kdl", "include \"a.kdl\"\n");
    home.write("niri/a.kdl", "include \"b.kdl\"\n");
    home.write("niri/b.kdl", "include \"~/.config/easyshot/binds.kdl\"\n");
    assert!(!home.niri().is_included(&home.config_dir()).unwrap());
}

#[test]
fn a_missing_main_config_is_not_found_rather_than_not_included() {
    let home = Home::new();
    for (name, found) in included_everywhere(&home, &home.config_dir()) {
        assert!(matches!(found, Err(Error::NotFound(_))), "{name}");
    }
}

fn round_trip_binds() -> Vec<(Trigger, Route)> {
    vec![
        (
            Trigger("SUPER+CTRL+ALT+SHIFT+S".into()),
            Route::CaptureRegion,
        ),
        (Trigger("SUPER+SHIFT+4".into()), Route::CaptureFullscreen),
        (Trigger("Print".into()), Route::CaptureWindow),
        (Trigger("CTRL+ALT+apostrophe".into()), Route::CaptureRegion),
    ]
}

#[test]
fn the_binds_file_reads_back_as_the_binds_it_was_written_from() {
    let home = Home::new();
    for syntax in [BindSyntax::HyprLua, BindSyntax::Hyprlang] {
        let helper = home.hypr(syntax);
        let written = round_trip_binds();
        let read = helper.file_binds(&helper.file_text(&written));
        let expected: Vec<(Trigger, String)> = written
            .iter()
            .map(|(t, r)| {
                (
                    t.clone(),
                    format!("easyshot {}", r.command_args().join(" ")),
                )
            })
            .collect();
        assert_eq!(read, expected, "{syntax:?}");
    }
}

#[test]
fn an_empty_binds_file_reads_back_as_no_binds() {
    let home = Home::new();
    for syntax in [BindSyntax::HyprLua, BindSyntax::Hyprlang] {
        let helper = home.hypr(syntax);
        assert!(helper.file_binds(&helper.file_text(&[])).is_empty());
    }
}
