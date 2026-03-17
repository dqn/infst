use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::error::Result;
use crate::process::{ByteBuffer, ReadMemory};

use super::SongInfo;

/// Fetch entire song database from memory using bulk read.
///
/// Reads all entries in a single ReadProcessMemory call (~5.7MB) instead of
/// ~5000 individual calls. Falls back to `fetch_song_database` on failure.
pub fn fetch_song_database_bulk<R: ReadMemory>(
    reader: &R,
    song_list_addr: u64,
    entry_stride: usize,
) -> Result<HashMap<u32, SongInfo>> {
    const MAX_ENTRIES: usize = 5000;
    let bulk_size = MAX_ENTRIES * entry_stride;

    // Try bulk read
    let buffer = match reader.read_bytes(song_list_addr, bulk_size) {
        Ok(buf) => buf,
        Err(e) => {
            warn!("Bulk read failed ({}), falling back to per-entry read", e);
            return fetch_song_database(reader, song_list_addr, entry_stride);
        }
    };

    // Also bulk-read metadata table for fallback song_id resolution
    let metadata_buffer = reader
        .read_bytes(
            song_list_addr + SongInfo::METADATA_TABLE_OFFSET as u64,
            bulk_size,
        )
        .ok();

    let mut result = HashMap::new();
    let mut consecutive_failures: u32 = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 10;

    for entry_index in 0..MAX_ENTRIES {
        let offset = entry_index * entry_stride;
        if offset + SongInfo::MEMORY_SIZE > buffer.len() {
            break;
        }

        match SongInfo::parse_from_buffer(&buffer, offset) {
            Ok(Some(song)) if !song.title.is_empty() && song.id > 0 => {
                result.entry(song.id).or_insert(song);
                consecutive_failures = 0;
            }
            Ok(Some(mut song)) if song.id == 0 && !song.title.is_empty() => {
                // Try metadata table fallback
                if let Some(ref meta_buf) = metadata_buffer {
                    let meta_offset = entry_index * entry_stride;
                    if meta_offset + 8 <= meta_buf.len() {
                        let meta = ByteBuffer::new(&meta_buf[meta_offset..]);
                        let alt_song_id = meta.read_i32_at(0).unwrap_or(0);
                        let alt_folder = meta.read_i32_at(4).unwrap_or(0);
                        if (1000..=50000).contains(&alt_song_id) {
                            song.id = alt_song_id as u32;
                            if (1..=50).contains(&alt_folder) {
                                song.folder = alt_folder;
                            }
                            result.entry(song.id).or_insert(song);
                            consecutive_failures = 0;
                            continue;
                        }
                    }
                }
                consecutive_failures += 1;
            }
            _ => {
                consecutive_failures += 1;
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    debug!(
                        "Stopping bulk song fetch after {} consecutive failures at entry {}",
                        consecutive_failures, entry_index
                    );
                    break;
                }
            }
        }
    }

    info!("Fetched {} songs from bulk read", result.len());
    Ok(result)
}

/// Fetch entire song database from memory
pub fn fetch_song_database<R: ReadMemory>(
    reader: &R,
    song_list_addr: u64,
    entry_stride: usize,
) -> Result<HashMap<u32, SongInfo>> {
    let mut result = HashMap::new();
    let mut entry_index: u64 = 0;
    let mut consecutive_failures = 0;
    const MAX_CONSECUTIVE_FAILURES: u32 = 10;

    loop {
        let address = song_list_addr + entry_index * entry_stride as u64;

        // Use fallback method for new INFINITAS versions where metadata is split
        match SongInfo::read_from_memory_with_fallback(
            reader,
            address,
            song_list_addr,
            entry_index,
        )? {
            Some(song) if !song.title.is_empty() && song.id > 0 => {
                // Avoid duplicates
                result.entry(song.id).or_insert(song);
                consecutive_failures = 0;
            }
            _ => {
                consecutive_failures += 1;
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    // End of song list after multiple consecutive failures
                    debug!(
                        "Stopping song fetch after {} consecutive failures at entry {}",
                        consecutive_failures, entry_index
                    );
                    break;
                }
            }
        }

        entry_index += 1;

        // Safety limit
        if entry_index > 5000 {
            warn!("Song database fetch reached safety limit of 5000 entries");
            break;
        }
    }

    info!("Fetched {} songs from database", result.len());
    Ok(result)
}
