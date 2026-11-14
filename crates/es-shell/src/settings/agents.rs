use super::SettingsWindow;
use super::widgets::{FIELD_WIDTH, caption, group, row, switch};
use crate::local_time::local_clock;
use crate::strings::fl;
use es_app::agent_access::{AccessEvent, AgentAccess, AgentAccessHandle, Grant};
use es_model::{ActionRecord, Config, Consent};
use es_ui::components::{ButtonKind, EditorButton};
use es_ui::theme::tokens;
use es_ui::tokens::Typeset;
use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, ParentElement, SharedString, Styled,
    Subscription, Window, div, px,
};
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
};

const LOG_SHOWN: usize = 20;

pub(super) struct Agents {
    outputs: Entity<InputState>,
    windows: Entity<InputState>,
    access: Option<Entity<AgentAccess>>,
    _subscriptions: Vec<Subscription>,
}

/// Comma-separated names, trimmed and without repeats; none means every one.
fn names(value: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in value.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    }
    out
}

/// The grant in force, in words, for Settings and the tray.
pub fn grant_text(grant: Option<Grant>) -> String {
    match grant {
        Some(Grant::Until(t)) => fl!("agents-grant-until", time = local_clock(t, false)),
        Some(Grant::Session) => fl!("agents-grant-session"),
        Some(Grant::Always(_)) => fl!("agents-grant-always"),
        None => fl!("agents-grant-none"),
    }
}

impl Agents {
    pub(super) fn follow(
        &self,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) {
        for (input, values) in [
            (&self.outputs, &config.agent.allowed_outputs),
            (&self.windows, &config.agent.allowed_windows),
        ] {
            if names(&input.read(cx).value()) != *values {
                let joined = values.join(", ");
                input.update(cx, |state, cx| state.set_value(joined, window, cx));
            }
        }
    }

    pub(super) fn new(
        config: &Config,
        window: &mut Window,
        cx: &mut Context<SettingsWindow>,
    ) -> Self {
        let access = cx.try_global::<AgentAccessHandle>().map(|h| h.0.clone());
        let mut subs = Vec::new();
        let mut list =
            |values: &[String], placeholder: String, set: fn(&mut Config, Vec<String>)| {
                let joined = values.join(", ");
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(placeholder)
                        .default_value(joined)
                });
                subs.push(
                    cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
                        if let InputEvent::Change = event {
                            let value = names(&input.read(cx).value());
                            this.edit(move |c| set(c, value), cx);
                        }
                    }),
                );
                input
            };
        let outputs = list(
            &config.agent.allowed_outputs,
            fl!("agents-scope-outputs-placeholder"),
            |c, v| c.agent.allowed_outputs = v,
        );
        let windows = list(
            &config.agent.allowed_windows,
            fl!("agents-scope-windows-placeholder"),
            |c, v| c.agent.allowed_windows = v,
        );
        if let Some(access) = &access {
            subs.push(cx.subscribe(access, |_, _, _: &AccessEvent, cx| cx.notify()));
        }
        Agents {
            outputs,
            windows,
            access,
            _subscriptions: subs,
        }
    }
}

fn set_consent(c: &mut Config, v: Consent) {
    c.agent.consent = v;
}

fn result_text(result: &str) -> String {
    match result {
        "ok" => fl!("agents-log-ok"),
        "access_denied" => fl!("agents-log-denied"),
        "access_off" => fl!("agents-log-off"),
        other => other.replace('_', " "),
    }
}

impl SettingsWindow {
    pub fn set_agent_enabled(&mut self, on: bool, cx: &mut Context<Self>) {
        self.edit(move |c| c.agent.enabled = on, cx);
        if !on && let Some(access) = &self.agents.access {
            access.update(cx, |access, cx| access.revoke(cx));
        }
    }

    pub fn set_agent_consent(&mut self, consent: Consent, cx: &mut Context<Self>) {
        self.edit(move |c| set_consent(c, consent), cx);
    }

    pub fn set_agent_redact(&mut self, on: bool, cx: &mut Context<Self>) {
        self.edit(move |c| c.agent.redact_secrets = on, cx);
    }

    pub fn set_agent_keep_captures(&mut self, on: bool, cx: &mut Context<Self>) {
        self.edit(move |c| c.agent.keep_captures = on, cx);
    }

    /// The allowed-outputs and allowed-windows fields.
    pub fn agent_scope_fields(&self) -> (Entity<InputState>, Entity<InputState>) {
        (self.agents.outputs.clone(), self.agents.windows.clone())
    }

