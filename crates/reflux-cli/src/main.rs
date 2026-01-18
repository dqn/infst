use anyhow::{Result, bail};
use clap::Parser;
use reflux_core::game::find_game_version;
use reflux_core::{
    Config, CustomTypes, EncodingFixes, MemoryReader, OffsetSearcher, OffsetsCollection,
    ProcessHandle, Reflux, ScoreMap, builtin_signatures, export_song_list,
    fetch_song_database_with_fixes,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

fn load_song_database_with_retry(
    reader: &MemoryReader,
    song_list: u64,
    encoding_fixes: Option<&EncodingFixes>,
) -> Result<std::collections::HashMap<u32, reflux_core::SongInfo>> {
    const RETRY_DELAY_MS: u64 = 5000;
    const EXTRA_DELAY_MS: u64 = 1000;
    const MIN_EXPECTED_SONGS: usize = 1000;
    const READY_SONG_ID: u32 = 80003;
    const READY_DIFF_INDEX: usize = 3; // SPB, SPN, SPH, SPA, ...
    const READY_MIN_NOTES: u32 = 10;
    const MAX_ATTEMPTS: u32 = 12;

    let mut attempts = 0u32;
    let mut last_error: Option<String> = None;
    loop {
        if attempts >= MAX_ATTEMPTS {
            bail!(
                "Failed to load song database after {} attempts: {}",
                MAX_ATTEMPTS,
                last_error.unwrap_or_else(|| "unknown error".to_string())
            );
        }
        attempts += 1;

        // データ初期化のタイミングに合わせて少し待つ
        thread::sleep(Duration::from_millis(EXTRA_DELAY_MS));

        match fetch_song_database_with_fixes(reader, song_list, encoding_fixes) {
            Ok(db) => {
                if db.len() < MIN_EXPECTED_SONGS {
                    last_error = Some(format!("song list too small ({})", db.len()));
                    warn!(
                        "Song list not fully populated ({} songs), retrying in {}s (attempt {}/{})",
                        db.len(),
                        RETRY_DELAY_MS / 1000,
                        attempts,
                        MAX_ATTEMPTS
                    );
                    thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    continue;
                }

                if let Some(song) = db.get(&READY_SONG_ID) {
                    let notes = song.total_notes.get(READY_DIFF_INDEX).copied().unwrap_or(0);
                    if notes < READY_MIN_NOTES {
                        last_error = Some(format!(
                            "notecount too small (song {}, notes {})",
                            READY_SONG_ID, notes
                        ));
                        warn!(
                            "Notecount data seems bad (song {}, notes {}), retrying in {}s (attempt {}/{})",
                            READY_SONG_ID,
                            notes,
                            RETRY_DELAY_MS / 1000,
                            attempts,
                            MAX_ATTEMPTS
                        );
                        thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                        continue;
                    }
                } else {
                    warn!(
                        "Song {} not found in song list, accepting current list",
                        READY_SONG_ID
                    );
                }

                return Ok(db);
            }
            Err(e) => {
                last_error = Some(e.to_string());
                warn!(
                    "Failed to load song database ({}), retrying in {}s (attempt {}/{})",
                    e,
                    RETRY_DELAY_MS / 1000,
                    attempts,
                    MAX_ATTEMPTS
                );
                thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
            }
        }
    }
}

#[derive(Parser)]
#[command(name = "reflux")]
#[command(about = "INFINITAS score tracker", version)]
struct Args {}

#[tokio::main]
async fn main() -> Result<()> {
    Args::parse();

    // Initialize logging (RUST_LOG がなければ info を既定にする)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("reflux=info,reflux_core=info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    // Setup graceful shutdown handler
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);
    ctrlc::set_handler(move || {
        info!("Received shutdown signal, stopping...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Print version and check for updates
    let current_version = env!("CARGO_PKG_VERSION");
    info!("Reflux-RS {}", current_version);

    // Load config
    let config = Config::default();
    info!("Using default config");

    // Create Reflux instance
    let mut reflux = Reflux::new(config, OffsetsCollection::default());

    // Load tracker
    if let Err(e) = reflux.load_tracker("tracker.db") {
        warn!("Failed to load tracker: {}", e);
    }

    // Main loop: wait for process (exits on Ctrl+C)
    info!("Waiting for INFINITAS process...");
    while running.load(Ordering::SeqCst) {
        match ProcessHandle::find_and_open() {
            Ok(process) => {
                info!(
                    "Found INFINITAS process (base: {:#x})",
                    process.base_address
                );

                // Create memory reader
                let reader = MemoryReader::new(&process);

                // Game version detection (best-effort)
                let game_version = match find_game_version(&reader, process.base_address) {
                    Ok(Some(version)) => {
                        info!("Game version: {}", version);
                        Some(version)
                    }
                    Ok(None) => {
                        warn!("Could not detect game version");
                        None
                    }
                    Err(e) => {
                        warn!("Failed to check game version: {}", e);
                        None
                    }
                };

                // Check if offsets are valid before proceeding
                if !reflux.offsets().is_valid() {
                    warn!("Invalid offsets detected. Attempting signature search...");

                    let mut searcher = OffsetSearcher::new(&reader);
                    let signatures = builtin_signatures();

                    let mut offsets = match searcher.search_all_with_signatures(&signatures) {
                        Ok(offsets) => offsets,
                        Err(e) => {
                            error!("Signature-based offset detection failed: {}", e);
                            bail!("Signature-based offset detection failed: {}", e);
                        }
                    };

                    if let Some(version) = &game_version {
                        offsets.version = version.clone();
                    }

                    if !searcher.validate_signature_offsets(&offsets) {
                        error!("Signature-based offsets failed validation");
                        bail!("Signature-based offsets failed validation");
                    }

                    info!("Signature-based offset detection successful!");
                    reflux.update_offsets(offsets);
                }

                if !reflux.offsets().is_valid() {
                    error!("Invalid offsets detected. Exiting.");
                    bail!("Invalid offsets detected");
                }

                // Load encoding fixes
                let encoding_fixes = match EncodingFixes::load("encodingfixes.txt") {
                    Ok(ef) => {
                        info!("Loaded {} encoding fixes", ef.len());
                        Some(ef)
                    }
                    Err(e) => {
                        if e.is_not_found() {
                            info!("Encoding fixes file not found, using defaults");
                        } else {
                            warn!("Failed to load encoding fixes: {}", e);
                        }
                        None
                    }
                };

                // Load song database from game memory
                info!("Loading song database...");
                let song_db = load_song_database_with_retry(
                    &reader,
                    reflux.offsets().song_list,
                    encoding_fixes.as_ref(),
                )?;
                info!("Loaded {} songs", song_db.len());
                reflux.set_song_db(song_db.clone());

                // Output song list for debugging if configured
                if reflux.config().debug.output_db {
                    info!("Outputting song list to songs.tsv...");
                    if let Err(e) = export_song_list("songs.tsv", &song_db) {
                        warn!("Failed to export song list: {}", e);
                    }
                }

                // Load score map from game memory
                info!("Loading score map...");
                let score_map = match ScoreMap::load_from_memory(
                    &reader,
                    reflux.offsets().data_map,
                    &song_db,
                ) {
                    Ok(map) => {
                        info!("Loaded {} score entries", map.len());
                        map
                    }
                    Err(e) => {
                        warn!("Failed to load score map: {}", e);
                        ScoreMap::new()
                    }
                };
                reflux.set_score_map(score_map);

                // Load custom types
                match CustomTypes::load("customtypes.txt") {
                    Ok(ct) => {
                        let mut types = std::collections::HashMap::new();
                        let mut parse_failures = 0usize;
                        for (k, v) in ct.iter() {
                            match k.parse::<u32>() {
                                Ok(id) => {
                                    types.insert(id, v.clone());
                                }
                                Err(_) => {
                                    if parse_failures == 0 {
                                        warn!(
                                            "Failed to parse custom type ID '{}' (further errors suppressed)",
                                            k
                                        );
                                    }
                                    parse_failures += 1;
                                }
                            }
                        }
                        if parse_failures > 1 {
                            warn!("{} custom type IDs failed to parse", parse_failures);
                        }
                        info!("Loaded {} custom types", types.len());
                        reflux.set_custom_types(types);
                    }
                    Err(e) => {
                        if e.is_not_found() {
                            info!("Custom types file not found, using defaults");
                        } else {
                            warn!("Failed to load custom types: {}", e);
                        }
                    }
                }

                // Load unlock database
                if let Err(e) = reflux.load_unlock_db("unlockdb") {
                    warn!("Failed to load unlock db: {}", e);
                }
                if let Err(e) = reflux.load_unlock_state(&reader) {
                    warn!("Failed to load unlock state: {}", e);
                }

                // Sync with server
                if reflux.config().record.save_remote {
                    info!("Syncing with server...");
                    if let Err(e) = reflux.sync_with_server().await {
                        warn!("Server sync failed: {}", e);
                    }
                }

                // Run tracker loop
                if let Err(e) = reflux.run(&process) {
                    error!("Tracker error: {}", e);
                }

                // Save unlock database on disconnect
                if let Err(e) = reflux.save_unlock_db("unlockdb") {
                    error!("Failed to save unlock db: {}", e);
                }

                // Save tracker on disconnect
                if let Err(e) = reflux.save_tracker("tracker.db") {
                    error!("Failed to save tracker: {}", e);
                }

                // Export tracker.tsv on disconnect
                if reflux.config().record.save_local
                    && let Err(e) = reflux.export_tracker_tsv("tracker.tsv")
                {
                    error!("Failed to export tracker.tsv: {}", e);
                }

                info!("Process disconnected, waiting for reconnect...");
            }
            Err(_) => {
                // Process not found, wait and retry
            }
        }

        // Check if we should continue or exit
        if !running.load(Ordering::SeqCst) {
            break;
        }

        thread::sleep(Duration::from_secs(5));
    }

    info!("Shutdown complete");
    Ok(())
}
