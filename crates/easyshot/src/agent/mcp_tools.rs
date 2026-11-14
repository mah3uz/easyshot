use es_protocol::agent::{
    AUDIO, DEFAULT_MAX_SECONDS, FORMATS, FPS, LOOKS, MAX_BRIGHTNESS_MILLI, MAX_DELAY_SECONDS,
    MAX_SECONDS, QUALITIES, SIZES,
};
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde_json::{Value, json};

pub(super) const CAPTURE: &str = "capture";
pub(super) const ANNOTATE: &str = "annotate";

pub(super) struct ToolDef {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub read_only: bool,
    pub destructive: bool,
    pub schema: fn() -> Value,
    pub argv: fn(&JsonObject) -> Result<Vec<String>, String>,
}

pub(super) fn find(name: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|t| t.name == name)
}

pub(super) fn tools() -> Vec<Tool> {
    TOOLS.iter().map(tool).collect()
}

fn tool(def: &ToolDef) -> Tool {
    let schema = match (def.schema)() {
        Value::Object(map) => map,
        _ => JsonObject::new(),
    };
    Tool::new(def.name, def.description, schema)
        .with_title(def.title)
        .with_annotations(
            ToolAnnotations::new()
                .read_only(def.read_only)
                .destructive(def.destructive)
                .idempotent(def.read_only)
                .open_world(false),
        )
}

const TARGET_HELP: &str = "Give exactly one of output, window, geometry or fullscreen.";

static TOOLS: [ToolDef; 8] = [
    ToolDef {
        name: "list_outputs",
        title: "List outputs",
        description: "List the monitors: name, position, size and scale.",
        read_only: true,
        destructive: false,
        schema: empty_schema,
        argv: |_| Ok(words(&["list", "outputs"])),
    },
    ToolDef {
        name: "list_windows",
        title: "List windows",
        description: "List the open windows on every workspace: id, app id, title, geometry, \
                      output and whether a screen shows it.",
        read_only: true,
        destructive: false,
        schema: empty_schema,
        argv: |_| Ok(words(&["list", "windows"])),
    },
    ToolDef {
        name: CAPTURE,
        title: "Capture a screenshot",
        description: "Take a screenshot of one target. Secrets found on screen are hidden before \
                      the image is returned. The reply carries a downscaled preview image and the \
                      saved file's path and full size.",
        read_only: false,
        destructive: false,
        schema: capture_schema,
        argv: capture_argv,
    },
    ToolDef {
        name: "ocr",
        title: "Read text and QR codes",
        description: "Read the text and QR codes on a target, or in a PNG or JPEG file given as \
                      file. Returns the text and the lines with their positions.",
        read_only: false,
        destructive: false,
        schema: ocr_schema,
        argv: ocr_argv,
    },
    ToolDef {
        name: "record_start",
        title: "Start a screen recording",
        description: "Start recording one target and return the recording's id. It stops by \
                      itself after max_seconds. Call record_stop to get the video.",
        read_only: false,
        destructive: false,
        schema: record_start_schema,
        argv: record_start_argv,
    },
    ToolDef {
        name: "record_stop",
        title: "Stop a screen recording",
        description: "Stop a recording and return where the video was saved.",
        read_only: false,
        destructive: false,
        schema: record_stop_schema,
        argv: record_stop_argv,
    },
    ToolDef {
        name: "export",
        title: "Export a recording",
        description: "Re-encode a recording as MP4, WebM or GIF, optionally trimmed, scaled or \
                      colour-graded. Returns the new file's path.",
        read_only: false,
        destructive: true,
        schema: export_schema,
        argv: export_argv,
    },
    ToolDef {
        name: ANNOTATE,
        title: "Annotate an image",
        description: "Draw arrows, boxes, text, numbered badges, highlights, blur and pixelation \
                      on a PNG or JPEG file and write the result as a new PNG, optionally on a \
                      background. It reads and writes only the files named, needs no access to \
                      the screen, and never replaces a file unless force is set. The reply \
                      carries a downscaled preview image and the file's path and size. \
                      Coordinates are pixels of the image, from its top left corner.",
        read_only: false,
        destructive: false,
        schema: annotate_schema,
        argv: |_| Err("annotate does not run as a command line".to_owned()),
    },
];

