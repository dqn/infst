//! Search-related constants for offset detection

pub const INITIAL_SEARCH_SIZE: usize = 2 * 1024 * 1024; // 2MB
pub const MAX_SEARCH_SIZE: usize = 300 * 1024 * 1024; // 300MB

/// Minimum number of songs expected in INFINITAS (for validation)
pub const MIN_EXPECTED_SONGS: usize = 1000;

// Relative offsets derived from historical analysis (9 versions)
// These are remarkably stable across versions

/// Expected offset: judgeData - playSettings ≈ 0x2ACE00 (variation: 0x100)
pub const JUDGE_TO_PLAY_SETTINGS: u64 = 0x2ACE00;
/// Search range for playSettings (±8KB, ~32x measured variation)
pub const PLAY_SETTINGS_SEARCH_RANGE: usize = 0x2000;

/// Expected offset: songList - judgeData ≈ 0x94E000 (variation: 0x600)
pub const JUDGE_TO_SONG_LIST: u64 = 0x94E000;
/// Search range for songList (±64KB, ~27x measured variation)
pub const SONG_LIST_SEARCH_RANGE: usize = 0x10000;

/// Expected offset: playData - playSettings ≈ 0x2B0 (variation: 0x10)
pub const PLAY_SETTINGS_TO_PLAY_DATA: u64 = 0x2B0;
/// Search range for playData (±256 bytes, ~16x measured variation)
pub const PLAY_DATA_SEARCH_RANGE: usize = 0x100;

/// Expected offset: currentSong - judgeData ≈ 0x1E4 (variation: 0x10)
pub const JUDGE_TO_CURRENT_SONG: u64 = 0x1E4;
/// Search range for currentSong (±256 bytes, ~16x measured variation)
pub const CURRENT_SONG_SEARCH_RANGE: usize = 0x100;
