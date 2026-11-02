use crate::session::io::Choice;
use crate::toolbar::{FlipAxis, Rotation, ToolRowEvent};
use crate::window::{EditorWindow, Job};
use es_anno::StylePatch;
use es_model::Tool;
use es_protocol::{CATALOGUE, Modifiers, Scope, Trigger};
use gpui::{Action, App, Context, KeyBinding, KeyBindingContextPredicate, KeyDownEvent, Window};
use std::collections::BTreeMap;
use std::rc::Rc;

pub const CONTEXT: &str = "EditorWindow";

const NUDGE: f64 = 1.0;
const NUDGE_BIG: f64 = 10.0;
const TEXT_SIZE_STEP: f64 = 2.0;
const TEXT_SIZE_RANGE: (f64, f64) = (4.0, 300.0);
const STROKE_STEP: f64 = 1.0;
const STROKE_RANGE: (f64, f64) = (1.0, 24.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Tool(Tool),
    Crop,
    Rotate(Rotation),
    Flip(FlipAxis),
    ToggleInspector,
    Save,
    Export,
    Copy,
    Undo,
    Redo,
    SelectAll,
    Delete,
    Deselect,
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    IncreaseSize,
    DecreaseSize,
    Nudge(f64, f64),
    Duplicate,
    CopyShapes,
    PasteShapes,
    Forward,
    Backward,
    Front,
    Back,
    NextShape,
    PrevShape,
    Close,
}

impl Command {
    fn repeats(self) -> bool {
        matches!(
            self,
            Self::Nudge(..)
                | Self::Delete
                | Self::Undo
                | Self::Redo
                | Self::ZoomIn
                | Self::ZoomOut
                | Self::IncreaseSize
                | Self::DecreaseSize
                | Self::NextShape
                | Self::PrevShape
        )
    }
}

#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = easyshot_editor, no_json)]
pub struct Run(pub Command);

/// The command a catalogue id stands for; ids without one (share, background) do nothing here.
pub fn command_for(id: &str) -> Option<Command> {
    Some(match id {
        "image-select" => Command::Tool(Tool::Select),
        "image-crop" => Command::Crop,
        "image-rectangle" => Command::Tool(Tool::Rectangle),
        "image-filled-rectangle" => Command::Tool(Tool::FilledRectangle),
        "image-ellipse" => Command::Tool(Tool::Ellipse),
        "image-line" => Command::Tool(Tool::Line),
        "image-arrow" => Command::Tool(Tool::Arrow),
        "image-freehand" => Command::Tool(Tool::Freehand),
        "image-counter" => Command::Tool(Tool::NumberedCircle),
        "image-text" => Command::Tool(Tool::Text),
        "image-highlight" => Command::Tool(Tool::Highlight),
        "image-pixelate" => Command::Tool(Tool::Pixelate),
        "image-blur" => Command::Tool(Tool::Blur),
        "image-background" => Command::ToggleInspector,
        "image-save" => Command::Save,
        "image-export" => Command::Export,
        "image-copy" => Command::Copy,
        "image-undo" => Command::Undo,
        "image-redo" => Command::Redo,
        "image-select-all" => Command::SelectAll,
        "image-delete" => Command::Delete,
        "image-zoom-in" => Command::ZoomIn,
        "image-zoom-out" => Command::ZoomOut,
        "image-fit" => Command::Fit,
        "image-actual-size" => Command::ActualSize,
        "image-increase-size" => Command::IncreaseSize,
        "image-decrease-size" => Command::DecreaseSize,
        "image-rotate-left" => Command::Rotate(Rotation::Left),
        "image-rotate-right" => Command::Rotate(Rotation::Right),
        "image-flip-horizontal" => Command::Flip(FlipAxis::Horizontal),
        "image-flip-vertical" => Command::Flip(FlipAxis::Vertical),
        _ => return None,
    })
}

/// Keys with no catalogue entry: aliases and the selection helpers.
const EXTRAS: &[(&str, Command)] = &[
    ("Backspace", Command::Delete),
    ("Ctrl+Y", Command::Redo),
    ("Escape", Command::Deselect),
    ("Ctrl+W", Command::Close),
    ("Ctrl+D", Command::Duplicate),
    ("Ctrl+C", Command::CopyShapes),
    ("Ctrl+V", Command::PasteShapes),
    ("Tab", Command::NextShape),
    ("Shift+Tab", Command::PrevShape),
];

