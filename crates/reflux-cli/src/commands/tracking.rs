//! Main tracking mode command.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use reflux_core::game::find_game_version;
use reflux_core::{
    CustomTypes, EncodingFixes, MemoryReader, OffsetSearcher, OffsetsCollection,
    ProcessHandle, Reflux, ScoreMap, load_offsets,
};
use tracing::{debug, error, info, warn};

use crate::input;
use crate::retry::{load_song_database_with_retry, search_offsets_with_retry};
use crate::shutdown::ShutdownSignal;

/// Run the main tracking mode
pub fn run(offsets_file: Option<&str>) -> Result<()> {
    // Setup graceful shutdown handler
    let shutdown = Arc::new(ShutdownSignal::new());
    let shutdown_ctrlc = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        info!("Received shutdown signal, stopping...");
        shutdown_ctrlc.trigger();
    })?;

    // Spawn keyboard input monitor (Esc, q, Q to quit)
    let shutdown_keyboard = Arc::clone(&shutdown);
    let _keyboard_handle = input::spawn_keyboard_monitor(shutdown_keyboard);

    // Print version and check for updates
    let current_version = env!("CARGO_PKG_VERSION");
    info!("Reflux-RS {}", current_version);

    // Load offsets from file if specified
    let (initial_offsets, offsets_from_file) = if let Some(path) = offsets_file {
        match load_offsets(path) {
            Ok(offsets) => {
                info!("Loaded offsets from {}", path);
                debug!(
                    "  SongList: {:#x}, JudgeData: {:#x}, PlaySettings: {:#x}",
                    offsets.song_list, offsets.judge_data, offsets.play_settings
                );
                (offsets, true)
            }
            Err(e) => {
                warn!("Failed to load offsets from {}: {}", path, e);
                (OffsetsCollection::default(), false)
            }
        }
    } else {
        (OffsetsCollection::default(), false)
    };

    // Create Reflux instance
    let mut reflux = Reflux::new(initial_offsets);

    // Main loop: wait for process (exits on Ctrl+C, Esc, or q)
    println!("Waiting for INFINITAS... (Press Esc or q to quit)");
    while !shutdown.is_shutdown() {
        match ProcessHandle::find_and_open() {
            Ok(process) => {
                debug!(
                    "Found INFINITAS process (base: {:#x})",
                    process.base_address
                );

                // Create memory reader
                let reader = MemoryReader::new(&process);

                // Game version detection (best-effort)
                let game_version = match find_game_version(&reader, process.base_address) {
                    Ok(Some(version)) => {
                        debug!("Game version: {}", version);
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
                // First check basic validity (all offsets non-zero)
                // Then validate signature offsets against the actual memory state
                // Note: For offsets loaded from file, skip distance-based validation
                // as the relative distances may differ between game versions
                let needs_search = if !reflux.offsets().is_valid() {
                    info!("Invalid offsets detected (some offsets are zero)");
                    true
                } else if offsets_from_file {
                    // For file-loaded offsets, just verify memory is readable
                    let searcher = OffsetSearcher::new(&reader);
                    if searcher.validate_basic_memory_access(reflux.offsets()) {
                        debug!("File-loaded offsets: basic memory access validated");
                        false
                    } else {
                        info!("File-loaded offsets: memory access failed. Attempting signature search...");
                        true
                    }
                } else {
                    let searcher = OffsetSearcher::new(&reader);
                    if !searcher.validate_signature_offsets(reflux.offsets()) {
                        info!(
                            "Offset validation failed (offsets may be stale or incorrect). Attempting signature search..."
                        );
                        true
                    } else {
                        debug!("Loaded offsets validated successfully");
                        false
                    }
                };

                if needs_search {
                    let offsets =
                        search_offsets_with_retry(&reader, game_version.as_ref(), &shutdown)?;
                    let Some(offsets) = offsets else {
                        // Shutdown requested during offset search
                        break;
                    };

                    debug!("Signature-based offset detection successful!");
                    reflux.update_offsets(offsets);
                }

                // Load encoding fixes
                let encoding_fixes = match EncodingFixes::load("encodingfixes.txt") {
                    Ok(ef) => {
                        debug!("Loaded {} encoding fixes", ef.len());
                        Some(ef)
                    }
                    Err(e) => {
                        if e.is_not_found() {
                            debug!("Encoding fixes file not found, using defaults");
                        } else {
                            warn!("Failed to load encoding fixes: {}", e);
                        }
                        None
                    }
                };

                // Load song database
                // Strategy:
                // 1. If TSV exists, use it as primary source (complete metadata)
                // 2. Scan memory for song_id mappings
                // 3. Match TSV entries with memory song_ids
                // 4. Fall back to memory-only or legacy approach if needed

                let tsv_path = "tracker.tsv";
                let song_db = if std::path::Path::new(tsv_path).exists() {
                    debug!("Building song database from TSV + memory scan...");
                    let db = reflux_core::game::build_song_database_from_tsv_with_memory(
                        &reader,
                        reflux.offsets().song_list,
                        tsv_path,
                        0x100000, // 1MB scan
                    );
                    if db.is_empty() {
                        debug!("TSV+memory approach returned empty, trying legacy...");
                        let legacy_db = load_song_database_with_retry(
                            &reader,
                            reflux.offsets().song_list,
                            encoding_fixes.as_ref(),
                            &shutdown,
                        )?;
                        let Some(db) = legacy_db else {
                            break;
                        };
                        db
                    } else {
                        db
                    }
                } else {
                    // No TSV, use memory-only approach
                    debug!("No TSV file found, using memory scan...");
                    let song_db = reflux_core::game::fetch_song_database_from_memory_scan(
                        &reader,
                        reflux.offsets().song_list,
                        0x100000,
                    );

                    if song_db.is_empty() {
                        debug!("Memory scan found no songs, trying legacy approach...");
                        let db = load_song_database_with_retry(
                            &reader,
                            reflux.offsets().song_list,
                            encoding_fixes.as_ref(),
                            &shutdown,
                        )?;
                        let Some(db) = db else {
                            break;
                        };
                        db
                    } else {
                        info!("Loaded {} songs from memory scan", song_db.len());
                        song_db
                    }
                };

                debug!("Loaded {} songs", song_db.len());
                reflux.set_song_db(song_db.clone());

                // Load score map from game memory
                debug!("Loading score map...");
                let score_map = match ScoreMap::load_from_memory(
                    &reader,
                    reflux.offsets().data_map,
                    &song_db,
                ) {
                    Ok(map) => {
                        debug!("Loaded {} score entries", map.len());
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
                        let mut types = HashMap::new();
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
                        debug!("Loaded {} custom types", types.len());
                        reflux.set_custom_types(types);
                    }
                    Err(e) => {
                        if e.is_not_found() {
                            debug!("Custom types file not found, using defaults");
                        } else {
                            warn!("Failed to load custom types: {}", e);
                        }
                    }
                }

                // Load unlock state from memory
                if let Err(e) = reflux.load_unlock_state(&reader) {
                    warn!("Failed to load unlock state: {}", e);
                }

                println!("Ready to track. Waiting for plays...");

                // Run tracker loop
                if let Err(e) = reflux.run(&process, shutdown.as_atomic()) {
                    error!("Tracker error: {}", e);
                }

                // Export tracker.tsv on disconnect
                if let Err(e) = reflux.export_tracker_tsv("tracker.tsv") {
                    error!("Failed to export tracker.tsv: {}", e);
                }

                debug!("Process disconnected, waiting for reconnect...");
            }
            Err(_) => {
                // Process not found, wait and retry
            }
        }

        // Interruptible wait before retry
        if shutdown.wait(Duration::from_secs(5)) {
            break;
        }
    }

    info!("Shutdown complete");
    Ok(())
}
