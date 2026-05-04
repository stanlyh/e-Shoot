use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub api: ApiConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub recipients: Vec<RecipientGroup>,
    #[serde(default)]
    pub templates: Vec<TemplateDefinition>,
    #[serde(default)]
    pub jobs: Vec<JobDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    pub base_url: String,
    pub token: String,
    pub phone_number_id: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: LogFormat,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Pretty,
    Json,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RecipientGroup {
    pub name: String,
    pub numbers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateDefinition {
    pub name: String,
    pub language: String,
    #[serde(default)]
    pub components: Vec<ComponentDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentDefinition {
    #[serde(rename = "type")]
    pub kind: ComponentType,
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    Header,
    Body,
    Footer,
    Button,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParameterDefinition {
    #[serde(rename = "type")]
    pub kind: ParameterType,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterType {
    Text,
    Image,
    Document,
    Video,
    Currency,
    DateTime,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobDefinition {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub description: Option<String>,
    pub schedule: Option<String>,
    pub schedule_once: Option<DateTime<Utc>>,
    pub timezone: Option<String>,
    pub template: String,
    pub recipients: String,
    #[serde(default)]
    pub variables: HashMap<String, Vec<String>>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: LogFormat::Pretty,
            file: None,
        }
    }
}

fn default_connect_timeout() -> u64 { 5 }
fn default_request_timeout() -> u64 { 30 }
fn default_max_retries() -> u32 { 3 }
fn default_retry_backoff() -> u64 { 2 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> LogFormat { LogFormat::Pretty }
fn default_true() -> bool { true }