fn words(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| (*s).to_owned()).collect()
}

fn empty_schema() -> Value {
    json!({"type": "object", "properties": {}, "additionalProperties": false})
}

fn target_properties() -> Value {
    json!({
        "output": {"type": "string", "description": "An output name such as DP-1."},
        "window": {"type": "string", "description": "A window id, app id or title (see list_windows)."},
        "geometry": {"type": "string", "description": "A rectangle of the desktop written \"x,y WxH\", such as \"0,0 800x600\"."},
        "fullscreen": {"type": "boolean", "description": "The focused output."}
    })
}

fn object(mut properties: Value, extra: Value) -> Value {
    if let (Some(into), Value::Object(from)) = (properties.as_object_mut(), extra) {
        into.extend(from);
    }
    json!({"type": "object", "properties": properties, "additionalProperties": false})
}

fn capture_schema() -> Value {
    object(
        target_properties(),
        json!({
            "include_image": {"type": "boolean", "default": true, "description": "Attach the downscaled preview image (long edge at most 1280 px)."},
            "out": {"type": "string", "description": "Also write the PNG to this absolute path; its folder must exist."},
            "delay": {"type": "integer", "minimum": 0, "maximum": MAX_DELAY_SECONDS, "default": 0, "description": "Seconds to wait before the capture."},
            "ui": {"type": "boolean", "default": false, "description": "Also show the user the capture card and notification. Off by default."},
            "temp": {"type": "boolean", "default": false, "description": "Do not keep this capture in the user's gallery; use it for repeated or bulk checks."}
        }),
    )
}

fn ocr_schema() -> Value {
    object(
        target_properties(),
        json!({
            "file": {"type": "string", "description": "A PNG or JPEG file to read instead of the screen."},
            "lang": {"type": "string", "description": "Tesseract languages such as eng+deu; the default is the user's setting."}
        }),
    )
}

fn record_start_schema() -> Value {
    object(
        target_properties(),
        json!({
            "audio": {"type": "string", "enum": AUDIO, "description": "Which sound to record; the default is the user's recording settings."},
            "fps": {"type": "integer", "enum": FPS, "description": "Frames per second; the default is the user's recording setting."},
            "keystrokes": {"type": "boolean", "description": "Record clicks and keys for the editor."},
            "max_seconds": {"type": "integer", "minimum": 1, "maximum": MAX_SECONDS, "default": DEFAULT_MAX_SECONDS, "description": "Stop by itself after this many seconds."}
        }),
    )
}

fn record_stop_schema() -> Value {
    object(
        json!({}),
        json!({"id": {"type": "string", "description": "The recording's id; the default is the one running."}}),
    )
}

fn export_schema() -> Value {
    let limit = MAX_BRIGHTNESS_MILLI as f64 / 1000.0;
    json!({
        "type": "object",
        "properties": {
            "input": {"type": "string", "description": "The recording to export, an absolute path."},
            "format": {"type": "string", "enum": FORMATS, "description": "The file type; the default is that of out, else mp4."},
            "size": {"type": "string", "enum": SIZES, "description": "The height to scale down to; a smaller video is never enlarged."},
            "quality": {"type": "string", "enum": QUALITIES},
            "look": {"type": "string", "enum": LOOKS, "description": "A colour look."},
            "brightness": {"type": "number", "minimum": -limit, "maximum": limit},
            "trim": {"type": "string", "description": "Keep only the seconds from A to B, as in 2.5..8 or ..10."},
            "out": {"type": "string", "description": "Write the file here; its folder must exist. The default is beside the video."},
            "force": {"type": "boolean", "description": "Replace the file if it already exists."}
        },
        "required": ["input"],
        "additionalProperties": false
    })
}

