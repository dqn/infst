//! Tests for song parsing and database functions
//!
//! Tests the SongInfo memory reading, title normalization, and song database operations.

use infst::chart::{EntryLayout, SongInfo, fetch_song_by_id, fetch_song_database_from_memory_scan};
use infst::process::MockMemoryReader;

/// Tests for SongInfo memory reading
mod song_info_read {
    use super::*;

    fn create_song_entry_buffer(
        title: &str,
        song_id: u32,
        folder: i32,
        levels: [u8; 10],
        _bpm_max: i32,
        _bpm_min: i32,
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; SongInfo::MEMORY_SIZE];

        // Song ID at offset 0
        buffer[0..4].copy_from_slice(&(song_id as i32).to_le_bytes());

        // Folder at offset 4 (i32)
        buffer[4..8].copy_from_slice(&folder.to_le_bytes());

        // Title at offset 0x180 (Shift-JIS encoded)
        let title_bytes = title.as_bytes();
        let copy_len = title_bytes.len().min(63);
        buffer[0x180..0x180 + copy_len].copy_from_slice(&title_bytes[..copy_len]);

        // Levels at offset 0x360 (10 bytes)
        buffer[0x360..0x36A].copy_from_slice(&levels);

        // BPM is not stored in version 2016051600+ structure

        // Notes at offset 0x378 (10 x u32, 8-byte stride)
        // Skip for now, leave as zeros

        buffer
    }

    #[test]
    fn test_read_valid_song_entry() {
        let buffer = create_song_entry_buffer(
            "Test Song",
            1234,
            5,
            [0, 3, 6, 9, 12, 0, 3, 6, 9, 12],
            150,
            130,
        );

        let reader = MockMemoryReader::new(buffer);
        let result = SongInfo::read_from_memory(&reader, 0x1000).unwrap();

        assert!(result.is_some());
        let song = result.unwrap();
        assert_eq!(song.id, 1234);
        assert_eq!(&*song.title, "Test Song");
        assert_eq!(song.folder, 5);
        assert_eq!(song.levels, [0, 3, 6, 9, 12, 0, 3, 6, 9, 12]);
    }

    #[test]
    fn test_read_empty_entry_returns_none() {
        // Buffer with first 4 bytes = 0 (empty entry)
        let buffer = vec![0u8; SongInfo::MEMORY_SIZE];

        let reader = MockMemoryReader::new(buffer);
        let result = SongInfo::read_from_memory(&reader, 0x1000).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn test_get_level() {
        let song = SongInfo {
            levels: [0, 3, 6, 9, 12, 0, 3, 6, 9, 12],
            ..Default::default()
        };

        assert_eq!(song.get_level(0), 0); // SPB
        assert_eq!(song.get_level(1), 3); // SPN
        assert_eq!(song.get_level(3), 9); // SPA
        assert_eq!(song.get_level(10), 0); // Out of bounds
    }

    #[test]
    fn test_get_total_notes() {
        let song = SongInfo {
            total_notes: [100, 200, 300, 400, 500, 100, 200, 300, 400, 500],
            ..Default::default()
        };

        assert_eq!(song.get_total_notes(0), 100);
        assert_eq!(song.get_total_notes(4), 500);
        assert_eq!(song.get_total_notes(10), 0); // Out of bounds
    }
}

/// Tests for fetch_song_by_id
mod fetch_song_by_id_tests {
    use super::*;

    fn create_song_entry(song_id: u32, title: &str) -> Vec<u8> {
        let mut buffer = vec![0u8; SongInfo::MEMORY_SIZE];

        // Song ID at offset 0
        buffer[0..4].copy_from_slice(&(song_id as i32).to_le_bytes());

        // Folder at offset 4 (i32)
        buffer[4..8].copy_from_slice(&1i32.to_le_bytes());

        // Title at offset 0x180
        let title_bytes = title.as_bytes();
        let copy_len = title_bytes.len().min(63);
        buffer[0x180..0x180 + copy_len].copy_from_slice(&title_bytes[..copy_len]);

        buffer
    }

    #[test]
    fn test_fetch_existing_song() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let mut buffer = Vec::new();

        // Create 3 song entries
        buffer.extend(create_song_entry(1001, "Song One"));
        buffer.extend(create_song_entry(1002, "Song Two"));
        buffer.extend(create_song_entry(1003, "Song Three"));

        let reader = MockMemoryReader::new(buffer);
        let base = 0x1000;

        // Fetch middle song
        let layout = EntryLayout::v3_default();
        let result = fetch_song_by_id(
            &reader,
            base,
            1002,
            entry_size * 5,
            SongInfo::MEMORY_SIZE,
            &layout,
        );
        assert!(result.is_some());
        let song = result.unwrap();
        assert_eq!(song.id, 1002);
        assert_eq!(&*song.title, "Song Two");
    }

    #[test]
    fn test_fetch_first_song() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let mut buffer = Vec::new();

        buffer.extend(create_song_entry(1001, "First Song"));
        buffer.extend(create_song_entry(1002, "Second Song"));

        let reader = MockMemoryReader::new(buffer);
        let layout = EntryLayout::v3_default();
        let result = fetch_song_by_id(
            &reader,
            0x1000,
            1001,
            entry_size * 3,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert!(result.is_some());
        assert_eq!(result.unwrap().id, 1001);
    }

    #[test]
    fn test_fetch_nonexistent_song() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let mut buffer = Vec::new();

        buffer.extend(create_song_entry(1001, "Only Song"));

        let reader = MockMemoryReader::new(buffer);
        let layout = EntryLayout::v3_default();
        let result = fetch_song_by_id(
            &reader,
            0x1000,
            9999,
            entry_size * 2,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert!(result.is_none());
    }

    #[test]
    fn test_fetch_with_zero_address() {
        let reader = MockMemoryReader::new(vec![0u8; 100]);
        let layout = EntryLayout::v3_default();
        let result = fetch_song_by_id(&reader, 0, 1001, 1000, SongInfo::MEMORY_SIZE, &layout);

        assert!(result.is_none());
    }
}

