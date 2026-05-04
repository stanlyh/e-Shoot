use crate::error::Result;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};
use std::path::Path;

pub async fn connect(db_path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;
    Ok(pool)
}

pub async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

// ── Job execution history ────────────────────────────────────────────────────

pub struct ExecutionRecord {
    pub id: i64,
    pub job_id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub sent: i64,
    pub failed: i64,
    pub errors_json: Option<String>,
}

pub async fn insert_execution(
    pool: &SqlitePool,
    job_id: &str,
    started_at: DateTime<Utc>,
    finished_at: DateTime<Utc>,
    sent: i64,
    failed: i64,
    errors_json: Option<&str>,
) -> Result<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO job_executions (job_id, started_at, finished_at, sent, failed, errors_json)
         VALUES (?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(job_id)
    .bind(started_at.to_rfc3339())
    .bind(finished_at.to_rfc3339())
    .bind(sent)
    .bind(failed)
    .bind(errors_json)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn list_executions(
    pool: &SqlitePool,
    job_id: Option<&str>,
    limit: i64,
) -> Result<Vec<ExecutionRecord>> {
    let rows = if let Some(jid) = job_id {
        sqlx::query(
            "SELECT id, job_id, started_at, finished_at, sent, failed, errors_json
             FROM job_executions
             WHERE job_id = ?
             ORDER BY started_at DESC
             LIMIT ?",
        )
        .bind(jid)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            "SELECT id, job_id, started_at, finished_at, sent, failed, errors_json
             FROM job_executions
             ORDER BY started_at DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    rows.into_iter()
        .map(|row| {
            let started_str: String = row.try_get("started_at")?;
            let finished_str: String = row.try_get("finished_at")?;
            let started_at = started_str
                .parse::<DateTime<Utc>>()
                .unwrap_or_default();
            let finished_at = finished_str
                .parse::<DateTime<Utc>>()
                .unwrap_or_default();
            Ok(ExecutionRecord {
                id: row.try_get("id")?,
                job_id: row.try_get("job_id")?,
                started_at,
                finished_at,
                sent: row.try_get("sent")?,
                failed: row.try_get("failed")?,
                errors_json: row.try_get("errors_json")?,
            })
        })
        .collect()
}

// ── One-shot job tracking ────────────────────────────────────────────────────

pub async fn mark_once_job_fired(pool: &SqlitePool, job_id: &str) -> Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO fired_once_jobs (job_id, fired_at) VALUES (?, ?)",
    )
    .bind(job_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn is_once_job_fired(pool: &SqlitePool, job_id: &str) -> Result<bool> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM fired_once_jobs WHERE job_id = ?")
            .bind(job_id)
            .fetch_one(pool)
            .await?;
    Ok(count > 0)
}
