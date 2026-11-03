use es_app::settings_model::SettingsModel;
use es_desktop::{AppearanceEvent, ColorScheme};
use es_ui::tokens::{Appearance, ThemeMode};
use gpui::{App, Entity, Global, Subscription};
use std::collections::BTreeMap;

/// The desktop's colour scheme and what was last applied, so a settings change redoes only what it touched.
#[derive(Default)]
struct Applied {
    system: Option<Appearance>,
    theme: Option<ThemeMode>,
    shortcuts: Option<BTreeMap<String, String>>,
    editor: Option<es_model::config::Editor>,
}

impl Global for Applied {}

/// Fonts, theme, focus keys and editor keys for every window, kept in step with the settings.
pub(super) fn install(
    settings: &Entity<SettingsModel>,
    desktop: Option<&dyn es_desktop::Appearance>,
    cx: &mut App,
) -> Subscription {
    es_ui::init(cx);
    if let Some(desktop) = desktop {
        es_ui::motion::set_reduce_motion(desktop.reduce_motion(), cx);
    }
    cx.set_global(Applied {
        system: desktop.map(|d| appearance(d.color_scheme())),
        ..Default::default()
    });
    apply(settings, cx);
    cx.observe(settings, |settings, cx| apply(&settings, cx))
}

pub(super) fn desktop_changed(
    event: AppearanceEvent,
    settings: &Entity<SettingsModel>,
    cx: &mut App,
) {
    match event {
        AppearanceEvent::ColorScheme(scheme) => {
            cx.global_mut::<Applied>().system = Some(appearance(scheme));
            apply(settings, cx);
        }
        AppearanceEvent::ReduceMotion(reduce) => es_ui::motion::set_reduce_motion(reduce, cx),
    }
}

fn apply(settings: &Entity<SettingsModel>, cx: &mut App) {
    let config = settings.read(cx).config().clone();
    let applied = cx.global::<Applied>();
    let mode = theme_mode(&config.general.theme, applied.system);
    let retheme = applied.theme != Some(mode);
    let rebind = applied.shortcuts.as_ref() != Some(&config.shortcuts)
        || applied.editor.as_ref() != Some(&config.editor);
    if retheme {
        es_ui::theme::set_mode(mode, cx);
    }
    if rebind {
        es_editor::keyboard::bind(cx, &config.shortcuts, &config.editor);
    }
    let applied = cx.global_mut::<Applied>();
    applied.theme = Some(mode);
    applied.shortcuts = Some(config.shortcuts.clone());
    applied.editor = Some(config.editor.clone());
}

fn theme_mode(setting: &str, system: Option<Appearance>) -> ThemeMode {
    match setting {
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => ThemeMode::System(system),
    }
}

