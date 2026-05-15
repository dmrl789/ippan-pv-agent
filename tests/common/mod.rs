use ippan_pv_agent::config::{
    AgentConfig, Config, EventsConfig, IppanConfig, KeyConfig, StorageConfig,
};
use std::path::Path;

#[allow(dead_code)]
pub fn make_config(base_dir: &Path) -> Config {
    Config {
        agent: AgentConfig {
            agent_id: "pv-agent-palermo-001".into(),
            agent_type: "pv_plant_agent".into(),
            plant_id: "palermo-pv-001".into(),
            operator_key_ref: "key:plant-palermo-001".into(),
            production_mode: false,
        },
        storage: StorageConfig {
            base_dir: base_dir.to_string_lossy().into_owned(),
        },
        ippan: IppanConfig {
            endpoint: "http://127.0.0.1:1".into(),
            anchor_path: "/v1/anchors".into(),
            admin_token_env: Some("IPPAN_ADMIN_TOKEN".into()),
            submit_anchors: false,
        },
        events: EventsConfig {
            lookback_minutes: 240,
        },
        key: KeyConfig { key_file: None },
    }
}
