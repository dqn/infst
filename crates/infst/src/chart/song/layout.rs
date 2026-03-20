//! Automatic detection of song entry field layout.
//!
//! Different INFINITAS versions use different field offsets within song entries.
//! This module detects the layout by analyzing multiple raw entries, eliminating
//! the need for hardcoded constants.

use serde::{Deserialize, Serialize};
use tracing::debug;

/// Detected field offsets within a song entry.
///
/// Each field records the byte offset from the start of the entry.
/// Optional fields are `None` when detection did not succeed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryLayout {
    /// Total entry size in bytes.
    pub entry_size: usize,

    // --- Required fields ---
    /// Offset of song_id (i32, 1000-50000).
    pub song_id: usize,
    /// Offset of folder (i32, 1-200). Always song_id + 4.
    pub folder: usize,
    /// Offset of title (64-byte Shift-JIS block).
    pub title: usize,
    /// Offset of levels array (10 bytes, each 0-12).
    pub levels: usize,

    // --- Optional text fields (64 bytes each) ---
    pub title_english: Option<usize>,
    pub genre: Option<usize>,
    pub artist: Option<usize>,

    // --- Optional score/metadata fields ---
    /// Offset of BPM/notes array (10 x u32).
    pub bpm_notes: Option<usize>,
    /// Stride between BPM/notes values (typically 4 or 8 bytes).
    pub bpm_notes_stride: usize,
    /// Offset of EX scores (10 x u32, contiguous).
    pub ex_scores: Option<usize>,
    /// Offset of clear lamps (10 x u32, contiguous).
    pub lamps: Option<usize>,
}

impl Default for EntryLayout {
    fn default() -> Self {
        Self::v3_default()
    }
}

impl EntryLayout {
    /// The text block size used in all known versions (64 bytes).
    const TEXT_BLOCK_SIZE: usize = 64;

    /// Returns the V3 hardcoded layout (current fallback).
    pub fn v3_default() -> Self {
        Self {
            entry_size: 0x630,
            song_id: 0x000,
            folder: 0x004,
            title: 0x180,
            levels: 0x360,
            title_english: Some(0x200),
            genre: Some(0x240),
            artist: Some(0x2C0),
            bpm_notes: Some(0x378),
            bpm_notes_stride: 8,
            ex_scores: Some(0x3F0),
            lamps: Some(0x430),
        }
    }

    /// Detect field layout by analyzing a contiguous buffer of raw entries.
    ///
    /// `buffer` must contain at least 3 consecutive entries, each of `entry_size` bytes.
    /// Returns `None` if detection fails.
    pub fn detect(buffer: &[u8], entry_size: usize) -> Option<Self> {
        if entry_size < 64 || buffer.len() < entry_size * 3 {
            return None;
        }

        let entry_count = buffer.len() / entry_size;
        let entries: Vec<&[u8]> = (0..entry_count)
            .map(|i| &buffer[i * entry_size..(i + 1) * entry_size])
            .collect();

        // Step 1: Detect song_id + folder
        let song_id_offset = detect_song_id_offset(&entries)?;
        let folder_offset = song_id_offset + 4;
        debug!(
            "EntryLayout: detected song_id at 0x{:X}, folder at 0x{:X}",
            song_id_offset, folder_offset
        );

        // Step 2: Detect text blocks
        let text_blocks = detect_text_blocks(&entries);
        let title_offset = *text_blocks.first()?;
        debug!(
            "EntryLayout: detected {} text blocks, title at 0x{:X}",
            text_blocks.len(),
            title_offset
        );

        // Step 3: Detect levels (scan after the text region)
        let text_region_end = text_blocks
            .last()
            .map_or(0, |&off| off + Self::TEXT_BLOCK_SIZE);
        let levels_offset = detect_levels_offset(&entries, text_region_end)?;
        debug!("EntryLayout: detected levels at 0x{:X}", levels_offset);

        // Step 4: Assign text block roles based on cluster position
        let (title_english, genre, artist) = assign_text_roles(&text_blocks);

        // Step 5: Detect score fields (optional)
        let (bpm_notes, bpm_notes_stride) =
            detect_bpm_notes(&entries, levels_offset).unwrap_or((None, 8));

        // EX scores start after BPM/notes array (or after levels if BPM not detected)
        let ex_scores_scan_start = bpm_notes
            .map(|off| off + 10 * bpm_notes_stride)
            .unwrap_or(levels_offset + 24);
        let ex_scores = detect_u32_array(&entries, ex_scores_scan_start, 0..200_000);

        // Lamps start after EX scores (or after BPM region)
        let lamps_scan_start = ex_scores.map_or(ex_scores_scan_start + 40, |o| o + 40);
        let lamps = detect_u32_array(&entries, lamps_scan_start, 0..8);

        Some(EntryLayout {
            entry_size,
            song_id: song_id_offset,
            folder: folder_offset,
            title: title_offset,
            levels: levels_offset,
            title_english,
            genre,
            artist,
            bpm_notes,
            bpm_notes_stride,
            ex_scores,
            lamps,
        })
    }
}

