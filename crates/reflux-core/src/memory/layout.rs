//! Memory layout constants for INFINITAS data structures
//!
//! This module centralizes all memory layout constants used for reading game data.
//! Constants are organized by structure type.

/// Memory layout constants for JudgeData structure
pub mod judge {
    /// Word size (4 bytes / 32-bit integer)
    pub const WORD: u64 = 4;

    // Player 1 judge data (offsets 0-4)
    pub const P1_PGREAT: u64 = 0;
    pub const P1_GREAT: u64 = WORD;
    pub const P1_GOOD: u64 = WORD * 2;
    pub const P1_BAD: u64 = WORD * 3;
    pub const P1_POOR: u64 = WORD * 4;

    // Player 2 judge data (offsets 5-9)
    pub const P2_PGREAT: u64 = WORD * 5;
    pub const P2_GREAT: u64 = WORD * 6;
    pub const P2_GOOD: u64 = WORD * 7;
    pub const P2_BAD: u64 = WORD * 8;
    pub const P2_POOR: u64 = WORD * 9;

    // Combo break data (offsets 10-11)
    pub const P1_COMBO_BREAK: u64 = WORD * 10;
    pub const P2_COMBO_BREAK: u64 = WORD * 11;

    // Fast/Slow data (offsets 12-15)
    pub const P1_FAST: u64 = WORD * 12;
    pub const P2_FAST: u64 = WORD * 13;
    pub const P1_SLOW: u64 = WORD * 14;
    pub const P2_SLOW: u64 = WORD * 15;

    // Measure end markers (offsets 16-17)
    pub const P1_MEASURE_END: u64 = WORD * 16;
    pub const P2_MEASURE_END: u64 = WORD * 17;

    // Game state detection markers (offsets 54-55)
    pub const STATE_MARKER_1: u64 = WORD * 54;
    pub const STATE_MARKER_2: u64 = WORD * 55;

    // Gauge percentage (offsets 81-82)
    pub const P1_GAUGE: u64 = WORD * 81;
    pub const P2_GAUGE: u64 = WORD * 82;

    /// Size of initial zero region in song select state (18 i32 values = 72 bytes)
    /// P1 (5) + P2 (5) + CB (2) + Fast/Slow (4) + MeasureEnd (2) = 18
    pub const INITIAL_ZERO_SIZE: usize = 72;
}

/// Memory layout constants for PlayData structure
pub mod play {
    pub const WORD: u64 = 4;

    pub const SONG_ID: u64 = 0;
    pub const DIFFICULTY: u64 = WORD;
    pub const LAMP: u64 = WORD * 6;
}

/// Memory layout constants for PlaySettings structure
pub mod settings {
    pub const WORD: u64 = 4;

    /// Song select marker position (negative offset from PlaySettings)
    pub const SONG_SELECT_MARKER: u64 = WORD * 6;
}

/// Timing constants for polling and rate limiting
pub mod timing {
    /// Interval between game state checks in the main loop (ms)
    pub const GAME_STATE_POLL_INTERVAL_MS: u64 = 100;

    /// Delay between API requests when syncing scores to avoid server overload (ms)
    pub const SERVER_SYNC_REQUEST_DELAY_MS: u64 = 20;
}
