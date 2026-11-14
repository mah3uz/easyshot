use crate::doctor_tools::{self, ToolRunner};
use crate::strings::fl;
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

/// Adds the GStreamer elements that `has` lacks to the missing tools, each with the package to install.
pub fn with_gstreamer(raw: &str, has: &dyn Fn(&str) -> bool) -> String {
    let features = [
        ("playing videos", es_record::gst_plugins::playback_gaps(has)),
        (
            "recording a window",
            es_record::gst_plugins::window_recording_gaps(has),
        ),
    ];
    let Ok(Value::Object(mut obj)) = serde_json::from_str::<Value>(raw) else {
        return raw.to_owned();
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut found = Vec::new();
    for (feature, gaps) in &features {
        for gap in gaps {
            if !seen.contains(&gap.element) {
                seen.push(gap.element);
                found.push(json!({
                    "tool": format!("GStreamer {} ({feature})", gap.element),
                    "package": gap.package,
                }));
            }
        }
    }
    if let Some(Value::Array(missing)) = obj.get_mut("missing_tools") {
        missing.extend(found);
    } else {
        obj.insert("missing_tools".into(), Value::Array(found));
    }
    Value::Object(obj).to_string()
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

pub fn with_skill(raw: &str, notes: &[crate::agent::SkillNote]) -> String {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) if value.is_object() => crate::agent::with_notes(value, notes).to_string(),
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
    render_services_and_tools(&v, &mut out);
    render_input(&v, &mut out);
    render_skill(&v, &mut out);
    render_config(&v, &mut out);
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

fn render_services_and_tools(v: &Value, out: &mut String) {
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
}

pub(crate) fn render_input(v: &Value, out: &mut String) {
    let i = &v["input"];
    if let Some(status) = i["status"].as_str() {
        let shown = match status {
            "granted" => fl!("doctor-input-granted"),
            "denied" => fl!("doctor-input-denied"),
            "group_pending" => fl!("doctor-input-group-pending"),
            _ => fl!("doctor-input-absent"),
        };
        let _ = writeln!(out, "\n{}", fl!("doctor-input", state = shown));
        if let Some(fix) = i["fix"].as_str() {
            let _ = writeln!(out, "  {}", fl!("doctor-input-fix", fix = fix));
        }
        let what = i["records"].as_str().unwrap_or("-");
        let _ = writeln!(out, "  {}", fl!("doctor-input-records", what = what));
    }
}

pub(crate) fn render_skill(v: &Value, out: &mut String) {
    for note in v["skill"].as_array().into_iter().flatten() {
        let key = if note["state"] == "edited" {
            fl!(
                "doctor-skill-edited",
                path = note["dir"].as_str().unwrap_or("-"),
                fix = note["fix"].as_str().unwrap_or("-")
            )
        } else {
            fl!(
                "doctor-skill-outdated",
                path = note["dir"].as_str().unwrap_or("-"),
                fix = note["fix"].as_str().unwrap_or("-")
            )
        };
        let _ = writeln!(out, "\n{key}");
    }
}

pub(crate) fn render_config(v: &Value, out: &mut String) {
    let c = &v["config"];
    if c.is_object() {
        let detail = c.get("detail").map_or(String::new(), |d| {
            format!(": {}", d.as_str().map_or(d.to_string(), str::to_owned))
        });
        let _ = writeln!(
            out,
            "\n{}",
            fl!(
                "doctor-config",
                path = c["path"].as_str().unwrap_or("-"),
                status = c["status"].as_str().unwrap_or("-"),
                detail = detail
            )
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_tells_the_user_which_package_brings_back_video_playback() {
        let raw = json!({"missing_tools": [{"tool": "wf-recorder", "package": "wf-recorder"}]});
        let raw = with_gstreamer(&raw.to_string(), &|e| e != "videobalance");
        let v: Value = serde_json::from_str(&raw).unwrap();
        let missing = v["missing_tools"].as_array().unwrap();
        assert_eq!(missing.len(), 2);
        assert_eq!(missing[0]["tool"], "wf-recorder");
        assert_eq!(missing[1]["package"], "gst-plugins-good");
        let shown = crate::doctor_summary::render(&raw, false);
        assert!(
            shown.contains("videobalance") && shown.contains("gst-plugins-good"),
            "{shown}"
        );
    }

    #[test]
    fn doctor_adds_nothing_when_every_element_is_present() {
        let raw = with_gstreamer(&json!({}).to_string(), &|_| true);
        let v: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["missing_tools"], json!([]));
    }
}
