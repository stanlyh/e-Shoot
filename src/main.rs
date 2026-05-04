mod cli;
mod config;
mod db;
mod error;
mod executor;
mod init;
mod messaging;
mod scheduler;

use crate::cli::{Cli, Commands};
use crate::config::schema::LogFormat;
use crate::messaging::client::HttpMessagingClient;
use crate::messaging::provider::whatsapp::WhatsAppProvider;
use crate::scheduler::SchedulerService;
use anyhow::Context;
use clap::Parser;
use executor::JobExecutor;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // `init` creates the config file — handle it before trying to load one
    if let Commands::Init { output } = cli.command {
        return init::run(output.or(cli.config)).map_err(Into::into);
    }

    let config_path =
        config::resolve_path(cli.config.as_ref()).context("finding config file")?;

    let cfg = Arc::new(config::load(&config_path).context("loading config")?);

    init_tracing(&cfg.logging.level, &cfg.logging.format);

    info!(config = %config_path.display(), "config loaded");

    let db_path = resolve_db_path(cli.db.as_ref());
    std::fs::create_dir_all(db_path.parent().unwrap_or(std::path::Path::new(".")))?;
    let pool = db::connect(&db_path).await.context("opening database")?;
    db::migrate(&pool).await.context("running migrations")?;

    info!(db = %db_path.display(), "database ready");

    let http_client = Arc::new(
        HttpMessagingClient::new(
            cfg.api.base_url.clone(),
            cfg.api.token.clone(),
            cfg.api.connect_timeout_secs,
            cfg.api.request_timeout_secs,
            cfg.api.max_retries,
            cfg.api.retry_backoff_secs,
        )
        .context("building HTTP client")?,
    );

    let provider = Arc::new(WhatsAppProvider {
        phone_number_id: cfg.api.phone_number_id.clone(),
    });

    let executor = Arc::new(JobExecutor {
        client: Arc::clone(&http_client),
        provider,
        pool: pool.clone(),
    });

    match cli.command {
        Commands::Start => {
            let svc =
                SchedulerService::new(Arc::clone(&cfg), Arc::clone(&executor), pool).await?;
            svc.register_all_jobs().await?;
            info!(jobs = cfg.jobs.len(), "all jobs registered");
            svc.start().await?;
        }

        Commands::RunNow { job_id } => {
            let svc =
                SchedulerService::new(Arc::clone(&cfg), Arc::clone(&executor), pool).await?;
            let sp = indicatif::ProgressBar::new_spinner();
            sp.set_message(format!("running job '{}'...", job_id));
            sp.enable_steady_tick(std::time::Duration::from_millis(100));
            svc.run_job_now(&job_id).await?;
            sp.finish_with_message(format!("job '{}' done", job_id));
        }

        Commands::Check => {
            cmd_check(&cfg);
        }

        Commands::List => {
            cmd_list(&cfg);
        }

        Commands::History { job_id, limit } => {
            cmd_history(&pool, job_id.as_deref(), limit).await?;
        }

        Commands::ShowConfig => {
            cmd_show_config(&cfg)?;
        }

        Commands::Init { .. } => unreachable!("handled above"),
    }

    Ok(())
}

fn resolve_db_path(override_path: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = override_path {
        return p.clone();
    }
    if let Some(data_dir) = dirs::data_local_dir() {
        return data_dir.join("e-shoot").join("e-shoot.db");
    }
    PathBuf::from("e-shoot.db")
}

fn init_tracing(level: &str, format: &LogFormat) {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    match format {
        LogFormat::Pretty => {
            fmt().with_env_filter(filter).init();
        }
        LogFormat::Json => {
            fmt().json().with_env_filter(filter).init();
        }
    }
}

fn cmd_check(cfg: &config::Config) {
    println!("Config OK");
    println!("  API base URL : {}", cfg.api.base_url);
    println!("  Phone ID     : {}", cfg.api.phone_number_id);
    println!("  Templates    : {}", cfg.templates.len());
    println!("  Recipients   : {}", cfg.recipients.len());
    println!("  Jobs         : {}", cfg.jobs.len());
    for job in &cfg.jobs {
        let sched = if let Some(c) = &job.schedule {
            c.clone()
        } else if let Some(d) = &job.schedule_once {
            d.to_string()
        } else {
            "???".to_string()
        };
        let status = if job.enabled { "enabled" } else { "disabled" };
        println!(
            "    [{status}] {} — {} — template: {}",
            job.id, sched, job.template
        );
    }
}

fn cmd_list(cfg: &config::Config) {
    println!(
        "{:<30} {:<10} {:<35} {}",
        "JOB ID", "STATUS", "SCHEDULE", "TEMPLATE"
    );
    println!("{}", "-".repeat(90));
    for job in &cfg.jobs {
        let sched = if let Some(c) = &job.schedule {
            c.clone()
        } else if let Some(d) = &job.schedule_once {
            format!("once @ {}", d.format("%Y-%m-%d %H:%M UTC"))
        } else {
            "???".to_string()
        };
        let status = if job.enabled { "enabled" } else { "disabled" };
        println!(
            "{:<30} {:<10} {:<35} {}",
            job.id, status, sched, job.template
        );
    }
}

async fn cmd_history(
    pool: &sqlx::SqlitePool,
    job_id: Option<&str>,
    limit: i64,
) -> anyhow::Result<()> {
    let records = db::list_executions(pool, job_id, limit).await?;
    if records.is_empty() {
        println!("No execution history found.");
        return Ok(());
    }
    println!(
        "{:<6} {:<30} {:<22} {:>6} {:>6}",
        "ID", "JOB ID", "STARTED AT", "SENT", "FAILED"
    );
    println!("{}", "-".repeat(80));
    for r in records {
        println!(
            "{:<6} {:<30} {:<22} {:>6} {:>6}",
            r.id,
            r.job_id,
            r.started_at.format("%Y-%m-%d %H:%M:%S UTC"),
            r.sent,
            r.failed,
        );
    }
    Ok(())
}

fn cmd_show_config(cfg: &config::Config) -> anyhow::Result<()> {
    let mut copy = cfg.clone();
    copy.api.token = "[REDACTED]".to_string();
    let serialized = toml::to_string_pretty(&copy)?;
    println!("{}", serialized);
    Ok(())
}
