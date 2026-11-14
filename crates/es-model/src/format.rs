use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

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
            version: kind.version(),
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
    Agent,
    Gradients,
    Shares,
    BucketKeys,
    SkillInstalls,
}

impl FormatKind {
    pub const ALL: [FormatKind; 15] = [
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
        FormatKind::Agent,
        FormatKind::Gradients,
        FormatKind::Shares,
        FormatKind::BucketKeys,
        FormatKind::SkillInstalls,
    ];

    pub fn from_name(name: &str) -> Option<FormatKind> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The version this build writes.
    pub fn version(self) -> u32 {
        match self {
            FormatKind::Document => 4,
            FormatKind::State | FormatKind::Style | FormatKind::Recovery | FormatKind::Capture => 2,
            _ => 1,
        }
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
            FormatKind::Agent => "easyshot.agent",
            FormatKind::Gradients => "easyshot.gradients",
            FormatKind::Shares => "easyshot.shares",
            FormatKind::BucketKeys => "easyshot.bucket-keys",
            FormatKind::SkillInstalls => "easyshot.skill-installs",
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
    if version == 0 {
        return Err(Error::Protocol(format!(
            "{}: invalid version 0",
            kind.name()
        )));
    }
    if version > kind.version() {
        return Ok(Migrated::NewerReadOnly { version });
    }
    let mut value = value;
    for from in version..kind.version() {
        value = step(kind, from, value)?;
    }
    Ok(Migrated::Current(value))
}

fn step(kind: FormatKind, from: u32, mut value: serde_json::Value) -> Result<serde_json::Value> {
    match (kind, from) {
        (FormatKind::State, 1) => {
            if let Some(body) = value.as_object_mut() {
                let region = body.get("last_region").filter(|r| !r.is_null()).cloned();
                if let Some(region) = region {
                    body.insert(
                        "last_target".into(),
                        serde_json::json!({ "region": region }),
                    );
                }
                body.insert("version".into(), 2.into());
            }
            Ok(value)
        }
        (FormatKind::Document, 1..=3)
        | (FormatKind::Style | FormatKind::Recovery | FormatKind::Capture, 1) => {
            if let Some(body) = value.as_object_mut() {
                body.insert("version".into(), (from + 1).into());
            }
            Ok(value)
        }
        _ => Err(Error::Protocol(format!(
            "{}: no migration from version {from}",
            kind.name()
        ))),
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
    fn state_v1_becomes_v2_with_its_region_as_the_last_target() {
        let region = json!({"output": "DP-2", "x": 1, "y": 2, "w": 3, "h": 4});
        let v1 =
            json!({"format": "easyshot.state", "version": 1, "last_region": region, "counter": 7});
        let Migrated::Current(v2) = migrate(FormatKind::State, 1, v1).unwrap() else {
            panic!("a v1 state is not newer");
        };
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["last_target"], json!({ "region": region }));
        assert_eq!((&v2["last_region"], &v2["counter"]), (&region, &json!(7)));
    }

    #[test]
    fn state_v1_without_a_region_gets_no_target() {
        let v1 = json!({"format": "easyshot.state", "version": 1, "last_region": null});
        let Migrated::Current(v2) = migrate(FormatKind::State, 1, v1).unwrap() else {
            panic!("a v1 state is not newer");
        };
        assert!(v2.get("last_target").is_none());
    }

    #[test]
    fn newer_version_is_read_only_not_an_error() {
        let out = migrate(FormatKind::Document, 5, json!({})).unwrap();
        assert_eq!(out, Migrated::NewerReadOnly { version: 5 });
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
        assert_eq!((e.format.as_str(), e.version), ("easyshot.state", 2));
        let e = Envelope::new(FormatKind::History, 5u8);
        assert_eq!(e.version, 1);
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
