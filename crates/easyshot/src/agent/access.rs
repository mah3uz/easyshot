use clap::Subcommand;
use clap::builder::{PossibleValuesParser, TypedValueParser};
use es_protocol::agent::{AccessStatus, AgentKey, ManageRoute};
use std::io::Write;

fn key_parser() -> impl TypedValueParser<Value = AgentKey> {
    PossibleValuesParser::new(AgentKey::ALL.map(AgentKey::name))
        .try_map(|name| AgentKey::from_name(&name).ok_or("not an agent setting"))
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Print one setting
    Get {
        #[arg(value_parser = key_parser())]
        key: AgentKey,
    },
    /// Change one setting; lists are comma-separated and empty means all
    Set {
        #[arg(value_parser = key_parser())]
        key: AgentKey,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
}

impl ConfigCommand {
    pub fn route(&self) -> ManageRoute {
        match self {
            ConfigCommand::Get { key } => ManageRoute::ConfigGet(*key),
            ConfigCommand::Set { key, value } => ManageRoute::ConfigSet(*key, value.clone()),
        }
    }
}

fn list(names: &[String]) -> String {
    if names.is_empty() {
        "all".to_owned()
    } else {
        names.join(", ")
    }
}

pub fn print_status(s: &AccessStatus, out: &mut dyn Write) {
    let on = |b: bool| if b { "on" } else { "off" };
    let grant = match &s.grant {
        None => "none".to_owned(),
        Some(g) => {
            let span = serde_json::to_value(g.span)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            match &g.expires {
                Some(t) => format!("{span} until {t}"),
                None => span,
            }
        }
    };
    let _ = writeln!(out, "agent access\t{}", on(s.enabled));
    let _ = writeln!(out, "consent\t{}", s.consent);
    let _ = writeln!(out, "grant\t{grant}");
    let _ = writeln!(out, "outputs\t{}", list(&s.allowed_outputs));
    let _ = writeln!(out, "windows\t{}", list(&s.allowed_windows));
    let _ = writeln!(out, "redact secrets\t{}", on(s.redact_secrets));
    let _ = writeln!(out, "keep captures\t{}", on(s.keep_captures));
    let _ = writeln!(out, "recent actions\t{}", s.recent_actions);
}
