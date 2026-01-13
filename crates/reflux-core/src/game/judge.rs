use serde::{Deserialize, Serialize};

use crate::game::PlayType;

/// Judge information from a play
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Judge {
    pub play_type: PlayType,
    pub pgreat: u32,
    pub great: u32,
    pub good: u32,
    pub bad: u32,
    pub poor: u32,
    pub fast: u32,
    pub slow: u32,
    pub combo_break: u32,
    pub premature_end: bool,
}

impl Judge {
    /// Memory layout offsets (each value is 4 bytes)
    /// P1: 0-4 (pgreat, great, good, bad, poor)
    /// P2: 5-9 (pgreat, great, good, bad, poor)
    /// CB: 10-11 (p1, p2)
    /// Fast: 12-13 (p1, p2)
    /// Slow: 14-15 (p1, p2)
    /// Measure end: 16-17 (p1, p2)
    pub const WORD_SIZE: u64 = 4;

    /// Check if this is a Perfect Full Combo (no good/bad/poor)
    pub fn is_pfc(&self) -> bool {
        self.good == 0 && self.bad == 0 && self.poor == 0
    }

    /// Calculate EX score (pgreat * 2 + great)
    pub fn ex_score(&self) -> u32 {
        self.pgreat * 2 + self.great
    }

    /// Calculate miss count (bad + poor)
    pub fn miss_count(&self) -> u32 {
        self.bad + self.poor
    }

    /// Build judge data from P1 and P2 values
    #[allow(clippy::too_many_arguments)] // Mapping raw memory layout requires many parameters
    pub fn from_raw_values(
        p1_pgreat: u32,
        p1_great: u32,
        p1_good: u32,
        p1_bad: u32,
        p1_poor: u32,
        p2_pgreat: u32,
        p2_great: u32,
        p2_good: u32,
        p2_bad: u32,
        p2_poor: u32,
        p1_cb: u32,
        p2_cb: u32,
        p1_fast: u32,
        p2_fast: u32,
        p1_slow: u32,
        p2_slow: u32,
        p1_measure_end: u32,
        p2_measure_end: u32,
    ) -> Self {
        // Determine play type based on which side has judgments
        let p1_total = p1_pgreat + p1_great + p1_good + p1_bad + p1_poor;
        let p2_total = p2_pgreat + p2_great + p2_good + p2_bad + p2_poor;

        let play_type = if p1_total == 0 && p2_total > 0 {
            PlayType::P2
        } else if p1_total > 0 && p2_total > 0 {
            PlayType::Dp
        } else {
            PlayType::P1
        };

        Self {
            play_type,
            pgreat: p1_pgreat + p2_pgreat,
            great: p1_great + p2_great,
            good: p1_good + p2_good,
            bad: p1_bad + p2_bad,
            poor: p1_poor + p2_poor,
            fast: p1_fast + p2_fast,
            slow: p1_slow + p2_slow,
            combo_break: p1_cb + p2_cb,
            premature_end: (p1_measure_end + p2_measure_end) != 0,
        }
    }
}
