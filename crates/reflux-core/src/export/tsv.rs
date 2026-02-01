//! TSV export format implementation

use crate::play::PlayData;

use super::format::ExportFormat;

/// TSV (Tab-Separated Values) exporter
#[derive(Debug, Clone, Copy, Default)]
pub struct TsvExporter;

impl ExportFormat for TsvExporter {
    fn header(&self) -> Option<String> {
        Some(format_full_tsv_header())
    }

    fn format_row(&self, play_data: &PlayData) -> String {
        format_full_tsv_row(play_data)
    }
}

/// Row data structure for simple TSV export
pub struct TsvRowData<'a> {
    pub timestamp: &'a str,
    pub title: &'a str,
    pub difficulty: &'a str,
    pub level: u8,
    pub ex_score: u32,
    pub grade: &'a str,
    pub lamp: &'a str,
    pub pgreat: u32,
    pub great: u32,
    pub good: u32,
    pub bad: u32,
    pub poor: u32,
    pub fast: u32,
    pub slow: u32,
    pub combo_break: u32,
}

/// Generate simple TSV header
pub fn format_tsv_header() -> String {
    [
        "Timestamp",
        "Title",
        "Difficulty",
        "Level",
        "EX Score",
        "Grade",
        "Lamp",
        "PGreat",
        "Great",
        "Good",
        "Bad",
        "Poor",
        "Fast",
        "Slow",
        "ComboBreak",
    ]
    .join("\t")
}

/// Generate TSV header with all columns
pub fn format_full_tsv_header() -> String {
    let columns = vec![
        "title",
        "difficulty",
        "title2",
        "bpm",
        "artist",
        "genre",
        "notecount",
        "level",
        "playtype",
        "grade",
        "lamp",
        "misscount",
        "exscore",
        "pgreat",
        "great",
        "good",
        "bad",
        "poor",
        "combobreak",
        "fast",
        "slow",
        "style",
        "style2",
        "assist",
        "range",
        "date",
    ];

    columns.join("\t")
}

/// Generate TSV row with all columns
pub fn format_full_tsv_row(play_data: &PlayData) -> String {
    let values: Vec<String> = vec![
        play_data.chart.title.to_string(),
        play_data.chart.difficulty.short_name().to_string(),
        play_data.chart.title_english.to_string(),
        play_data.chart.bpm.to_string(),
        play_data.chart.artist.to_string(),
        play_data.chart.genre.to_string(),
        play_data.chart.total_notes.to_string(),
        play_data.chart.level.to_string(),
        play_data.judge.play_type.short_name().to_string(),
        play_data.grade.short_name().to_string(),
        play_data.lamp.short_name().to_string(),
        if play_data.miss_count_valid() {
            play_data.miss_count().to_string()
        } else {
            "-".to_string()
        },
        play_data.ex_score.to_string(),
        play_data.judge.pgreat.to_string(),
        play_data.judge.great.to_string(),
        play_data.judge.good.to_string(),
        play_data.judge.bad.to_string(),
        play_data.judge.poor.to_string(),
        play_data.judge.combo_break.to_string(),
        play_data.judge.fast.to_string(),
        play_data.judge.slow.to_string(),
        play_data.settings.style.as_str().to_string(),
        play_data
            .settings
            .style2
            .map(|s| s.as_str())
            .unwrap_or("OFF")
            .to_string(),
        play_data.settings.assist.as_str().to_string(),
        play_data.settings.range.as_str().to_string(),
        play_data.timestamp.to_rfc3339(),
    ];

    values.join("\t")
}

/// Format simple TSV row from TsvRowData
pub fn format_tsv_row(data: &TsvRowData) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        data.timestamp,
        data.title,
        data.difficulty,
        data.level,
        data.ex_score,
        data.grade,
        data.lamp,
        data.pgreat,
        data.great,
        data.good,
        data.bad,
        data.poor,
        data.fast,
        data.slow,
        data.combo_break
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tsv_header() {
        let header = format_tsv_header();
        assert!(header.contains("Timestamp"));
        assert!(header.contains("Title"));
        assert!(header.contains("Difficulty"));
        assert!(header.contains("EX Score"));
        assert!(header.contains("Lamp"));
    }

    #[test]
    fn test_format_full_tsv_header() {
        let header = format_full_tsv_header();
        assert!(header.contains("title"));
        assert!(header.contains("difficulty"));
        assert!(header.contains("notecount"));
        assert!(header.contains("exscore"));
        assert!(header.contains("date"));
    }

    #[test]
    fn test_tsv_row_data() {
        let data = TsvRowData {
            timestamp: "2025-01-30T12:00:00Z",
            title: "Test Song",
            difficulty: "SPA",
            level: 12,
            ex_score: 2500,
            grade: "AAA",
            lamp: "HARD",
            pgreat: 1200,
            great: 100,
            good: 5,
            bad: 2,
            poor: 1,
            fast: 30,
            slow: 20,
            combo_break: 3,
        };
        let row = format_tsv_row(&data);

        assert!(row.contains("Test Song"));
        assert!(row.contains("SPA"));
        assert!(row.contains("2500"));
        assert!(row.contains("AAA"));
        assert!(row.contains("HARD"));
    }
}
