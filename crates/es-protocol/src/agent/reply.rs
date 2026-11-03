use super::OcrInfo;
use super::access::{AccessStatus, ConfigEntry};
use serde::{Deserialize, Serialize};

/// Each code has its own process exit status, so a script can branch without reading the JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Failed,
    Cancelled,
    NoDaemon,
    InvalidArguments,
    NotFound,
    Unsupported,
    AccessOff,
    AccessDenied,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 8] = [
        ErrorCode::Failed,
        ErrorCode::Cancelled,
        ErrorCode::NoDaemon,
        ErrorCode::InvalidArguments,
        ErrorCode::NotFound,
        ErrorCode::Unsupported,
        ErrorCode::AccessOff,
        ErrorCode::AccessDenied,
    ];

    /// 1 to 3 are the exit statuses every easyshot command already uses.
    pub fn exit_code(self) -> u8 {
        match self {
            ErrorCode::Failed => 1,
            ErrorCode::Cancelled => 2,
            ErrorCode::NoDaemon => 3,
            ErrorCode::InvalidArguments => 4,
            ErrorCode::NotFound => 5,
            ErrorCode::Unsupported => 6,
            ErrorCode::AccessOff => 7,
            ErrorCode::AccessDenied => 8,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Failed => "failed",
            ErrorCode::Cancelled => "cancelled",
            ErrorCode::NoDaemon => "no_daemon",
            ErrorCode::InvalidArguments => "invalid_arguments",
            ErrorCode::NotFound => "not_found",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::AccessOff => "access_off",
            ErrorCode::AccessDenied => "access_denied",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyKind {
    /// The command line itself could not be read.
    Usage,
    Outputs,
    Windows,
    Capture,
    Access,
    Config,
    Export,
    Ocr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
}

/// Fields the compositor does not report are left out rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
}

/// What a capture made; the size is in pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureInfo {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowEntry>,
}

/// What an export wrote; the size is in pixels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportInfo {
    pub path: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub bytes: u64,
}

/// The one JSON object an agent command answers with: `{ok, kind, ...fields, error}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "WireReply")]
pub struct AgentReply {
    pub ok: bool,
    pub kind: ReplyKind,
    /// Set when the command returned before its job finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<OutputEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub windows: Option<Vec<WindowEntry>>,
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub capture: Option<Box<CaptureInfo>>,
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub export: Option<Box<ExportInfo>>,
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub ocr: Option<Box<OcrInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<ConfigEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ReplyError>,
}

/// A capture and an export both flatten `path`, `width` and `height`; flattened side by side the
/// first would swallow them, so reading goes through one untagged choice.
#[derive(Deserialize)]
struct WireReply {
    ok: bool,
    kind: ReplyKind,
    #[serde(default)]
    job: Option<u64>,
    #[serde(default)]
    outputs: Option<Vec<OutputEntry>>,
    #[serde(default)]
    windows: Option<Vec<WindowEntry>>,
    #[serde(flatten, default)]
    made: Option<Made>,
    #[serde(default)]
    access: Option<AccessStatus>,
    #[serde(default)]
    config: Option<ConfigEntry>,
    #[serde(default)]
    error: Option<ReplyError>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Made {
    Export(Box<ExportInfo>),
    Capture(Box<CaptureInfo>),
    Ocr(Box<OcrInfo>),
}

impl From<WireReply> for AgentReply {
    fn from(w: WireReply) -> AgentReply {
        let (capture, export, ocr) = match w.made {
            Some(Made::Capture(c)) => (Some(c), None, None),
            Some(Made::Export(e)) => (None, Some(e), None),
            Some(Made::Ocr(o)) => (None, None, Some(o)),
            None => (None, None, None),
        };
        AgentReply {
            ok: w.ok,
            kind: w.kind,
            job: w.job,
            outputs: w.outputs,
            windows: w.windows,
            capture,
            export,
            ocr,
            access: w.access,
            config: w.config,
            error: w.error,
        }
    }
}

impl AgentReply {
    pub fn ok(kind: ReplyKind) -> AgentReply {
        AgentReply {
            ok: true,
            kind,
            job: None,
            outputs: None,
            windows: None,
            capture: None,
            access: None,
            config: None,
            export: None,
            ocr: None,
            error: None,
        }
    }

    pub fn failed(kind: ReplyKind, code: ErrorCode, message: impl Into<String>) -> AgentReply {
        AgentReply {
            ok: false,
            error: Some(ReplyError {
                code,
                message: message.into(),
            }),
            ..AgentReply::ok(kind)
        }
    }

    pub fn exit_code(&self) -> u8 {
        match (&self.error, self.ok) {
            (Some(e), _) => e.code.exit_code(),
            (None, true) => 0,
            (None, false) => ErrorCode::Failed.exit_code(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            r#"{"ok":false,"kind":"usage","error":{"code":"failed","message":"the reply could not be written"}}"#
                .to_owned()
        })
    }

