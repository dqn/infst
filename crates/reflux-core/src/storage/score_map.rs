use crate::game::{Difficulty, Lamp};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ScoreData {
    pub song_id: String,
    pub lamp: [Lamp; 9],
    pub score: [u32; 9],
    pub miss_count: [Option<u32>; 9],
}

impl ScoreData {
    pub fn new(song_id: String) -> Self {
        Self {
            song_id,
            ..Default::default()
        }
    }

    pub fn get_lamp(&self, difficulty: Difficulty) -> Lamp {
        self.lamp[difficulty as usize]
    }

    pub fn get_score(&self, difficulty: Difficulty) -> u32 {
        self.score[difficulty as usize]
    }

    pub fn set_lamp(&mut self, difficulty: Difficulty, lamp: Lamp) {
        self.lamp[difficulty as usize] = lamp;
    }

    pub fn set_score(&mut self, difficulty: Difficulty, score: u32) {
        self.score[difficulty as usize] = score;
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScoreMap {
    scores: HashMap<String, ScoreData>,
}

impl ScoreMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, song_id: &str) -> Option<&ScoreData> {
        self.scores.get(song_id)
    }

    pub fn get_mut(&mut self, song_id: &str) -> Option<&mut ScoreData> {
        self.scores.get_mut(song_id)
    }

    pub fn insert(&mut self, song_id: String, data: ScoreData) {
        self.scores.insert(song_id, data);
    }

    pub fn get_or_insert(&mut self, song_id: String) -> &mut ScoreData {
        self.scores
            .entry(song_id.clone())
            .or_insert_with(|| ScoreData::new(song_id))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ScoreData)> {
        self.scores.iter()
    }

    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }
}
