use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("HTTP error calling {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("template '{name}' not found in config")]
    TemplateNotFound { name: String },

    #[error("recipient group '{name}' not found in config")]
    RecipientGroupNotFound { name: String },

    #[error("job '{id}' not found in config")]
    JobNotFound { id: String },

    #[error("scheduler error: {0}")]
    Scheduler(String),

    #[error("job '{id}' failed after {attempts} retries: {reason}")]
    JobFailed {
        id: String,
        attempts: u32,
        reason: String,
    },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("config file not found; tried: {paths}")]
    NotFound { paths: String },
}
