mod database;
mod game_id;
pub mod layout;
mod scan;
mod tsv;

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::play::UnlockType;
use crate::process::{ByteBuffer, ReadMemory, decode_shift_jis};

use super::encoding_fixes::{fix_artist_encoding, fix_title_encoding};

// Re-export submodule public items
pub use database::{
    fetch_song_database, fetch_song_database_bulk, fetch_song_database_bulk_with_layout,
};
pub use game_id::{apply_game_id_mapping, build_game_id_index, build_game_id_index_with_layout};
pub use layout::EntryLayout;
pub use scan::{
    analyze_metadata_table, build_song_id_title_map, fetch_song_by_id,
    fetch_song_database_from_memory_scan,
};
pub use tsv::{
    build_song_database_from_tsv_with_memory, build_song_database_from_tsv_with_memory_layout,
    load_song_database_from_tsv, merge_song_databases,
};

/// Song metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongInfo {
    pub id: u32,
    pub title: Arc<str>,
    pub title_english: Arc<str>,
    pub artist: Arc<str>,
    pub genre: Arc<str>,
    pub bpm: Arc<str>,
    pub folder: i32,
    /// Level for each difficulty: SPB, SPN, SPH, SPA, SPL, DPB, DPN, DPH, DPA, DPL
    pub levels: [u8; 10],
    /// Total notes for each difficulty
    pub total_notes: [u32; 10],
    /// Embedded EX scores from entry table (10 x u32: SPB,SPN,SPH,SPA,SPL,DPB,DPN,DPH,DPA,DPL)
    pub embedded_ex_scores: [u32; 10],
    /// Embedded clear lamps from entry table (10 x u32: 0=NO PLAY..7=FC)
    pub embedded_lamps: [u32; 10],
    pub unlock_type: UnlockType,
}

impl SongInfo {
    /// Size of one song entry in memory
    /// Version 2016051600+: 0x630 = 1584 bytes
    /// (was 0x4B0 in 2026012800, 0x3F0 in older versions)
    pub const MEMORY_SIZE: usize = 0x630; // 1584 bytes

    /// Offset from text table to metadata table (legacy, kept for compatibility)
    pub const METADATA_TABLE_OFFSET: usize = 0x7E0;

    // String block size (64 bytes per Shift-JIS string field)
    const SLAB: usize = 64;

    /// Get level for a specific difficulty index
    pub fn get_level(&self, difficulty_index: usize) -> u8 {
        self.levels.get(difficulty_index).copied().unwrap_or(0)
    }

    /// Get total notes for a specific difficulty index
    pub fn get_total_notes(&self, difficulty_index: usize) -> u32 {
        self.total_notes.get(difficulty_index).copied().unwrap_or(0)
    }

    /// Parse song info from a pre-loaded buffer at the given offset.
    ///
    /// This is the buffer-based variant of `read_from_memory` that avoids
    /// individual ReadProcessMemory calls when the buffer has been bulk-loaded.
    pub fn parse_from_buffer(buffer: &[u8], offset: usize) -> Result<Option<Self>> {
        Self::parse_from_buffer_with_layout(buffer, offset, &EntryLayout::v3_default())
    }

    /// Parse song info from a pre-loaded buffer using a detected layout.
    pub fn parse_from_buffer_with_layout(
        buffer: &[u8],
        offset: usize,
        layout: &EntryLayout,
    ) -> Result<Option<Self>> {
        let entry_size = layout.entry_size;
        if offset + entry_size > buffer.len() {
            return Ok(None);
        }
        let entry = &buffer[offset..offset + entry_size];
        Self::parse_entry_with_layout(entry, layout)
    }

    /// Parse a single song entry from a MEMORY_SIZE-length slice (V3 default layout).
    fn parse_entry(entry: &[u8]) -> Result<Option<Self>> {
        Self::parse_entry_with_layout(entry, &EntryLayout::v3_default())
    }

