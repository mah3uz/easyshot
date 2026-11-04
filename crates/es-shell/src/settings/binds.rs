use es_backend::{Backend, BindHelper, BindSyntax, Trigger as CompositorTrigger};
use es_protocol::{Modifiers, Route, Trigger};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindStatus {
    Bound,
    /// Our trigger runs something else; the payload is that command.
    Conflict(String),
}

/// The backend owns its helper; this lends it out to work that outlives a borrow.
struct BackendBinds(Arc<Backend>);

impl BindHelper for BackendBinds {
    fn syntax(&self) -> BindSyntax {
        self.0
            .binds
            .as_ref()
            .map_or(BindSyntax::Sway, |b| b.syntax())
    }
    fn snippet(&self, binds: &[(CompositorTrigger, Route)]) -> String {
        self.0
            .binds
            .as_ref()
            .map(|b| b.snippet(binds))
            .unwrap_or_default()
    }
    fn existing_binds(&self) -> es_backend::Result<Vec<(CompositorTrigger, String)>> {
        match &self.0.binds {
            Some(b) => b.existing_binds(),
            None => Ok(Vec::new()),
        }
    }
    fn file_name(&self) -> &'static str {
        self.0.binds.as_ref().map_or("", |b| b.file_name())
    }
    fn file_text(&self, binds: &[(CompositorTrigger, Route)]) -> String {
        self.0
            .binds
            .as_ref()
            .map(|b| b.file_text(binds))
            .unwrap_or_default()
    }
    fn include_line(&self, dir: &str) -> String {
        self.0
            .binds
            .as_ref()
            .map(|b| b.include_line(dir))
            .unwrap_or_default()
    }
    fn is_included(&self, dir: &Path) -> es_backend::Result<bool> {
        match &self.0.binds {
            Some(b) => b.is_included(dir),
            None => Ok(false),
        }
    }
    fn needs_reload(&self) -> bool {
        self.0.binds.as_ref().is_some_and(|b| b.needs_reload())
    }
    fn reload(&self) -> es_backend::Result<()> {
        match &self.0.binds {
            Some(b) => b.reload(),
            None => Err(es_backend::Error::Unsupported),
        }
    }
}

pub fn from_backend(backend: Arc<Backend>) -> Option<Arc<dyn BindHelper>> {
    backend.binds.as_ref()?;
    Some(Arc::new(BackendBinds(backend)))
}

