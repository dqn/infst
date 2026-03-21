//! Tracker data export (TSV and JSON formats)

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Serialize;

use tracing::warn;

use crate::chart::{Difficulty, SongInfo, UnlockData, get_unlock_state_for_difficulty};
use crate::error::Result;
use crate::play::{PlayData, UnlockType, calculate_dj_points};
use crate::score::{Grade, Lamp, ScoreMap};

/// Per-chart export data (shared between TSV and JSON)
struct ChartExportData {
    difficulty: Difficulty,
    unlocked: bool,
    level: u8,
    lamp: Lamp,
    grade: Grade,
    ex_score: u32,
    miss_count: Option<u32>,
    total_notes: u32,
    dj_points: f64,
}

/// Per-song export data
struct SongExportData {
    song_id: u32,
    title: String,
    artist: String,
    unlock_type: UnlockType,
    charts: Vec<ChartExportData>,
    sp_dj_points: f64,
    dp_dj_points: f64,
}

/// All 10 difficulties in order
const ALL_DIFFICULTIES: [Difficulty; 10] = [
    Difficulty::SpB,
    Difficulty::SpN,
    Difficulty::SpH,
    Difficulty::SpA,
    Difficulty::SpL,
    Difficulty::DpB,
    Difficulty::DpN,
    Difficulty::DpH,
    Difficulty::DpA,
    Difficulty::DpL,
];