    /// Parse a single song entry using a detected layout.
    pub fn parse_entry_with_layout(entry: &[u8], layout: &EntryLayout) -> Result<Option<Self>> {
        let buf = ByteBuffer::new(entry);

        // Validate song_id: must be positive. Reject 0, negative values, and
        // out-of-range values to prevent corrupt data from entering the database.
        // Negative i32 cast to u32 would silently wrap to a large positive value.
        let song_id = buf.read_i32_at(layout.song_id).unwrap_or(0);
        if song_id <= 0 {
            return Ok(None);
        }

        // Parse folder (i32)
        let folder = buf.read_i32_at(layout.folder).unwrap_or(0);

        // Helper: decode Shift-JIS and trim whitespace.
        // Entry table fields may contain trailing spaces before the null
        // terminator (or fill the entire 64-byte buffer without one).
        // Trimming here -- the most upstream location -- ensures all
        // consumers get clean data and title-based JOINs succeed.
        let decode_and_trim =
            |bytes: &[u8]| -> Arc<str> { Arc::from(decode_shift_jis(bytes).trim()) };

        // Parse title (always present)
        let mut title = decode_and_trim(buf.slice_at(layout.title, Self::SLAB)?);

        // Parse optional text fields
        let title_english = if let Some(off) = layout.title_english {
            decode_and_trim(buf.slice_at(off, Self::SLAB)?)
        } else {
            Arc::from("")
        };
        let genre = if let Some(off) = layout.genre {
            decode_and_trim(buf.slice_at(off, Self::SLAB)?)
        } else {
            Arc::from("")
        };
        let mut artist = if let Some(off) = layout.artist {
            decode_and_trim(buf.slice_at(off, Self::SLAB)?)
        } else {
            Arc::from("")
        };

        if let Some(fixed) = fix_title_encoding(&title) {
            title = fixed;
        }
        if let Some(fixed) = fix_artist_encoding(&artist) {
            artist = fixed;
        }

        // Parse difficulty levels (10 bytes)
        let mut levels = [0u8; 10];
        levels.copy_from_slice(buf.slice_at(layout.levels, 10)?);

        // Read BPM/notes array if offset is known
        let mut raw_values = [0u32; 10];
        if let Some(bpm_off) = layout.bpm_notes {
            for (i, val) in raw_values.iter_mut().enumerate() {
                *val = buf.read_u32_at(bpm_off + i * layout.bpm_notes_stride)?;
            }
        }

        // Detect BPM vs total_notes: when all non-zero values are identical,
        // this is BPM data, not per-chart note counts.
        let non_zero: Vec<u32> = raw_values.iter().copied().filter(|&v| v > 0).collect();
        let all_identical = !non_zero.is_empty() && non_zero.iter().all(|&v| v == non_zero[0]);

        let (bpm, total_notes) = if all_identical {
            let bpm: Arc<str> = Arc::from(non_zero[0].to_string());
            (bpm, [0u32; 10])
        } else {
            let bpm: Arc<str> = Arc::from("");
            (bpm, raw_values)
        };

        // Read embedded EX scores
        let mut embedded_ex_scores = [0u32; 10];
        if let Some(ex_off) = layout.ex_scores {
            for (i, score) in embedded_ex_scores.iter_mut().enumerate() {
                *score = buf.read_u32_at(ex_off + i * 4)?;
            }
        }

        // Read embedded clear lamps
        let mut embedded_lamps = [0u32; 10];
        if let Some(lamp_off) = layout.lamps {
            for (i, lamp) in embedded_lamps.iter_mut().enumerate() {
                *lamp = buf.read_u32_at(lamp_off + i * 4)?;
            }
        }

        Ok(Some(SongInfo {
            id: song_id as u32,
            title,
            title_english,
            artist,
            genre,
            bpm,
            folder,
            levels,
            total_notes,
            embedded_ex_scores,
            embedded_lamps,
            unlock_type: UnlockType::default(),
        }))
    }

