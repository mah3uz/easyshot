use super::Ipc;
use crate::error::{Error, Result};
use crate::traits::LayerFade;
use serde::Deserialize;
use std::time::Duration;

/// Each leaf with the parents it inherits from when it is not set itself.
const LAYER_CLOSE: [&[&str]; 2] = [
    &["layersOut", "layers", "global"],
    &["fadeLayersOut", "fadeLayers", "fade", "global"],
];

/// Hyprland's animation speed is in tenths of a second.
const SPEED_UNIT_MS: f64 = 100.0;

#[derive(Debug, Clone, Deserialize)]
pub struct Animation {
    pub name: String,
    #[serde(default)]
    pub overridden: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub speed: f64,
}

fn leaf(animations: &[Animation], chain: &[&str]) -> Duration {
    let set = chain
        .iter()
        .find_map(|name| animations.iter().find(|a| a.name == *name && a.overridden));
    match set {
        Some(a) if a.enabled && a.speed.is_finite() && a.speed > 0.0 => {
            Duration::from_secs_f64(a.speed * SPEED_UNIT_MS / 1000.0)
        }
        _ => Duration::ZERO,
    }
}

/// How long a closed layer surface keeps being drawn: its fade and its slide or pop run side by side.
pub fn layer_close(animations_on: bool, animations: &[Animation]) -> Duration {
    if !animations_on {
        return Duration::ZERO;
    }
    LAYER_CLOSE
        .iter()
        .map(|chain| leaf(animations, chain))
        .max()
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct Toggle {
    #[serde(default)]
    bool: Option<bool>,
    #[serde(default)]
    int: Option<i64>,
}

pub fn parse(option: &[u8], animations: &[u8]) -> Result<Duration> {
    let bad = |what: &str, e: serde_json::Error| Error::Protocol(format!("hyprland {what}: {e}"));
    let toggle: Toggle = serde_json::from_slice(option).map_err(|e| bad("getoption", e))?;
    let on = toggle.bool.or(toggle.int.map(|i| i != 0)).unwrap_or(true);
    let (list, _beziers): (Vec<Animation>, serde_json::Value) =
        serde_json::from_slice(animations).map_err(|e| bad("animations", e))?;
    Ok(layer_close(on, &list))
}

pub struct HyprFade {
    pub ipc: Ipc,
}

impl LayerFade for HyprFade {
    fn linger(&self) -> Result<Duration> {
        let option = self.ipc.request("j/getoption animations:enabled")?;
        let animations = self.ipc.request("j/animations")?;
        parse(&option, &animations)
    }
}
