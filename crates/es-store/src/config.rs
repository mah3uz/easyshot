use crate::atomic::write_atomic;
use crate::error::{Error, Result, io};
use es_model::Config;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, Value};

pub const CURRENT_CONFIG_VERSION: u32 = 1;

pub struct ConfigFile {
    path: PathBuf,
    config: Config,
    doc: DocumentMut,
    error: Option<Error>,
    newer: Option<u32>,
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let head = &text[..offset.min(text.len())];
    let line = head.matches('\n').count() + 1;
    let col = head.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

fn parse(text: &str) -> Result<(Config, DocumentMut)> {
    let at = |span: Option<std::ops::Range<usize>>, message: String| {
        let (line, col) = line_col(text, span.map_or(0, |s| s.start));
        Error::Parse { line, col, message }
    };
    let doc = text
        .parse::<DocumentMut>()
        .map_err(|e| at(e.span(), e.message().to_owned()))?;
    let config =
        toml::from_str::<Config>(text).map_err(|e| at(e.span(), e.message().to_owned()))?;
    Ok((config, doc))
}

fn unknown_keys(doc: &Table, known: &Table, prefix: &str, out: &mut Vec<String>) {
    for (key, item) in doc {
        let path = if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        };
        match (item, known.get(key)) {
            (_, None) => out.push(path),
            (Item::Table(d), Some(Item::Table(k))) => unknown_keys(d, k, &path, out),
            _ => {}
        }
    }
}

fn bare(v: &Value) -> Value {
    let mut v = v.clone();
    v.decor_mut().clear();
    if let Value::Array(a) = &mut v {
        a.fmt();
    }
    v
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_float(), b.as_float()) {
        (Some(x), Some(y)) => x == y,
        _ => bare(a).to_string() == bare(b).to_string(),
    }
}

fn merge(dst: &mut Table, src: &Table) {
    for (key, item) in src {
        match (dst.get_mut(key), item) {
            (Some(Item::Table(d)), Item::Table(s)) => merge(d, s),
            (Some(Item::Value(d)), Item::Value(s)) => {
                if !same(d, s) {
                    let decor = d.decor().clone();
                    *d = s.clone();
                    *d.decor_mut() = decor;
                }
            }
            (Some(d), s) => *d = s.clone(),
            (None, s) => {
                dst.insert(key, s.clone());
            }
        }
    }
}

impl ConfigFile {
    /// A file that does not parse yields the defaults plus `error()`, and `save` then refuses.
    pub fn load(path: &Path) -> Result<Self> {
        let mut file = Self {
            path: path.to_path_buf(),
            config: Config::default(),
            doc: DocumentMut::new(),
            error: None,
            newer: None,
        };
        match file.reload() {
            Ok(_) | Err(Error::Parse { .. }) => Ok(file),
            Err(e) => Err(e),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    pub fn newer_version(&self) -> Option<u32> {
        self.newer
    }

    /// Ok(true) when the loaded config changed. A parse error keeps the last good config and returns Err.
    pub fn reload(&mut self) -> Result<bool> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(io(&self.path)(e)),
        };
        match parse(&text) {
            Ok((config, doc)) => {
                self.warn_unknown(&doc, &config);
                let changed = config != self.config || self.error.is_some();
                self.newer = (config.config_version > CURRENT_CONFIG_VERSION)
                    .then_some(config.config_version);
                self.config = config;
                self.doc = doc;
                self.error = None;
                Ok(changed)
            }
            Err(e) => {
                let copy = match &e {
                    Error::Parse { line, col, message } => Error::Parse {
                        line: *line,
                        col: *col,
                        message: message.clone(),
                    },
                    _ => Error::Damaged,
                };
                self.error = Some(copy);
                Err(e)
            }
        }
    }

    fn warn_unknown(&self, doc: &DocumentMut, config: &Config) {
        let Ok(known) = toml::to_string(config)
            .map_err(|_| ())
            .and_then(|s| s.parse::<DocumentMut>().map_err(|_| ()))
        else {
            return;
        };
        let mut keys = Vec::new();
        unknown_keys(doc.as_table(), known.as_table(), "", &mut keys);
        for key in keys {
            tracing::warn!(key, "unknown config key kept");
        }
    }

    /// Changes only the keys whose value differs; comments and unknown keys stay.
    pub fn save(&mut self, config: &Config) -> Result<()> {
        if self.error.is_some() {
            return Err(Error::Damaged);
        }
        if let Some(version) = self.newer {
            return Err(Error::NewerVersion { version });
        }
        let text = toml::to_string(config).map_err(|e| Error::Parse {
            line: 0,
            col: 0,
            message: e.to_string(),
        })?;
        let fresh = text.parse::<DocumentMut>().map_err(|e| Error::Parse {
            line: 0,
            col: 0,
            message: e.to_string(),
        })?;
        let mut doc = self.doc.clone();
        merge(doc.as_table_mut(), fresh.as_table());
        if config.general.save_dir.is_none()
            && let Some(Item::Table(general)) = doc.get_mut("general")
        {
            general.remove("save_dir");
        }
        write_atomic(&self.path, doc.to_string().as_bytes())?;
        self.doc = doc;
        self.config = config.clone();
        Ok(())
    }
}
