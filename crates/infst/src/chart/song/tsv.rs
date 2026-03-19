use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::play::UnlockType;
use crate::process::ReadMemory;

use super::SongInfo;
use super::scan::fetch_song_database_from_memory_scan;
use crate::chart::encoding_fixes::fix_title_encoding;

/// Load song database from a TSV file (tracker export format)
///
/// The TSV file should have columns:
/// Title, Type, Label, Cost Normal, Cost Hyper, Cost Another, SP DJ Points, DP DJ Points,
/// SPB Unlocked, SPB Rating, ..., DPL DJ Points
///
/// This function extracts:
/// - Title (column 0)
/// - Difficulty levels (SPB Rating, SPN Rating, ... columns)
/// - Note counts (SPB Note Count, SPN Note Count, ... columns)
pub fn load_song_database_from_tsv<P: AsRef<Path>>(
    path: P,
) -> std::result::Result<HashMap<Arc<str>, SongInfo>, std::io::Error> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut result = HashMap::new();

    // Column indices (0-based):
    // 0: Title, 1: Type, 2: Label
    // SPB: 9=Rating, 14=Note Count
    // SPN: 17=Rating, 22=Note Count
    // SPH: 25=Rating, 30=Note Count
    // SPA: 33=Rating, 38=Note Count
    // SPL: 41=Rating, 46=Note Count
    // DPN: 49=Rating, 54=Note Count (note: no DPB in this format)
    // DPH: 57=Rating, 62=Note Count
    // DPA: 65=Rating, 70=Note Count
    // DPL: 73=Rating, 78=Note Count

    // Column indices are 0-based. Column 0 = Song ID, Column 1 = Title.
    // Per difficulty: +0=Unlocked, +1=Rating, +2=Lamp, +3=Letter, +4=EX Score,
    //                 +5=Miss Count, +6=Note Count, +7=DJ Points
    // SPB starts at column 10, each difficulty block is 8 columns.
    const RATING_COLS: [usize; 10] = [10, 18, 26, 34, 42, 0, 50, 58, 66, 74]; // 0 for DPB (not in file)
    const NOTE_COLS: [usize; 10] = [15, 23, 31, 39, 47, 0, 55, 63, 71, 79]; // 0 for DPB

    let mut line_num = 0;
    for line_result in reader.lines() {
        line_num += 1;
        let line = line_result?;

        // Skip header
        if line_num == 1 {
            continue;
        }

        let cols: Vec<&str> = line.split('\t').collect();
        if cols.is_empty() {
            continue;
        }

        // Column 0 is Song ID, column 1 is Title
        let title = if cols.len() > 1 {
            cols[1].trim()
        } else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let title = fix_title_encoding(title)
            .map(|arc: Arc<str>| arc.to_string())
            .unwrap_or_else(|| title.to_string());
        let title = title.as_str();

        // Parse difficulty levels
        let mut levels = [0u8; 10];
        for (i, &col_idx) in RATING_COLS.iter().enumerate() {
            if col_idx > 0 && col_idx < cols.len() {
                levels[i] = cols[col_idx].parse().unwrap_or(0);
            }
        }

        // Parse note counts
        let mut total_notes = [0u32; 10];
        for (i, &col_idx) in NOTE_COLS.iter().enumerate() {
            if col_idx > 0 && col_idx < cols.len() {
                total_notes[i] = cols[col_idx].parse().unwrap_or(0);
            }
        }

        let song = SongInfo {
            id: 0, // Will be filled in when matched with memory data
            title: Arc::from(title),
            title_english: Arc::from(""),
            artist: Arc::from(""),
            genre: Arc::from(""),
            bpm: Arc::from(""),
            folder: 0,
            levels,
            total_notes,
            embedded_ex_scores: [0u32; 10],
            embedded_lamps: [0u32; 10],
            unlock_type: UnlockType::default(),
        };

        result.insert(Arc::from(title), song);
    }

    info!("Loaded {} songs from TSV file", result.len());
    Ok(result)
}

/// Merge memory-based song_id->title map with TSV-based title->SongInfo
///
/// This creates a complete song database by:
/// 1. Using song_id from memory scan
/// 2. Looking up song details from TSV by title
pub fn merge_song_databases(
    id_to_title: &HashMap<u32, Arc<str>>,
    tsv_db: &HashMap<Arc<str>, SongInfo>,
) -> HashMap<u32, SongInfo> {
    let mut result = HashMap::new();

    for (&song_id, title) in id_to_title {
        if let Some(tsv_song) = tsv_db.get(title) {
            let mut song = tsv_song.clone();
            song.id = song_id;
            result.insert(song_id, song);
        } else {
            // Song not in TSV, create minimal entry
            debug!("Song {} ({}) not found in TSV database", song_id, title);
            result.insert(
                song_id,
                SongInfo {
                    id: song_id,
                    title: title.clone(),
                    ..Default::default()
                },
            );
        }
    }

    info!(
        "Merged song database: {} songs (from {} memory mappings, {} TSV entries)",
        result.len(),
        id_to_title.len(),
        tsv_db.len()
    );
    result
}

