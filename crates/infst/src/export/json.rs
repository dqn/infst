//! JSON export format implementation

use serde::Serialize;
use serde_json::{Value as JsonValue, json};

use crate::play::PlayData;

use super::format::ExportFormat;

/// JSON exporter (one object per line, NDJSON format)
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonExporter;

impl ExportFormat for JsonExporter {
    fn header(&self) -> Option<String> {
        None // JSON doesn't need a header
    }

    fn format_row(&self, play_data: &PlayData) -> String {
        format_json_entry(play_data).to_string()
    }
}

/// Generate JSON entry for session file (simple format)
pub fn format_json_entry(play_data: &PlayData) -> JsonValue {
    let miss_count = if play_data.miss_count_valid() {
        Some(play_data.miss_count())
    } else {
        None
    };

    json!({
        "timestamp": play_data.timestamp.to_rfc3339(),
        "song_id": play_data.chart.song_id,
        "title": play_data.chart.title,
        "difficulty": play_data.chart.difficulty.short_name(),
        "level": play_data.chart.level,
        "ex_score": play_data.ex_score,
        "grade": play_data.grade.short_name(),
        "lamp": play_data.lamp.expand_name(),
        "judge": {
            "pgreat": play_data.judge.pgreat,
            "great": play_data.judge.great,
            "good": play_data.judge.good,
            "bad": play_data.judge.bad,
            "poor": play_data.judge.poor,
            "fast": play_data.judge.fast,
            "slow": play_data.judge.slow,
            "combo_break": play_data.judge.combo_break
        },
        "miss_count": miss_count
    })
}

/// Play data JSON structure for serialization
#[derive(Debug, Clone, Serialize)]
pub struct PlayDataJson {
    pub timestamp: String,
    pub song_id: u32,
    pub title: String,
    pub difficulty: String,
    pub level: u8,
    pub ex_score: u32,
    pub grade: String,
    pub lamp: String,
    pub judge: JudgeJson,
}

/// Judge data JSON structure
#[derive(Debug, Clone, Serialize)]
pub struct JudgeJson {
    pub pgreat: u32,
    pub great: u32,
    pub good: u32,
    pub bad: u32,
    pub poor: u32,
    pub fast: u32,
    pub slow: u32,
    pub combo_break: u32,
}
