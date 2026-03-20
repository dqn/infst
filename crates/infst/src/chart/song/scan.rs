use std::collections::HashMap;
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::process::{ByteBuffer, ReadMemory, decode_shift_jis};

use super::SongInfo;
use crate::chart::encoding_fixes::fix_title_encoding;

/// Analyze metadata table structure for new INFINITAS versions
///
/// This function scans the metadata table to find valid song_ids and determine
/// the actual entry size used by the new version.
pub fn analyze_metadata_table<R: ReadMemory>(reader: &R, text_base: u64) {
    let metadata_base = text_base + SongInfo::METADATA_TABLE_OFFSET as u64;
    info!("=== Metadata Table Analysis at 0x{:X} ===", metadata_base);

    // Read a large chunk to analyze
    let Ok(buffer) = reader.read_bytes(metadata_base, 0x10000) else {
        warn!("Failed to read metadata table");
        return;
    };

    let buf = ByteBuffer::new(&buffer);

    // Scan for valid song_ids (pattern: 1000-50000 followed by reasonable folder 1-50)
    let mut found_ids: Vec<(usize, i32, i32)> = Vec::new();

    for offset in (0..buffer.len() - 8).step_by(4) {
        let song_id = buf.read_i32_at(offset).unwrap_or(0);
        let folder = buf.read_i32_at(offset + 4).unwrap_or(0);

        if (1000..=50000).contains(&song_id) && (1..=50).contains(&folder) {
            found_ids.push((offset, song_id, folder));
        }
    }

    info!("Found {} potential song entries", found_ids.len());

    // Analyze spacing between entries
    if found_ids.len() >= 2 {
        let mut deltas: Vec<usize> = Vec::new();
        for i in 1..found_ids.len().min(20) {
            let delta = found_ids[i].0 - found_ids[i - 1].0;
            deltas.push(delta);
        }

        info!("Entry spacing (first 20): {:?}", deltas);

        // Find most common delta
        let mut delta_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for d in &deltas {
            *delta_counts.entry(*d).or_insert(0) += 1;
        }
        if let Some((most_common, count)) = delta_counts.iter().max_by_key(|(_, v)| *v) {
            info!(
                "Most common entry size: 0x{:X} ({} bytes), {} occurrences",
                most_common, most_common, count
            );
        }
    }

    // Show first 10 entries
    for (i, (offset, song_id, folder)) in found_ids.iter().take(10).enumerate() {
        let abs_addr = metadata_base + *offset as u64;
        debug!(
            "  Entry {}: song_id={}, folder={} at 0x{:X} (offset 0x{:X})",
            i, song_id, folder, abs_addr, offset
        );

        // Show bytes around this entry
        if let Ok(entry_bytes) = buf.slice_at(*offset, 32) {
            debug!("    Bytes: {:02X?}", entry_bytes);
        }
    }
}