/// Build song database with TSV as primary source
///
/// Strategy:
/// 1. Load TSV for complete song metadata (1749+ songs)
/// 2. Scan memory for song_id -> title mappings
/// 3. Match TSV entries to song_ids by title
/// 4. For unmatched TSV entries, create placeholder entries
///
/// This ensures we have complete song data even with lazy-loaded versions.
pub fn build_song_database_from_tsv_with_memory<R: ReadMemory>(
    reader: &R,
    song_list_addr: u64,
    tsv_path: &str,
    scan_size: usize,
) -> HashMap<u32, SongInfo> {
    use std::path::Path;

    // Step 1: Load TSV database
    let tsv_db = if Path::new(tsv_path).exists() {
        match load_song_database_from_tsv(tsv_path) {
            Ok(db) => {
                info!("Loaded {} songs from TSV", db.len());
                db
            }
            Err(e) => {
                warn!("Failed to load TSV: {}", e);
                HashMap::new()
            }
        }
    } else {
        debug!("TSV file not found: {}", tsv_path);
        HashMap::new()
    };

    // Step 2: Scan memory for song_id -> title mappings
    let memory_songs = fetch_song_database_from_memory_scan(
        reader,
        song_list_addr,
        scan_size,
        SongInfo::MEMORY_SIZE,
    );
    info!("Found {} songs in memory scan", memory_songs.len());

    // Build reverse mapping: normalized_title -> song_id
    let mut title_to_id: HashMap<String, u32> = HashMap::new();
    for song in memory_songs.values() {
        let normalized = normalize_title_for_matching(&song.title);
        title_to_id.insert(normalized, song.id);
    }

    // Debug: show first entries from each source
    if let Some(first_mem) = memory_songs.values().next() {
        debug!(
            "First memory title: {:?} (normalized: {:?})",
            first_mem.title,
            normalize_title_for_matching(&first_mem.title)
        );
    }
    if let Some((first_tsv_title, _)) = tsv_db.iter().next() {
        debug!(
            "First TSV title: {:?} (normalized: {:?})",
            first_tsv_title,
            normalize_title_for_matching(first_tsv_title)
        );
    }
    debug!(
        "title_to_id has {} entries, tsv_db has {} entries",
        title_to_id.len(),
        tsv_db.len()
    );

    // Step 3: Match TSV entries with song_ids
    let mut result: HashMap<u32, SongInfo> = HashMap::new();
    let mut matched_count = 0usize;
    let mut unmatched_titles: Vec<String> = Vec::new();

    for (title, tsv_song) in &tsv_db {
        let normalized = normalize_title_for_matching(title);

        if let Some(&song_id) = title_to_id.get(&normalized) {
            // Found a match - use TSV data with memory-derived song_id
            let memory_song = memory_songs.get(&song_id);
            let mut song = tsv_song.clone();
            song.id = song_id;

            // Use memory data for folder if available
            if let Some(mem) = memory_song {
                song.folder = mem.folder;
                // Prefer memory levels if available
                if mem.levels.iter().any(|&l| l > 0) {
                    song.levels = mem.levels;
                }
            }

            result.insert(song_id, song);
            matched_count += 1;
        } else {
            // No match found - track for logging
            unmatched_titles.push(title.to_string());
        }
    }

    // Step 4: Add memory-only songs (not in TSV)
    for (song_id, song) in &memory_songs {
        if !result.contains_key(song_id) {
            result.insert(*song_id, song.clone());
        }
    }

    info!(
        "Song database built: {} total ({} matched with TSV, {} TSV-only, {} memory-only)",
        result.len(),
        matched_count,
        unmatched_titles.len(),
        memory_songs.len().saturating_sub(matched_count)
    );

    if !unmatched_titles.is_empty() && unmatched_titles.len() <= 20 {
        debug!("Unmatched TSV titles: {:?}", unmatched_titles);
    } else if !unmatched_titles.is_empty() {
        debug!(
            "Unmatched TSV titles: {} (showing first 10: {:?})",
            unmatched_titles.len(),
            &unmatched_titles[..10.min(unmatched_titles.len())]
        );
    }

    result
}

/// Normalize a title for matching
///
/// Removes whitespace, converts to lowercase, and removes certain punctuation
/// to improve matching between memory and TSV titles.
pub(crate) fn normalize_title_for_matching(title: &str) -> String {
    title
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .filter(|c| c.is_alphanumeric() || *c > '\u{007F}') // Keep non-ASCII (Japanese)
        .collect()
}
