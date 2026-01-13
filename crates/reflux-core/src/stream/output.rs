use crate::error::Result;
use crate::game::GameState;
use std::fs;
use std::path::Path;

pub struct StreamOutput {
    enabled: bool,
    base_dir: String,
}

impl StreamOutput {
    pub fn new(enabled: bool, base_dir: String) -> Self {
        Self { enabled, base_dir }
    }

    pub fn write_play_state(&self, state: GameState) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let state_str = match state {
            GameState::SongSelect => "SELECT",
            GameState::Playing => "PLAYING",
            GameState::ResultScreen => "RESULT",
            GameState::Unknown => "UNKNOWN",
        };

        self.write_file("playstate.txt", state_str)
    }

    pub fn write_current_song(&self, title: &str, difficulty: &str, level: u8) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let content = format!("{} [{}{}]", title, difficulty, level);
        self.write_file("currentsong.txt", &content)
    }

    pub fn write_latest_result(
        &self,
        title: &str,
        difficulty: &str,
        level: u8,
        grade: &str,
        lamp: &str,
        ex_score: u32,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let content = format!(
            "{} [{}{}] {} {} {}",
            title, difficulty, level, grade, lamp, ex_score
        );
        self.write_file("latest.txt", &content)?;
        self.write_file("latest-grade.txt", grade)?;
        self.write_file("latest-lamp.txt", lamp)?;

        Ok(())
    }

    pub fn write_marquee(&self, text: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.write_file("marquee.txt", text)
    }

    fn write_file(&self, filename: &str, content: &str) -> Result<()> {
        let path = Path::new(&self.base_dir).join(filename);
        fs::write(path, content)?;
        Ok(())
    }
}