fn appearance(scheme: ColorScheme) -> Appearance {
    match scheme {
        ColorScheme::Light => Appearance::Light,
        ColorScheme::Dark => Appearance::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_editor::keyboard::{CONTEXT, Command, Run};
    use es_store::{ConfigFile, Paths};
    use gpui::{AppContext as _, KeyContext, Keystroke, TestAppContext};

    struct Desktop(ColorScheme);

    impl es_desktop::Appearance for Desktop {
        fn color_scheme(&self) -> ColorScheme {
            self.0
        }
        fn reduce_motion(&self) -> bool {
            false
        }
        fn changes(&self) -> async_channel::Receiver<AppearanceEvent> {
            async_channel::unbounded().1
        }
    }

    fn boot(
        desktop: Option<ColorScheme>,
        cx: &mut TestAppContext,
    ) -> (Entity<SettingsModel>, Subscription, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::rooted(dir.path());
        paths.ensure_all().unwrap();
        let config = ConfigFile::load(&paths.config_file()).unwrap();
        let (settings, look) = cx.update(|cx| {
            let settings = cx.new(|_| SettingsModel::new(config));
            let desktop = desktop.map(Desktop);
            let look = install(
                &settings,
                desktop.as_ref().map(|d| d as &dyn es_desktop::Appearance),
                cx,
            );
            (settings, look)
        });
        (settings, look, dir)
    }

    fn editor_command(key: &'static str, cx: &mut TestAppContext) -> Option<Command> {
        let keystroke = Keystroke::parse(key).ok()?;
        cx.update(|cx| {
            let context = [KeyContext::parse(CONTEXT).unwrap()];
            let (found, _) = cx
                .key_bindings()
                .borrow()
                .bindings_for_input(&[keystroke], &context);
            found
                .first()
                .and_then(|b| b.action().as_any().downcast_ref::<Run>())
                .map(|run| run.0)
        })
    }

    fn edit(
        settings: &Entity<SettingsModel>,
        cx: &mut TestAppContext,
        f: fn(&mut es_model::Config),
    ) {
        cx.update(|cx| settings.update(cx, |s, cx| s.update(f, cx)));
        cx.run_until_parked();
    }

    fn shown(cx: &mut TestAppContext) -> Appearance {
        cx.update(|cx| es_ui::theme::tokens(cx).appearance)
    }

    // A startup that skips the editor keymap leaves every edit key dead in a real editor window.
    #[gpui::test]
    fn startup_binds_the_editor_keys_in_the_editor_context(cx: &mut TestAppContext) {
        let _app = boot(None, cx);
        assert_eq!(editor_command("delete", cx), Some(Command::Delete));
        assert_eq!(editor_command("backspace", cx), Some(Command::Delete));
        assert_eq!(editor_command("ctrl-z", cx), Some(Command::Undo));
        assert_eq!(editor_command("ctrl-shift-z", cx), Some(Command::Redo));
    }

    #[gpui::test]
    fn a_shortcut_changed_in_settings_replaces_the_old_key(cx: &mut TestAppContext) {
        let (settings, _look, _dir) = boot(None, cx);
        let others = |cx: &mut TestAppContext| {
            cx.update(|cx| {
                cx.key_bindings()
                    .borrow()
                    .bindings()
                    .filter(|b| !b.action().as_any().is::<Run>())
                    .count()
            })
        };
        let before = others(cx);
        edit(&settings, cx, |c| {
            c.shortcuts.insert("image-delete".into(), "X".into());
        });
        assert_eq!(editor_command("x", cx), Some(Command::Delete));
        assert_eq!(editor_command("delete", cx), None);
        assert_eq!(editor_command("ctrl-z", cx), Some(Command::Undo));
        assert_eq!(others(cx), before, "rebinding lost the app's other keys");
    }

    #[gpui::test]
    fn the_theme_setting_rethemes_at_once(cx: &mut TestAppContext) {
        let (settings, _look, _dir) = boot(Some(ColorScheme::Dark), cx);
        assert_eq!(shown(cx), Appearance::Dark);
        edit(&settings, cx, |c| c.general.theme = "light".into());
        assert_eq!(shown(cx), Appearance::Light);
        let kit_dark = cx.update(|cx| gpui_component::Theme::global(cx).mode.is_dark());
        assert!(!kit_dark, "components kept the dark theme");
        edit(&settings, cx, |c| c.general.theme = "dark".into());
        assert_eq!(shown(cx), Appearance::Dark);
    }

    #[gpui::test]
    fn follow_system_tracks_the_desktop_and_a_fixed_choice_does_not(cx: &mut TestAppContext) {
        let (settings, _look, _dir) = boot(Some(ColorScheme::Light), cx);
        assert_eq!(shown(cx), Appearance::Light);
        let change = |scheme, cx: &mut TestAppContext| {
            cx.update(|cx| desktop_changed(AppearanceEvent::ColorScheme(scheme), &settings, cx))
        };
        change(ColorScheme::Dark, cx);
        assert_eq!(shown(cx), Appearance::Dark);
        edit(&settings, cx, |c| c.general.theme = "light".into());
        change(ColorScheme::Dark, cx);
        assert_eq!(shown(cx), Appearance::Light);
    }
}