    pub(super) fn render_agents(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let c = self.config(cx);
        let a = &c.agent;
        let on = a.enabled;
        let redact = a.redact_secrets;
        let keep = a.keep_captures;
        let grant = self
            .agents
            .access
            .as_ref()
            .and_then(|access| access.read(cx).grant());
        let log: Vec<ActionRecord> = self
            .agents
            .access
            .as_ref()
            .map(|access| {
                access
                    .read(cx)
                    .log()
                    .iter()
                    .take(LOG_SHOWN)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let access = self.agents.access.clone();
        let revoke = EditorButton::new("agents-revoke")
            .label(fl!("agents-revoke"))
            .kind(ButtonKind::Bordered)
            .disabled(grant.is_none())
            .on_click(move |_, _, cx| {
                if let Some(access) = &access {
                    access.update(cx, |access, cx| access.revoke(cx));
                }
            });
        let field = |input: &Entity<InputState>| Input::new(input).w(px(FIELD_WIDTH));
        let mut out = vec![
            group(
                None,
                vec![row(
                    fl!("agents-enabled"),
                    Some(fl!("agents-enabled-help").into()),
                    switch(
                        "agents-enabled",
                        fl!("agents-enabled"),
                        on,
                        cx.listener(|this, on: &bool, _, cx| this.set_agent_enabled(*on, cx)),
                    ),
                    cx,
                )],
                None,
                cx,
            )
            .into_any_element(),
            group(
                Some(fl!("agents-asking").into()),
                vec![
                    self.choice_row(
                        "agents-consent",
                        fl!("agents-consent"),
                        Some(fl!("agents-consent-help")),
                        vec![
                            (Consent::OncePerSession, fl!("agents-consent-once")),
                            (Consent::AlwaysAsk, fl!("agents-consent-always-ask")),
                            (Consent::AlwaysAllow, fl!("agents-consent-never-ask")),
                        ],
                        &a.consent,
                        set_consent,
                        cx,
                    ),
                    row(
                        grant_text(grant),
                        Some(fl!("agents-grant-help").into()),
                        revoke,
                        cx,
                    ),
                ],
                None,
                cx,
            )
            .into_any_element(),
            group(
                Some(fl!("agents-scope").into()),
                vec![
                    row(
                        fl!("agents-scope-outputs"),
                        None,
                        field(&self.agents.outputs),
                        cx,
                    ),
                    row(
                        fl!("agents-scope-windows"),
                        None,
                        field(&self.agents.windows),
                        cx,
                    ),
                    row(
                        fl!("agents-redact"),
                        Some(fl!("agents-redact-help").into()),
                        switch(
                            "agents-redact",
                            fl!("agents-redact"),
                            redact,
                            cx.listener(|this, on: &bool, _, cx| this.set_agent_redact(*on, cx)),
                        ),
                        cx,
                    ),
                    row(
                        fl!("agents-keep-captures"),
                        Some(fl!("agents-keep-captures-help").into()),
                        switch(
                            "agents-keep-captures",
                            fl!("agents-keep-captures"),
                            keep,
                            cx.listener(|this, on: &bool, _, cx| {
                                this.set_agent_keep_captures(*on, cx)
                            }),
                        ),
                        cx,
                    ),
                ],
                Some(fl!("agents-scope-footer").into()),
                cx,
            )
            .into_any_element(),
        ];
        let t = tokens(cx);
        let rows: Vec<AnyElement> = if log.is_empty() {
            vec![row(fl!("agents-log-empty"), None, div(), cx)]
        } else {
            log.iter()
                .map(|e| {
                    let program = match (&e.comm, e.pid) {
                        (Some(comm), _) => comm.clone(),
                        (None, Some(pid)) => fl!("agents-log-pid", pid = pid),
                        (None, None) => fl!("agents-log-unknown"),
                    };
                    let what = e.route.trim_start_matches("agent/").to_owned();
                    let what = match &e.target {
                        Some(target) => format!("{what} · {target}"),
                        None => what,
                    };
                    let time = local_clock(e.at, true);
                    h_flex()
                        .px(px(16.))
                        .py(px(8.))
                        .gap(px(12.))
                        .typeset(t.type_scale.footer)
                        .child(
                            div()
                                .w(px(64.))
                                .flex_none()
                                .text_color(t.text_secondary)
                                .typeset(t.type_scale.value)
                                .text_size(t.type_scale.footer.size)
                                .child(time),
                        )
                        .child(div().w(px(110.)).flex_none().truncate().child(program))
                        .child(div().flex_1().min_w_0().truncate().child(what))
                        .child(
                            div()
                                .flex_none()
                                .text_color(if e.result == "ok" {
                                    t.text_secondary
                                } else {
                                    t.destructive
                                })
                                .child(result_text(&e.result)),
                        )
                        .into_any_element()
                })
                .collect()
        };
        out.push(
            group(
                Some(fl!("agents-log").into()),
                rows,
                Some(SharedString::from(fl!("agents-log-footer"))),
                cx,
            )
            .into_any_element(),
        );
        out.push(
            caption(fl!("agents-cli-help"), cx)
                .px(px(4.))
                .into_any_element(),
        );
        out
    }
}
