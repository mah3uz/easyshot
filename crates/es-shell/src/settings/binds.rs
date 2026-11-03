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

/// What `existing` says about one action: run by some bind, or its trigger taken by another command.
pub fn status(
    existing: &[(CompositorTrigger, String)],
    route: &Route,
    ours: Option<&CompositorTrigger>,
) -> Option<BindStatus> {
    if existing.iter().any(|(_, command)| runs(command, route)) {
        return Some(BindStatus::Bound);
    }
    let ours = normalise(&ours?.0);
    existing
        .iter()
        .find(|(trigger, _)| normalise(&trigger.0) == ours)
        .map(|(_, command)| BindStatus::Conflict(command.clone()))
}
