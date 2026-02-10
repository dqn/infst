//! Play settings and play data validation.

use crate::process::ReadMemory;
use crate::process::layout::settings;

use super::super::constants::*;

/// Validate if the given address contains valid PlaySettings.
///
/// Memory layout:
/// - 0x00: style (4 bytes, range 0-6)
/// - 0x04: gauge (4 bytes, range 0-4)
/// - 0x08: assist (4 bytes, range 0-5)
/// - 0x0C: flip (4 bytes, 0 or 1)
/// - 0x10: range (4 bytes, range 0-5)
pub fn validate_play_settings_at<R: ReadMemory + ?Sized>(reader: &R, addr: u64) -> Option<u64> {
    let style = reader.read_i32(addr).ok()?;
    let gauge = reader.read_i32(addr + 4).ok()?;
    let assist = reader.read_i32(addr + 8).ok()?;
    let flip = reader.read_i32(addr + 12).ok()?;
    let range = reader.read_i32(addr + 16).ok()?;

    // Valid ranges check (aligned with C# implementation)
    if !(0..=6).contains(&style)
        || !(0..=4).contains(&gauge)
        || !(0..=5).contains(&assist)
        || !(0..=1).contains(&flip)
        || !(0..=5).contains(&range)
    {
        return None;
    }

    // Additional validation: song_select_marker should be 0 or 1
    let song_select_marker = reader
        .read_i32(addr.wrapping_sub(settings::SONG_SELECT_MARKER))
        .ok()?;
    if !(0..=1).contains(&song_select_marker) {
        return None;
    }

    Some(addr)
}

/// Validate if an address contains valid PlayData.
///
/// Memory layout (from C# PlayData.cs):
/// - offset 0: song_id (i32)
/// - offset 4: difficulty (i32)
/// - offset 8-20: unknown/unused
/// - offset 24 (WORD*6): lamp (i32, 0-7)
///
/// Initial state (all zeros) is NOT accepted during offset search.
/// We need actual play data with valid song_id to verify the offset is correct.
pub fn validate_play_data_address<R: ReadMemory + ?Sized>(reader: &R, addr: u64) -> bool {
    use crate::process::layout::play;

    let song_id = reader.read_i32(addr + play::SONG_ID).unwrap_or(-1);
    let difficulty = reader.read_i32(addr + play::DIFFICULTY).unwrap_or(-1);
    let lamp = reader.read_i32(addr + play::LAMP).unwrap_or(-1);

    // Do NOT accept initial state (all zeros) during offset search.
    // Zero values can appear at wrong addresses - we need actual data to validate.
    // The game should have play data populated when we're searching for offsets.
    if song_id == 0 && difficulty == 0 && lamp == 0 {
        return false;
    }

    // Require song_id in valid IIDX range (>= 1000)
    (MIN_SONG_ID..=MAX_SONG_ID).contains(&song_id)
        && (0..=9).contains(&difficulty)
        && (0..=7).contains(&lamp)
}
