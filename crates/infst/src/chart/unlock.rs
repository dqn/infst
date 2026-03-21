use std::collections::HashMap;

use crate::chart::{Difficulty, SongInfo};
use crate::error::Result;
use crate::play::UnlockType;
use crate::process::{ByteBuffer, ReadMemory};

/// Unlock data structure from memory
#[derive(Debug, Clone, Default)]
pub struct UnlockData {
    pub song_id: u32,
    pub unlock_type: UnlockType,
    pub unlocks: i32, // Bitmask of unlocked difficulties
}

impl UnlockData {
    /// Size of unlock data structure in memory (32 bytes)
    pub const MEMORY_SIZE: usize = 32;

    /// Check if a specific difficulty is unlocked (raw bit check)
    pub fn is_difficulty_unlocked(&self, difficulty: Difficulty) -> bool {
        let bit = 1 << (difficulty as i32);
        (self.unlocks & bit) != 0
    }

    /// Parse from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::MEMORY_SIZE {
            return None;
        }

        let buf = ByteBuffer::new(bytes);
        let song_id = buf.read_u32_at(0).ok()?;
        let unlock_type_val = buf.read_i32_at(4).ok()?;
        let unlocks = buf.read_i32_at(8).ok()?;

        let unlock_type = match unlock_type_val {
            1 => UnlockType::Base,
            2 => UnlockType::Bits,
            3 => UnlockType::Sub,
            v => {
                tracing::warn!(
                    unlock_type_val = v,
                    "Unknown unlock type, defaulting to Base"
                );
                UnlockType::Base
            }
        };

        Some(Self {
            song_id,
            unlock_type,
            unlocks,
        })
    }
}

/// Maximum number of unlock entries to read before giving up.
/// Prevents unbounded loops when many entries have unknown song_ids.
const MAX_UNLOCK_ENTRIES: usize = 10_000;

/// Load unlock states from memory for all songs
pub fn get_unlock_states<R: ReadMemory>(
    reader: &R,
    unlock_data_addr: u64,
    song_db: &HashMap<u32, SongInfo>,
) -> Result<HashMap<u32, UnlockData>> {
    let mut result = HashMap::new();

    let song_count = song_db.len();
    if song_count == 0 {
        return Ok(result);
    }

    let mut position_entries = 0usize;
    let mut batch_entries = song_count;

    loop {
        if position_entries + batch_entries > MAX_UNLOCK_ENTRIES {
            break;
        }

        let buffer_size = UnlockData::MEMORY_SIZE * batch_entries;
        let buffer = reader.read_bytes(
            unlock_data_addr + (position_entries * UnlockData::MEMORY_SIZE) as u64,
            buffer_size,
        )?;

        let extra_entries = parse_unlock_buffer(&buffer, song_db, &mut result);
        if extra_entries == 0 {
            break;
        }

        position_entries += batch_entries;
        batch_entries = extra_entries;
    }

    Ok(result)
}

fn parse_unlock_buffer(
    buffer: &[u8],
    song_db: &HashMap<u32, SongInfo>,
    result: &mut HashMap<u32, UnlockData>,
) -> usize {
    let mut position = 0;
    let mut extra_entries = 0;

    while position + UnlockData::MEMORY_SIZE <= buffer.len() {
        let chunk = &buffer[position..position + UnlockData::MEMORY_SIZE];

        if let Some(data) = UnlockData::from_bytes(chunk) {
            if data.song_id == 0 {
                break;
            }

            if !song_db.contains_key(&data.song_id) {
                extra_entries += 1;
            }
            result.insert(data.song_id, data);
        }

        position += UnlockData::MEMORY_SIZE;
    }

    extra_entries
}

/// Get unlock state for a specific difficulty, considering special cases
///
/// Special handling for:
/// - SPB (Beginner): For non-Sub songs, check if note count is non-zero
/// - SPL/DPL (Leggendaria): For Sub songs, requires both SPA and DPA to be unlocked
pub fn get_unlock_state_for_difficulty(
    unlock_db: &HashMap<u32, UnlockData>,
    song_db: &HashMap<u32, SongInfo>,
    song_id: u32,
    difficulty: Difficulty,
) -> bool {
    let Some(unlock_data) = unlock_db.get(&song_id) else {
        return false;
    };

    let song_info = song_db.get(&song_id);

    // Handle Beginner difficulty specially
    if difficulty == Difficulty::SpB {
        if unlock_data.unlock_type == UnlockType::Sub {
            // For Sub songs, use the unlock bit
            return unlock_data.is_difficulty_unlocked(difficulty);
        } else {
            // For other songs, check if note count is non-zero
            return song_info.map(|s| s.total_notes[0] > 0).unwrap_or(false);
        }
    }

    // Handle Leggendaria difficulties (SPL/DPL)
    if difficulty == Difficulty::SpL || difficulty == Difficulty::DpL {
        if unlock_data.unlock_type == UnlockType::Sub {
            // For Sub songs, require both SPA and DPA to be unlocked
            let spa_unlocked = unlock_data.is_difficulty_unlocked(Difficulty::SpA);
            let dpa_unlocked = unlock_data.is_difficulty_unlocked(Difficulty::DpA);
            return spa_unlocked && dpa_unlocked;
        } else {
            // For other songs, just check the unlock bit
            return unlock_data.is_difficulty_unlocked(difficulty);
        }
    }

    // Standard case: just check the unlock bit
    unlock_data.is_difficulty_unlocked(difficulty)
}

