//! Agent configuration (TOML).

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub storage: StorageConfig,
    pub ippan: IppanConfig,
    #[serde(default)]
    pub events: EventsConfig,
    #[serde(default)]
    pub key: KeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: String,
    pub agent_type: String,
    pub plant_id: String,
    pub operator_key_ref: String,
    #[serde(default)]
    pub production_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub base_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IppanConfig {
    pub endpoint: String,
    #[serde(default = "default_anchor_path")]
    pub anchor_path: String,
    #[serde(default)]
    pub admin_token_env: Option<String>,
    #[serde(default)]
    pub submit_anchors: bool,
}

fn default_anchor_path() -> String {
    "/v1/anchors".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsConfig {
    #[serde(default = "default_lookback")]
    pub lookback_minutes: i64,
}

fn default_lookback() -> i64 {
    240
}

impl Default for EventsConfig {
    fn default() -> Self {
        EventsConfig {
            lookback_minutes: default_lookback(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyConfig {
    /// Path to the operator key JSON file. Optional in demo mode.
    pub key_file: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        let cfg: Config = toml::from_str(&text)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_example_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("c.toml");
        let text = r#"
[agent]
agent_id = "pv-agent-palermo-001"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"

[storage]
base_dir = "data/pv-agent"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240
"#;
        std::fs::write(&path, text).unwrap();
        let c = Config::load(&path).unwrap();
        assert_eq!(c.agent.plant_id, "palermo-pv-001");
        assert!(!c.ippan.submit_anchors);
        assert_eq!(c.events.lookback_minutes, 240);
    }

    #[test]
    fn defaults_apply() {
        let text = r#"
[agent]
agent_id = "a"
agent_type = "pv_plant_agent"
plant_id = "p"
operator_key_ref = "key:x"

[storage]
base_dir = "data"

[ippan]
endpoint = "http://127.0.0.1:18181"
"#;
        let c: Config = toml::from_str(text).unwrap();
        assert_eq!(c.ippan.anchor_path, "/v1/anchors");
        assert!(!c.ippan.submit_anchors);
        assert_eq!(c.events.lookback_minutes, 240);
    }
}
