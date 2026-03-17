use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OffsetsCollection {
    pub version: String,
    pub song_list: u64,
    pub data_map: u64,
    pub judge_data: u64,
    pub play_data: u64,
    pub play_settings: u64,
    pub unlock_data: u64,
    pub current_song: u64,
    /// Song entry table address (new-format entries with song_id at offset 0).
    /// When non-zero, use this instead of song_list for database loading.
    /// song_list remains the anchor for relative offset detection.
    #[serde(default)]
    pub song_entry_table: u64,
    /// Auto-detected song entry size in bytes.
    /// 0 means not detected (callers should fall back to SongInfo::MEMORY_SIZE).
    #[serde(default)]
    pub song_entry_size: usize,
}

impl OffsetsCollection {
    /// Check if all required offsets are valid
    pub fn is_valid(&self) -> bool {
        !self.version.is_empty()
            && self.song_list != 0
            && self.data_map != 0
            && self.judge_data != 0
            && self.play_data != 0
            && self.play_settings != 0
            && self.unlock_data != 0
            && self.current_song != 0
    }

    /// Check if offsets required for state detection are valid
    pub fn has_state_detection_offsets(&self) -> bool {
        self.judge_data != 0 && self.play_settings != 0
    }

    /// Get the address to use for song database loading.
    /// Prefers song_entry_table (new-format) over song_list (text table anchor).
    pub fn song_db_address(&self) -> u64 {
        if self.song_entry_table != 0 {
            self.song_entry_table
        } else {
            self.song_list
        }
    }

    /// Get the entry stride for song database iteration.
    pub fn entry_stride(&self) -> usize {
        if self.song_entry_size > 0 {
            self.song_entry_size
        } else {
            crate::chart::SongInfo::MEMORY_SIZE
        }
    }
}