/// Compare old and new unlock states and return only changed entries
///
/// This function:
/// 1. Reads current unlock states from memory
/// 2. Compares with previous states
/// 3. Returns only entries where `unlocks` value has changed
pub fn update_unlock_states<R: ReadMemory>(
    reader: &R,
    old_state: &HashMap<u32, UnlockData>,
    unlock_data_addr: u64,
    song_db: &HashMap<u32, SongInfo>,
) -> Result<HashMap<u32, UnlockData>> {
    // Get current state from memory
    let current_state = get_unlock_states(reader, unlock_data_addr, song_db)?;

    let mut changes = HashMap::new();

    for (&song_id, current_data) in &current_state {
        if let Some(old_data) = old_state.get(&song_id) {
            // Check if unlock state changed
            if current_data.unlocks != old_data.unlocks {
                changes.insert(song_id, current_data.clone());
            }
        }
        // Note: New songs not in old_state are not considered "changes"
        // They should be handled by the server sync logic
    }

    Ok(changes)
}

/// Detect unlock state changes without re-reading from memory
/// (for use when you already have the new state)
pub fn detect_unlock_changes(
    old_state: &HashMap<u32, UnlockData>,
    new_state: &HashMap<u32, UnlockData>,
) -> HashMap<u32, UnlockData> {
    let mut changes = HashMap::new();

    for (&song_id, new_data) in new_state {
        if let Some(old_data) = old_state.get(&song_id)
            && new_data.unlocks != old_data.unlocks
        {
            changes.insert(song_id, new_data.clone());
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_difficulty_unlocked() {
        let unlock = UnlockData {
            song_id: 1000,
            unlock_type: UnlockType::Base,
            unlocks: 0b11111, // SPB through SPL unlocked
        };

        assert!(unlock.is_difficulty_unlocked(Difficulty::SpB));
        assert!(unlock.is_difficulty_unlocked(Difficulty::SpN));
        assert!(unlock.is_difficulty_unlocked(Difficulty::SpH));
        assert!(unlock.is_difficulty_unlocked(Difficulty::SpA));
        assert!(unlock.is_difficulty_unlocked(Difficulty::SpL));
        assert!(!unlock.is_difficulty_unlocked(Difficulty::DpN));
    }

    #[test]
    fn test_from_bytes() {
        let bytes = [
            0xE8, 0x03, 0x00, 0x00, // song_id = 1000
            0x01, 0x00, 0x00, 0x00, // unlock_type = Base
            0x1F, 0x00, 0x00, 0x00, // unlocks = 0x1F
            0x00, 0x00, 0x00, 0x00, // padding
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let unlock = UnlockData::from_bytes(&bytes).unwrap();
        assert_eq!(unlock.song_id, 1000);
        assert_eq!(unlock.unlock_type, UnlockType::Base);
        assert_eq!(unlock.unlocks, 0x1F);
    }

    /// Helper to build a 32-byte unlock buffer with a specific unlock_type i32 at offset 4.
    fn build_unlock_bytes_with_type(unlock_type_val: i32) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        // song_id = 1000 at offset 0
        bytes[0..4].copy_from_slice(&1000u32.to_le_bytes());
        // unlock_type at offset 4
        bytes[4..8].copy_from_slice(&unlock_type_val.to_le_bytes());
        // unlocks = 0x1F at offset 8
        bytes[8..12].copy_from_slice(&0x1Fi32.to_le_bytes());
        bytes
    }

    #[test]
    fn test_from_bytes_unknown_type_zero_defaults_to_base() {
        let bytes = build_unlock_bytes_with_type(0);
        let unlock = UnlockData::from_bytes(&bytes).unwrap();
        assert_eq!(unlock.song_id, 1000);
        assert_eq!(unlock.unlock_type, UnlockType::Base);
        assert_eq!(unlock.unlocks, 0x1F);
    }

    #[test]
    fn test_from_bytes_unknown_type_four_defaults_to_base() {
        let bytes = build_unlock_bytes_with_type(4);
        let unlock = UnlockData::from_bytes(&bytes).unwrap();
        assert_eq!(unlock.unlock_type, UnlockType::Base);
    }

    #[test]
    fn test_from_bytes_unknown_type_large_value_defaults_to_base() {
        // 0x107 = 263, tests i32 values > 255
        let bytes = build_unlock_bytes_with_type(0x107);
        let unlock = UnlockData::from_bytes(&bytes).unwrap();
        assert_eq!(unlock.unlock_type, UnlockType::Base);
    }

    #[test]
    fn test_from_bytes_unknown_type_negative_defaults_to_base() {
        let bytes = build_unlock_bytes_with_type(-1);
        let unlock = UnlockData::from_bytes(&bytes).unwrap();
        assert_eq!(unlock.unlock_type, UnlockType::Base);
    }
}