// ---------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------

/// Detect the offset of song_id within entries.
///
/// Scans for `[i32(1000-50000), i32(1-200)]` pairs at 4-byte alignment.
/// The offset must be consistent across all entries, and song_ids must differ.
fn detect_song_id_offset(entries: &[&[u8]]) -> Option<usize> {
    let entry_size = entries[0].len();

    // Collect candidate offsets where all entries have valid [song_id, folder]
    let mut best_offset: Option<usize> = None;
    let mut best_unique_ids = 0;

    for offset in (0..entry_size.saturating_sub(8)).step_by(4) {
        let mut ids = Vec::new();
        let mut all_valid = true;

        for entry in entries {
            let sid =
                i32::from_le_bytes(entry[offset..offset + 4].try_into().ok().unwrap_or([0; 4]));
            let fld = i32::from_le_bytes(
                entry[offset + 4..offset + 8]
                    .try_into()
                    .ok()
                    .unwrap_or([0; 4]),
            );

            if !(1000..=50000).contains(&sid) || !(1..=200).contains(&fld) {
                all_valid = false;
                break;
            }
            ids.push(sid);
        }

        if !all_valid {
            continue;
        }

        // Count unique song_ids -- they must differ across entries
        ids.sort();
        ids.dedup();
        let unique_count = ids.len();

        if unique_count > best_unique_ids {
            best_unique_ids = unique_count;
            best_offset = Some(offset);
        }
    }

    // Require at least 2 unique song_ids to avoid false positives
    if best_unique_ids >= 2 {
        best_offset
    } else {
        None
    }
}

/// Detect text block offsets by scanning for valid Shift-JIS strings.
///
/// Returns a sorted list of offsets where valid text was found in at least
/// half of the entries.
fn detect_text_blocks(entries: &[&[u8]]) -> Vec<usize> {
    let entry_size = entries[0].len();
    let block_size = EntryLayout::TEXT_BLOCK_SIZE;
    let mut text_offsets: Vec<usize> = Vec::new();

    for offset in (0..entry_size.saturating_sub(block_size)).step_by(block_size) {
        let mut valid_count = 0;
        let mut non_empty_count = 0;

        for entry in entries {
            let block = &entry[offset..offset + block_size];
            if is_valid_text_block(block) {
                valid_count += 1;
                if has_meaningful_text(block) {
                    non_empty_count += 1;
                }
            }
        }

        // Require valid text in at least half of entries, and non-empty in at least one
        if valid_count >= entries.len() / 2 && non_empty_count >= 1 {
            text_offsets.push(offset);
        }
    }

    text_offsets
}

