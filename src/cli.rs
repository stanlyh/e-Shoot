use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "e-shoot",
    about = "Scheduled message shooter via templates",
    version
)]
pub struct Cli {
    /// Path to config file (default: ~/.config/e-shoot/config.toml or ./config.toml)
    #[arg(long, env = "E_SHOOT_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Path to SQLite database (default: ~/.local/share/e-shoot/e-shoot.db)
    #[arg(long, env = "E_SHOOT_DB", global = true)]
    pub db: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the scheduler daemon (runs until Ctrl+C)
    Start,

    /// Run a specific job immediately, bypassing its schedule
    RunNow {
        #[arg(help = "Job ID as defined in config")]
        job_id: String,
    },

    /// Validate the config file and print a summary
    Check,

    /// List all configured jobs with schedule info
    List,

    /// Show execution history from the database
    History {
        /// Filter by job ID
        #[arg(long)]
        job_id: Option<String>,

        /// Number of records to show
        #[arg(long, default_value = "20")]
        limit: i64,
    },

    /// Print the resolved config (token redacted)
    ShowConfig,

    /// Create a new config.toml interactively
    Init {
        /// Output path (default: ./config.toml)
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}