/// The form `Trigger::parse` reads back, used for `[shortcuts]` values.
pub fn trigger_string(trigger: &Trigger) -> String {
    let mut out = String::new();
    for (flag, name) in [
        (Modifiers::SUPER, "Super"),
        (Modifiers::CTRL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
    ] {
        if trigger.modifiers.contains(flag) {
            out.push_str(name);
            out.push('+');
        }
    }
    out + &trigger.key
}

fn keysym(key: &str, syntax: BindSyntax) -> String {
    let named = match key {
        "Minus" => "minus",
        "Equal" => "equal",
        "Plus" => "plus",
        "Space" => "space",
        "Comma" => "comma",
        "Period" => "period",
        "Slash" => "slash",
        "Backslash" => "backslash",
        "Semicolon" => "semicolon",
        "Quote" => "apostrophe",
        "Grave" => "grave",
        "LeftBracket" => "bracketleft",
        "RightBracket" => "bracketright",
        "PrintScreen" => "Print",
        "Backspace" => "BackSpace",
        "PageUp" => "Prior",
        "PageDown" => "Next",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_owned();
    }
    if key.chars().count() == 1 && syntax == BindSyntax::Sway {
        return key.to_lowercase();
    }
    key.to_owned()
}

pub fn compositor_trigger(trigger: &Trigger, syntax: BindSyntax) -> CompositorTrigger {
    let names: [&str; 4] = match syntax {
        BindSyntax::HyprLua | BindSyntax::Hyprlang => ["SUPER", "CTRL", "ALT", "SHIFT"],
        BindSyntax::Sway => ["Mod4", "Ctrl", "Mod1", "Shift"],
        BindSyntax::NiriKdl => ["Mod", "Ctrl", "Alt", "Shift"],
    };
    let mut parts: Vec<String> = [
        Modifiers::SUPER,
        Modifiers::CTRL,
        Modifiers::ALT,
        Modifiers::SHIFT,
    ]
    .iter()
    .zip(names)
    .filter(|(flag, _)| trigger.modifiers.contains(**flag))
    .map(|(_, name)| name.to_owned())
    .collect();
    parts.push(keysym(&trigger.key, syntax));
    CompositorTrigger(parts.join("+"))
}

/// Modifier set and lower-case key, so `$mod+Shift+4`, `SUPER SHIFT, 4` and `Mod+Shift+4` compare equal.
pub(crate) fn normalise(trigger: &str) -> (Modifiers, String) {
    let mut words: Vec<String> = trigger
        .split(|c: char| c == '+' || c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let key = words.pop().unwrap_or_default();
    let mut modifiers = Modifiers::empty();
    for word in words {
        modifiers |= match word.as_str() {
            "super" | "mod" | "$mod" | "mod4" | "logo" | "win" => Modifiers::SUPER,
            "ctrl" | "control" => Modifiers::CTRL,
            "alt" | "mod1" => Modifiers::ALT,
            "shift" => Modifiers::SHIFT,
            _ => Modifiers::empty(),
        };
    }
    (modifiers, key)
}

pub(crate) fn runs(command: &str, route: &Route) -> bool {
    let words: Vec<&str> = command
        .split(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == ';')
        .filter(|w| !w.is_empty())
        .collect();
    let Some(start) = words.iter().position(|w| w.ends_with("easyshot")) else {
        return false;
    };
    words[start + 1..] == route.command_args()[..]
}

/// A Lua config reports a bind's action as a handle, not the command text.
fn opaque(command: &str) -> bool {
    command.starts_with("HL.Dispatcher(")
}

/// What `existing` says about one action: run by some bind, or its trigger taken by another command.
/// With our binds file included, an opaque bind on our own trigger is ours.
pub fn status(
    existing: &[(CompositorTrigger, String)],
    route: &Route,
    ours: Option<&CompositorTrigger>,
    included: bool,
) -> Option<BindStatus> {
    if existing.iter().any(|(_, command)| runs(command, route)) {
        return Some(BindStatus::Bound);
    }
    let ours = normalise(&ours?.0);
    let (_, command) = existing
        .iter()
        .find(|(trigger, _)| normalise(&trigger.0) == ours)?;
    if included && opaque(command) {
        return Some(BindStatus::Bound);
    }
    Some(BindStatus::Conflict(command.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region() -> Route {
        Route::all()
            .into_iter()
            .find(|r| r.command_args() == ["capture", "region"])
            .expect("region route")
    }

    fn lua() -> Vec<(CompositorTrigger, String)> {
        let json = br#"[{"locked": false, "mouse": false, "release": false, "repeat": false, "longPress": false, "non_consuming": false, "auto_consuming": false, "has_description": false, "modmask": 65, "submap": "", "submap_universal": "false", "key": "S", "keycode": 0, "catch_all": false, "description": "", "allow_input_capture": false, "dispatcher": "HL.Dispatcher(exec_cmd)", "arg": "85"}]"#;
        es_backend::hyprland::parse_binds(json).expect("binds")
    }

    fn conf(command: &str) -> Vec<(CompositorTrigger, String)> {
        let json = format!(
            r#"[{{"modmask": 65, "submap": "", "key": "S", "keycode": 0, "dispatcher": "exec", "arg": "{command}"}}]"#
        );
        es_backend::hyprland::parse_binds(json.as_bytes()).expect("binds")
    }

    fn ours() -> CompositorTrigger {
        CompositorTrigger("SUPER+SHIFT+S".into())
    }

    #[test]
    fn own_lua_bind_is_bound_when_included() {
        let got = status(&lua(), &region(), Some(&ours()), true);
        assert_eq!(got, Some(BindStatus::Bound));
    }

    #[test]
    fn own_lua_trigger_without_the_include_is_not_bound() {
        let got = status(&lua(), &region(), Some(&ours()), false);
        assert!(matches!(got, Some(BindStatus::Conflict(_))));
    }

    #[test]
    fn lua_bind_on_another_trigger_is_not_ours() {
        let other = CompositorTrigger("SUPER+SHIFT+D".into());
        assert_eq!(status(&lua(), &region(), Some(&other), true), None);
    }

    #[test]
    fn classic_conf_with_our_command_is_bound() {
        let got = status(&conf("easyshot capture region"), &region(), None, false);
        assert_eq!(got, Some(BindStatus::Bound));
    }

    #[test]
    fn classic_conf_foreign_command_on_our_trigger_conflicts() {
        let got = status(&conf("grim"), &region(), Some(&ours()), true);
        assert_eq!(got, Some(BindStatus::Conflict("exec grim".into())));
    }
}
