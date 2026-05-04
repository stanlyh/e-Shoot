use crate::config::schema::{Config, JobDefinition};
use crate::db;
use crate::error::{AppError, Result};
use crate::executor::JobExecutor;
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, warn};

pub struct SchedulerService {
    inner: JobScheduler,
    executor: Arc<JobExecutor>,
    config: Arc<Config>,
    pool: SqlitePool,
}

impl SchedulerService {
    pub async fn new(config: Arc<Config>, executor: Arc<JobExecutor>, pool: SqlitePool) -> Result<Self> {
        let inner = JobScheduler::new()
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;
        Ok(Self { inner, executor, config, pool })
    }

    pub async fn register_all_jobs(&self) -> Result<()> {
        let enabled: Vec<&JobDefinition> = self
            .config
            .jobs
            .iter()
            .filter(|j| j.enabled)
            .collect();

        info!(count = enabled.len(), "registering jobs");

        for job_def in enabled {
            if let Some(cron_expr) = &job_def.schedule {
                self.register_cron_job(job_def, cron_expr).await?;
            } else if let Some(fire_at) = job_def.schedule_once {
                if db::is_once_job_fired(&self.pool, &job_def.id).await? {
                    info!(job_id = %job_def.id, "skipping already-fired once job");
                    continue;
                }
                let now = Utc::now();
                if fire_at <= now {
                    warn!(job_id = %job_def.id, scheduled_at = %fire_at, "once job is in the past, skipping");
                    continue;
                }
                self.register_once_job(job_def, fire_at).await?;
            }
        }
        Ok(())
    }

    async fn register_cron_job(&self, job_def: &JobDefinition, cron_expr: &str) -> Result<()> {
        let executor = Arc::clone(&self.executor);
        let config = Arc::clone(&self.config);
        let job_id = job_def.id.clone();
        let cron = cron_expr.to_string();

        let job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
            let executor = Arc::clone(&executor);
            let config = Arc::clone(&config);
            let job_id = job_id.clone();
            Box::pin(async move {
                let Some(job_def) = config.jobs.iter().find(|j| j.id == job_id) else {
                    error!(job_id, "job not found in config during execution");
                    return;
                };
                if let Err(e) = executor.execute(job_def, &config).await {
                    error!(job_id, error = %e, "job execution failed");
                }
            })
        })
        .map_err(|e| AppError::Scheduler(e.to_string()))?;

        self.inner
            .add(job)
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;

        info!(job_id = %job_def.id, cron = cron_expr, "registered cron job");
        Ok(())
    }

    async fn register_once_job(
        &self,
        job_def: &JobDefinition,
        fire_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let executor = Arc::clone(&self.executor);
        let config = Arc::clone(&self.config);
        let pool = self.pool.clone();
        let job_id = job_def.id.clone();

        let delay = (fire_at - Utc::now())
            .to_std()
            .unwrap_or(std::time::Duration::ZERO);
        let instant = std::time::Instant::now() + delay;

        let job = Job::new_one_shot_at_instant_async(
            instant,
            move |_uuid, _lock| {
                let executor = Arc::clone(&executor);
                let config = Arc::clone(&config);
                let pool = pool.clone();
                let job_id = job_id.clone();
                Box::pin(async move {
                    let Some(job_def) = config.jobs.iter().find(|j| j.id == job_id) else {
                        error!(job_id, "once job not found in config");
                        return;
                    };
                    if let Err(e) = executor.execute(job_def, &config).await {
                        error!(job_id, error = %e, "once job execution failed");
                        return;
                    }
                    if let Err(e) = db::mark_once_job_fired(&pool, &job_id).await {
                        error!(job_id, error = %e, "failed to mark once job as fired");
                    }
                })
            },
        )
        .map_err(|e| AppError::Scheduler(e.to_string()))?;

        self.inner
            .add(job)
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;

        info!(job_id = %job_def.id, fire_at = %fire_at, "registered one-shot job");
        Ok(())
    }

    pub async fn run_job_now(&self, job_id: &str) -> Result<()> {
        let job_def = self
            .config
            .jobs
            .iter()
            .find(|j| j.id == job_id)
            .ok_or_else(|| AppError::JobNotFound { id: job_id.to_string() })?;

        info!(job_id, "running job immediately");
        self.executor.execute(job_def, &self.config).await?;
        Ok(())
    }

    pub async fn start(mut self) -> Result<()> {
        self.inner
            .start()
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;

        info!("scheduler running — press Ctrl+C to stop");

        tokio::signal::ctrl_c()
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;

        info!("shutdown signal received, stopping scheduler");

        self.inner
            .shutdown()
            .await
            .map_err(|e| AppError::Scheduler(e.to_string()))?;

        Ok(())
    }
}
