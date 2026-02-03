//! AppImage Auto-Integration Daemon CLI
//!
//! Main binary for the appimage-auto daemon.

use appimage_auto::{Config, Daemon, State, daemon};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "appimage-auto")]
#[command(about = "Automatic AppImage integration daemon for Linux desktop environments")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file (default: ~/.config/appimage-auto/config.toml)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Verbose output (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon in the foreground
    Daemon,

    /// Scan directories once and exit (no watching)
    Scan,

    /// Show daemon status and statistics
    Status,

    /// List all integrated AppImages
    List,

    /// Manually integrate a specific AppImage
    Integrate {
        /// Path to the AppImage file
        path: PathBuf,
    },

    /// Manually remove integration for an AppImage
    Remove {
        /// Path to the AppImage file
        path: PathBuf,
    },

    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,

    /// Show configuration file path
    Path,

    /// Add a directory to watch
    AddWatch {
        /// Directory path to add
        directory: PathBuf,
    },

    /// Remove a directory from watching
    RemoveWatch {
        /// Directory path to remove
        directory: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    // Set up logging
    let log_level = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("appimage_auto={}", log_level)));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Load config if specified
    let config = if let Some(config_path) = &cli.config {
        match Config::load_from(config_path) {
            Ok(c) => Some(c),
            Err(e) => {
                error!("Failed to load config from {:?}: {}", config_path, e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Run the appropriate command
    let result = match cli.command {
        Commands::Daemon => run_daemon(config),
        Commands::Scan => run_scan(config),
        Commands::Status => run_status(),
        Commands::List => run_list(),
        Commands::Integrate { path } => run_integrate(config, &path),
        Commands::Remove { path } => run_remove(&path),
        Commands::Config { action } => run_config(action),
    };

    if let Err(e) = result {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_daemon(config: Option<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting appimage-auto daemon...");

    let mut daemon = match config {
        Some(c) => Daemon::with_config(c)?,
        None => Daemon::new()?,
    };

    // Set up signal handling
    let running = daemon.running_flag();
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running.store(false, Ordering::SeqCst);
    })?;

    daemon.init()?;
    daemon.run()?;

    Ok(())
}

fn run_scan(config: Option<Config>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running one-shot scan...");
    daemon::oneshot(config)?;
    Ok(())
}

fn run_status() -> Result<(), Box<dyn std::error::Error>> {
    let state = State::load()?;
    let config = Config::load()?;

    println!("AppImage Auto-Integration Status");
    println!("=================================");
    println!();
    println!("Integrated AppImages: {}", state.count());
    println!();
    println!("Watched directories:");
    for dir in &config.watch.directories {
        let expanded = shellexpand::tilde(dir);
        let exists = std::path::Path::new(expanded.as_ref()).exists();
        let status = if exists { "OK" } else { "NOT FOUND" };
        println!("  {} [{}]", dir, status);
    }
    println!();
    println!("Config file: {:?}", Config::config_path()?);
    println!("State file:  {:?}", State::state_path()?);

    Ok(())
}

fn run_list() -> Result<(), Box<dyn std::error::Error>> {
    let state = State::load()?;

    if state.count() == 0 {
        println!("No integrated AppImages.");
        return Ok(());
    }

    println!("Integrated AppImages:");
    println!();

    for app in state.all() {
        let name = app.name.as_deref().unwrap_or("Unknown");
        let exists = app.appimage_path.exists();
        let status = if exists { "" } else { " [MISSING]" };

        println!("  {} ({}){}", name, app.identifier, status);
        println!("    Path: {:?}", app.appimage_path);
        println!("    Desktop: {:?}", app.desktop_path);
        println!();
    }

    Ok(())
}

fn run_integrate(config: Option<Config>, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    use appimage_auto::appimage;

    if !path.exists() {
        return Err(format!("File not found: {:?}", path).into());
    }

    if !appimage::is_appimage(path) {
        return Err(format!("Not a valid AppImage: {:?}", path).into());
    }

    let mut daemon = match config {
        Some(c) => Daemon::with_config(c)?,
        None => Daemon::new()?,
    };

    daemon.integrate(path)?;
    println!("Successfully integrated: {:?}", path);

    Ok(())
}

fn run_remove(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let mut daemon = Daemon::with_config(config)?;

    if daemon.state().is_integrated(path) {
        daemon.unintegrate(path)?;
        println!("Successfully removed integration for: {:?}", path);
    } else {
        println!("AppImage not integrated: {:?}", path);
    }

    Ok(())
}

fn run_config(action: Option<ConfigAction>) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        None | Some(ConfigAction::Show) => {
            let config = Config::load()?;
            let toml = toml::to_string_pretty(&config)?;
            println!("{}", toml);
        }

        Some(ConfigAction::Path) => {
            println!("{:?}", Config::config_path()?);
        }

        Some(ConfigAction::AddWatch { directory }) => {
            let mut config = Config::load()?;
            let dir_str = directory.to_string_lossy().to_string();

            if config.watch.directories.contains(&dir_str) {
                println!("Directory already in watch list: {}", dir_str);
            } else {
                config.watch.directories.push(dir_str.clone());
                config.save()?;
                println!("Added watch directory: {}", dir_str);
            }
        }

        Some(ConfigAction::RemoveWatch { directory }) => {
            let mut config = Config::load()?;
            let dir_str = directory.to_string_lossy().to_string();

            let original_len = config.watch.directories.len();
            config.watch.directories.retain(|d| d != &dir_str);

            if config.watch.directories.len() < original_len {
                config.save()?;
                println!("Removed watch directory: {}", dir_str);
            } else {
                println!("Directory not in watch list: {}", dir_str);
            }
        }
    }

    Ok(())
}