/// Build a song_id to title mapping by scanning metadata table
///
/// For new INFINITAS versions (2026012800+), the title is located 0x7E0 bytes
/// BEFORE the metadata entry. This function scans for valid metadata entries
/// and extracts the corresponding titles.
///
/// Memory structure:
/// - text_entry[i] = text_base + i * ENTRY_SIZE
/// - meta_entry[i] = text_base + METADATA_OFFSET + i * ENTRY_SIZE
pub fn build_song_id_title_map<R: ReadMemory>(
    reader: &R,
    text_base: u64,
    scan_size: usize,
) -> HashMap<u32, Arc<str>> {
    const ENTRY_SIZE: u64 = SongInfo::MEMORY_SIZE as u64;
    const METADATA_OFFSET: u64 = SongInfo::METADATA_TABLE_OFFSET as u64;

    let mut result = HashMap::new();
    let max_entries = (scan_size as u64 / ENTRY_SIZE).min(5000);

    // Note: With lazy loading, songs may be scattered across the entry table.
    // We scan all entries without early termination to find all loaded songs.
    for i in 0..max_entries {
        let text_addr = text_base + i * ENTRY_SIZE;
        let meta_addr = text_addr + METADATA_OFFSET;

        // Read metadata
        let Ok(meta_bytes) = reader.read_bytes(meta_addr, 8) else {
            continue;
        };

        let buf = ByteBuffer::new(&meta_bytes);
        let song_id = buf.read_i32_at(0).unwrap_or(0);
        let folder = buf.read_i32_at(4).unwrap_or(0);

        // Validate song_id and folder ranges
        // Note: folder values vary widely in new INFINITAS versions (e.g., 1-200+)
        if !(1000..=90000).contains(&song_id) || !(1..=200).contains(&folder) {
            continue;
        }

        // Skip if we already have this song_id
        if result.contains_key(&(song_id as u32)) {
            continue;
        }

        // Read title from text table
        if let Ok(title_bytes) = reader.read_bytes(text_addr, 64) {
            let title_decoded = decode_shift_jis(&title_bytes);
            let title_trimmed = title_decoded.trim();
            let title_arc =
                fix_title_encoding(title_trimmed).unwrap_or_else(|| Arc::from(title_trimmed));
            let title: &str = &title_arc;
            if !title.is_empty()
                && title
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_graphic() || !c.is_ascii())
            {
                debug!(
                    "Mapped song_id={} to title={:?} (folder={})",
                    song_id, title, folder
                );
                result.insert(song_id as u32, Arc::from(title));
            }
        }
    }

    info!("Built song_id->title mapping with {} entries", result.len());
    result
}

/// Fetch a single song by its song_id from memory
///
/// This function searches through the song list entries to find a specific song.
/// Uses the provided `EntryLayout` to correctly parse entries regardless of version.
///
/// Memory structure:
/// - entry[i] = song_list_addr + i * ENTRY_SIZE (0x630 = 1584 bytes)
/// - song_id is at offset 0 within each entry
pub fn fetch_song_by_id<R: ReadMemory>(
    reader: &R,
    song_list_addr: u64,
    target_song_id: u32,
    scan_size: usize,
    entry_stride: usize,
    layout: &super::EntryLayout,
) -> Option<SongInfo> {
    if song_list_addr == 0 {
        return None;
    }

    let stride = entry_stride as u64;
    let max_entries = (scan_size as u64 / stride).min(5000);

    // Scan each entry for the target song_id
    for i in 0..max_entries {
        let entry_addr = song_list_addr + i * stride;

        match SongInfo::read_from_memory_with_layout(reader, entry_addr, layout) {
            Ok(Some(song)) if song.id == target_song_id => {
                debug!(
                    "Dynamically loaded song_id={} title={:?} folder={}",
                    song.id, song.title, song.folder
                );
                return Some(song);
            }
            _ => continue,
        }
    }

    None
}

/// Build song database directly from memory for new INFINITAS versions
///
/// This function scans memory to find all loaded songs. Uses the provided
/// `EntryLayout` to correctly parse entries regardless of version.
///
/// Memory structure:
/// - entry[i] = song_list_base + i * ENTRY_SIZE (0x630 = 1584 bytes)
pub fn fetch_song_database_from_memory_scan<R: ReadMemory>(
    reader: &R,
    song_list_base: u64,
    scan_size: usize,
    entry_stride: usize,
    layout: &super::EntryLayout,
) -> HashMap<u32, SongInfo> {
    let stride = entry_stride as u64;

    let mut result = HashMap::new();
    let max_entries = (scan_size as u64 / stride).min(5000);

    // Note: With lazy loading, songs may be scattered across the entry table.
    // We scan all entries to find all loaded songs.
    for i in 0..max_entries {
        let entry_addr = song_list_base + i * stride;

        let song = match SongInfo::read_from_memory_with_layout(reader, entry_addr, layout) {
            Ok(Some(song)) => song,
            _ => continue,
        };

        // Validate song_id range
        if song.id < 1000 || song.id > 90000 {
            continue;
        }

        // Skip if we already have this song_id
        if result.contains_key(&song.id) {
            continue;
        }

        debug!(
            "Found song_id={} title={:?} folder={}",
            song.id, song.title, song.folder
        );

        result.insert(song.id, song);
    }

    info!("Fetched {} songs from memory scan", result.len());
    result
}
