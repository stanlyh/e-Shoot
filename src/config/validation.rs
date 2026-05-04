use super::schema::Config;
use crate::error::ConfigError;

pub fn validate(config: &Config) -> Result<(), ConfigError> {
    let template_names: Vec<&str> = config.templates.iter().map(|t| t.name.as_str()).collect();
    let recipient_names: Vec<&str> = config.recipients.iter().map(|r| r.name.as_str()).collect();

    for job in &config.jobs {
        if job.schedule.is_none() && job.schedule_once.is_none() {
            return Err(ConfigError::Validation(format!(
                "job '{}' must have either 'schedule' or 'schedule_once'",
                job.id
            )));
        }
        if job.schedule.is_some() && job.schedule_once.is_some() {
            return Err(ConfigError::Validation(format!(
                "job '{}' cannot have both 'schedule' and 'schedule_once'",
                job.id
            )));
        }
        if let Some(cron_expr) = &job.schedule {
            validate_cron(cron_expr, &job.id)?;
        }
        if !template_names.contains(&job.template.as_str()) {
            return Err(ConfigError::Validation(format!(
                "job '{}' references unknown template '{}'",
                job.id, job.template
            )));
        }
        if !recipient_names.contains(&job.recipients.as_str()) {
            return Err(ConfigError::Validation(format!(
                "job '{}' references unknown recipient group '{}'",
                job.id, job.recipients
            )));
        }
    }

    let mut job_ids: Vec<&str> = Vec::new();
    for job in &config.jobs {
        if job_ids.contains(&job.id.as_str()) {
            return Err(ConfigError::Validation(format!(
                "duplicate job id '{}'",
                job.id
            )));
        }
        job_ids.push(&job.id);
    }

    Ok(())
}

fn validate_cron(expr: &str, job_id: &str) -> Result<(), ConfigError> {
    // tokio-cron-scheduler uses cron crate format
    // 6-field: sec min hour dom month dow
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 6 {
        return Err(ConfigError::Validation(format!(
            "job '{}' has invalid cron expression '{}': expected 6 fields (sec min hour dom month dow), got {}",
            job_id, expr, parts.len()
        )));
    }
    Ok(())
}
