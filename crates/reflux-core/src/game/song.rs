use serde::{Deserialize, Serialize};

use crate::game::UnlockType;

/// Song metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SongInfo {
    pub id: String,
    pub title: String,
    pub title_english: String,
    pub artist: String,
    pub genre: String,
    pub bpm: String,
    pub folder: i32,
    /// Level for each difficulty: SPB, SPN, SPH, SPA, SPL, DPB, DPN, DPH, DPA, DPL
    pub levels: [u8; 10],
    /// Total notes for each difficulty
    pub total_notes: [u32; 10],
    pub unlock_type: UnlockType,
}

impl SongInfo {
    /// Size of one song entry in memory (0x3F0 = 1008 bytes)
    pub const MEMORY_SIZE: usize = 0x3F0;

    /// Get level for a specific difficulty index
    pub fn get_level(&self, difficulty_index: usize) -> u8 {
        self.levels.get(difficulty_index).copied().unwrap_or(0)
    }

    /// Get total notes for a specific difficulty index
    pub fn get_total_notes(&self, difficulty_index: usize) -> u32 {
        self.total_notes.get(difficulty_index).copied().unwrap_or(0)
    }
}
