use crate::atomic::write_json_atomic;
use crate::error::{Error, Result, io};
use es_model::{Envelope, FormatKind, Header, Migrated, migrate};
use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

pub struct JsonFile<T> {
    path: PathBuf,
    kind: FormatKind,
    pub value: T,
    newer: Option<u32>,
}

fn parse_err(e: &serde_json::Error) -> Error {
    Error::Parse {
        line: e.line(),
        col: e.column(),
        message: e.to_string(),
    }
}

pub fn parse_envelope<T: DeserializeOwned + Default>(
    kind: FormatKind,
    text: &str,
) -> Result<(T, Option<u32>)> {
    let raw: serde_json::Value = serde_json::from_str(text).map_err(|e| parse_err(&e))?;
    let header: Header = serde_json::from_value(raw.clone()).map_err(|e| parse_err(&e))?;
    if header.format != kind.name() {
        return Err(Error::WrongFormat {
            expected: kind.name().into(),
            found: header.format,
        });
    }
    match migrate(kind, header.version, raw.clone())? {
        Migrated::Current(v) => {
            let env: Envelope<T> = if v == raw {
                serde_json::from_str(text).map_err(|e| parse_err(&e))?
            } else {
                serde_json::from_value(v).map_err(|e| parse_err(&e))?
            };
            Ok((env.body, None))
        }
        Migrated::NewerReadOnly { version } => {
            let body = serde_json::from_value::<Envelope<T>>(raw)
                .map(|e| e.body)
                .unwrap_or_default();
            Ok((body, Some(version)))
        }
    }
}

impl<T: Serialize + DeserializeOwned + Default> JsonFile<T> {
    pub fn open(path: &Path, kind: FormatKind) -> Result<Self> {
        Ok(Self::open_existing(path, kind)?.unwrap_or_else(|| Self::fresh(path, kind)))
    }

    pub fn open_existing(path: &Path, kind: FormatKind) -> Result<Option<Self>> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(path)(e)),
        };
        let (value, newer) = parse_envelope::<T>(kind, &text)?;
        Ok(Some(Self {
            path: path.to_path_buf(),
            kind,
            value,
            newer,
        }))
    }

    pub fn fresh(path: &Path, kind: FormatKind) -> Self {
        Self {
            path: path.to_path_buf(),
            kind,
            value: T::default(),
            newer: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn newer_version(&self) -> Option<u32> {
        self.newer
    }

    pub fn save(&self) -> Result<()> {
        if let Some(version) = self.newer {
            return Err(Error::NewerVersion { version });
        }
        write_json_atomic(&self.path, &Envelope::new(self.kind, &self.value))
    }
}
