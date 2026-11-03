use es_model::agent::MAX_LOG_LIMIT;
use es_model::{AgentCfg, Consent};
use es_protocol::agent::{AgentKey, OutputName};

const MAX_LIST: usize = 64;
const MAX_APP_ID: usize = 255;

pub fn get(cfg: &AgentCfg, key: AgentKey) -> String {
    match key {
        AgentKey::Enabled => cfg.enabled.to_string(),
        AgentKey::Consent => cfg.consent.name().to_owned(),
        AgentKey::AllowedOutputs => cfg.allowed_outputs.join(","),
        AgentKey::AllowedWindows => cfg.allowed_windows.join(","),
        AgentKey::RedactSecrets => cfg.redact_secrets.to_string(),
        AgentKey::LogLimit => cfg.log_limit.to_string(),
    }
}

/// Leaves `cfg` untouched when the value is refused.
pub fn set(cfg: &mut AgentCfg, key: AgentKey, value: &str) -> Result<(), String> {
    let value = value.trim();
    match key {
        AgentKey::Enabled => cfg.enabled = flag(value)?,
        AgentKey::RedactSecrets => cfg.redact_secrets = flag(value)?,
        AgentKey::Consent => {
            cfg.consent = Consent::from_name(value).ok_or_else(|| {
                let names: Vec<&str> = Consent::ALL.iter().map(|c| c.name()).collect();
                format!("consent is one of {}", names.join(", "))
            })?;
        }
        AgentKey::AllowedOutputs => {
            cfg.allowed_outputs =
                list(value, |o| OutputName::new(o).map(|n| n.as_str().to_owned()))?;
        }
        AgentKey::AllowedWindows => {
            cfg.allowed_windows = list(value, |a| {
                if a.len() > MAX_APP_ID {
                    return Err(format!("an app id has at most {MAX_APP_ID} bytes"));
                }
                Ok(a.to_owned())
            })?;
        }
        AgentKey::LogLimit => {
            cfg.log_limit = value
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=MAX_LOG_LIMIT).contains(n))
                .ok_or_else(|| format!("log_limit is a whole number from 1 to {MAX_LOG_LIMIT}"))?;
        }
    }
    Ok(())
}

fn flag(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err("write true or false".to_owned()),
    }
}

/// Comma-separated; an empty value means every one.
fn list(value: &str, item: impl Fn(&str) -> Result<String, String>) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let part = item(part)?;
        if !out.contains(&part) {
            out.push(part);
        }
    }
    if out.len() > MAX_LIST {
        return Err(format!("at most {MAX_LIST} names"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_reads_back_what_was_set() {
        let mut cfg = AgentCfg::default();
        for (key, value) in [
            (AgentKey::Enabled, "true"),
            (AgentKey::Consent, "always-ask"),
            (AgentKey::AllowedOutputs, "DP-1,HDMI-A-1"),
            (AgentKey::AllowedWindows, "firefox,org.gnome.Nautilus"),
            (AgentKey::RedactSecrets, "false"),
            (AgentKey::LogLimit, "50"),
        ] {
            set(&mut cfg, key, value).unwrap();
            assert_eq!(get(&cfg, key), value, "{key:?}");
        }
        assert!(cfg.enabled && !cfg.redact_secrets);
    }

    #[test]
    fn lists_are_trimmed_deduplicated_and_empty_means_all() {
        let mut cfg = AgentCfg::default();
        set(
            &mut cfg,
            AgentKey::AllowedOutputs,
            " DP-1 , DP-1,, HDMI-A-1 ",
        )
        .unwrap();
        assert_eq!(cfg.allowed_outputs, ["DP-1", "HDMI-A-1"]);
        set(&mut cfg, AgentKey::AllowedOutputs, "").unwrap();
        assert!(cfg.allowed_outputs.is_empty());
    }

    #[test]
    fn a_refused_value_changes_nothing() {
        let mut cfg = AgentCfg::default();
        for (key, value) in [
            (AgentKey::Enabled, "yes"),
            (AgentKey::Consent, "never"),
            (AgentKey::AllowedOutputs, "DP-1,../x"),
            (AgentKey::LogLimit, "0"),
            (AgentKey::LogLimit, "10001"),
            (AgentKey::LogLimit, "-1"),
        ] {
            assert!(set(&mut cfg, key, value).is_err(), "{key:?} {value}");
        }
        assert_eq!(cfg, AgentCfg::default());
    }
}
