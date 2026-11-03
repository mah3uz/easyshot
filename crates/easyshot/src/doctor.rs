use crate::doctor_tools::{self, ToolRunner};
use anyhow::{Context, Result};
use es_backend::{Backend, Probe};
use es_store::bundle::{BundleEntry, write_bundle};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const LOG_TAIL: usize = 1 << 20;

pub fn standalone_report(
    probe: &Probe,
    runner: &dyn ToolRunner,
    config: &Path,
    input: Value,
) -> String {
    let mut report: Value =
        serde_json::from_str(&Backend::detect(probe).doctor_json()).unwrap_or_else(|_| json!({}));
    if let Some(obj) = report.as_object_mut() {
        obj.insert("source".into(), json!("standalone"));
        obj.insert("services".into(), doctor_tools::services(&probe.dbus));
        obj.insert("tools".into(), doctor_tools::tools(runner));
        obj.insert("config".into(), doctor_tools::config_status(config));
        obj.insert("input".into(), input);
    }
    report.to_string()
}

pub fn with_input(raw: &str, input: Value) -> String {
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Object(mut obj)) => {
            obj.insert("input".into(), input);
            Value::Object(obj).to_string()
        }
        _ => raw.to_owned(),
    }
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
    render_probes(&v, &mut out);
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

fn render_probes(v: &Value, out: &mut String) {
    let yes = |b: &Value| {
        if b.as_bool() == Some(true) {
            "present"
        } else {
            "absent"
        }
    };
    let version = |v: &Value| v.as_str().unwrap_or("not found").to_owned();
    let list = |v: &Value| {
        let items: Vec<_> = v
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        if items.is_empty() {
            "none".to_owned()
        } else {
            items.join(", ")
        }
    };
    if v["services"].is_object() {
        let _ = writeln!(out, "\nServices on the session bus:");
        let _ = writeln!(
            out,
            "  tray watcher: {}",
            yes(&v["services"]["tray_watcher"])
        );
        let _ = writeln!(
            out,
            "  notification server: {}",
            yes(&v["services"]["notification_server"])
        );
    }
    let t = &v["tools"];
    if t.is_object() {
        let _ = writeln!(out, "\nTool versions:");
        let _ = writeln!(
            out,
            "  gpu-screen-recorder: {}",
            version(&t["gpu-screen-recorder"])
        );
        let _ = writeln!(out, "  wf-recorder: {}", version(&t["wf-recorder"]));
        let _ = writeln!(out, "  ffmpeg: {}", version(&t["ffmpeg"]["version"]));
        let _ = writeln!(out, "    encoders: {}", list(&t["ffmpeg"]["encoders"]));
        let _ = writeln!(out, "  tesseract: {}", version(&t["tesseract"]["version"]));
        let _ = writeln!(out, "    languages: {}", list(&t["tesseract"]["languages"]));
    }
    let i = &v["input"];
    if let Some(status) = i["status"].as_str() {
        let shown = match status {
            "granted" => "can read the keyboard and mouse",
            "denied" => "cannot read the keyboard and mouse",
            _ => "not in this build",
        };
        let _ = writeln!(out, "\nClicks and keys in recordings: {shown}");
        if let Some(fix) = i["fix"].as_str() {
            let _ = writeln!(out, "  Fix: {fix}");
        }
        let _ = writeln!(out, "  Records {}", i["records"].as_str().unwrap_or("-"));
    }
    let c = &v["config"];
    if c.is_object() {
        let _ = write!(
            out,
            "\nSettings file: {} ({}",
            c["path"].as_str().unwrap_or("-"),
            c["status"].as_str().unwrap_or("-")
        );
        if let Some(detail) = c.get("detail") {
            let _ = write!(
                out,
                ": {}",
                detail.as_str().map_or(detail.to_string(), str::to_owned)
            );
        }
        let _ = writeln!(out, ")");
    }
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