/// Check if a 64-byte block contains valid Shift-JIS text.
///
/// Valid means: the bytes before the null terminator are valid Shift-JIS,
/// and the decoded result contains printable characters.
fn is_valid_text_block(block: &[u8]) -> bool {
    // Find null terminator
    let len = block.iter().position(|&b| b == 0).unwrap_or(block.len());
    if len == 0 {
        // All zeros is "valid" (empty string)
        return true;
    }

    let text_bytes = &block[..len];

    // Decode as Shift-JIS
    let (decoded, _, had_errors) = encoding_rs::SHIFT_JIS.decode(text_bytes);
    if had_errors {
        return false;
    }

    // Check if decoded text contains printable characters
    decoded
        .chars()
        .all(|c| c.is_alphanumeric() || c.is_whitespace() || is_printable_symbol(c))
}

/// Check if decoded text has meaningful content (not just whitespace/zeros).
fn has_meaningful_text(block: &[u8]) -> bool {
    let len = block.iter().position(|&b| b == 0).unwrap_or(block.len());
    if len < 2 {
        return false;
    }

    let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&block[..len]);
    decoded.chars().any(|c| c.is_alphanumeric())
}

/// Check if a character is a printable symbol (not control character).
fn is_printable_symbol(c: char) -> bool {
    !c.is_control() && !c.is_alphanumeric() && !c.is_whitespace()
}

/// Assign text block roles (title_english, genre, artist) based on position.
///
/// The first block is title (already known). Remaining blocks in a contiguous
/// cluster are assigned in order:
///   - title_english: 2nd valid block (skip 1 for unknown)
///   - genre: 3rd valid block (skip 1 for unknown)
///   - artist: 5th valid block (skip 1 for unknown)
///
/// This mapping works for V3's 6-block layout:
///   [title, unknown, title_en, genre, unknown, artist]
fn assign_text_roles(text_blocks: &[usize]) -> (Option<usize>, Option<usize>, Option<usize>) {
    if text_blocks.len() < 2 {
        return (None, None, None);
    }

    let block_size = EntryLayout::TEXT_BLOCK_SIZE;

    // Find the contiguous cluster starting from the first block
    let start = text_blocks[0];
    let cluster: Vec<usize> = text_blocks
        .iter()
        .copied()
        .filter(|&off| off >= start && (off - start).is_multiple_of(block_size))
        .collect();

    match cluster.len() {
        0..=1 => (None, None, None),
        2 => (Some(cluster[1]), None, None),
        3 => (Some(cluster[1]), Some(cluster[2]), None),
        4 => (Some(cluster[1]), Some(cluster[2]), Some(cluster[3])),
        5 => (Some(cluster[1]), Some(cluster[2]), Some(cluster[4])),
        // 6+ blocks: skip unknowns at positions 1 and 4
        _ => (Some(cluster[2]), Some(cluster[3]), Some(cluster[5])),
    }
}

/// Detect levels array offset.
///
/// Scans for 10 consecutive bytes where each is in [0, 12], with at least 2
/// non-zero values. Cross-validates across entries: same offset, different patterns.
fn detect_levels_offset(entries: &[&[u8]], after_offset: usize) -> Option<usize> {
    let entry_size = entries[0].len();
    let mut best_offset: Option<usize> = None;
    let mut best_score = 0;

    // Scan from after the text region to avoid false positives
    let scan_start = after_offset;

    // Scan at 4-byte alignment (struct fields are always aligned in all known versions)
    for offset in (scan_start..entry_size.saturating_sub(10)).step_by(4) {
        let mut all_valid = true;
        let mut total_nonzero = 0;
        let mut patterns: Vec<[u8; 10]> = Vec::new();

        for entry in entries {
            let bytes = &entry[offset..offset + 10];
            let mut pattern = [0u8; 10];
            pattern.copy_from_slice(bytes);

            // All bytes must be in [0, 12]
            if !bytes.iter().all(|&b| b <= 12) {
                all_valid = false;
                break;
            }

            let nonzero = bytes.iter().filter(|&&b| b > 0).count();
            total_nonzero += nonzero;
            patterns.push(pattern);
        }

        if !all_valid {
            continue;
        }

        // Require at least 2 non-zero values total across all entries
        if total_nonzero < 2 {
            continue;
        }

        // Score: prefer offsets where more entries have non-zero values
        // and where patterns differ between entries
        let unique_patterns = {
            let mut sorted = patterns.clone();
            sorted.sort();
            sorted.dedup();
            sorted.len()
        };

        let score = total_nonzero * 10 + unique_patterns;
        if score > best_score {
            best_score = score;
            best_offset = Some(offset);
        }
    }

    best_offset
}

