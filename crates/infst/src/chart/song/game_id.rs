use std::collections::HashMap;

use tracing::{debug, info};

use crate::process::ReadMemory;

use super::SongInfo;
use super::layout::EntryLayout;

const ENTRY_EX_SCORE_COUNT: usize = 10;

/// Build game_id -> internal_id mapping by comparing EX scores.
///
/// In V3, the DataMap (score system) uses game_ids while the entry table
/// uses internal_ids. These often differ (e.g., game_id=32001 for ADAMANT
/// but internal_id=32000). By matching EX scores between the two sources,
/// we detect mismatches and build a correction mapping.
///
/// To avoid false positives (scores are not unique across songs), this function:
/// 1. Skips game_ids where internal_id == game_id and scores match (self-match)
/// 2. Requires exactly one matching entry per game_id (uniqueness check)
/// 3. Rejects mappings where multiple game_ids claim the same internal_id
///
/// Returns: HashMap<game_id, internal_id> for reliably detected mismatches.
pub fn build_game_id_index<R: ReadMemory>(
    reader: &R,
    entry_table_addr: u64,
    entry_stride: usize,
    score_map: &crate::score::ScoreMap,
    _song_db: &HashMap<u32, SongInfo>,
) -> HashMap<u32, u32> {
    build_game_id_index_with_layout(
        reader,
        entry_table_addr,
        entry_stride,
        score_map,
        _song_db,
        &EntryLayout::v3_default(),
    )
}

/// Build game_id -> internal_id mapping using a detected entry layout.
pub fn build_game_id_index_with_layout<R: ReadMemory>(
    reader: &R,
    entry_table_addr: u64,
    entry_stride: usize,
    score_map: &crate::score::ScoreMap,
    _song_db: &HashMap<u32, SongInfo>,
    layout: &EntryLayout,
) -> HashMap<u32, u32> {
    let ex_score_offset = layout.ex_scores.unwrap_or(0x3F0) as u64;
    let song_id_offset = layout.song_id as u64;

    // Read all entry table EX scores: internal_id -> [10 x u32]
    let mut entry_scores: HashMap<u32, [u32; ENTRY_EX_SCORE_COUNT]> = HashMap::new();
    for i in 0..5000u64 {
        let addr = entry_table_addr + i * entry_stride as u64;
        let id = match reader.read_i32(addr + song_id_offset) {
            Ok(id) if (1000..=90000).contains(&id) => id as u32,
            Ok(0) => continue,
            _ => break,
        };

        if let Ok(bytes) = reader.read_bytes(addr + ex_score_offset, 40) {
            let mut scores = [0u32; ENTRY_EX_SCORE_COUNT];
            for (j, score) in scores.iter_mut().enumerate() {
                *score = u32::from_le_bytes(bytes[j * 4..j * 4 + 4].try_into().unwrap());
            }
            if scores.iter().any(|&s| s > 0) {
                entry_scores.insert(id, scores);
            }
        }
    }

    if entry_scores.is_empty() {
        return HashMap::new();
    }

    // Phase 1: For each game_id, collect all matching internal_ids.
    let mut checked = 0u32;
    let mut self_matched = 0u32;
    // game_id -> list of matching internal_ids (excluding self)
    let mut candidates: HashMap<u32, Vec<u32>> = HashMap::new();

    for (&game_id, score_data) in score_map.iter() {
        if score_data.score.iter().all(|&s| s == 0) {
            continue;
        }

        let game_nonzero: Vec<(usize, u32)> = score_data
            .score
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s > 0)
            .map(|(i, &s)| (i, s))
            .collect();

        // Self-match: if entry with internal_id == game_id has matching scores,
        // no mapping is needed (game_id IS the internal_id for this song).
        if let Some(self_ex) = entry_scores.get(&game_id)
            && game_nonzero
                .iter()
                .all(|&(idx, score)| self_ex[idx] == score)
        {
            self_matched += 1;
            checked += 1;
            continue;
        }

        // Collect ALL entries with matching scores (excluding self)
        let matching_iids: Vec<u32> = entry_scores
            .iter()
            .filter(|(iid, ex)| {
                **iid != game_id && game_nonzero.iter().all(|&(idx, score)| ex[idx] == score)
            })
            .map(|(iid, _)| *iid)
            .collect();

        if !matching_iids.is_empty() {
            candidates.insert(game_id, matching_iids);
        }
        checked += 1;
    }

    // Phase 2: Accept only unique matches (exactly 1 candidate).
    let mut mapping = HashMap::new();
    let mut ambiguous = 0u32;

    for (game_id, iids) in &candidates {
        if iids.len() == 1 {
            mapping.insert(*game_id, iids[0]);
        } else {
            debug!(
                "game_id={}: ambiguous, matches {} entries {:?} (skipped)",
                game_id,
                iids.len(),
                iids
            );
            ambiguous += 1;
        }
    }

    // Phase 3: Detect reverse conflicts (multiple game_ids -> same internal_id).
    let mut reverse_map: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&game_id, &internal_id) in &mapping {
        reverse_map.entry(internal_id).or_default().push(game_id);
    }

    let mut conflicts = 0u32;
    for (internal_id, game_ids) in &reverse_map {
        if game_ids.len() > 1 {
            debug!(
                "internal_id={}: conflict, claimed by game_ids {:?} (all removed)",
                internal_id, game_ids
            );
            for gid in game_ids {
                mapping.remove(gid);
            }
            conflicts += 1;
        }
    }

    info!(
        "Game ID mapping: {} reliable, {} self-matched, {} ambiguous, {} conflicts (checked {})",
        mapping.len(),
        self_matched,
        ambiguous,
        conflicts,
        checked
    );

    mapping
}

/// Apply game_id -> internal_id mapping to the song database.
///
/// For each mapping entry, clones the SongInfo from the internal_id key
/// and inserts/overwrites it under the game_id key (with the id field updated).
/// This fixes cases where game_id X pointed to the wrong song because
/// another song happened to have internal_id X.
pub fn apply_game_id_mapping(song_db: &mut HashMap<u32, SongInfo>, mapping: &HashMap<u32, u32>) {
    for (&game_id, &internal_id) in mapping {
        if let Some(song) = song_db.get(&internal_id).cloned() {
            let mut aliased = song;
            aliased.id = game_id;
            debug!(
                "song_db[{}] = {:?} (was internal_id={})",
                game_id, aliased.title, internal_id
            );
            song_db.insert(game_id, aliased);
        }
    }

    if !mapping.is_empty() {
        info!(
            "Applied {} game_id corrections to song database",
            mapping.len()
        );
    }
}
