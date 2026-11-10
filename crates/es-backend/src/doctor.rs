use crate::backend::{Backend, Caps, CompositorInfo, ProviderReport};
use serde_json::{Value, json};

impl Backend {
    pub fn doctor_json(&self) -> String {
        doctor_json(&self.report(), self.caps(), &self.compositor)
    }
}

pub fn doctor_json(report: &ProviderReport, caps: Caps, compositor: &CompositorInfo) -> String {
    let providers: Vec<Value> = report
        .entries
        .iter()
        .map(|e| {
            json!({
                "capability": e.capability,
                "chosen": e.chosen,
                "forced": e.forced,
                "rejected": e.rejected.iter()
                    .map(|(provider, reason)| json!({ "provider": provider, "reason": reason }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let caps: Vec<&str> = caps.iter_names().map(|(name, _)| name).collect();
    let missing: Vec<Value> = report
        .missing_tools
        .iter()
        .map(|(tool, package)| json!({ "tool": tool, "package": package }))
        .collect();
    json!({
        "compositor": { "name": compositor.name, "version": compositor.version },
        "providers": providers,
        "caps": caps,
        "missing_tools": missing,
        "errors": report.errors,
        "wayland_globals": report.wayland_globals,
        "portals": report.portals,
    })
    .to_string()
}
