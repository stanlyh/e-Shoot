pub mod schema;
mod validation;

pub use schema::*;

use crate::error::ConfigError;
use std::path::{Path, PathBuf};

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut config: Config = toml::from_str(&content)?;

    // Allow token override via environment variable
    if let Ok(token) = std::env::var("E_SHOOT_API_TOKEN") {
        config.api.token = token;
    }

    validation::validate(&config)?;
    Ok(config)
}

pub fn resolve_path(override_path: Option<&PathBuf>) -> Result<PathBuf, ConfigError> {
    if let Some(p) = override_path {
        return Ok(p.clone());
    }

    let mut candidates = Vec::new();

    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("e-shoot").join("config.toml"));
    }
    candidates.push(PathBuf::from("config.toml"));

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    let tried = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    Err(ConfigError::NotFound { paths: tried })
}