/// Detect BPM/notes array: 10 x u32 values with a consistent stride.
///
/// Tries both 4-byte and 8-byte strides. Returns (offset, stride).
fn detect_bpm_notes(entries: &[&[u8]], after_offset: usize) -> Option<(Option<usize>, usize)> {
    let entry_size = entries[0].len();

    for stride in [8, 4] {
        let total_size = 10 * stride;
        // Start after levels (10 bytes), aligned up to 4-byte boundary
        let scan_start = (after_offset + 10 + 3) & !3;

        for offset in (scan_start..entry_size.saturating_sub(total_size)).step_by(4) {
            let mut all_valid = true;

            for entry in entries {
                for i in 0..10 {
                    let off = offset + i * stride;
                    let val =
                        u32::from_le_bytes(entry[off..off + 4].try_into().ok().unwrap_or([0; 4]));
                    // BPM values typically 50-400, notes 0-10000
                    if val > 50000 {
                        all_valid = false;
                        break;
                    }
                }
                if !all_valid {
                    break;
                }
            }

            if all_valid {
                // Require the first value to be non-zero in at least half of entries.
                // This avoids false positives from padding bytes before the real array.
                let first_nonzero_count = entries
                    .iter()
                    .filter(|entry| {
                        u32::from_le_bytes(
                            entry[offset..offset + 4].try_into().ok().unwrap_or([0; 4]),
                        ) > 0
                    })
                    .count();

                if first_nonzero_count >= entries.len() / 2 {
                    return Some((Some(offset), stride));
                }
            }
        }
    }

    None
}

