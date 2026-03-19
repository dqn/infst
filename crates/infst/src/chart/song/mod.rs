mod database;
mod game_id;
mod scan;
mod tsv;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Result;
use crate::play::UnlockType;
use crate::process::{ByteBuffer, ReadMemory, decode_shift_jis};

use super::encoding_fixes::{fix_artist_encoding, fix_title_encoding};

// Re-export submodule public items
pub use database::{fetch_song_database, fetch_song_database_bulk};
pub use game_id::{apply_game_id_mapping, build_game_id_index};
pub use scan::{
    analyze_metadata_table, build_song_id_title_map, fetch_song_by_id,
    fetch_song_database_from_memory_scan,
};
pub use tsv::{
    build_song_database_from_tsv_with_memory, load_song_database_from_tsv, merge_song_databases,
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

    // Memory layout constants
    const SLAB: usize = 64; // String block size (64 bytes per Shift-JIS string field)

    // Memory offsets (relative to song entry start)
    // Version 2016051600+ layout:
    //
    // Header:
    //   0x000: Song ID (i32)
    //   0x004: Folder (i32)
    //   0x008: Identifier string (~22 bytes, includes 0xFF88 marker)
    //   0x020-0x17F: Reserved/padding
    //
    // String fields (each 64 bytes, Shift-JIS encoded):
    //   0x180: Title
    //   0x1C0: (unknown, often empty)
    //   0x200: Title (English)
    //   0x240: Genre
    //   0x280: (unknown, often empty)
    //   0x2C0: Artist
    //
    // Metadata:
    //   0x360: Difficulty levels (10 bytes)
    //   0x378: Total notes (10 x u32, 8-byte stride)
    //
    // Score data (player-specific):
    //   0x3F0: EX scores (10 x u32)
    //   0x430+: Clear lamps, DJ points, etc.
    const SONG_ID_OFFSET: usize = 0;
    const FOLDER_OFFSET: usize = 4;
    const TITLE_OFFSET: usize = 0x180; // 384
    const TITLE_ENGLISH_OFFSET: usize = 0x200; // 512
    const GENRE_OFFSET: usize = 0x240; // 576
    const ARTIST_OFFSET: usize = 0x2C0; // 704
    const LEVELS_OFFSET: usize = 0x360; // 864
    const NOTES_OFFSET: usize = 0x378; // 888
    const NOTES_STRIDE: usize = 8; // 8 bytes per note entry (u32 + 4 bytes padding)
    const EX_SCORE_OFFSET: usize = 0x3F0; // 10 x u32 (40 bytes)
    const LAMP_OFFSET: usize = 0x430; // 10 x u32 (40 bytes)

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
        if offset + Self::MEMORY_SIZE > buffer.len() {
            return Ok(None);
        }
        let entry = &buffer[offset..offset + Self::MEMORY_SIZE];
        Self::parse_entry(entry)
    }

    /// Parse a single song entry from a MEMORY_SIZE-length slice
    fn parse_entry(entry: &[u8]) -> Result<Option<Self>> {
        let buf = ByteBuffer::new(entry);

        // Check if entry is valid (song_id at offset 0 should not be 0)
        let song_id = buf.read_i32_at(Self::SONG_ID_OFFSET).unwrap_or(0);
        if song_id == 0 {
            return Ok(None);
        }

        // Parse folder (i32)
        let folder = buf.read_i32_at(Self::FOLDER_OFFSET).unwrap_or(0);

        // Parse strings (Shift-JIS encoded, with encoding fixes for non-Shift-JIS characters)
        let mut title = decode_shift_jis(buf.slice_at(Self::TITLE_OFFSET, Self::SLAB)?);
        let title_english = decode_shift_jis(buf.slice_at(Self::TITLE_ENGLISH_OFFSET, Self::SLAB)?);
        let genre = decode_shift_jis(buf.slice_at(Self::GENRE_OFFSET, Self::SLAB)?);
        let mut artist = decode_shift_jis(buf.slice_at(Self::ARTIST_OFFSET, Self::SLAB)?);

        if let Some(fixed) = fix_title_encoding(&title) {
            title = fixed;
        }
        if let Some(fixed) = fix_artist_encoding(&artist) {
            artist = fixed;
        }

        // Parse difficulty levels (10 bytes)
        let mut levels = [0u8; 10];
        levels.copy_from_slice(buf.slice_at(Self::LEVELS_OFFSET, 10)?);

        // BPM is not stored in this structure version (2016051600+)
        let bpm: Arc<str> = Arc::from("");

        // Parse note counts (10 entries, 8-byte stride: u32 value + 4 bytes padding)
        let mut total_notes = [0u32; 10];
        for (i, note_count) in total_notes.iter_mut().enumerate() {
            *note_count = buf.read_u32_at(Self::NOTES_OFFSET + i * Self::NOTES_STRIDE)?;
        }

        // Read embedded EX scores (10 x u32 at offset 0x3F0)
        let mut embedded_ex_scores = [0u32; 10];
        for (i, score) in embedded_ex_scores.iter_mut().enumerate() {
            *score = buf.read_u32_at(Self::EX_SCORE_OFFSET + i * 4)?;
        }

        // Read embedded clear lamps (10 x u32 at offset 0x430)
        let mut embedded_lamps = [0u32; 10];
        for (i, lamp) in embedded_lamps.iter_mut().enumerate() {
            *lamp = buf.read_u32_at(Self::LAMP_OFFSET + i * 4)?;
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

    /// Read song info with fallback to metadata table for new INFINITAS versions.
    ///
    /// In version 2026012800+, the song_id may be stored in a separate metadata table.
    /// This method tries the standard read first, and if song_id is 0 but title exists,
    /// it attempts to read song_id from the metadata table.
    ///
    /// # Arguments
    /// * `reader` - Memory reader
    /// * `text_address` - Address of the text entry
    /// * `text_base` - Base address of the text table
    /// * `entry_index` - Index of this entry in the table
    pub fn read_from_memory_with_fallback<R: ReadMemory>(
        reader: &R,
        text_address: u64,
        text_base: u64,
        entry_index: u64,
    ) -> Result<Option<Self>> {
        // First, try standard read
        let result = Self::read_from_memory(reader, text_address)?;

        match result {
            Some(mut song) if song.id == 0 && !song.title.is_empty() => {
                // Try to read song_id from metadata table
                let metadata_addr = text_base
                    + Self::METADATA_TABLE_OFFSET as u64
                    + entry_index * Self::MEMORY_SIZE as u64;

                if let Ok(metadata) = reader.read_bytes(metadata_addr, 32) {
                    let buf = ByteBuffer::new(&metadata);
                    let alt_song_id = buf.read_i32_at(0).unwrap_or(0);
                    let alt_folder = buf.read_i32_at(4).unwrap_or(0);

                    // Validate: song_id should be 1000-50000, folder 1-50
                    if (1000..=50000).contains(&alt_song_id) {
                        debug!(
                            "Using metadata table for song '{}': id={}, folder={}",
                            song.title, alt_song_id, alt_folder
                        );
                        song.id = alt_song_id as u32;
                        if (1..=50).contains(&alt_folder) {
                            song.folder = alt_folder;
                        }
                    }
                }

                if song.id == 0 {
                    // Still no valid song_id, skip this entry
                    debug!(
                        "Skipping entry with title '{}' - no valid song_id found",
                        song.title
                    );
                    return Ok(None);
                }

                Ok(Some(song))
            }
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::MockMemoryBuilder;

    /// Build a mock song entry buffer with a title and song_id
    fn build_song_entry(title: &str, song_id: u32) -> Vec<u8> {
        let mut entry = vec![0u8; SongInfo::MEMORY_SIZE];
        // Write song_id at offset 0
        entry[SongInfo::SONG_ID_OFFSET..SongInfo::SONG_ID_OFFSET + 4]
            .copy_from_slice(&(song_id as i32).to_le_bytes());
        // Write title as Shift-JIS at TITLE_OFFSET (0x180)
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(title);
        let title_bytes = encoded.as_ref();
        let len = title_bytes.len().min(SongInfo::SLAB);
        entry[SongInfo::TITLE_OFFSET..SongInfo::TITLE_OFFSET + len]
            .copy_from_slice(&title_bytes[..len]);
        // Write at least one non-zero level and note count for the entry to be meaningful
        entry[SongInfo::LEVELS_OFFSET] = 12; // SPB level = 12
        entry[SongInfo::NOTES_OFFSET..SongInfo::NOTES_OFFSET + 4]
            .copy_from_slice(&100u32.to_le_bytes()); // SPB notes = 100
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
}
