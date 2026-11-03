use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{AgentTarget, WindowEntry};
use crate::{ArgValue, RouteArgs};

const MAX_PATH: usize = 4096;
const MAX_LANGUAGES: usize = 8;
const MAX_LANGUAGE: usize = 32;
const FILE_ARG: &str = "file";
const LANG_ARG: &str = "lang";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OcrInput {
    Target(AgentTarget),
    /// An absolute path to a `.png`, `.jpg` or `.jpeg` file.
    File(PathBuf),
}

/// What one `agent ocr` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OcrSpec {
    pub input: OcrInput,
    /// Tesseract language codes; none means the `[ocr] languages` setting.
    pub languages: Vec<String>,
}

impl OcrSpec {
    pub const ARGS: [&'static str; 2] = [FILE_ARG, LANG_ARG];

    pub const fn new(input: OcrInput) -> OcrSpec {
        OcrSpec {
            input,
            languages: Vec::new(),
        }
    }

    pub fn write_args(&self, args: &mut RouteArgs) {
        match &self.input {
            OcrInput::Target(target) => target.write_args(args),
            OcrInput::File(path) => {
                args.0.insert(
                    FILE_ARG.to_owned(),
                    ArgValue::Str(path.to_string_lossy().into_owned()),
                );
            }
        }
        if !self.languages.is_empty() {
            args.0
                .insert(LANG_ARG.to_owned(), ArgValue::Str(self.languages.join("+")));
        }
    }

    pub fn from_args(args: &RouteArgs) -> Result<OcrSpec, String> {
        let languages = match args.0.get(LANG_ARG) {
            None => Vec::new(),
            Some(ArgValue::Str(s)) => parse_languages(s)?,
            Some(_) => return Err(format!("{LANG_ARG} must be a string")),
        };
        let input = match args.0.get(FILE_ARG) {
            None => OcrInput::Target(AgentTarget::from_args(args)?),
            Some(ArgValue::Str(s)) => {
                if AgentTarget::ARGS.iter().any(|k| args.0.contains_key(*k)) {
                    return Err("name a target or a file, not both".to_owned());
                }
                OcrInput::File(check_file(s)?)
            }
            Some(_) => return Err(format!("{FILE_ARG} must be a string")),
        };
        Ok(OcrSpec { input, languages })
    }
}

/// Codes joined by `+` or `,`, each as Tesseract names a language: `eng`, `deu`, `chi_sim`.
pub fn parse_languages(text: &str) -> Result<Vec<String>, String> {
    let codes: Vec<String> = text.split(['+', ',']).map(str::to_owned).collect();
    let ok = |c: &String| {
        (1..=MAX_LANGUAGE).contains(&c.len())
            && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    };
    if codes.len() > MAX_LANGUAGES || !codes.iter().all(ok) {
        return Err(format!(
            "{text:?} is not a list of up to {MAX_LANGUAGES} language codes such as eng+deu"
        ));
    }
    Ok(codes)
}

pub fn check_file(text: &str) -> Result<PathBuf, String> {
    if text.is_empty() || text.len() > MAX_PATH || text.contains('\0') {
        return Err(format!("a file path has 1 to {MAX_PATH} bytes and no NUL"));
    }
    let path = Path::new(text);
    if !path.is_absolute() {
        return Err(format!("{text:?} is not an absolute path"));
    }
    let image = path.extension().is_some_and(|e| {
        ["png", "jpg", "jpeg"]
            .iter()
            .any(|want| e.eq_ignore_ascii_case(want))
    });
    if !image || path.file_name().is_none() {
        return Err(format!("{text:?} is not a .png or .jpg file"));
    }
    Ok(path.to_path_buf())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrCode {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// What was read; empty when the source held nothing legible.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OcrSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

/// Positions are pixels of the image that was read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrInfo {
    pub text: String,
    pub lines: Vec<OcrLine>,
    pub qr: Vec<OcrCode>,
    pub source: OcrSource,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Geometry, OutputName};

    fn round_trip(spec: &OcrSpec) -> Result<OcrSpec, String> {
        let mut args = RouteArgs::default();
        spec.write_args(&mut args);
        OcrSpec::from_args(&args)
    }

    #[test]
    fn a_target_or_a_file_and_the_languages_survive_the_route_arguments() {
        let specs = [
            OcrSpec {
                input: OcrInput::Target(AgentTarget::Geometry(
                    Geometry::parse("0,0 64x48").unwrap(),
                )),
                languages: vec!["eng".into(), "deu".into()],
            },
            OcrSpec::new(OcrInput::Target(AgentTarget::Output(
                OutputName::new("DP-1").unwrap(),
            ))),
            OcrSpec::new(OcrInput::File("/tmp/a.JPG".into())),
        ];
        for spec in specs {
            assert_eq!(round_trip(&spec).unwrap(), spec);
        }
    }

    #[test]
    fn the_daemon_refuses_what_a_command_would_not_send() {
        let with = |pairs: &[(&str, ArgValue)]| {
            let mut args = RouteArgs::default();
            for (k, v) in pairs {
                args.0.insert((*k).to_owned(), v.clone());
            }
            OcrSpec::from_args(&args)
        };
        let text = |s: &str| ArgValue::Str(s.to_owned());
        assert!(with(&[]).is_err(), "a source is required");
        assert!(with(&[(FILE_ARG, text("/tmp/a.png"))]).is_ok());
        for bad in ["", "a.png", "/tmp/a.gif", "/tmp/a", "/tmp/a\0.png"] {
            assert!(with(&[(FILE_ARG, text(bad))]).is_err(), "{bad:?}");
        }
        assert!(with(&[(FILE_ARG, ArgValue::Int(1))]).is_err());
        assert!(
            with(&[
                (FILE_ARG, text("/tmp/a.png")),
                ("fullscreen", ArgValue::Bool(true))
            ])
            .is_err(),
            "a file and a target"
        );
        let file = |lang: ArgValue| with(&[(FILE_ARG, text("/tmp/a.png")), (LANG_ARG, lang)]);
        assert!(file(text("eng+chi_sim")).is_ok());
        for bad in ["", "eng+", "en g", "-l", "../x", "a+b+c+d+e+f+g+h+i"] {
            assert!(file(text(bad)).is_err(), "{bad:?}");
        }
        assert!(file(ArgValue::Int(1)).is_err());
    }

    #[test]
    fn an_ocr_answer_has_text_lines_codes_and_its_source_and_nothing_of_a_capture() {
        use crate::agent::{AgentReply, ReplyKind};
        let mut reply = AgentReply::ok(ReplyKind::Ocr);
        reply.ocr = Some(Box::new(OcrInfo {
            text: "https://x.test\nHello".into(),
            lines: vec![OcrLine {
                text: "Hello".into(),
                x: 4,
                y: 5,
                width: 60,
                height: 12,
                confidence: 91.5,
            }],
            qr: vec![OcrCode {
                text: "https://x.test".into(),
                x: 1,
                y: 2,
                width: 30,
                height: 30,
            }],
            source: OcrSource {
                file: Some("/tmp/a.png".into()),
                ..OcrSource::default()
            },
        }));
        let json: serde_json::Value = serde_json::from_str(&reply.to_json()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "ok": true,
                "kind": "ocr",
                "text": "https://x.test\nHello",
                "lines": [{"text": "Hello", "x": 4, "y": 5, "width": 60, "height": 12, "confidence": 91.5}],
                "qr": [{"text": "https://x.test", "x": 1, "y": 2, "width": 30, "height": 30}],
                "source": {"file": "/tmp/a.png"}
            })
        );
        assert_eq!(AgentReply::from_json(&reply.to_json()).unwrap(), reply);
    }

    #[test]
    fn languages_may_be_separated_by_plus_or_comma() {
        assert_eq!(
            parse_languages("eng,deu+fra").unwrap(),
            ["eng", "deu", "fra"]
        );
    }
}
