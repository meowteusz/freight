use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use tracing::info;

mod config;
mod daemon;
mod socket;
mod tui;
mod worker;

pub use config::Config;
pub use socket::{SocketServer, WorkerMessage};
pub use worker::{WorkerManager, WorkerStatus};

#[derive(Parser)]
#[command(name = "freight")]
#[command(about = "NFS Migration Suite Orchestrator")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize freight project in current directory
    Init {
        /// Migration source directory (defaults to current directory)
        #[arg(short, long)]
        source: Option<String>,
    },
    /// Start daemon and show dashboard
    Dashboard {
        /// Migration source directory
        #[arg(short, long)]
        source: Option<String>,
        /// Migration destination directory  
        #[arg(short, long)]
        dest: Option<String>,
    },
    /// Start migration with dashboard
    Migrate {
        /// Migration source directory
        source: String,
        /// Migration destination directory
        dest: String,
    },
    /// Start daemon only (background)
    Daemon {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },
    /// Connect TUI client to existing daemon
    Connect,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Init { source } => {
            let current_dir = std::env::current_dir().context("Failed to get current directory")?;
            let source_path = source
                .map(|s| std::path::PathBuf::from(s))
                .unwrap_or(current_dir)
                .canonicalize()
                .context("Failed to resolve absolute path")?;

            info!("Initializing freight project in: {}", source_path.display());
            Config::init_project(source_path.to_str().unwrap())?;
            println!("Freight project initialized successfully!");
            Ok(())
        }
        Commands::Dashboard { source, dest } => {
            info!("Starting freight dashboard");

            // Start daemon in background
            let daemon_handle = tokio::spawn(async move { daemon::start_daemon().await });

            // Start TUI client
            let tui_result = tui::run_dashboard().await;

            // Clean shutdown
            daemon_handle.abort();
            tui_result
        }
        Commands::Migrate { source, dest } => {
            info!("Starting migration: {} -> {}", source, dest);

            // Load or create config
            let config = Config::load_or_create(&source, &dest)?;

            // Start daemon with migration
            let daemon_handle =
                tokio::spawn(async move { daemon::start_migration_daemon(config).await });

            // Start TUI client
            let tui_result = tui::run_dashboard().await;

            // Clean shutdown
            daemon_handle.abort();
            tui_result
        }
        Commands::Daemon { foreground } => {
            if foreground {
                info!("Starting freight daemon in foreground");
                daemon::start_daemon().await
            } else {
                info!("Starting freight daemon in background");
                daemon::daemonize_and_start().await
            }
        }
        Commands::Connect => {
            info!("Connecting to existing freight daemon");
            tui::run_dashboard().await
        }
    }
}
