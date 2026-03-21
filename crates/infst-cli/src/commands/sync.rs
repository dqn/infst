//! Sync command for reading game memory and uploading directly to the web service.
//!
//! Reads titles and game_ids from the text table (stride 0x630 at `offsets.song_list`),
//! then looks up score data from DataMap/ScoreMap using those game_ids.
//! This avoids the game_id/internal_id mismatch by reading game_id from the text table.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::time::Duration;

use super::upload::resolve_credentials;
use crate::cli_utils;
use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use infst::{
    MemoryReader, OffsetSearcher, ReadMemory, ScoreMap, chart::Difficulty,
    decode_shift_jis_to_string, score::Lamp,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Clone)]
struct LampEntry {
    #[serde(rename = "songId")]
    song_id: u32,
    title: String,
    difficulty: String,
    lamp: String,
    #[serde(rename = "exScore")]
    ex_score: u32,
    #[serde(rename = "missCount", skip_serializing_if = "Option::is_none")]
    miss_count: Option<u32>,
}

const ALL_DIFFICULTIES: [Difficulty; 10] = [
    Difficulty::SpB,
    Difficulty::SpN,
    Difficulty::SpH,
    Difficulty::SpA,
    Difficulty::SpL,
    Difficulty::DpB,
    Difficulty::DpN,
    Difficulty::DpH,
    Difficulty::DpA,
    Difficulty::DpL,
];

/// Text table layout constants (stride 0x630 = 1584 bytes per entry)
const TEXT_TABLE_STRIDE: usize = 0x630;
/// V3 title within this text table block belongs to the NEXT entry's game_id.
/// So for entry i, the V3 title at 0x5B0 is the title for entry (i+1)'s game_id.
/// We read title from offset 0x5B0 of the PREVIOUS entry (= current entry - stride + 0x5B0).
/// Equivalently, we shift the title read by -1 entry.
const TEXT_TABLE_V3_TITLE_OFFSET: usize = 0x5B0;
const TEXT_TABLE_TITLE_SIZE: usize = 64;
const TEXT_TABLE_LEVELS_OFFSET: usize = 0x160;
const TEXT_TABLE_LEVELS_SIZE: usize = 10;
const TEXT_TABLE_GAME_ID_OFFSET: usize = 0x430;
const TEXT_TABLE_MAX_ENTRIES: usize = 5000;
const TEXT_TABLE_MAX_CONSECUTIVE_EMPTY: usize = 10;

// --- Sync cache for differential sync ---

const SYNC_CACHE_FILENAME: &str = "sync-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CachedEntry {
    lamp: String,
    ex_score: u32,
    miss_count: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SyncCache {
    entries: HashMap<String, CachedEntry>,
}

impl SyncCache {
    fn cache_path() -> Option<std::path::PathBuf> {
        let cache_dir = dirs::cache_dir()?.join("infst");
        Some(cache_dir.join(SYNC_CACHE_FILENAME))
    }

