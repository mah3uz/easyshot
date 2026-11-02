use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub format: String,
    pub version: u32,
    #[serde(flatten)]
    pub body: T,
}

impl<T> Envelope<T> {
    pub fn new(kind: FormatKind, body: T) -> Self {
        Self {
            format: kind.name().into(),
            version: CURRENT_VERSION,
            body,
        }
    }
}

/// The two fields every file starts with, readable even when the body is from a newer version.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Header {
    pub format: String,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Style,
    BgPresets,
    Document,
    History,
    DeckItem,
    Capture,
    Input,
    Project,
    Recovery,
    State,
}

impl FormatKind {
    pub const ALL: [FormatKind; 10] = [
        FormatKind::Style,
        FormatKind::BgPresets,
        FormatKind::Document,
        FormatKind::History,
        FormatKind::DeckItem,
        FormatKind::Capture,
        FormatKind::Input,
        FormatKind::Project,
        FormatKind::Recovery,
        FormatKind::State,
    ];

    pub fn from_name(name: &str) -> Option<FormatKind> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            FormatKind::Style => "easyshot.style",
            FormatKind::BgPresets => "easyshot.bg-presets",
            FormatKind::Document => "easyshot.document",
            FormatKind::History => "easyshot.history",
            FormatKind::DeckItem => "easyshot.deck-item",
            FormatKind::Capture => "easyshot.capture",
            FormatKind::Input => "easyshot.input",
            FormatKind::Project => "easyshot.project",
            FormatKind::Recovery => "easyshot.recovery",
            FormatKind::State => "easyshot.state",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Migrated {
    Current(serde_json::Value),
    /// Written by a newer easyshot: open read-only, never write it back.
    NewerReadOnly {
        version: u32,
    },
}

pub fn migrate(kind: FormatKind, version: u32, value: serde_json::Value) -> Result<Migrated> {
    match version {
        0 => Err(Error::Protocol(format!(
            "{}: invalid version 0",
            kind.name()
        ))),
        CURRENT_VERSION => Ok(Migrated::Current(value)),
        v => Ok(Migrated::NewerReadOnly { version: v }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn v1_passes_through_unchanged() {
        let v = json!({"format": "easyshot.history", "version": 1, "entries": []});
        let out = migrate(FormatKind::History, 1, v.clone()).unwrap();
        assert_eq!(out, Migrated::Current(v));
    }

    #[test]
    fn newer_version_is_read_only_not_an_error() {
        let out = migrate(FormatKind::Document, 2, json!({})).unwrap();
        assert_eq!(out, Migrated::NewerReadOnly { version: 2 });
    }

    #[test]
    fn version_zero_is_rejected() {
        assert!(migrate(FormatKind::State, 0, json!({})).is_err());
    }

    #[test]
    fn names_round_trip_for_every_kind() {
        for k in FormatKind::ALL {
            assert_eq!(FormatKind::from_name(k.name()), Some(k));
            assert!(k.name().starts_with("easyshot."));
        }
        assert_eq!(FormatKind::from_name("easyshot.nope"), None);
    }

    #[test]
    fn header_reads_a_body_it_does_not_understand() {
        let h: Header = serde_json::from_str(
            r#"{"format":"easyshot.document","version":9,"shapes":{"new":"layout"}}"#,
        )
        .unwrap();
        assert_eq!((h.format.as_str(), h.version), ("easyshot.document", 9));
    }

    #[test]
    fn envelope_new_stamps_the_current_version() {
        let e = Envelope::new(FormatKind::State, 5u8);
        assert_eq!(
            (e.format.as_str(), e.version),
            ("easyshot.state", CURRENT_VERSION)
        );
    }

    #[test]
    fn envelope_flattens_body_next_to_format_and_version() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Body {
            n: u32,
        }
        let e = Envelope {
            format: "easyshot.state".into(),
            version: 1,
            body: Body { n: 7 },
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v, json!({"format": "easyshot.state", "version": 1, "n": 7}));
        assert_eq!(serde_json::from_value::<Envelope<Body>>(v).unwrap(), e);
    }
}
