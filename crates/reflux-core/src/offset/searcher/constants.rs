//! Search-related constants for offset detection
//!
//! # Search Strategy
//!
//! The offset searcher uses JudgeData as an anchor point, then finds other
//! offsets via relative positions. This approach is reliable because:
//!
//! 1. JudgeData has a unique pattern (18 consecutive zeros in song select)
//! 2. Relative offsets between structures are stable across game versions
//!
//! # Offset Relationships
//!
//! ```text
//!                        Memory Layout (approximate)
//! ┌─────────────────────────────────────────────────────────┐
//! │  PlaySettings  ◄──── 0x2ACE00 ────► JudgeData          │
//! │       │                                  │               │
//! │       │ 0x2B0                           │ 0x1E4         │
//! │       ▼                                  ▼               │
//! │   PlayData                          CurrentSong         │
//! │                                          │               │
//! │                                          │ ~0x94E000     │
//! │                                          ▼               │
//! │                                      SongList           │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Historical Analysis
//!
//! These values are derived from analysis of 9 game versions and remain
//! remarkably stable across updates.

/// Initial buffer size for memory search (2MB)
pub const INITIAL_SEARCH_SIZE: usize = 2 * 1024 * 1024;
/// Maximum buffer size for memory search (300MB)
pub const MAX_SEARCH_SIZE: usize = 300 * 1024 * 1024;

/// Minimum number of songs expected in INFINITAS (for validation)
pub const MIN_EXPECTED_SONGS: usize = 1000;

// ============================================================================
// Relative Offsets (derived from historical analysis of 9 versions)
// ============================================================================

/// Expected offset: judgeData - playSettings ≈ 0x2ACE00
///
/// Historical variation: ±0x100 (256 bytes)
pub const JUDGE_TO_PLAY_SETTINGS: u64 = 0x2ACE00;

/// Search range for playSettings (±8KB)
///
/// This is ~32x the measured variation to ensure reliable detection.
pub const PLAY_SETTINGS_SEARCH_RANGE: usize = 0x2000;

/// Expected offset: songList - judgeData ≈ 0x94E000
///
/// Historical variation: ±0x600 (1.5KB)
pub const JUDGE_TO_SONG_LIST: u64 = 0x94E000;

/// Search range for songList (±64KB)
///
/// This is ~27x the measured variation to ensure reliable detection.
pub const SONG_LIST_SEARCH_RANGE: usize = 0x10000;

/// Expected offset: playData - playSettings ≈ 0x2B0
///
/// Historical variation: ±0x10 (16 bytes)
pub const PLAY_SETTINGS_TO_PLAY_DATA: u64 = 0x2B0;

/// Search range for playData (±256 bytes)
///
/// This is ~16x the measured variation to ensure reliable detection.
pub const PLAY_DATA_SEARCH_RANGE: usize = 0x100;

/// Expected offset: currentSong - judgeData ≈ 0x1E4
///
/// Historical variation: ±0x10 (16 bytes)
pub const JUDGE_TO_CURRENT_SONG: u64 = 0x1E4;

/// Search range for currentSong (±256 bytes)
///
/// This is ~16x the measured variation to ensure reliable detection.
pub const CURRENT_SONG_SEARCH_RANGE: usize = 0x100;