    /// Read song info from memory at the given address
    pub fn read_from_memory<R: ReadMemory>(reader: &R, address: u64) -> Result<Option<Self>> {
        let buffer = reader.read_bytes(address, Self::MEMORY_SIZE)?;
        Self::parse_entry(&buffer)
    }

    /// Read song info from memory using a detected layout.
    pub fn read_from_memory_with_layout<R: ReadMemory>(
        reader: &R,
        address: u64,
        layout: &EntryLayout,
    ) -> Result<Option<Self>> {
        let buffer = reader.read_bytes(address, layout.entry_size)?;
        Self::parse_entry_with_layout(&buffer, layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::MockMemoryBuilder;

    /// Build a mock song entry buffer with a title and song_id (V3 layout)
    fn build_song_entry(title: &str, song_id: u32) -> Vec<u8> {
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        // Write song_id
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&(song_id as i32).to_le_bytes());
        // Write title as Shift-JIS
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(title);
        let title_bytes = encoded.as_ref();
        let len = title_bytes.len().min(SongInfo::SLAB);
        entry[layout.title..layout.title + len].copy_from_slice(&title_bytes[..len]);
        // Write at least one non-zero level for the entry to be meaningful
        entry[layout.levels] = 12; // SPB level = 12
        // Write distinct note counts so all-identical BPM detection does NOT trigger
        let bpm_off = layout.bpm_notes.unwrap();
        entry[bpm_off..bpm_off + 4].copy_from_slice(&100u32.to_le_bytes()); // SPB notes = 100
        entry[bpm_off + layout.bpm_notes_stride..bpm_off + layout.bpm_notes_stride + 4]
            .copy_from_slice(&200u32.to_le_bytes()); // SPN notes = 200
        entry
    }

    #[test]
    fn test_parse_from_buffer_valid_entry() {
        let entry = build_song_entry("TestSong", 1001);
        let result = SongInfo::parse_from_buffer(&entry, 0).unwrap();
        assert!(result.is_some());
        let song = result.unwrap();
        assert_eq!(song.id, 1001);
        assert!(song.title.contains("TestSong"));
    }

    #[test]
    fn test_parse_from_buffer_empty_entry() {
        let entry = vec![0u8; SongInfo::MEMORY_SIZE];
        let result = SongInfo::parse_from_buffer(&entry, 0).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_entry_rejects_negative_song_id() {
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        // Write a negative song_id (-1 = 0xFFFFFFFF as i32)
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&(-1i32).to_le_bytes());
        // Write a valid folder so this would otherwise look like a plausible entry
        entry[layout.folder..layout.folder + 4].copy_from_slice(&43i32.to_le_bytes());
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode("Corrupt Entry");
        let title_bytes = encoded.as_ref();
        let len = title_bytes.len().min(SongInfo::SLAB);
        entry[layout.title..layout.title + len].copy_from_slice(&title_bytes[..len]);

        let result = SongInfo::parse_entry_with_layout(&entry, &layout).unwrap();
        // Negative song_id must be rejected (not silently wrapped to u32::MAX)
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_from_buffer_out_of_bounds() {
        let entry = vec![0u8; 100]; // Too small
        let result = SongInfo::parse_from_buffer(&entry, 0).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_from_buffer_matches_read_from_memory() {
        let entry = build_song_entry("Consistency", 2000);
        let base: u64 = 0x1000;
        let reader = MockMemoryBuilder::new()
            .base(base)
            .write_bytes(0, &entry)
            .build();

        let from_memory = SongInfo::read_from_memory(&reader, base).unwrap();
        let from_buffer = SongInfo::parse_from_buffer(&entry, 0).unwrap();

        assert!(from_memory.is_some());
        assert!(from_buffer.is_some());
        let mem_song = from_memory.unwrap();
        let buf_song = from_buffer.unwrap();
        assert_eq!(mem_song.id, buf_song.id);
        assert_eq!(mem_song.title.as_ref(), buf_song.title.as_ref());
        assert_eq!(mem_song.levels, buf_song.levels);
        assert_eq!(mem_song.total_notes, buf_song.total_notes);
    }

    #[test]
    fn test_fetch_song_database_bulk_basic() {
        // Build buffer with 3 songs + 10 empty entries (consecutive failures trigger stop)
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&build_song_entry("Song1", 1001));
        buffer.extend_from_slice(&build_song_entry("Song2", 1002));
        buffer.extend_from_slice(&build_song_entry("Song3", 1003));
        // Add enough empty entries to trigger MAX_CONSECUTIVE_FAILURES
        for _ in 0..10 {
            buffer.extend_from_slice(&vec![0u8; SongInfo::MEMORY_SIZE]);
        }

        let base: u64 = 0x1000;
        let reader = MockMemoryBuilder::new()
            .base(base)
            .write_bytes(0, &buffer)
            .build();

        let db = fetch_song_database_bulk(&reader, base, SongInfo::MEMORY_SIZE).unwrap();
        assert_eq!(db.len(), 3);
        assert!(db.contains_key(&1001));
        assert!(db.contains_key(&1002));
        assert!(db.contains_key(&1003));
    }

    #[test]
    fn test_fetch_song_database_bulk_matches_per_entry() {
        // Build buffer with 2 songs
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&build_song_entry("Alpha", 5000));
        buffer.extend_from_slice(&build_song_entry("Beta", 5001));
        for _ in 0..10 {
            buffer.extend_from_slice(&vec![0u8; SongInfo::MEMORY_SIZE]);
        }

        let base: u64 = 0x1000;
        let reader = MockMemoryBuilder::new()
            .base(base)
            .write_bytes(0, &buffer)
            .build();

        let bulk_db = fetch_song_database_bulk(&reader, base, SongInfo::MEMORY_SIZE).unwrap();
        let per_entry_db = fetch_song_database(&reader, base, SongInfo::MEMORY_SIZE).unwrap();

        assert_eq!(bulk_db.len(), per_entry_db.len());
        for (id, bulk_song) in &bulk_db {
            let per_song = per_entry_db.get(id).expect("missing in per-entry db");
            assert_eq!(bulk_song.id, per_song.id);
            assert_eq!(bulk_song.title.as_ref(), per_song.title.as_ref());
        }
    }

    #[test]
    fn test_merge_song_databases() {
        use std::collections::HashMap;

        let mut id_to_title: HashMap<u32, Arc<str>> = HashMap::new();
        id_to_title.insert(1001, Arc::from("Song A"));
        id_to_title.insert(1002, Arc::from("Song B"));
        id_to_title.insert(1003, Arc::from("Song C")); // Not in TSV

        let mut tsv_db: HashMap<Arc<str>, SongInfo> = HashMap::new();
        tsv_db.insert(
            Arc::from("Song A"),
            SongInfo {
                title: Arc::from("Song A"),
                levels: [0, 5, 8, 10, 12, 0, 5, 8, 10, 12],
                ..Default::default()
            },
        );
        tsv_db.insert(
            Arc::from("Song B"),
            SongInfo {
                title: Arc::from("Song B"),
                levels: [0, 3, 6, 9, 0, 0, 3, 6, 9, 0],
                ..Default::default()
            },
        );

        let merged = merge_song_databases(&id_to_title, &tsv_db);
        assert_eq!(merged.len(), 3);

        // Song A should have TSV data + memory song_id
        let song_a = merged.get(&1001).unwrap();
        assert_eq!(song_a.id, 1001);
        assert_eq!(song_a.levels[1], 5); // SPN from TSV

        // Song C should have minimal entry (not in TSV)
        let song_c = merged.get(&1003).unwrap();
        assert_eq!(song_c.id, 1003);
        assert_eq!(&*song_c.title, "Song C");
    }

    #[test]
    fn test_apply_game_id_mapping() {
        use std::collections::HashMap;

        let mut song_db: HashMap<u32, SongInfo> = HashMap::new();
        song_db.insert(
            1001,
            SongInfo {
                id: 1001,
                title: Arc::from("Real Song"),
                ..Default::default()
            },
        );
        song_db.insert(
            2001,
            SongInfo {
                id: 2001,
                title: Arc::from("Other Song"),
                ..Default::default()
            },
        );

        let mut mapping: HashMap<u32, u32> = HashMap::new();
        // game_id 2001 should point to internal_id 1001
        mapping.insert(2001, 1001);

        apply_game_id_mapping(&mut song_db, &mapping);

        // song_db[2001] should now have the data from song_db[1001]
        let remapped = song_db.get(&2001).unwrap();
        assert_eq!(remapped.id, 2001); // id updated to game_id
        assert_eq!(&*remapped.title, "Real Song"); // data from internal_id 1001

        // Original should be unchanged
        let original = song_db.get(&1001).unwrap();
        assert_eq!(&*original.title, "Real Song");
    }

    #[test]
    fn test_normalize_title_for_matching() {
        use tsv::normalize_title_for_matching;

        assert_eq!(normalize_title_for_matching("Hello World"), "helloworld");
        assert_eq!(normalize_title_for_matching("A!B@C#D"), "abcd");
        assert_eq!(normalize_title_for_matching("テスト曲名"), "テスト曲名");
        assert_eq!(normalize_title_for_matching(""), "");
    }

    /// Helper to build a song entry with the same BPM value across all 10 slots
    /// (simulating V3 layout where 0x378 holds BPM, not total_notes)
    fn build_song_entry_with_bpm(title: &str, song_id: u32, bpm: u32) -> Vec<u8> {
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&(song_id as i32).to_le_bytes());
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(title);
        let title_bytes = encoded.as_ref();
        let len = title_bytes.len().min(SongInfo::SLAB);
        entry[layout.title..layout.title + len].copy_from_slice(&title_bytes[..len]);
        entry[layout.levels] = 12;
        let bpm_off = layout.bpm_notes.unwrap();
        // Write same BPM value to all 10 slots
        for i in 0..10 {
            let off = bpm_off + i * layout.bpm_notes_stride;
            entry[off..off + 4].copy_from_slice(&bpm.to_le_bytes());
        }
        entry
    }

