use std::path::PathBuf;

/// Static server configuration. Phase 1 keeps this minimal.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Accepted trainer magic hash values. Any inbound hash must match one of these.
    pub trainer_hashes: Vec<String>,
    /// Directory where session logs are written. `None` disables file logging.
    pub log_dir: Option<PathBuf>,
    /// Builtin scenario loaded automatically when creating sessions.
    pub builtin_scenario_toml: Option<String>,
}

impl ServerConfig {
    pub fn test_default() -> Self {
        Self {
            trainer_hashes: vec!["test-trainer-hash".into()],
            log_dir: None,
            builtin_scenario_toml: None,
        }
    }
}
