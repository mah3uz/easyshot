use anyhow::{Context, Result};
use es_backend::{Backend, Probe};
use es_store::bundle::{BundleEntry, write_bundle};
use serde_json::{Map, Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const LOG_TAIL: usize = 1 << 20;

pub fn standalone_report(probe: &Probe) -> String {
    let mut report: Value =
        serde_json::from_str(&Backend::detect(probe).doctor_json()).unwrap_or_else(|_| json!({}));
    let globals: Map<String, Value> = probe
        .globals
        .all
        .iter()
        .map(|(name, version)| (name.clone(), json!(version)))
        .collect();
    let p = &probe.portals;
    if let Some(obj) = report.as_object_mut() {
        obj.insert("source".into(), json!("standalone"));
        obj.insert("wayland_globals".into(), Value::Object(globals));
        obj.insert(
            "portals".into(),
            json!({
                "FileChooser": p.file_chooser,
                "Settings": p.settings,
                "GlobalShortcuts": p.global_shortcuts,
                "ScreenCast": p.screencast,
                "Screenshot": p.screenshot,
            }),
        );
    }
    report.to_string()
}

pub fn pretty(raw: &str) -> String {
    serde_json::from_str::<Value>(raw)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| raw.to_owned())
}

pub fn render_text(raw: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return format!("{raw}\n");
    };
    let mut out = String::new();
    let text = |v: &Value| v.as_str().unwrap_or("-").to_owned();
    if let Some(source) = v.get("source").and_then(Value::as_str) {
        let _ = writeln!(out, "Source: {source} probe (the daemon is not running)");
    }
    let compositor = &v["compositor"];
    let _ = writeln!(
        out,
        "Compositor: {} {}",
        text(&compositor["name"]),
        compositor["version"].as_str().unwrap_or("")
    );
    let caps: Vec<_> = v["caps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let _ = writeln!(out, "Capabilities: {}", caps.join(", "));
    let _ = writeln!(out, "\nProviders:");
    for p in v["providers"].as_array().into_iter().flatten() {
        let forced = p["forced"]
            .as_str()
            .map_or(String::new(), |f| format!(" (forced: {f})"));
        let _ = writeln!(
            out,
            "  {}: {}{forced}",
            text(&p["capability"]),
            p["chosen"].as_str().unwrap_or("none")
        );
        for r in p["rejected"].as_array().into_iter().flatten() {
            let _ = writeln!(
                out,
                "    rejected {}: {}",
                text(&r["provider"]),
                text(&r["reason"])
            );
        }
    }
    for (title, key) in [
        ("Wayland globals", "wayland_globals"),
        ("Portals", "portals"),
    ] {
        if let Some(map) = v[key].as_object() {
            let _ = writeln!(out, "\n{title}:");
            for (name, value) in map {
                let _ = writeln!(out, "  {name}: {value}");
            }
        }
    }
    let missing = v["missing_tools"].as_array().map_or(&[][..], Vec::as_slice);
    if !missing.is_empty() {
        let _ = writeln!(out, "\nMissing tools:");
        for m in missing {
            let _ = writeln!(
                out,
                "  {} (package: {})",
                text(&m["tool"]),
                text(&m["package"])
            );
        }
    }
    let errors = v["errors"].as_array().map_or(&[][..], Vec::as_slice);
    if !errors.is_empty() {
        let _ = writeln!(out, "\nErrors:");
        for e in errors {
            let _ = writeln!(out, "  {}", text(e));
        }
    }
    out
}

pub fn write_report_bundle(
    raw: &str,
    log_dir: Option<&Path>,
    dest_dir: &Path,
    now: u64,
) -> Result<PathBuf> {
    let mut entries = vec![BundleEntry {
        name: "doctor.json".into(),
        bytes: pretty(raw).into_bytes(),
    }];
    let mut files: Vec<PathBuf> = log_dir
        .and_then(|d| std::fs::read_dir(d).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for path in files {
        let (Some(name), Ok(mut bytes)) = (
            path.file_name().and_then(|n| n.to_str()),
            std::fs::read(&path),
        ) else {
            continue;
        };
        if bytes.len() > LOG_TAIL {
            bytes.drain(..bytes.len() - LOG_TAIL);
        }
        entries.push(BundleEntry {
            name: format!("log/{name}"),
            bytes,
        });
    }
    let path = dest_dir.join(format!("easyshot-doctor-{now}.tar"));
    write_bundle(&path, now, &entries).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}