    fn load() -> Option<Self> {
        let path = Self::cache_path()?;
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save(&self) {
        let Some(path) = Self::cache_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(content) = serde_json::to_string(self) else {
            return;
        };
        let tmp_path = path.with_extension("tmp");
        if let Err(e) = fs::write(&tmp_path, &content).and_then(|()| fs::rename(&tmp_path, &path)) {
            eprintln!("Warning: failed to save sync cache: {}", e);
        }
    }

    fn make_key(title: &str, difficulty: &str) -> String {
        format!("{}:{}", title, difficulty)
    }
}

/// Data extracted from a single text table entry.
struct TextTableEntry {
    title: String,
    levels: [u8; 10],
    game_id: u32,
}

/// Read all entries from the "5.1.1." text table.
fn read_text_table(reader: &MemoryReader, song_list_addr: u64) -> Result<Vec<TextTableEntry>> {
    // Bulk read the entire table to minimize process memory reads.
    // Each entry is TEXT_TABLE_STRIDE bytes; read up to MAX_ENTRIES.
    let total_size = TEXT_TABLE_STRIDE * TEXT_TABLE_MAX_ENTRIES;
    let buffer = reader
        .read_bytes(song_list_addr, total_size)
        .context("Failed to read text table from memory")?;

    let mut entries = Vec::new();
    let mut consecutive_empty = 0;

    // Start from entry 1: entry 0 has no previous entry for V3 title lookup
    // (V3 title is at entry (i-1) + 0x5B0), so it always produces an empty title.
    for i in 1..TEXT_TABLE_MAX_ENTRIES {
        let base = i * TEXT_TABLE_STRIDE;
        if base + TEXT_TABLE_STRIDE > buffer.len() {
            break;
        }

        let entry_buf = &buffer[base..base + TEXT_TABLE_STRIDE];

        // Read V3 title from the PREVIOUS entry's 0x5B0 offset.
        // Entry i's game_id corresponds to the V3 title at entry (i-1) + 0x5B0.
        let prev_base = (i - 1) * TEXT_TABLE_STRIDE;
        let title_start = prev_base + TEXT_TABLE_V3_TITLE_OFFSET;
        let title = if title_start + TEXT_TABLE_TITLE_SIZE <= buffer.len() {
            decode_shift_jis_to_string(&buffer[title_start..title_start + TEXT_TABLE_TITLE_SIZE])
                .trim()
                .to_string()
        } else {
            String::new()
        };

        if title.is_empty() {
            consecutive_empty += 1;
            if consecutive_empty >= TEXT_TABLE_MAX_CONSECUTIVE_EMPTY {
                break;
            }
            continue;
        }
        consecutive_empty = 0;

        // Read levels (10 bytes at offset 0x160)
        let mut levels = [0u8; 10];
        levels.copy_from_slice(
            &entry_buf[TEXT_TABLE_LEVELS_OFFSET..TEXT_TABLE_LEVELS_OFFSET + TEXT_TABLE_LEVELS_SIZE],
        );

        // Read game_id (u32 at offset 0x430)
        let game_id = u32::from_le_bytes(
            entry_buf[TEXT_TABLE_GAME_ID_OFFSET..TEXT_TABLE_GAME_ID_OFFSET + 4]
                .try_into()
                .unwrap(),
        );

        // Skip entries with invalid game_id (empty slots in the table)
        if game_id == 0 || !(1000..=90000).contains(&game_id) {
            continue;
        }

        // Apply encoding fixes (same as SongInfo::parse_entry for V3 title)
        let title = infst::chart::fix_title_encoding(&title)
            .map(|arc| arc.to_string())
            .unwrap_or(title);

        // Skip if title is empty after decoding (V3 title field may be empty for some entries)
        if title.is_empty() {
            continue;
        }

        entries.push(TextTableEntry {
            title,
            levels,
            game_id,
        });
    }

    Ok(entries)
}

pub fn run(endpoint: Option<&str>, token: Option<&str>, pid: Option<u32>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    eprintln!("infst {} - Sync Mode", current_version);

    // Resolve credentials
    let (resolved_endpoint, resolved_token) = resolve_credentials(endpoint, token)?;

    let process = cli_utils::open_process(pid)?;

    eprintln!(
        "Found process (PID: {}, Base: 0x{:X})",
        process.pid, process.base_address
    );

    let reader = MemoryReader::new(&process);
    // Need song_list (for text table) and data_map (for ScoreMap)
    let mut searcher = OffsetSearcher::new(&reader);
    let offsets = searcher.search_sync_offsets()?;
    eprintln!("Offsets detected");

    // Read text table: title + levels + game_id per entry
    eprintln!("Reading text table...");
    let text_entries = read_text_table(&reader, offsets.song_list)?;
    eprintln!("Read {} entries from text table", text_entries.len());

    // Load ScoreMap from DataMap (keyed by game_id)
    // ScoreMap::load_from_memory doesn't use the song_db parameter, so pass an empty map
    // to skip the expensive ~7.5MB song database read.
    eprintln!("Loading score data...");
    let score_map = ScoreMap::load_from_memory(&reader, offsets.data_map, &HashMap::new())?;
    eprintln!("Loaded {} score entries", score_map.len());

    // Build LampEntry list: text table provides game_id + levels + V3 title,
    // ScoreMap provides lamps + EX scores.
    // V3 title (at offset 0x5B0) matches export/charts since both come from V3 entry table.
    let mut entries: Vec<LampEntry> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for text_entry in &text_entries {
        // ScoreMap is keyed by game_id (from DataMap linked list), and text_entry.game_id
        // also comes from the text table. Both use game_id natively, so no
        // apply_game_id_mapping (game_id -> internal_id remapping) is needed here.
        let score_data = match score_map.get(text_entry.game_id) {
            Some(data) => data,
            None => continue,
        };

        for &diff in &ALL_DIFFICULTIES {
            let diff_idx = diff as usize;

            // Sync only level 11/12 charts
            let level = text_entry.levels[diff_idx];
            if level != 11 && level != 12 {
                continue;
            }

            let lamp = score_data.get_lamp(diff);

            // Skip NO PLAY
            if lamp == Lamp::NoPlay {
                continue;
            }

            let diff_name = diff.short_name().to_string();
            if !seen.insert((text_entry.title.clone(), diff_name.clone())) {
                continue;
            }

            entries.push(LampEntry {
                song_id: 0, // not used for title-based JOIN
                title: text_entry.title.clone(),
                difficulty: diff_name,
                lamp: lamp.short_name().to_string(),
                ex_score: score_data.get_score(diff),
                miss_count: score_data.miss_count[diff_idx],
            });
        }
    }

    if entries.is_empty() {
        println!("No play data found to sync.");
        return Ok(());
    }

    // Differential sync: filter to changed entries only
    let cache = SyncCache::load();
    let entries_to_send: Vec<LampEntry> = if let Some(ref cache) = cache {
        entries
            .iter()
            .filter(|e| {
                let key = SyncCache::make_key(&e.title, &e.difficulty);
                match cache.entries.get(&key) {
                    Some(cached) => {
                        cached.lamp != e.lamp
                            || cached.ex_score != e.ex_score
                            || cached.miss_count != e.miss_count
                    }
                    None => true, // New entry
                }
            })
            .cloned()
            .collect()
    } else {
        entries.clone()
    };

    if entries_to_send.is_empty() {
        println!("No changes detected since last sync.");
        return Ok(());
    }

    eprintln!(
        "Uploading {} entries ({} total, {} changed)...",
        entries_to_send.len(),
        entries.len(),
        entries_to_send.len()
    );

    // POST /api/lamps/bulk with gzip compression
    let url = format!("{}/api/lamps/bulk", resolved_endpoint.trim_end_matches('/'));
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let body = serde_json::json!({ "entries": entries_to_send });
    let json_bytes = serde_json::to_vec(&body).context("Failed to serialize JSON")?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&json_bytes)
        .context("Failed to compress data")?;
    let compressed = encoder.finish().context("Failed to finish compression")?;

    eprintln!(
        "Payload: {} bytes -> {} bytes (gzip)",
        json_bytes.len(),
        compressed.len()
    );

    let mut response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", resolved_token))
        .header("Content-Type", "application/json")
        .header("Content-Encoding", "gzip")
        .send(compressed.as_slice())
        .context("Failed to upload data")?;

    let status = response.status();
    if status != 200 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        anyhow::bail!("Upload failed (HTTP {}): {}", status, body);
    }

    println!("Sync complete (status: {})", status);
    println!("Synced {} entries.", entries_to_send.len());

    // Update cache with all current entries
    let mut new_cache = SyncCache {
        entries: HashMap::new(),
    };
    for e in &entries {
        let key = SyncCache::make_key(&e.title, &e.difficulty);
        new_cache.entries.insert(
            key,
            CachedEntry {
                lamp: e.lamp.clone(),
                ex_score: e.ex_score,
                miss_count: e.miss_count,
            },
        );
    }
    new_cache.save();

    Ok(())
}