/// Detect a contiguous array of 10 x u32 values within a value range.
///
/// Requires at least one entry to have a non-zero value to avoid matching padding.
fn detect_u32_array(
    entries: &[&[u8]],
    after_offset: usize,
    range: std::ops::Range<u32>,
) -> Option<usize> {
    let entry_size = entries[0].len();
    let array_size = 10 * 4;

    for offset in (after_offset..entry_size.saturating_sub(array_size)).step_by(4) {
        let mut all_valid = true;
        let mut max_nonzero_in_entry = 0usize;

        for entry in entries {
            let mut nonzero_count = 0;
            for i in 0..10 {
                let off = offset + i * 4;
                let val = u32::from_le_bytes(entry[off..off + 4].try_into().ok().unwrap_or([0; 4]));
                if !range.contains(&val) {
                    all_valid = false;
                    break;
                }
                if val > 0 {
                    nonzero_count += 1;
                }
            }
            if !all_valid {
                break;
            }
            max_nonzero_in_entry = max_nonzero_in_entry.max(nonzero_count);
        }

        // Require: (a) first value is non-zero in at least one entry, and
        // (b) at least 3 non-zero values in the best entry.
        // This avoids matching padding that partially overlaps with real data.
        let first_val_nonzero = entries.iter().any(|entry| {
            u32::from_le_bytes(entry[offset..offset + 4].try_into().ok().unwrap_or([0; 4])) > 0
        });
        if all_valid && first_val_nonzero && max_nonzero_in_entry >= 3 {
            return Some(offset);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a string as Shift-JIS and write it into a buffer at the given offset.
    fn write_shift_jis(buf: &mut [u8], offset: usize, max_len: usize, text: &str) {
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        let len = encoded.len().min(max_len);
        buf[offset..offset + len].copy_from_slice(&encoded[..len]);
    }

    /// Build a V1-style entry (title-first layout, entry size 0x3F0).
    ///
    /// V1 layout:
    ///   0x000: title (64B Shift-JIS)
    ///   0x040: title_english (64B)
    ///   0x080: genre (64B)
    ///   0x0C0: artist (64B)
    ///   0x120: levels (10 bytes)
    ///   0x270: song_id (i32)
    ///   0x274: folder (i32)
    fn build_v1_entry(song_id: i32, folder: i32, title: &str, levels: &[u8; 10]) -> Vec<u8> {
        let mut entry = vec![0u8; 0x3F0];
        write_shift_jis(&mut entry, 0x000, 64, title);
        write_shift_jis(&mut entry, 0x040, 64, "English Title");
        write_shift_jis(&mut entry, 0x080, 64, "TECHNO");
        write_shift_jis(&mut entry, 0x0C0, 64, "DJ TAKA");
        entry[0x120..0x12A].copy_from_slice(levels);
        entry[0x270..0x274].copy_from_slice(&song_id.to_le_bytes());
        entry[0x274..0x278].copy_from_slice(&folder.to_le_bytes());
        entry
    }

    /// Build a V2-style entry (title-first layout, entry size 0x4B0).
    ///
    /// V2 layout:
    ///   0x000: title (64B Shift-JIS)
    ///   0x040: title_english (64B)
    ///   0x080: genre (64B)
    ///   0x0C0: artist (64B)
    ///   0x1E0: levels (10 bytes)
    ///   0x330: song_id (i32)
    ///   0x334: folder (i32)
    fn build_v2_entry(song_id: i32, folder: i32, title: &str, levels: &[u8; 10]) -> Vec<u8> {
        let mut entry = vec![0u8; 0x4B0];
        write_shift_jis(&mut entry, 0x000, 64, title);
        write_shift_jis(&mut entry, 0x040, 64, "English Title");
        write_shift_jis(&mut entry, 0x080, 64, "TRANCE");
        write_shift_jis(&mut entry, 0x0C0, 64, "kors k");
        entry[0x1E0..0x1EA].copy_from_slice(levels);
        entry[0x330..0x334].copy_from_slice(&song_id.to_le_bytes());
        entry[0x334..0x338].copy_from_slice(&folder.to_le_bytes());
        entry
    }

    /// Build a V3-style entry (song_id-first layout, entry size 0x630).
    ///
    /// V3 layout:
    ///   0x000: song_id (i32)
    ///   0x004: folder (i32)
    ///   0x180: title (64B)
    ///   0x1C0: (unknown, empty)
    ///   0x200: title_english (64B)
    ///   0x240: genre (64B)
    ///   0x280: (unknown, empty)
    ///   0x2C0: artist (64B)
    ///   0x360: levels (10 bytes)
    ///   0x378: BPM (10 x u32, 8-byte stride)
    ///   0x3F0: EX scores (10 x u32)
    ///   0x430: lamps (10 x u32)
    fn build_v3_entry(
        song_id: i32,
        folder: i32,
        title: &str,
        levels: &[u8; 10],
        bpm: u32,
    ) -> Vec<u8> {
        let mut entry = vec![0u8; 0x630];
        entry[0x000..0x004].copy_from_slice(&song_id.to_le_bytes());
        entry[0x004..0x008].copy_from_slice(&folder.to_le_bytes());
        write_shift_jis(&mut entry, 0x180, 64, title);
        // 0x1C0 = unknown (empty)
        write_shift_jis(&mut entry, 0x200, 64, "English Title");
        write_shift_jis(&mut entry, 0x240, 64, "HARDCORE");
        // 0x280 = unknown (empty)
        write_shift_jis(&mut entry, 0x2C0, 64, "DJ YOSHITAKA");
        entry[0x360..0x36A].copy_from_slice(levels);
        // BPM: 10 x u32 at 8-byte stride
        for i in 0..10 {
            let off = 0x378 + i * 8;
            entry[off..off + 4].copy_from_slice(&bpm.to_le_bytes());
        }
        entry
    }

    fn concat_entries(entries: &[Vec<u8>]) -> Vec<u8> {
        entries.iter().flat_map(|e| e.iter().copied()).collect()
    }

    // -----------------------------------------------------------------------
    // Tests: V3 detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_v3_layout() {
        let entries = vec![
            build_v3_entry(
                1001,
                43,
                "Sleepless Days",
                &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0],
                150,
            ),
            build_v3_entry(1002, 43, "GAMBOL", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0], 170),
            build_v3_entry(1003, 43, "Memories", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0], 130),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x630).expect("V3 detection should succeed");

        assert_eq!(layout.song_id, 0x000);
        assert_eq!(layout.folder, 0x004);
        assert_eq!(layout.title, 0x180);
        assert_eq!(layout.levels, 0x360);
    }

    // -----------------------------------------------------------------------
    // Tests: V2 detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_v2_layout() {
        let entries = vec![
            build_v2_entry(1001, 43, "5.1.1.", &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0]),
            build_v2_entry(1002, 43, "GAMBOL", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0]),
            build_v2_entry(
                1003,
                43,
                "ピアノ協奏曲第1番",
                &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0],
            ),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x4B0).expect("V2 detection should succeed");

        assert_eq!(layout.song_id, 0x330);
        assert_eq!(layout.folder, 0x334);
        assert_eq!(layout.title, 0x000);
        assert_eq!(layout.levels, 0x1E0);
    }

    // -----------------------------------------------------------------------
    // Tests: V1 detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_v1_layout() {
        let entries = vec![
            build_v1_entry(1001, 43, "5.1.1.", &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0]),
            build_v1_entry(1002, 43, "GAMBOL", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0]),
            build_v1_entry(1003, 43, "Memories", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0]),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x3F0).expect("V1 detection should succeed");

        assert_eq!(layout.song_id, 0x270);
        assert_eq!(layout.folder, 0x274);
        assert_eq!(layout.title, 0x000);
        assert_eq!(layout.levels, 0x120);
    }

    // -----------------------------------------------------------------------
    // Tests: Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_fails_with_insufficient_data() {
        let entries = vec![
            build_v3_entry(1001, 43, "Test", &[0, 7, 10, 12, 0, 0, 0, 0, 0, 0], 150),
            build_v3_entry(1002, 43, "Test2", &[0, 5, 8, 11, 0, 0, 0, 0, 0, 0], 150),
        ];
        let buffer = concat_entries(&entries);
        assert!(EntryLayout::detect(&buffer, 0x630).is_none());
    }

    #[test]
    fn test_detect_fails_with_all_zeros() {
        let buffer = vec![0u8; 0x630 * 3];
        assert!(EntryLayout::detect(&buffer, 0x630).is_none());
    }

    #[test]
    fn test_v3_default_matches_current_constants() {
        let layout = EntryLayout::v3_default();
        // Must match the hardcoded constants in SongInfo
        assert_eq!(layout.entry_size, 0x630);
        assert_eq!(layout.song_id, 0x000);
        assert_eq!(layout.folder, 0x004);
        assert_eq!(layout.title, 0x180);
        assert_eq!(layout.levels, 0x360);
        assert_eq!(layout.title_english, Some(0x200));
        assert_eq!(layout.genre, Some(0x240));
        assert_eq!(layout.artist, Some(0x2C0));
        assert_eq!(layout.bpm_notes, Some(0x378));
        assert_eq!(layout.bpm_notes_stride, 8);
        assert_eq!(layout.ex_scores, Some(0x3F0));
        assert_eq!(layout.lamps, Some(0x430));
    }

    #[test]
    fn test_detect_with_japanese_titles() {
        let entries = vec![
            build_v3_entry(
                1001,
                43,
                "灼熱Beach Side Bunny",
                &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0],
                153,
            ),
            build_v3_entry(1002, 43, "冥", &[0, 5, 8, 12, 0, 0, 5, 8, 12, 0], 200),
            build_v3_entry(1003, 43, "卑弥呼", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0], 185),
        ];
        let buffer = concat_entries(&entries);
        let layout =
            EntryLayout::detect(&buffer, 0x630).expect("Japanese title detection should succeed");

        assert_eq!(layout.song_id, 0x000);
        assert_eq!(layout.title, 0x180);
        assert_eq!(layout.levels, 0x360);
    }

    // -----------------------------------------------------------------------
    // Tests: Parse with detected layout
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_v1_entry_with_detected_layout() {
        use crate::chart::SongInfo;

        let entries = vec![
            build_v1_entry(
                1001,
                43,
                "Sleepless Days",
                &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0],
            ),
            build_v1_entry(1002, 43, "GAMBOL", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0]),
            build_v1_entry(1003, 12, "Memories", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0]),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x3F0).unwrap();

        let song = SongInfo::parse_entry_with_layout(&entries[0], &layout)
            .unwrap()
            .unwrap();
        assert_eq!(song.id, 1001);
        assert!(song.title.contains("Sleepless Days"));
        assert_eq!(song.folder, 43);
        assert_eq!(song.levels[1], 7); // SPN
        assert_eq!(song.levels[3], 12); // SPA
    }

    #[test]
    fn test_parse_v2_entry_with_detected_layout() {
        use crate::chart::SongInfo;

        let entries = vec![
            build_v2_entry(1001, 43, "5.1.1.", &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0]),
            build_v2_entry(1002, 43, "GAMBOL", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0]),
            build_v2_entry(
                1003,
                43,
                "ピアノ協奏曲第1番",
                &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0],
            ),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x4B0).unwrap();

        let song = SongInfo::parse_entry_with_layout(&entries[2], &layout)
            .unwrap()
            .unwrap();
        assert_eq!(song.id, 1003);
        assert!(song.title.contains("ピアノ協奏曲第1番"));
        assert_eq!(song.levels[2], 9); // SPH
    }

    #[test]
    fn test_parse_v3_entry_with_detected_layout() {
        use crate::chart::SongInfo;

        let entries = vec![
            build_v3_entry(
                1001,
                43,
                "灼熱Beach Side Bunny",
                &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0],
                153,
            ),
            build_v3_entry(1002, 43, "冥", &[0, 5, 8, 12, 0, 0, 5, 8, 12, 0], 200),
            build_v3_entry(1003, 43, "卑弥呼", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0], 185),
        ];
        let buffer = concat_entries(&entries);
        let layout = EntryLayout::detect(&buffer, 0x630).unwrap();

        let song = SongInfo::parse_entry_with_layout(&entries[0], &layout)
            .unwrap()
            .unwrap();
        assert_eq!(song.id, 1001);
        assert!(song.title.contains("灼熱"));
        assert_eq!(&*song.bpm, "153");
        assert_eq!(song.levels[3], 12); // SPA
    }

    #[test]
    fn test_detect_v3_bpm_and_scores() {
        let mut entries = vec![
            build_v3_entry(1001, 43, "Song A", &[0, 7, 10, 12, 0, 0, 7, 10, 12, 0], 150),
            build_v3_entry(1002, 43, "Song B", &[0, 5, 8, 11, 0, 0, 5, 8, 11, 0], 170),
            build_v3_entry(1003, 43, "Song C", &[0, 6, 9, 12, 0, 0, 6, 9, 12, 0], 130),
        ];

        // Add some EX scores
        for (i, entry) in entries.iter_mut().enumerate() {
            for d in 0..10 {
                let score = ((i + 1) * 100 + d * 50) as u32;
                let off = 0x3F0 + d * 4;
                entry[off..off + 4].copy_from_slice(&score.to_le_bytes());
            }
        }

        let buffer = concat_entries(&entries);
        let layout =
            EntryLayout::detect(&buffer, 0x630).expect("V3 score detection should succeed");

        assert_eq!(layout.bpm_notes, Some(0x378));
        assert_eq!(layout.bpm_notes_stride, 8);
        assert_eq!(layout.ex_scores, Some(0x3F0));
    }
}