    pub fn from_json(text: &str) -> Result<AgentReply, serde_json::Error> {
        serde_json::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn value(reply: &AgentReply) -> Value {
        serde_json::from_str(&reply.to_json()).unwrap()
    }

    #[test]
    fn exit_codes_are_distinct_stable_and_never_zero() {
        let codes: Vec<u8> = ErrorCode::ALL.iter().map(|c| c.exit_code()).collect();
        assert_eq!(codes, [1, 2, 3, 4, 5, 6, 7, 8]);
        for code in ErrorCode::ALL {
            assert_eq!(
                serde_json::to_value(code).unwrap(),
                Value::String(code.as_str().to_owned())
            );
        }
    }

    #[test]
    fn a_success_has_ok_kind_and_its_fields_and_no_error() {
        let mut reply = AgentReply::ok(ReplyKind::Outputs);
        reply.outputs = Some(vec![OutputEntry {
            name: "DP-1".into(),
            description: None,
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.5,
            focused: Some(true),
        }]);
        assert_eq!(
            value(&reply),
            json!({
                "ok": true,
                "kind": "outputs",
                "outputs": [{
                    "name": "DP-1", "x": -1920, "y": 0, "width": 1920, "height": 1080,
                    "scale": 1.5, "focused": true
                }]
            })
        );
        assert_eq!(reply.exit_code(), 0);
    }

    #[test]
    fn a_capture_reply_is_flat_with_the_window_only_when_there_is_one() {
        let mut reply = AgentReply::ok(ReplyKind::Capture);
        let mut info = CaptureInfo {
            path: "/tmp/a.png".into(),
            width: 640,
            height: 360,
            scale: 1.5,
            output: Some("DP-1".into()),
            window: None,
        };
        reply.capture = Some(Box::new(info.clone()));
        assert_eq!(
            value(&reply),
            json!({
                "ok": true, "kind": "capture", "path": "/tmp/a.png",
                "width": 640, "height": 360, "scale": 1.5, "output": "DP-1"
            })
        );
        info.window = Some(WindowEntry {
            id: "7".into(),
            app_id: None,
            title: None,
            x: None,
            y: None,
            width: None,
            height: None,
            output: None,
            focused: None,
        });
        reply.capture = Some(Box::new(info));
        assert_eq!(value(&reply)["window"], json!({"id": "7"}));
        assert_eq!(AgentReply::from_json(&reply.to_json()).unwrap(), reply);
    }

    #[test]
    fn a_window_without_geometry_or_focus_leaves_those_fields_out() {
        let mut reply = AgentReply::ok(ReplyKind::Windows);
        reply.windows = Some(vec![WindowEntry {
            id: "7".into(),
            app_id: Some("foot".into()),
            title: None,
            x: None,
            y: None,
            width: None,
            height: None,
            output: None,
            focused: None,
        }]);
        assert_eq!(
            value(&reply),
            json!({"ok": true, "kind": "windows", "windows": [{"id": "7", "app_id": "foot"}]})
        );
    }

    #[test]
    fn a_failure_has_ok_false_and_a_coded_error() {
        let reply = AgentReply::failed(ReplyKind::Windows, ErrorCode::NotFound, "no window 9");
        assert_eq!(
            value(&reply),
            json!({
                "ok": false,
                "kind": "windows",
                "error": {"code": "not_found", "message": "no window 9"}
            })
        );
        assert_eq!(reply.exit_code(), 5);
    }

    #[test]
    fn replies_round_trip_and_tolerate_fields_added_later() {
        let reply = AgentReply::failed(ReplyKind::Usage, ErrorCode::AccessOff, "off");
        assert_eq!(AgentReply::from_json(&reply.to_json()).unwrap(), reply);
        let newer = r#"{"ok":true,"kind":"outputs","outputs":[],"added_later":1}"#;
        assert_eq!(
            AgentReply::from_json(newer).unwrap().outputs,
            Some(Vec::new())
        );
        assert!(AgentReply::from_json(r#"{"ok":true}"#).is_err());
    }

    #[test]
    fn ok_false_without_an_error_still_fails() {
        let reply = AgentReply {
            ok: false,
            ..AgentReply::ok(ReplyKind::Outputs)
        };
        assert_eq!(reply.exit_code(), 1);
    }

    #[test]
    fn an_export_reply_is_flat_and_never_mistaken_for_a_capture() {
        let mut reply = AgentReply::ok(ReplyKind::Export);
        reply.export = Some(Box::new(ExportInfo {
            path: "/tmp/a.webm".into(),
            format: "webm".into(),
            width: 854,
            height: 480,
            duration_ms: 2000,
            bytes: 12345,
        }));
        assert_eq!(
            value(&reply),
            json!({
                "ok": true, "kind": "export", "path": "/tmp/a.webm", "format": "webm",
                "width": 854, "height": 480, "duration_ms": 2000, "bytes": 12345
            })
        );
        let back = AgentReply::from_json(&reply.to_json()).unwrap();
        assert_eq!(back, reply);
        assert!(back.capture.is_none());
    }
}
