use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::game::{AssistType, ChartInfo, Grade, Judge, Lamp, Settings};

/// Complete play data for a single play
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayData {
    pub timestamp: DateTime<Utc>,
    pub chart: ChartInfo,
    pub ex_score: u32,
    pub gauge: u8,
    pub grade: Grade,
    pub lamp: Lamp,
    pub judge: Judge,
    pub settings: Settings,
    /// False if play data isn't available (H-RAN, BATTLE or assist options enabled)
    pub data_available: bool,
}

impl PlayData {
    /// Check if miss count should be saved
    /// (not available when using assist options or premature end)
    pub fn miss_count_valid(&self) -> bool {
        self.data_available && !self.judge.premature_end && self.settings.assist == AssistType::Off
    }

    /// Get miss count (bad + poor)
    pub fn miss_count(&self) -> u32 {
        self.judge.miss_count()
    }

    /// Calculate grade from EX score
    pub fn calculate_grade(ex_score: u32, total_notes: u32) -> Grade {
        if total_notes == 0 {
            return Grade::F;
        }
        let max_ex = total_notes * 2;
        let ratio = ex_score as f64 / max_ex as f64;
        Grade::from_score_ratio(ratio)
    }

    /// Upgrade lamp to PFC if applicable
    pub fn upgrade_lamp_if_pfc(&mut self) {
        if self.judge.is_pfc() && self.lamp == Lamp::FullCombo {
            self.lamp = Lamp::Pfc;
        }
    }
}
