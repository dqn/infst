use crate::game::{Difficulty, UnlockType};

/// Unlock data structure from memory
#[derive(Debug, Clone, Default)]
pub struct UnlockData {
    pub song_id: i32,
    pub unlock_type: UnlockType,
    pub unlocks: i32, // Bitmask of unlocked difficulties
}

impl UnlockData {
    /// Size of unlock data structure in memory (32 bytes)
    pub const MEMORY_SIZE: usize = 32;

    /// Check if a specific difficulty is unlocked
    pub fn is_difficulty_unlocked(&self, difficulty: Difficulty) -> bool {
        let bit = 1 << (difficulty as i32);
        (self.unlocks & bit) != 0
    }

    /// Parse from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::MEMORY_SIZE {
            return None;
        }

        let song_id = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let unlock_type_val = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let unlocks = i32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        let unlock_type = match unlock_type_val {
            1 => UnlockType::Base,
            2 => UnlockType::Bits,
            3 => UnlockType::Sub,
            _ => UnlockType::Base,
        };

        Some(Self {
            song_id,
            unlock_type,
            unlocks,
        })
    }
}
