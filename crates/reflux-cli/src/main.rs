use anyhow::Result;
use clap::Parser;
use reflux_core::{Config, GameState, MemoryReader, ProcessHandle};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "reflux")]
#[command(about = "INFINITAS score tracker")]
struct Args {
    #[arg(short, long, default_value = "config.ini")]
    config: PathBuf,

    #[arg(short, long, default_value = "offsets.txt")]
    offsets: PathBuf,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("reflux=info".parse()?))
        .init();

    let args = Args::parse();

    info!("Reflux starting...");

    // Load config
    let config = match Config::load(&args.config) {
        Ok(c) => {
            info!("Loaded config from {:?}", args.config);
            c
        }
        Err(e) => {
            warn!("Failed to load config: {}, using defaults", e);
            Config::default()
        }
    };

    // Load offsets
    let offsets = match reflux_core::load_offsets(&args.offsets) {
        Ok(o) => {
            info!("Loaded offsets version: {}", o.version);
            Some(o)
        }
        Err(e) => {
            warn!("Failed to load offsets: {}", e);
            None
        }
    };

    // Main loop: wait for process
    loop {
        info!("Waiting for INFINITAS process...");

        match ProcessHandle::find_and_open() {
            Ok(process) => {
                info!(
                    "Found INFINITAS process (base: {:#x})",
                    process.base_address
                );

                if let Err(e) = run_tracker(&process, &config, offsets.as_ref()) {
                    error!("Tracker error: {}", e);
                }

                info!("Process disconnected, waiting for reconnect...");
            }
            Err(_) => {
                // Process not found, wait and retry
            }
        }

        thread::sleep(Duration::from_secs(5));
    }
}

fn run_tracker(
    process: &ProcessHandle,
    _config: &Config,
    _offsets: Option<&reflux_core::OffsetsCollection>,
) -> Result<()> {
    let reader = MemoryReader::new(process);
    let mut last_state = GameState::Unknown;

    info!("Starting tracker loop...");

    loop {
        // Check if process is still alive by trying to read memory
        if reader.read_bytes(process.base_address, 4).is_err() {
            info!("Process terminated");
            break;
        }

        // TODO: Implement game state detection and data extraction
        // This requires proper offset values to read game memory

        let current_state = GameState::Unknown; // Placeholder

        if current_state != last_state {
            info!("Game state changed: {:?} -> {:?}", last_state, current_state);
            last_state = current_state;
        }

        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
