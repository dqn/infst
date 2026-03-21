use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::chart::EntryLayout;
use crate::process::ReadMemory;

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
    /// Address of the "IIDX" header in memory (V3+).
    /// Used to resolve game_id -> internal_id via the current song pointer structure.
    /// The pointer at iidx_header - 0x40 points to the current song's entry title field.
    #[serde(default)]
    pub iidx_header: u64,
    /// Song count read from the IIDX header (V3+).
    /// Used to compute the entry table boundary for pointer validation.
    #[serde(default)]
    pub iidx_song_count: u32,
    /// Auto-detected entry field layout.
    /// When `None`, callers should use `EntryLayout::v3_default()`.
    #[serde(default)]
    pub entry_layout: Option<EntryLayout>,
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

    /// Get the effective entry layout (detected or V3 default).
    pub fn effective_layout(&self) -> EntryLayout {
        self.entry_layout
            .clone()
            .unwrap_or_else(EntryLayout::v3_default)
    }

    /// Resolve the current song's internal_id via the IIDX pointer structure.
    ///
    /// The structure before the IIDX header contains a pointer to the current song's
    /// entry in the entry table (title field at +0x180). By dereferencing and reading
    /// offset 0, we get the true internal_id regardless of the game_id used elsewhere.
    ///
    /// Returns Some(internal_id) if the pointer is valid and points into the entry table.
    /// The caller should verify that the returned internal_id is different from the game_id.
    pub fn resolve_current_song_internal_id<R: ReadMemory>(
        &self,
        reader: &R,
        expected_game_id: u32,
    ) -> Option<u32> {
        if self.iidx_header == 0 || self.song_entry_table == 0 {
            return None;
        }

        // The current song pointer is at iidx_header - 0x40 (u64)
        // The game_id verification is at iidx_header - 0x34 (i32)
        let ptr_addr = self.iidx_header.checked_sub(0x40)?;
        let gid_addr = self.iidx_header.checked_sub(0x34)?;

        // Verify the game_id matches what we expect
        let gid = reader.read_i32(gid_addr).ok()?;
        if gid as u32 != expected_game_id {
            debug!(
                "IIDX game_id mismatch: expected {}, got {}",
                expected_game_id, gid
            );
            return None;
        }

        // Read the pointer to the entry's title field
        let title_ptr = reader.read_u64(ptr_addr).ok()?;
        if title_ptr == 0 {
            return None;
        }

        // Entry start = title_ptr - title_offset
        let title_offset = self.effective_layout().title as u64;
        let entry_start = title_ptr.checked_sub(title_offset)?;

        // Validate: entry_start should be within the entry table range
        let table_start = self.song_entry_table;
        let stride = self.entry_stride() as u64;
        let max_entries = if self.iidx_song_count > 0 {
            self.iidx_song_count as u64 + 200 // margin for future song additions
        } else {
            3000 // fallback matching find_iidx_header validation range
        };
        let table_end = max_entries
            .checked_mul(stride)
            .and_then(|size| table_start.checked_add(size))?;
        if entry_start < table_start || entry_start >= table_end {
            debug!(
                "IIDX pointer out of entry table range: 0x{:X} (table: 0x{:X}-0x{:X})",
                entry_start, table_start, table_end
            );
            return None;
        }

        // Verify alignment to entry stride
        let offset_from_start = entry_start - table_start;
        if !offset_from_start.is_multiple_of(stride) {
            debug!(
                "IIDX pointer not aligned to stride: offset 0x{:X}, stride 0x{:X}",
                offset_from_start, stride
            );
            return None;
        }

        // Read internal_id at entry start
        let internal_id = reader.read_i32(entry_start).ok()?;
        if (1000..=90000).contains(&internal_id) {
            Some(internal_id as u32)
        } else {
            None
        }
    }
}

/// Search for the "IIDX" header in memory near the entry table.
///
/// The IIDX header is a structure containing the ASCII bytes "IIDX" followed by
/// metadata including an entry count close to the total number of songs.
/// It is located before the entry table in memory.
///
/// Returns `(header_address, song_count)` if found.
pub fn find_iidx_header<R: ReadMemory>(reader: &R, entry_table_addr: u64) -> Option<(u64, u32)> {
    if entry_table_addr == 0 {
        return None;
    }

    // Search 256KB before the entry table
    let search_size: u64 = 0x40000;
    let search_start = entry_table_addr.saturating_sub(search_size);
    let buf_size = (entry_table_addr - search_start) as usize;

    let buffer = reader.read_bytes(search_start, buf_size).ok()?;
    let iidx_pattern = b"IIDX";

    // Search backwards (IIDX is typically near the entry table)
    for i in (0..buffer.len().saturating_sub(12)).rev() {
        if &buffer[i..i + 4] == iidx_pattern {
            let addr = search_start + i as u64;

            // Validate: next 4 bytes should be a small value (entry size ~80)
            let entry_size = u32::from_le_bytes(buffer[i + 4..i + 8].try_into().ok()?);
            if entry_size > 256 {
                continue;
            }

            // Next 4 bytes should be the count (~1800-2000 songs)
            let count = u32::from_le_bytes(buffer[i + 8..i + 12].try_into().ok()?);
            if !(1500..=3000).contains(&count) {
                continue;
            }

            debug!(
                "Found IIDX header at 0x{:X} (entry_size={}, count={})",
                addr, entry_size, count
            );
            return Some((addr, count));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::mock::MockMemoryBuilder;

    /// Build an OffsetsCollection with the given iidx_header and song_entry_table.
    fn make_offsets(iidx_header: u64, song_entry_table: u64) -> OffsetsCollection {
        OffsetsCollection {
            iidx_header,
            song_entry_table,
            ..Default::default()
        }
    }

    #[test]
    fn resolve_returns_none_when_iidx_header_too_small_to_subtract() {
        // iidx_header = 0x10, which is less than 0x40.
        // Without checked_sub this would wrap around to a huge address.
        let offsets = make_offsets(0x10, 0x5000);
        let reader = MockMemoryBuilder::new().base(0x0).with_size(0x100).build();

        assert_eq!(
            offsets.resolve_current_song_internal_id(&reader, 1001),
            None
        );
    }

    #[test]
    fn resolve_returns_none_when_iidx_header_is_exactly_threshold() {
        // iidx_header = 0x3F: one less than 0x40, still underflows.
        let offsets = make_offsets(0x3F, 0x5000);
        let reader = MockMemoryBuilder::new().base(0x0).with_size(0x100).build();

        assert_eq!(
            offsets.resolve_current_song_internal_id(&reader, 1001),
            None
        );
    }

    #[test]
    fn resolve_returns_none_when_iidx_header_zero() {
        let offsets = make_offsets(0, 0x5000);
        let reader = MockMemoryBuilder::new().base(0x0).with_size(0x100).build();

        // Early return because iidx_header == 0
        assert_eq!(
            offsets.resolve_current_song_internal_id(&reader, 1001),
            None
        );
    }
}