fn gpui_key(key: &str) -> Option<String> {
    Some(match key {
        "Equal" => "=".into(),
        "Minus" => "-".into(),
        "Plus" => "+".into(),
        "BracketLeft" => "[".into(),
        "BracketRight" => "]".into(),
        "Return" => "enter".into(),
        "Escape" => "escape".into(),
        "Backspace" => "backspace".into(),
        "Delete" => "delete".into(),
        "Tab" => "tab".into(),
        "Space" => "space".into(),
        "Up" => "up".into(),
        "Down" => "down".into(),
        "Left" => "left".into(),
        "Right" => "right".into(),
        k if k.len() == 1 || k.starts_with('F') => k.to_ascii_lowercase(),
        _ => return None,
    })
}

/// A catalogue trigger as GPUI spells it, for example `ctrl-shift-c`.
pub fn keystroke(trigger: &Trigger) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for (flag, name) in [
        (Modifiers::CTRL, "ctrl"),
        (Modifiers::ALT, "alt"),
        (Modifiers::SHIFT, "shift"),
        (Modifiers::SUPER, "super"),
    ] {
        if trigger.modifiers.contains(flag) {
            parts.push(name.into());
        }
    }
    parts.push(gpui_key(&trigger.key)?);
    Some(parts.join("-"))
}

/// Every key the editor listens to, from the catalogue and the user's overrides.
pub fn bindings(overrides: &BTreeMap<String, String>) -> Vec<(String, Command)> {
    let mut out = Vec::new();
    for entry in CATALOGUE
        .iter()
        .filter(|e| e.scope == Scope::Editor && e.id.starts_with("image-"))
    {
        let Some(command) = command_for(entry.id) else {
            continue;
        };
        let trigger = overrides
            .get(entry.id)
            .map(String::as_str)
            .or(entry.default_trigger);
        if let Some(key) = trigger
            .and_then(|t| Trigger::parse(t).ok())
            .and_then(|t| keystroke(&t))
        {
            out.push((key, command));
        }
    }
    let extras = EXTRAS
        .iter()
        .filter_map(|(t, c)| Some((keystroke(&Trigger::parse(t).ok()?)?, *c)));
    out.extend(extras);
    out.extend([
        ("ctrl-]".to_owned(), Command::Forward),
        ("ctrl-[".to_owned(), Command::Backward),
        ("ctrl-shift-]".to_owned(), Command::Front),
        ("ctrl-shift-[".to_owned(), Command::Back),
    ]);
    for (name, dx, dy) in [
        ("Left", -1.0, 0.0),
        ("Right", 1.0, 0.0),
        ("Up", 0.0, -1.0),
        ("Down", 0.0, 1.0),
    ] {
        out.push((
            name.to_ascii_lowercase(),
            Command::Nudge(dx * NUDGE, dy * NUDGE),
        ));
        out.push((
            format!("shift-{}", name.to_ascii_lowercase()),
            Command::Nudge(dx * NUDGE_BIG, dy * NUDGE_BIG),
        ));
    }
    out
}

/// Replaces the editor's bindings and keeps all others, so it can run again when the overrides change.
pub fn bind(cx: &mut App, overrides: &BTreeMap<String, String>) {
    let context = KeyBindingContextPredicate::parse(CONTEXT).ok().map(Rc::new);
    let editor: Vec<KeyBinding> = bindings(overrides)
        .into_iter()
        .filter_map(|(key, command)| {
            KeyBinding::load(
                &key,
                Box::new(Run(command)),
                context.clone(),
                false,
                None,
                cx,
            )
            .ok()
        })
        .collect();
    let kept: Vec<KeyBinding> = cx
        .key_bindings()
        .borrow()
        .bindings()
        .filter(|b| !b.action().as_any().is::<Run>())
        .cloned()
        .collect();
    cx.clear_key_bindings();
    cx.bind_keys(kept.into_iter().chain(editor));
}

impl EditorWindow {
    pub(crate) fn on_editor_key(&mut self, e: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(session) = self.session() else {
            return;
        };
        let k = &e.keystroke;
        let cancel = k.key == "." && k.modifiers.control;
        let io = session.io.clone();
        if cancel && io.read(cx).prompt_open() {
            io.update(cx, |io, cx| io.choose(Choice::Cancel, cx));
            cx.stop_propagation();
        }
    }