fn annotate_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "image": {"type": "string", "description": "The image to draw on, an absolute path."},
            "out": {"type": "string", "description": "Write the PNG here, an absolute path ending .png whose folder exists."},
            "ops": {
                "type": "array",
                "description": "The drawing operations, in order. Each is an object with an \"op\" of arrow (from, to: [x, y]; color, width, head), rect or ellipse (x, y, w, h; color, width, fill), text (x, y, text; size, color, background), counter (x, y; n, color), highlight (x, y, w, h; color), blur or pixelate (x, y, w, h; amount from 0 to 1). Colors are #rrggbb or #rrggbbaa.",
                "items": {"type": "object"}
            },
            "background": {
                "type": "object",
                "description": "Put the image on a background: padding and corners in pixels, color (#rrggbb) or gradient (a list of colors), shadow from 0 to 1."
            },
            "force": {"type": "boolean", "description": "Replace out if it already exists."},
            "include_image": {"type": "boolean", "default": true, "description": "Attach the downscaled preview image (long edge at most 1280 px)."}
        },
        "required": ["image", "out", "ops"],
        "additionalProperties": false
    })
}

pub(super) fn check_keys(def: &ToolDef, args: &JsonObject) -> Result<(), String> {
    let schema = (def.schema)();
    let known = schema.get("properties").and_then(Value::as_object);
    match args
        .keys()
        .find(|k| !known.is_some_and(|p| p.contains_key(*k)))
    {
        Some(extra) => Err(format!("unknown argument {extra}")),
        None => Ok(()),
    }
}

fn text(args: &JsonObject, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

fn flag(args: &JsonObject, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(format!("{key} must be true or false")),
    }
}

pub(super) fn include_image(args: &JsonObject) -> Result<bool, String> {
    match args.get("include_image") {
        None | Some(Value::Null) => Ok(true),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err("include_image must be true or false".to_owned()),
    }
}

fn number(args: &JsonObject, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => Ok(Some(n.to_string())),
        Some(_) => Err(format!("{key} must be a number")),
    }
}

fn option(argv: &mut Vec<String>, name: &str, value: Option<String>) {
    if let Some(v) = value {
        argv.push(format!("--{name}={v}"));
    }
}

fn target_flags(argv: &mut Vec<String>, args: &JsonObject, file: bool) -> Result<(), String> {
    let before = argv.len();
    if file {
        option(argv, "file", text(args, "file")?);
    }
    for key in ["output", "window", "geometry"] {
        option(argv, key, text(args, key)?);
    }
    if flag(args, "fullscreen")? {
        argv.push("--fullscreen".to_owned());
    }
    if argv.len() == before {
        let or_file = if file { " Or give file." } else { "" };
        return Err(format!("{TARGET_HELP}{or_file}"));
    }
    Ok(())
}

fn capture_argv(args: &JsonObject) -> Result<Vec<String>, String> {
    let mut argv = words(&["capture"]);
    target_flags(&mut argv, args, false)?;
    option(&mut argv, "out", text(args, "out")?);
    option(&mut argv, "delay", number(args, "delay")?);
    if flag(args, "ui")? {
        argv.push("--ui".to_owned());
    }
    if flag(args, "temp")? {
        argv.push("--temp".to_owned());
    }
    Ok(argv)
}

fn ocr_argv(args: &JsonObject) -> Result<Vec<String>, String> {
    let mut argv = words(&["ocr"]);
    target_flags(&mut argv, args, true)?;
    option(&mut argv, "lang", text(args, "lang")?);
    Ok(argv)
}

fn record_start_argv(args: &JsonObject) -> Result<Vec<String>, String> {
    let mut argv = words(&["record", "start"]);
    target_flags(&mut argv, args, false)?;
    option(&mut argv, "audio", text(args, "audio")?);
    option(&mut argv, "fps", number(args, "fps")?);
    option(&mut argv, "max-seconds", number(args, "max_seconds")?);
    if flag(args, "keystrokes")? {
        argv.push("--keystrokes".to_owned());
    }
    Ok(argv)
}

fn record_stop_argv(args: &JsonObject) -> Result<Vec<String>, String> {
    let mut argv = words(&["record", "stop"]);
    option(&mut argv, "id", text(args, "id")?);
    Ok(argv)
}

fn export_argv(args: &JsonObject) -> Result<Vec<String>, String> {
    let input = text(args, "input")?.ok_or("input is required")?;
    let mut argv = words(&["export"]);
    for key in ["format", "size", "quality", "look", "trim", "out"] {
        option(&mut argv, key, text(args, key)?);
    }
    option(&mut argv, "brightness", number(args, "brightness")?);
    if flag(args, "force")? {
        argv.push("--force".to_owned());
    }
    argv.push("--".to_owned());
    argv.push(input);
    Ok(argv)
}
