use crate::config::schema::{Config, JobDefinition};
use crate::db;
use crate::error::{AppError, Result};
use crate::messaging::client::HttpMessagingClient;
use crate::messaging::provider::Provider;
use crate::messaging::template;
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

#[derive(Debug, Clone, Serialize)]
pub struct SendError {
    pub recipient: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct JobReport {
    pub job_id: String,
    pub sent: u32,
    pub failed: u32,
    pub errors: Vec<SendError>,
    pub duration_ms: u64,
}

pub struct JobExecutor {
    pub client: Arc<HttpMessagingClient>,
    pub provider: Arc<dyn Provider>,
    pub pool: SqlitePool,
}

impl JobExecutor {
    #[instrument(skip(self, config), fields(job_id = %job.id))]
    pub async fn execute(&self, job: &JobDefinition, config: &Config) -> Result<JobReport> {
        let started_at = Utc::now();
        let start = std::time::Instant::now();

        let tmpl_def = config
            .templates
            .iter()
            .find(|t| t.name == job.template)
            .ok_or_else(|| AppError::TemplateNotFound {
                name: job.template.clone(),
            })?;

        let recipient_group = config
            .recipients
            .iter()
            .find(|r| r.name == job.recipients)
            .ok_or_else(|| AppError::RecipientGroupNotFound {
                name: job.recipients.clone(),
            })?;

        let mut sent = 0u32;
        let mut errors: Vec<SendError> = Vec::new();

        for number in &recipient_group.numbers {
            let vars = job
                .variables
                .get(number)
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            let rendered = match template::render_for_recipient(tmpl_def, vars) {
                Ok(r) => r,
                Err(e) => {
                    warn!(recipient = number, error = %e, "failed to render template");
                    errors.push(SendError {
                        recipient: number.clone(),
                        reason: e.to_string(),
                    });
                    continue;
                }
            };

            match self.provider.send(&self.client, number, rendered).await {
                Ok(_) => {
                    sent += 1;
                    info!(recipient = number, "message sent");
                }
                Err(e) => {
                    error!(recipient = number, error = %e, "failed to send message");
                    errors.push(SendError {
                        recipient: number.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        let failed = errors.len() as u32;
        let finished_at = Utc::now();

        let errors_json = if errors.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&errors)?)
        };

        db::insert_execution(
            &self.pool,
            &job.id,
            started_at,
            finished_at,
            sent as i64,
            failed as i64,
            errors_json.as_deref(),
        )
        .await?;

        info!(
            job_id = %job.id,
            sent,
            failed,
            duration_ms,
            "job execution complete"
        );

        Ok(JobReport {
            job_id: job.id.clone(),
            sent,
            failed,
            errors,
            duration_ms,
        })
    }
}