    /// Runs one editor command unless a dialog is open, a key is only repeating, or text is being typed.
    /// Returns `false` when the key must go on to whatever has focus, as it does while text is edited.
    pub fn run_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(session) = self.session() else {
            return false;
        };
        let (doc, viewport, io) = (
            session.doc.clone(),
            session.viewport.clone(),
            session.io.clone(),
        );
        if io.read(cx).prompt_open() {
            if command == Command::Deselect {
                io.update(cx, |io, cx| io.choose(Choice::Cancel, cx));
            }
            return true;
        }
        let editing = self
            .canvas()
            .is_some_and(|c| c.read(cx).is_editing_text(cx));
        if editing && command != Command::Save {
            return false;
        }
        let repeat = self.held.replace(command) == Some(command);
        if repeat && !command.repeats() {
            return true;
        }
        let row = self.toolbar();
        let emit = |event: ToolRowEvent, cx: &mut Context<Self>| {
            if let Some(row) = &row {
                row.update(cx, |_, cx| cx.emit(event));
            }
        };
        match command {
            Command::Tool(tool) => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.select_tool(tool)));
            }
            Command::Crop => emit(ToolRowEvent::Crop, cx),
            Command::Rotate(r) => emit(ToolRowEvent::Rotate(r), cx),
            Command::Flip(f) => emit(ToolRowEvent::Flip(f), cx),
            Command::ToggleInspector => emit(ToolRowEvent::ToggleInspector, cx),
            Command::Save => {
                self.request(Job::Save, cx);
            }
            Command::Export => {
                self.request(Job::Export, cx);
            }
            Command::Copy => {
                self.request(Job::Copy, cx);
            }
            Command::Undo => self.undo(cx),
            Command::Redo => self.redo(cx),
            Command::SelectAll => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.select_all()));
            }
            Command::Delete => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.delete_selected()));
            }
            Command::Deselect => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.select([])));
            }
            Command::ZoomIn => viewport.update(cx, |v, cx| v.zoom_in(cx)),
            Command::ZoomOut => viewport.update(cx, |v, cx| v.zoom_out(cx)),
            Command::Fit => viewport.update(cx, |v, cx| v.fit(cx)),
            Command::ActualSize => viewport.update(cx, |v, cx| v.set_percent(100.0, cx)),
            Command::IncreaseSize => self.step_size(1.0, cx),
            Command::DecreaseSize => self.step_size(-1.0, cx),
            Command::Nudge(dx, dy) => {
                let gesture = io.update(cx, |io, _| io.next_gesture());
                doc.update(cx, |d, cx| {
                    d.apply(cx, |e| e.nudge_selected(dx, dy, gesture))
                });
            }
            Command::Duplicate => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.duplicate_selected()));
            }
            Command::CopyShapes => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.copy_shapes()));
            }
            Command::PasteShapes => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.paste_shapes(None)));
            }
            Command::Forward => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.bring_forward()));
            }
            Command::Backward => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.send_backward()));
            }
            Command::Front => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.bring_to_front()));
            }
            Command::Back => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.send_to_back()));
            }
            Command::NextShape => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.select_next()));
            }
            Command::PrevShape => {
                doc.update(cx, |d, cx| d.apply(cx, |e| e.select_prev()));
            }
            Command::Close => {
                if self.should_close(window, cx) {
                    window.remove_window();
                }
            }
        }
        true
    }

    fn step_size(&mut self, direction: f64, cx: &mut Context<Self>) {
        let Some(session) = self.session() else {
            return;
        };
        let (doc, io) = (session.doc.clone(), session.io.clone());
        let (tool, style) = {
            let e = doc.read(cx).editor();
            (e.tool(), e.current_style().clone())
        };
        let patch = if tool == Tool::Text {
            let (lo, hi) = TEXT_SIZE_RANGE;
            StylePatch::FontSize((style.font_size + direction * TEXT_SIZE_STEP).clamp(lo, hi))
        } else {
            let (lo, hi) = STROKE_RANGE;
            StylePatch::StrokeWidth((style.stroke_width + direction * STROKE_STEP).clamp(lo, hi))
        };
        let gesture = io.update(cx, |io, _| io.next_gesture());
        doc.update(cx, |d, cx| d.apply(cx, |e| e.apply_style(patch, gesture)));
    }
}
