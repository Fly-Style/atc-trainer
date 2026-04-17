pub const TEST_TRAINER_HASH: &str = "test-trainer-hash";

pub const SCENARIO_GND_36_MEDIUM: &str = include_str!("../../../docs/scenario-gnd-36-medium.toml");

pub fn base_config() -> atc_server::ServerConfig {
    atc_server::ServerConfig {
        trainer_hashes: vec![TEST_TRAINER_HASH.to_string()],
        log_dir: None,
        builtin_scenario_toml: Some(SCENARIO_GND_36_MEDIUM.to_string()),
    }
}
