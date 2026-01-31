//! Retry utilities for data loading.

use std::collections::HashMap;

use anyhow::{Result, bail};
use reflux_core::config::database;
use reflux_core::{
    EncodingFixes, MemoryReader, OffsetSearcher, OffsetsCollection, SongInfo, builtin_signatures,
    fetch_song_database_with_fixes,
};
use tracing::{debug, info, warn};

use crate::shutdown::ShutdownSignal;
use crate::validation::{ValidationResult, validate_song_database};

/// Load song database with retry logic.
///
/// Waits for the game to fully populate the song database before returning.
/// Returns `Ok(None)` if shutdown was signaled.
pub fn load_song_database_with_retry(
    reader: &MemoryReader,
    song_list: u64,
    encoding_fixes: Option<&EncodingFixes>,
    shutdown: &ShutdownSignal,
) -> Result<Option<HashMap<u32, SongInfo>>> {
    let mut attempts = 0u32;
    let mut last_error: Option<String> = None;
    loop {
        // Check for shutdown signal
        if shutdown.is_shutdown() {
            return Ok(None);
        }

        if attempts >= database::MAX_LOAD_ATTEMPTS {
            bail!(
                "Failed to load song database after {} attempts: {}",
                database::MAX_LOAD_ATTEMPTS,
                last_error.unwrap_or_else(|| "unknown error".to_string())
            );
        }
        attempts += 1;

        // Wait for data initialization on retry only (interruptible)
        if attempts > 1 && shutdown.wait(database::EXTRA_DELAY) {
            return Ok(None);
        }

        match fetch_song_database_with_fixes(reader, song_list, encoding_fixes) {
            Ok(db) => match validate_song_database(&db) {
                ValidationResult::Valid => return Ok(Some(db)),
                ValidationResult::TooFewSongs(count) => {
                    last_error = Some(format!("song list too small ({})", count));
                    warn!(
                        "Song list not fully populated ({} songs), retrying in {}s (attempt {}/{})",
                        count,
                        database::RETRY_DELAY.as_secs(),
                        attempts,
                        database::MAX_LOAD_ATTEMPTS
                    );
                }
                ValidationResult::NotecountTooSmall(notes) => {
                    last_error = Some(format!("notecount too small ({})", notes));
                    warn!(
                        "Song data not fully loaded (reference song notecount: {}), retrying in {}s (attempt {}/{})",
                        notes,
                        database::RETRY_DELAY.as_secs(),
                        attempts,
                        database::MAX_LOAD_ATTEMPTS
                    );
                }
                ValidationResult::ReferenceSongMissing => {
                    last_error = Some("reference song missing".to_string());
                    warn!(
                        "Reference song not yet loaded, retrying in {}s (attempt {}/{})",
                        database::RETRY_DELAY.as_secs(),
                        attempts,
                        database::MAX_LOAD_ATTEMPTS
                    );
                }
            },
            Err(e) => {
                last_error = Some(e.to_string());
                debug!(
                    "Error loading song database: {}. Retrying in {}s (attempt {}/{})",
                    e,
                    database::RETRY_DELAY.as_secs(),
                    attempts,
                    database::MAX_LOAD_ATTEMPTS
                );
            }
        }

        // Wait before retry (interruptible)
        if shutdown.wait(database::RETRY_DELAY) {
            return Ok(None);
        }
    }
}

/// Search for offsets with retry logic.
///
/// Returns `Ok(None)` if shutdown was signaled.
pub fn search_offsets_with_retry(
    reader: &MemoryReader,
    game_version: Option<&String>,
    shutdown: &ShutdownSignal,
) -> Result<Option<OffsetsCollection>> {
    let signatures = builtin_signatures();

    loop {
        // Check for shutdown signal
        if shutdown.is_shutdown() {
            return Ok(None);
        }

        let mut searcher = OffsetSearcher::new(reader);

        match searcher.search_all_with_signatures(&signatures) {
            Ok(mut offsets) => {
                if let Some(version) = game_version {
                    offsets.version = version.clone();
                }

                // search_all_with_signatures already validates each offset individually
                // (song count, judge data markers, play settings ranges, etc.)
                // so we only need to check that all offsets are non-zero
                if offsets.is_valid() {
                    return Ok(Some(offsets));
                }

                info!(
                    "Offset detection incomplete, retrying in {}s...",
                    database::RETRY_DELAY.as_secs()
                );
            }
            Err(e) => {
                info!(
                    "Offset detection failed ({}), retrying in {}s...",
                    e,
                    database::RETRY_DELAY.as_secs()
                );
            }
        }

        // Wait before retry (interruptible)
        if shutdown.wait(database::RETRY_DELAY) {
            return Ok(None);
        }
    }
}