/// Collect per-song export data from the databases and score map.
///
/// Returns all 10 difficulties (including DpB). Formatters decide which to include/skip.
fn collect_song_export_data(
    song_id: u32,
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Option<SongExportData> {
    let song = song_db.get(&song_id)?;
    let unlock = match unlock_db.get(&song_id) {
        Some(u) => u,
        None => {
            if score_map.get(song_id).is_some() {
                warn!(
                    song_id,
                    title = %song.title,
                    "Song with score data excluded from export: no unlock data found (may be aliased)"
                );
            }
            return None;
        }
    };
    let scores = score_map.get(song_id);

    let mut sp_dj_points = 0.0f64;
    let mut dp_dj_points = 0.0f64;

    let mut charts = Vec::with_capacity(ALL_DIFFICULTIES.len());
    for diff in &ALL_DIFFICULTIES {
        let diff_index = *diff as usize;
        let unlocked = get_unlock_state_for_difficulty(unlock_db, song_db, song_id, *diff);
        let level = song.levels[diff_index];
        let total_notes = song.total_notes[diff_index];

        let (lamp, grade, ex_score, miss_count, djp) = if let Some(s) = scores {
            let lamp = s.lamp[diff_index];
            let ex_score = s.score[diff_index];
            let grade = if total_notes > 0 {
                PlayData::calculate_grade(ex_score, total_notes)
            } else {
                Grade::NoPlay
            };
            let djp = if total_notes > 0 {
                calculate_dj_points(ex_score, grade, lamp)
            } else {
                0.0
            };
            let miss_count = s.miss_count[diff_index];
            (lamp, grade, ex_score, miss_count, djp)
        } else {
            (Lamp::NoPlay, Grade::NoPlay, 0, None, 0.0)
        };

        if diff.is_sp() {
            sp_dj_points = sp_dj_points.max(djp);
        } else {
            dp_dj_points = dp_dj_points.max(djp);
        }

        charts.push(ChartExportData {
            difficulty: *diff,
            unlocked,
            level,
            lamp,
            grade,
            ex_score,
            miss_count,
            total_notes,
            dj_points: djp,
        });
    }

    Some(SongExportData {
        song_id,
        title: song.title.to_string(),
        artist: song.artist.to_string(),
        unlock_type: unlock.unlock_type,
        charts,
        sp_dj_points,
        dp_dj_points,
    })
}

/// Chart data for JSON export
#[derive(Debug, Serialize)]
pub struct ChartDataJson {
    pub difficulty: String,
    pub level: u8,
    pub lamp: String,
    pub grade: String,
    pub ex_score: u32,
    pub miss_count: Option<u32>,
    pub total_notes: u32,
    pub dj_points: f64,
}

/// Song data for JSON export
#[derive(Debug, Serialize)]
pub struct SongDataJson {
    pub song_id: u32,
    pub title: String,
    pub artist: String,
    pub charts: Vec<ChartDataJson>,
}

/// Export data for JSON export
#[derive(Debug, Serialize)]
pub struct ExportDataJson {
    pub songs: Vec<SongDataJson>,
}

/// Generate detailed tracker TSV header
pub fn format_tracker_tsv_header() -> String {
    let mut columns = vec![
        "Song ID".to_string(),
        "Title".to_string(),
        "Type".to_string(),
        "Label".to_string(),
        "Cost Normal".to_string(),
        "Cost Hyper".to_string(),
        "Cost Another".to_string(),
        "SP DJ Points".to_string(),
        "DP DJ Points".to_string(),
    ];

    // Add columns for each difficulty (skipping DPB which doesn't exist)
    let difficulties = [
        "SPB", "SPN", "SPH", "SPA", "SPL", "DPN", "DPH", "DPA", "DPL",
    ];
    for diff in difficulties {
        columns.push(format!("{} Unlocked", diff));
        columns.push(format!("{} Rating", diff));
        columns.push(format!("{} Lamp", diff));
        columns.push(format!("{} Letter", diff));
        columns.push(format!("{} EX Score", diff));
        columns.push(format!("{} Miss Count", diff));
        columns.push(format!("{} Note Count", diff));
        columns.push(format!("{} DJ Points", diff));
    }

    columns.join("\t")
}

/// Export detailed tracker data to TSV
pub fn export_tracker_tsv<P: AsRef<Path>>(
    path: P,
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Result<()> {
    let mut lines = vec![format_tracker_tsv_header()];

    // Get all song IDs from song database (sorted)
    let mut song_ids: Vec<&u32> = song_db.keys().collect();
    song_ids.sort();

    for &song_id in song_ids {
        if let Some(entry) = generate_tracker_entry(song_id, song_db, unlock_db, score_map) {
            lines.push(entry);
        }
    }

    // Write to temp file first, then rename atomically to avoid corruption
    let path = path.as_ref();
    let tmp_path = path.with_extension("tsv.tmp");
    fs::write(&tmp_path, lines.join("\n"))?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn generate_tracker_entry(
    song_id: u32,
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Option<String> {
    let data = collect_song_export_data(song_id, song_db, unlock_db, score_map)?;
    let song = song_db.get(&song_id)?;

    let mut columns = Vec::new();

    // Song ID
    columns.push(data.song_id.to_string());

    // Title
    columns.push(data.title.clone());

    // Type and Label (Label is same as Type)
    let type_name = match data.unlock_type {
        UnlockType::Base => "Base",
        UnlockType::Bits => "Bits",
        UnlockType::Sub => "Sub",
    };
    columns.push(type_name.to_string());
    columns.push(type_name.to_string()); // Label = Type

    // Bit costs (for N, H, A)
    for i in [1, 2, 3] {
        // SPN, SPH, SPA indices
        let cost = if data.unlock_type == UnlockType::Bits {
            let sp_level = song.levels[i] as i32;
            let dp_level = song.levels[i + 5] as i32; // DPN, DPH, DPA
            500 * (sp_level + dp_level)
        } else {
            0
        };
        columns.push(cost.to_string());
    }

    // Add SP/DP DJ Points
    columns.push(if data.sp_dj_points > 0.0 {
        format!("{}", data.sp_dj_points)
    } else {
        String::new()
    });
    columns.push(if data.dp_dj_points > 0.0 {
        format!("{}", data.dp_dj_points)
    } else {
        String::new()
    });

    // Add chart data columns (skip DPB, which doesn't exist in TSV format)
    for chart in &data.charts {
        if chart.difficulty == Difficulty::DpB {
            continue;
        }

        columns.push(if chart.unlocked { "TRUE" } else { "FALSE" }.to_string());
        columns.push(chart.level.to_string());
        columns.push(chart.lamp.short_name().to_string());
        columns.push(chart.grade.short_name().to_string());
        columns.push(chart.ex_score.to_string());
        columns.push(
            chart
                .miss_count
                .map(|m| m.to_string())
                .unwrap_or_else(|| "-".to_string()),
        );
        columns.push(chart.total_notes.to_string());
        columns.push(if chart.dj_points > 0.0 {
            format!("{}", chart.dj_points)
        } else {
            String::new()
        });
    }

    Some(columns.join("\t"))
}

/// Export song database to TSV for debugging
///
/// Format: id, title, title2 (English), artist, genre
/// Useful for checking encoding issues
pub fn export_song_list<P: AsRef<Path>>(path: P, song_db: &HashMap<u32, SongInfo>) -> Result<()> {
    let mut lines = vec!["id\ttitle\ttitle2\tartist\tgenre".to_string()];

    // Sort by song ID
    let mut song_ids: Vec<&u32> = song_db.keys().collect();
    song_ids.sort();

    for &song_id in song_ids {
        if let Some(song) = song_db.get(&song_id) {
            lines.push(format!(
                "{:05}\t{}\t{}\t{}\t{}",
                song_id, song.title, song.title_english, song.artist, song.genre
            ));
        }
    }

    let path = path.as_ref();
    let tmp_path = path.with_extension("txt.tmp");
    fs::write(&tmp_path, lines.join("\n"))?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Export detailed tracker data to JSON
pub fn export_tracker_json<P: AsRef<Path>>(
    path: P,
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Result<()> {
    let path = path.as_ref();
    let content = generate_tracker_json(song_db, unlock_db, score_map)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &content)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Generate tracker JSON string (for stdout output)
pub fn generate_tracker_json(
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Result<String> {
    let mut songs = Vec::new();

    // Get all song IDs from song database (sorted)
    let mut song_ids: Vec<&u32> = song_db.keys().collect();
    song_ids.sort();

    for &song_id in song_ids {
        if let Some(song_data) = generate_song_json(song_id, song_db, unlock_db, score_map) {
            songs.push(song_data);
        }
    }

    let export_data = ExportDataJson { songs };
    let json = serde_json::to_string_pretty(&export_data)?;
    Ok(json)
}

fn generate_song_json(
    song_id: u32,
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> Option<SongDataJson> {
    let data = collect_song_export_data(song_id, song_db, unlock_db, score_map)?;

    let charts = data
        .charts
        .iter()
        .filter(|c| c.difficulty != Difficulty::DpB && c.total_notes > 0)
        .map(|c| ChartDataJson {
            difficulty: c.difficulty.short_name().to_string(),
            level: c.level,
            lamp: c.lamp.expand_name().to_string(),
            grade: c.grade.short_name().to_string(),
            ex_score: c.ex_score,
            miss_count: c.miss_count,
            total_notes: c.total_notes,
            dj_points: c.dj_points,
        })
        .collect();

    Some(SongDataJson {
        song_id: data.song_id,
        title: data.title,
        artist: data.artist,
        charts,
    })
}

/// Generate tracker TSV string (for stdout output)
pub fn generate_tracker_tsv(
    song_db: &HashMap<u32, SongInfo>,
    unlock_db: &HashMap<u32, UnlockData>,
    score_map: &ScoreMap,
) -> String {
    let mut lines = vec![format_tracker_tsv_header()];

    // Get all song IDs from song database (sorted)
    let mut song_ids: Vec<&u32> = song_db.keys().collect();
    song_ids.sort();

    for &song_id in song_ids {
        if let Some(entry) = generate_tracker_entry(song_id, song_db, unlock_db, score_map) {
            lines.push(entry);
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::ScoreData;
    use std::sync::Arc;

    fn create_test_song(id: u32, title: &str) -> SongInfo {
        SongInfo {
            id,
            title: Arc::from(title),
            title_english: Arc::from(""),
            artist: Arc::from("Test Artist"),
            genre: Arc::from("Test Genre"),
            bpm: Arc::from("150"),
            folder: 1,
            levels: [0, 5, 8, 10, 12, 0, 5, 8, 10, 12],
            total_notes: [0, 500, 800, 1000, 1200, 0, 500, 800, 1000, 1200],
            embedded_ex_scores: [0u32; 10],
            embedded_lamps: [0u32; 10],
            unlock_type: UnlockType::Base,
        }
    }

    #[test]
    fn test_format_tracker_tsv_header() {
        let header = format_tracker_tsv_header();
        assert!(header.contains("Song ID"));
        assert!(header.contains("Title"));
        assert!(header.contains("Type"));
        assert!(header.contains("Label"));
        assert!(header.contains("SP DJ Points"));
        assert!(header.contains("DP DJ Points"));
        assert!(header.contains("SPA Lamp"));
        assert!(header.contains("DPA Lamp"));
    }

    #[test]
    fn test_generate_tracker_json_empty() {
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        let score_map = ScoreMap::new();

        let json = generate_tracker_json(&song_db, &unlock_db, &score_map).unwrap();

        // Check output contains expected structure
        assert!(json.contains("\"songs\""));
        assert!(json.contains("[]"));
    }

    #[test]
    fn test_generate_tracker_json_with_song() {
        let mut song_db: HashMap<u32, SongInfo> = HashMap::new();
        song_db.insert(1000, create_test_song(1000, "Test Song"));

        let mut unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        unlock_db.insert(
            1000,
            UnlockData {
                song_id: 1000,
                unlock_type: UnlockType::Base,
                unlocks: 0x3FF, // All 10 difficulties unlocked
            },
        );

        let score_map = ScoreMap::new();

        let json = generate_tracker_json(&song_db, &unlock_db, &score_map).unwrap();

        // Verify JSON structure contains expected data
        assert!(json.contains("\"song_id\": 1000"));
        assert!(json.contains("\"title\": \"Test Song\""));
    }

    #[test]
    fn test_collect_song_export_data() {
        let mut song_db: HashMap<u32, SongInfo> = HashMap::new();
        song_db.insert(1000, create_test_song(1000, "Test Song"));

        let mut unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        unlock_db.insert(
            1000,
            UnlockData {
                song_id: 1000,
                unlock_type: UnlockType::Base,
                unlocks: 0x3FF,
            },
        );

        // Set up scores for SPN (index 1) and DPN (index 6)
        let mut score_data = ScoreData::new(1000);
        score_data.set_lamp(Difficulty::SpN, Lamp::HardClear);
        score_data.set_score(Difficulty::SpN, 900);
        score_data.miss_count[Difficulty::SpN as usize] = Some(5);
        score_data.set_lamp(Difficulty::DpN, Lamp::Clear);
        score_data.set_score(Difficulty::DpN, 700);
        score_data.miss_count[Difficulty::DpN as usize] = Some(10);

        let mut score_map = ScoreMap::new();
        score_map.insert(1000, score_data);

        let data = collect_song_export_data(1000, &song_db, &unlock_db, &score_map).unwrap();

        // Basic song info
        assert_eq!(data.song_id, 1000);
        assert_eq!(data.title, "Test Song");
        assert_eq!(data.artist, "Test Artist");
        assert_eq!(data.unlock_type, UnlockType::Base);

        // All 10 difficulties present
        assert_eq!(data.charts.len(), 10);

        // Check SPN chart (index 1)
        let spn = &data.charts[Difficulty::SpN as usize];
        assert_eq!(spn.difficulty, Difficulty::SpN);
        assert_eq!(spn.level, 5);
        assert_eq!(spn.lamp, Lamp::HardClear);
        assert_eq!(spn.ex_score, 900);
        assert_eq!(spn.miss_count, Some(5));
        assert_eq!(spn.total_notes, 500);
        assert!(spn.dj_points > 0.0);

        // Check SPB chart (index 0) - total_notes == 0, should have NoPlay grade
        let spb = &data.charts[Difficulty::SpB as usize];
        assert_eq!(spb.difficulty, Difficulty::SpB);
        assert_eq!(spb.total_notes, 0);
        assert_eq!(spb.grade, Grade::NoPlay);
        assert_eq!(spb.dj_points, 0.0);

        // Check DPN chart (index 6)
        let dpn = &data.charts[Difficulty::DpN as usize];
        assert_eq!(dpn.difficulty, Difficulty::DpN);
        assert_eq!(dpn.lamp, Lamp::Clear);
        assert_eq!(dpn.ex_score, 700);
        assert_eq!(dpn.miss_count, Some(10));

        // SP DJ Points should be from the best SP chart
        assert!(data.sp_dj_points > 0.0);
        // DP DJ Points should be from the best DP chart
        assert!(data.dp_dj_points > 0.0);
    }

    #[test]
    fn test_collect_song_export_data_no_scores() {
        let mut song_db: HashMap<u32, SongInfo> = HashMap::new();
        let mut song = create_test_song(1000, "No Score Song");
        song.unlock_type = UnlockType::Bits;
        song_db.insert(1000, song);

        let mut unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        unlock_db.insert(
            1000,
            UnlockData {
                song_id: 1000,
                unlock_type: UnlockType::Bits,
                unlocks: 0x3FF,
            },
        );

        let score_map = ScoreMap::new();

        let data = collect_song_export_data(1000, &song_db, &unlock_db, &score_map).unwrap();

        assert_eq!(data.unlock_type, UnlockType::Bits);
        assert_eq!(data.sp_dj_points, 0.0);
        assert_eq!(data.dp_dj_points, 0.0);

        // All charts should have NoPlay defaults
        for chart in &data.charts {
            assert_eq!(chart.lamp, Lamp::NoPlay);
            assert_eq!(chart.grade, Grade::NoPlay);
            assert_eq!(chart.ex_score, 0);
            assert_eq!(chart.miss_count, None);
        }
    }

    #[test]
    fn test_collect_song_export_data_missing_from_db() {
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        let score_map = ScoreMap::new();

        let result = collect_song_export_data(9999, &song_db, &unlock_db, &score_map);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_tracker_tsv_header_only_when_empty() {
        let song_db: HashMap<u32, SongInfo> = HashMap::new();
        let unlock_db: HashMap<u32, UnlockData> = HashMap::new();
        let score_map = ScoreMap::new();

        let tsv = generate_tracker_tsv(&song_db, &unlock_db, &score_map);
        let lines: Vec<&str> = tsv.lines().collect();

        // Should only have header
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("Title"));
    }
}