    #[test]
    fn test_bpm_detection_all_identical_values() {
        let entry = build_song_entry_with_bpm("BPM Song", 1001, 150);
        let song = SongInfo::parse_from_buffer(&entry, 0).unwrap().unwrap();

        // All 10 values identical -> detected as BPM
        assert_eq!(&*song.bpm, "150");
        // total_notes should be zeroed out (unreliable)
        assert_eq!(song.total_notes, [0u32; 10]);
    }

    #[test]
    fn test_bpm_detection_mixed_values_treated_as_notes() {
        // build_song_entry writes 100 and 200 for SPB/SPN -> distinct values
        let entry = build_song_entry("Notes Song", 1002);
        let song = SongInfo::parse_from_buffer(&entry, 0).unwrap().unwrap();

        // Mixed values -> not BPM, treated as note counts
        assert_eq!(&*song.bpm, "");
        assert_eq!(song.total_notes[0], 100); // SPB
        assert_eq!(song.total_notes[1], 200); // SPN
    }

    #[test]
    fn test_bpm_detection_all_zeros_no_bpm() {
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&1001i32.to_le_bytes());
        // All 10 values at BPM offset are 0 (no non-zero values)
        let song = SongInfo::parse_from_buffer(&entry, 0).unwrap().unwrap();

