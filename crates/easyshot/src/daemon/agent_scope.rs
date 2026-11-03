use es_geom::Rect;
use es_model::AgentCfg;
use es_model::platform::{OutputInfo, WindowInfo};
use es_protocol::agent::{AgentReply, AgentTarget, WindowSelector};

const CHANGE_IT: &str = "change it with `easyshot agent config set` or in Settings › Agents";

/// The outputs and windows the user lets agents capture; an empty list allows every one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    outputs: Vec<String>,
    windows: Vec<String>,
}

impl Scope {
    pub fn new(cfg: &AgentCfg) -> Scope {
        Scope {
            outputs: cfg.allowed_outputs.clone(),
            windows: cfg.allowed_windows.clone(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.outputs.is_empty() && self.windows.is_empty()
    }

    pub fn allows_output(&self, name: &str) -> bool {
        self.outputs.is_empty() || self.outputs.iter().any(|o| o == name)
    }

    /// A window on an output the compositor does not name is outside an output limit.
    pub fn allows_window_on(&self, app_id: Option<&str>, output: Option<&str>) -> bool {
        let app =
            self.windows.is_empty() || app_id.is_some_and(|a| self.windows.iter().any(|w| w == a));
        let on = self.outputs.is_empty() || output.is_some_and(|o| self.allows_output(o));
        app && on
    }

    pub fn allows_window(&self, w: &WindowInfo) -> bool {
        self.allows_window_on(Some(&w.app_id), w.output.as_ref().map(|o| o.0.as_str()))
    }

    /// Run again on the window a capture resolves to: this sees the windows of a moment earlier.
    pub fn check(
        &self,
        target: &AgentTarget,
        outputs: &[OutputInfo],
        windows: &[WindowInfo],
    ) -> Result<(), String> {
        if self.is_open() {
            return Ok(());
        }
        if let AgentTarget::Window(selector) = target {
            return match windows
                .iter()
                .filter(|w| matches(selector, w))
                .find(|w| !self.allows_window(w))
            {
                Some(w) => Err(format!(
                    "window {} ({}) is outside the windows agents may capture; {CHANGE_IT}",
                    w.id.0, w.app_id
                )),
                None => Ok(()),
            };
        }
        if !self.windows.is_empty() {
            return Err(format!(
                "agents may capture only these windows: {}; {CHANGE_IT}",
                self.windows.join(", ")
            ));
        }
        let outside = |o: &&OutputInfo| !self.allows_output(&o.id.0);
        let refused = match target {
            AgentTarget::Output(name) => {
                (!self.allows_output(name.as_str())).then(|| name.as_str().to_owned())
            }
            AgentTarget::Fullscreen => outputs.iter().find(outside).map(|o| o.id.0.clone()),
            AgentTarget::Geometry(g) => {
                let area = Rect::new(
                    f64::from(g.x),
                    f64::from(g.y),
                    f64::from(g.x) + f64::from(g.width),
                    f64::from(g.y) + f64::from(g.height),
                );
                let touched: Vec<&OutputInfo> = outputs
                    .iter()
                    .filter(|o| o.logical.intersect(area).area() > 0.0)
                    .collect();
                if touched.is_empty() {
                    Some(g.to_string())
                } else {
                    touched
                        .into_iter()
                        .find(|o| outside(o))
                        .map(|o| o.id.0.clone())
                }
            }
            AgentTarget::Window(_) => None,
        };
        match refused {
            Some(what) => Err(format!(
                "{what} is outside the outputs agents may capture ({}); {CHANGE_IT}",
                self.outputs.join(", ")
            )),
            None => Ok(()),
        }
    }

    /// Lists show an agent only what it may capture.
    pub fn filter(&self, reply: &mut AgentReply) {
        if let Some(outputs) = &mut reply.outputs {
            outputs.retain(|o| self.allows_output(&o.name));
        }
        if let Some(windows) = &mut reply.windows {
            windows.retain(|w| self.allows_window_on(w.app_id.as_deref(), w.output.as_deref()));
        }
    }
}

/// Wider than any capture's own matching, so every window it could pick is checked.
fn matches(selector: &WindowSelector, w: &WindowInfo) -> bool {
    match selector {
        WindowSelector::Id(id) => w.id.0 == *id,
        WindowSelector::AppId(app) => w.app_id.eq_ignore_ascii_case(app),
        WindowSelector::Title(title) => w.title.to_lowercase().contains(&title.to_lowercase()),
        WindowSelector::Any(q) => [
            WindowSelector::Id(q.clone()),
            WindowSelector::AppId(q.clone()),
            WindowSelector::Title(q.clone()),
        ]
        .iter()
        .any(|s| matches(s, w)),
    }
}

pub fn describe(target: &AgentTarget) -> String {
    match target {
        AgentTarget::Fullscreen => "fullscreen".to_owned(),
        AgentTarget::Output(name) => format!("output {name}"),
        AgentTarget::Geometry(g) => format!("geometry {g}"),
        AgentTarget::Window(WindowSelector::Id(id)) => format!("window {id}"),
        AgentTarget::Window(WindowSelector::AppId(app)) => format!("app {app}"),
        AgentTarget::Window(WindowSelector::Title(title)) => format!("title {title}"),
        AgentTarget::Window(WindowSelector::Any(q)) => format!("window {q}"),
    }
}

#[cfg(test)]
#[path = "agent_scope_tests.rs"]
mod tests;