/// Tests for fetch_song_database_from_memory_scan
mod fetch_song_database_tests {
    use super::*;

    fn create_song_entry(song_id: u32, title: &str) -> Vec<u8> {
        let mut buffer = vec![0u8; SongInfo::MEMORY_SIZE];

        // Song ID at offset 0
        buffer[0..4].copy_from_slice(&(song_id as i32).to_le_bytes());

        // Folder at offset 4 (i32)
        buffer[4..8].copy_from_slice(&1i32.to_le_bytes());

        // Title at offset 0x180
        let title_bytes = title.as_bytes();
        let copy_len = title_bytes.len().min(63);
        buffer[0x180..0x180 + copy_len].copy_from_slice(&title_bytes[..copy_len]);

        buffer
    }

    #[test]
    fn test_scan_multiple_songs() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let layout = EntryLayout::v3_default();
        let mut buffer = Vec::new();

        buffer.extend(create_song_entry(1001, "Song A"));
        buffer.extend(create_song_entry(1002, "Song B"));
        buffer.extend(create_song_entry(1003, "Song C"));

        let reader = MockMemoryReader::new(buffer);
        let result = fetch_song_database_from_memory_scan(
            &reader,
            0x1000,
            entry_size * 4,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert_eq!(result.len(), 3);
        assert!(result.contains_key(&1001));
        assert!(result.contains_key(&1002));
        assert!(result.contains_key(&1003));
    }

    #[test]
    fn test_scan_skips_invalid_song_ids() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let layout = EntryLayout::v3_default();
        let mut buffer = Vec::new();

        // Valid song
        buffer.extend(create_song_entry(1001, "Valid Song"));

        // Invalid song_id (too low)
        buffer.extend(create_song_entry(500, "Invalid Low"));

        // Valid song
        buffer.extend(create_song_entry(2000, "Another Valid"));

        let reader = MockMemoryReader::new(buffer);
        let result = fetch_song_database_from_memory_scan(
            &reader,
            0x1000,
            entry_size * 4,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&1001));
        assert!(result.contains_key(&2000));
        assert!(!result.contains_key(&500));
    }

    #[test]
    fn test_scan_skips_duplicate_song_ids() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let layout = EntryLayout::v3_default();
        let mut buffer = Vec::new();

        buffer.extend(create_song_entry(1001, "First"));
        buffer.extend(create_song_entry(1001, "Duplicate")); // Same ID
        buffer.extend(create_song_entry(1002, "Second"));

        let reader = MockMemoryReader::new(buffer);
        let result = fetch_song_database_from_memory_scan(
            &reader,
            0x1000,
            entry_size * 4,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert_eq!(result.len(), 2);
        // Should keep the first occurrence
        assert_eq!(&*result.get(&1001).unwrap().title, "First");
    }

    #[test]
    fn test_scan_empty_buffer() {
        let layout = EntryLayout::v3_default();
        let reader = MockMemoryReader::new(vec![0u8; SongInfo::MEMORY_SIZE * 2]);
        let result = fetch_song_database_from_memory_scan(
            &reader,
            0x1000,
            SongInfo::MEMORY_SIZE * 3,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_with_gaps() {
        let entry_size = SongInfo::MEMORY_SIZE;
        let layout = EntryLayout::v3_default();
        let mut buffer = Vec::new();

        buffer.extend(create_song_entry(1001, "Song A"));
        buffer.extend(vec![0u8; entry_size]); // Empty entry (gap)
        buffer.extend(create_song_entry(1003, "Song C"));

        let reader = MockMemoryReader::new(buffer);
        let result = fetch_song_database_from_memory_scan(
            &reader,
            0x1000,
            entry_size * 4,
            SongInfo::MEMORY_SIZE,
            &layout,
        );

        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&1001));
        assert!(result.contains_key(&1003));
    }
}

/// Tests for SongInfo default values
mod song_info_defaults {
    use super::*;

    #[test]
    fn test_default_values() {
        let song = SongInfo::default();

        assert_eq!(song.id, 0);
        assert!(song.title.is_empty());
        assert!(song.title_english.is_empty());
        assert!(song.artist.is_empty());
        assert!(song.genre.is_empty());
        assert!(song.bpm.is_empty());
        assert_eq!(song.folder, 0);
        assert_eq!(song.levels, [0u8; 10]);
        assert_eq!(song.total_notes, [0u32; 10]);
    }

    #[test]
    fn test_memory_size_constant() {
        // Ensure the constant matches expected value
        assert_eq!(SongInfo::MEMORY_SIZE, 0x630);
        assert_eq!(SongInfo::MEMORY_SIZE, 1584);
    }
}
