use serde::Serialize;

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

#[derive(Debug, Clone, Serialize)]
pub struct PlayDataJson {
    pub timestamp: String,
    pub song_id: String,
    pub title: String,
    pub difficulty: String,
    pub level: u8,
    pub ex_score: u32,
    pub grade: String,
    pub lamp: String,
    pub judge: JudgeJson,
}

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
