//! Personal best comparison logic

use crate::play::PlayData;
use crate::score::{Grade, Lamp, ScoreData};

/// Personal best comparison result
#[derive(Debug, Clone, Default)]
pub struct PersonalBestComparison {
    /// Score difference (positive = improvement)
    pub score_diff: Option<i32>,
    /// Previous grade if improved
    pub previous_grade: Option<Grade>,
    /// Previous lamp if improved
    pub previous_lamp: Option<Lamp>,
    /// Miss count difference (negative = improvement)
    pub miss_count_diff: Option<i32>,
}

/// Compare current play data with personal best
pub fn compare_with_personal_best(
    play_data: &PlayData,
    best: Option<&ScoreData>,
) -> PersonalBestComparison {
    let Some(best) = best else {
        return PersonalBestComparison::default();
    };

    let diff_index = play_data.chart.difficulty as usize;
    let best_score = best.score[diff_index];
    let best_lamp = best.lamp[diff_index];

    let mut comparison = PersonalBestComparison::default();

    // Score comparison: only show diff if best score exists and current is higher
    if best_score > 0 && play_data.ex_score > best_score {
        comparison.score_diff = Some(play_data.ex_score as i32 - best_score as i32);
    }

    // Grade comparison: calculate grade from best score and compare
    if best_score > 0 && play_data.chart.total_notes > 0 {
        let best_grade = PlayData::calculate_grade(best_score, play_data.chart.total_notes);
        if play_data.grade > best_grade {
            comparison.previous_grade = Some(best_grade);
        }
    }

    // Lamp comparison: direct comparison (Lamp implements Ord)
    if best_lamp != Lamp::NoPlay && play_data.lamp > best_lamp {
        comparison.previous_lamp = Some(best_lamp);
    }

    // Miss count comparison: only show when improved (decreased)
    if play_data.miss_count_valid() {
        let best_miss = best.miss_count[diff_index];
        if let Some(best_miss) = best_miss {
            let diff = play_data.miss_count() as i32 - best_miss as i32;
            if diff < 0 {
                comparison.miss_count_diff = Some(diff);
            }
        }
    }

    comparison
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::chart::{ChartInfo, Difficulty};
    use crate::play::{PlayType, Settings};
    use crate::score::Judge;

    fn create_test_play_data(ex_score: u32, grade: Grade, lamp: Lamp) -> PlayData {
        PlayData {
            chart: ChartInfo {
                song_id: 1000,
                title: Arc::from("Test Song"),
                title_english: Arc::from(""),
                artist: Arc::from(""),
                genre: Arc::from(""),
                bpm: Arc::from("150"),
                difficulty: Difficulty::SpA,
                level: 12,
                total_notes: 1000, // max EX = 2000
                unlocked: true,
            },
            judge: Judge {
                play_type: PlayType::P1,
                pgreat: 900,
                great: 100,
                good: 0,
                bad: 0,
                poor: 0,
                fast: 30,
                slow: 20,
                combo_break: 0,
                premature_end: false,
            },
            settings: Settings::default(),
            ex_score,
            lamp,
            grade,
            data_available: true,
            timestamp: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_compare_with_personal_best_no_best() {
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);
        let comparison = compare_with_personal_best(&play_data, None);

        assert!(comparison.score_diff.is_none());
        assert!(comparison.previous_grade.is_none());
        assert!(comparison.previous_lamp.is_none());
    }

    #[test]
    fn test_compare_with_personal_best_score_improvement() {
        // Current: AAA (1800), Best: AAA (1780) - same grade, score up
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1780; // Also AAA
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear;

        let comparison = compare_with_personal_best(&play_data, Some(&best));

        assert_eq!(comparison.score_diff, Some(20));
        assert!(comparison.previous_grade.is_none()); // Both AAA
        assert!(comparison.previous_lamp.is_none()); // Same lamp
    }

    #[test]
    fn test_compare_with_personal_best_grade_improvement() {
        // Current: AAA (1778+), Best: AA (1556-1777)
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::Clear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1600; // AA
        best.lamp[Difficulty::SpA as usize] = Lamp::Clear;

        let comparison = compare_with_personal_best(&play_data, Some(&best));

        assert_eq!(comparison.score_diff, Some(200));
        assert_eq!(comparison.previous_grade, Some(Grade::Aa));
        assert!(comparison.previous_lamp.is_none());
    }

    #[test]
    fn test_compare_with_personal_best_lamp_improvement() {
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800; // Same score
        best.lamp[Difficulty::SpA as usize] = Lamp::Clear;

        let comparison = compare_with_personal_best(&play_data, Some(&best));

        assert!(comparison.score_diff.is_none()); // Same score
        assert!(comparison.previous_grade.is_none()); // Both AAA
        assert_eq!(comparison.previous_lamp, Some(Lamp::Clear));
    }

    #[test]
    fn test_compare_with_personal_best_no_improvement() {
        let play_data = create_test_play_data(1600, Grade::Aa, Lamp::Clear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800; // Better
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear; // Better

        let comparison = compare_with_personal_best(&play_data, Some(&best));

        assert!(comparison.score_diff.is_none());
        assert!(comparison.previous_grade.is_none());
        assert!(comparison.previous_lamp.is_none());
    }

    #[test]
    fn test_compare_with_personal_best_first_clear() {
        // First clear: best lamp is NoPlay
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 0;
        best.lamp[Difficulty::SpA as usize] = Lamp::NoPlay;

        let comparison = compare_with_personal_best(&play_data, Some(&best));

        // NoPlay to something is not shown as lamp improvement
        assert!(comparison.score_diff.is_none()); // No previous score
        assert!(comparison.previous_grade.is_none()); // No previous grade
        assert!(comparison.previous_lamp.is_none()); // NoPlay is not shown
    }

    #[test]
    fn test_compare_miss_count_improved() {
        // Current: 5 misses, Best: 10 misses → improvement (-5)
        let mut play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);
        play_data.judge.bad = 2;
        play_data.judge.poor = 3;

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800;
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear;
        best.miss_count[Difficulty::SpA as usize] = Some(10);

        let comparison = compare_with_personal_best(&play_data, Some(&best));
        assert_eq!(comparison.miss_count_diff, Some(-5));
    }

    #[test]
    fn test_compare_miss_count_not_improved() {
        // Current: 10 misses, Best: 5 misses → no improvement shown
        let mut play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);
        play_data.judge.bad = 5;
        play_data.judge.poor = 5;

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800;
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear;
        best.miss_count[Difficulty::SpA as usize] = Some(5);

        let comparison = compare_with_personal_best(&play_data, Some(&best));
        assert!(comparison.miss_count_diff.is_none());
    }

    #[test]
    fn test_compare_miss_count_no_best_data() {
        // Best miss count is None → no comparison
        let play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800;
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear;
        // miss_count defaults to None

        let comparison = compare_with_personal_best(&play_data, Some(&best));
        assert!(comparison.miss_count_diff.is_none());
    }

    #[test]
    fn test_compare_miss_count_invalid_play() {
        // data_available is false → miss count not valid
        let mut play_data = create_test_play_data(1800, Grade::Aaa, Lamp::HardClear);
        play_data.data_available = false;

        let mut best = ScoreData::new(1000);
        best.score[Difficulty::SpA as usize] = 1800;
        best.lamp[Difficulty::SpA as usize] = Lamp::HardClear;
        best.miss_count[Difficulty::SpA as usize] = Some(10);

        let comparison = compare_with_personal_best(&play_data, Some(&best));
        assert!(comparison.miss_count_diff.is_none());
    }
}
