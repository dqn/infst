use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OffsetsCollection {
    pub version: String,
    pub song_list: u64,
    pub data_map: u64,
    pub judge_data: u64,
    pub play_data: u64,
    pub play_settings: u64,
    pub unlock_data: u64,
    pub current_song: u64,
}

impl OffsetsCollection {
    pub fn is_valid(&self) -> bool {
        !self.version.is_empty()
            && self.song_list != 0
            && self.judge_data != 0
            && self.play_data != 0
    }
}
