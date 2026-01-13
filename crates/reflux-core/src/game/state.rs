use crate::game::GameState;

/// Game state detector
pub struct GameStateDetector {
    last_state: GameState,
}

impl GameStateDetector {
    pub fn new() -> Self {
        Self {
            last_state: GameState::Unknown,
        }
    }

    /// Determine game state from memory values
    ///
    /// Based on the original C# implementation:
    /// - Check marker at JudgeData + word * 54
    /// - If marker != 0, check next position to confirm playing
    /// - Check PlaySettings - word * 6 for song select marker
    pub fn detect(
        &mut self,
        judge_marker_54: i32,
        judge_marker_55: i32,
        song_select_marker: i32,
    ) -> GameState {
        // Check if playing
        if judge_marker_54 != 0 && judge_marker_55 != 0 {
            self.last_state = GameState::Playing;
            return GameState::Playing;
        }

        // Cannot go from song select to result screen directly
        if self.last_state == GameState::SongSelect {
            return GameState::SongSelect;
        }

        // Check if in song select
        if song_select_marker == 1 {
            self.last_state = GameState::SongSelect;
            return GameState::SongSelect;
        }

        // Otherwise must be result screen
        self.last_state = GameState::ResultScreen;
        GameState::ResultScreen
    }

    /// Reset state (e.g., when reconnecting to process)
    pub fn reset(&mut self) {
        self.last_state = GameState::Unknown;
    }

    pub fn last_state(&self) -> GameState {
        self.last_state
    }
}

impl Default for GameStateDetector {
    fn default() -> Self {
        Self::new()
    }
}
