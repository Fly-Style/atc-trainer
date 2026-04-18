use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Static server configuration. Phase 1 keeps this minimal.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Socket address to bind the axum server to.
    pub bind_address: String,
    /// Accepted trainer magic hash values. Any inbound hash must match one of these.
    pub trainer_hashes: Vec<String>,
    /// Directory where session logs are written. `None` disables file logging.
    pub log_dir: Option<PathBuf>,
    /// Builtin scenario loaded automatically when creating sessions.
    pub builtin_scenario_toml: Option<String>,
}

impl ServerConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let parsed: ServerConfigFile = toml::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        parsed.into_runtime(base_dir)
    }

    pub fn test_default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".into(),
            trainer_hashes: vec!["test-trainer-hash".into()],
            log_dir: None,
            builtin_scenario_toml: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerConfigFile {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default)]
    pub trainer_hashes: Vec<String>,
    pub log_dir: Option<PathBuf>,
    pub builtin_scenario_path: Option<PathBuf>,
}

impl ServerConfigFile {
    fn into_runtime(self, base_dir: PathBuf) -> Result<ServerConfig, ConfigError> {
        let builtin_scenario_toml = match self.builtin_scenario_path {
            Some(path) => {
                let resolved = resolve_path(&base_dir, path);
                Some(fs::read_to_string(&resolved).map_err(|source| ConfigError::Read {
                    path: resolved,
                    source,
                })?)
            }
            None => None,
        };

        Ok(ServerConfig {
            bind_address: self.bind_address,
            trainer_hashes: self.trainer_hashes,
            log_dir: self.log_dir.map(|path| resolve_path(&base_dir, path)),
            builtin_scenario_toml,
        })
    }
}

fn default_bind_address() -> String {
    "127.0.0.1:8080".into()
}

fn resolve_path(base_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}