        // All zeros -> non_zero vec is empty -> not treated as BPM
        assert_eq!(&*song.bpm, "");
        assert_eq!(song.total_notes, [0u32; 10]);
    }

    #[test]
    fn test_bpm_detection_partial_zeros_all_nonzero_identical() {
        // Some slots have levels (notes), some are 0 (no chart).
        // In V3 BPM: only difficulties with charts have a value, but all
        // non-zero values are the same BPM.
        let layout = EntryLayout::v3_default();
        let bpm_off = layout.bpm_notes.unwrap();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&1001i32.to_le_bytes());
        // Write BPM=170 to slots 1,2,3 only (SPN,SPH,SPA); rest are 0
        for i in [1, 2, 3] {
            let off = bpm_off + i * layout.bpm_notes_stride;
            entry[off..off + 4].copy_from_slice(&170u32.to_le_bytes());
        }
        let song = SongInfo::parse_from_buffer(&entry, 0).unwrap().unwrap();

        // All non-zero values identical -> BPM detected
        assert_eq!(&*song.bpm, "170");
        assert_eq!(song.total_notes, [0u32; 10]);
    }

    /// Build a song entry with trailing spaces in text fields to test trimming.
    fn build_song_entry_with_trailing_spaces(title: &str, artist: &str, song_id: u32) -> Vec<u8> {
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        // Write song_id
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&(song_id as i32).to_le_bytes());

        // Write title with trailing spaces as Shift-JIS
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(title);
        let title_bytes = encoded.as_ref();
        let len = title_bytes.len().min(SongInfo::SLAB);
        entry[layout.title..layout.title + len].copy_from_slice(&title_bytes[..len]);

        // Write artist with trailing spaces
        if let Some(artist_off) = layout.artist {
            let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(artist);
            let artist_bytes = encoded.as_ref();
            let len = artist_bytes.len().min(SongInfo::SLAB);
            entry[artist_off..artist_off + len].copy_from_slice(&artist_bytes[..len]);
        }

        // Write distinct note counts to avoid BPM detection
        let bpm_off = layout.bpm_notes.unwrap();
        entry[bpm_off..bpm_off + 4].copy_from_slice(&100u32.to_le_bytes());
        entry[bpm_off + layout.bpm_notes_stride..bpm_off + layout.bpm_notes_stride + 4]
            .copy_from_slice(&200u32.to_le_bytes());
        entry
    }

    #[test]
    fn test_parse_entry_trims_trailing_spaces_from_text_fields() {
        let entry =
            build_song_entry_with_trailing_spaces("Trailing Spaces   ", "Some Artist   ", 1001);
        let layout = EntryLayout::v3_default();
        let song = SongInfo::parse_entry_with_layout(&entry, &layout)
            .unwrap()
            .unwrap();

        // Title and artist must be trimmed; trailing spaces must not survive.
        assert_eq!(&*song.title, "Trailing Spaces");
        assert_eq!(&*song.artist, "Some Artist");
    }

    #[test]
    fn test_parse_entry_trims_field_filling_entire_buffer() {
        // Simulate a title that fills all 64 bytes with no null terminator,
        // ending in spaces.
        let layout = EntryLayout::v3_default();
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        entry[layout.song_id..layout.song_id + 4].copy_from_slice(&1001i32.to_le_bytes());

        // Fill entire 64-byte title field: "A" repeated then trailing spaces
        let prefix = b"FullBuffer";
        let mut title_field = vec![0x20u8; SongInfo::SLAB]; // all spaces
        title_field[..prefix.len()].copy_from_slice(prefix);
        entry[layout.title..layout.title + SongInfo::SLAB].copy_from_slice(&title_field);

        // Distinct notes to avoid BPM detection
        let bpm_off = layout.bpm_notes.unwrap();
        entry[bpm_off..bpm_off + 4].copy_from_slice(&100u32.to_le_bytes());
        entry[bpm_off + layout.bpm_notes_stride..bpm_off + layout.bpm_notes_stride + 4]
            .copy_from_slice(&200u32.to_le_bytes());

        let song = SongInfo::parse_entry_with_layout(&entry, &layout)
            .unwrap()
            .unwrap();

        // No null terminator, but trailing spaces must still be trimmed
        assert_eq!(&*song.title, "FullBuffer");
    }
}
