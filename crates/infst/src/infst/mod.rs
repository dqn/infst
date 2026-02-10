//! Main application logic for the INFST score tracker.
//!
//! This module contains the core `Infst` struct which orchestrates:
//! - Game state detection and tracking
//! - Score data collection from memory
//! - Session management and data export
//! - Integration with game memory via offsets
//!
//! ## Example
//!
//! ```ignore
//! use infst::infst::{Infst, InfstConfig};
//! use infst::offset::OffsetsCollection;
//!
//! // Create with default configuration
//! let offsets = OffsetsCollection::default();
//! let mut infst = Infst::new(offsets);
//!
//! // Or create with custom configuration
//! let config = InfstConfig::builder()
//!     .session_dir("my_sessions")
//!     .build();
//! let mut infst = Infst::with_config(offsets, config);
//!
//! // Set up song database and score map
//! infst.set_song_db(song_db);
//! infst.set_score_map(score_map);
//!
//! // Run the tracking loop
//! infst.run(&process, &running)?;
//! ```

mod game_loop;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::{debug, info};

use crate::chart::{Difficulty, SongInfo, UnlockData};
use crate::error::Result;
use crate::offset::OffsetsCollection;
use crate::play::GameStateDetector;
use crate::score::ScoreMap;
use crate::session::SessionManager;

/// Configuration for the Infst application
#[derive(Debug, Clone)]
pub struct InfstConfig {
    /// Directory for session files
    pub session_dir: PathBuf,
    /// Whether to automatically export tracker data on song select
    pub auto_export: bool,
    /// Path for auto-exported tracker file
    pub tracker_path: PathBuf,
}

impl Default for InfstConfig {
    fn default() -> Self {
        Self {
            session_dir: PathBuf::from("sessions"),
            auto_export: true,
            tracker_path: PathBuf::from("tracker.tsv"),
        }
    }
}

impl InfstConfig {
    /// Create a new configuration builder
    pub fn builder() -> InfstConfigBuilder {
        InfstConfigBuilder::default()
    }
}

/// Builder for InfstConfig
#[derive(Debug, Clone, Default)]
pub struct InfstConfigBuilder {
    session_dir: Option<PathBuf>,
    auto_export: Option<bool>,
    tracker_path: Option<PathBuf>,
}

impl InfstConfigBuilder {
    /// Set the session directory
    pub fn session_dir<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.session_dir = Some(path.into());
        self
    }

    /// Enable or disable auto-export on song select
    pub fn auto_export(mut self, enabled: bool) -> Self {
        self.auto_export = Some(enabled);
        self
    }

    /// Set the tracker file path for auto-export
    pub fn tracker_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.tracker_path = Some(path.into());
        self
    }

    /// Build the configuration
    pub fn build(self) -> InfstConfig {
        let default = InfstConfig::default();
        InfstConfig {
            session_dir: self.session_dir.unwrap_or(default.session_dir),
            auto_export: self.auto_export.unwrap_or(default.auto_export),
            tracker_path: self.tracker_path.unwrap_or(default.tracker_path),
        }
    }
}

/// Game data loaded from memory and files
pub struct GameData {
    /// Song database loaded from game memory
    pub song_db: HashMap<u32, SongInfo>,
    /// Score map from game memory
    pub score_map: ScoreMap,
    /// Current unlock state from memory
    pub unlock_state: HashMap<u32, UnlockData>,
}

impl GameData {
    fn new() -> Self {
        Self {
            song_db: HashMap::new(),
            score_map: ScoreMap::new(),
            unlock_state: HashMap::new(),
        }
    }
}

/// Main application
pub struct Infst {
    pub(crate) offsets: OffsetsCollection,
    /// Application configuration
    pub(crate) config: InfstConfig,
    /// Game data from memory
    pub(crate) game_data: GameData,
    pub(crate) state_detector: GameStateDetector,
    pub(crate) session_manager: SessionManager,
    /// Currently playing chart (set during Playing state)
    /// Used for cross-validation when fetching play data on ResultScreen
    pub(crate) current_playing: Option<(u32, Difficulty)>,
}

impl Infst {
    /// Create a new Infst instance with default configuration
    pub fn new(offsets: OffsetsCollection) -> Self {
        Self::with_config(offsets, InfstConfig::default())
    }

    /// Create a new Infst instance with custom configuration
    pub fn with_config(offsets: OffsetsCollection, config: InfstConfig) -> Self {
        // Log offset validation status
        if offsets.has_state_detection_offsets() {
            debug!(
                "State detection offsets: judge_data=0x{:X}, play_settings=0x{:X}",
                offsets.judge_data, offsets.play_settings
            );
        } else {
            info!(
                "State detection offsets not fully initialized: judge_data=0x{:X}, play_settings=0x{:X}",
                offsets.judge_data, offsets.play_settings
            );
        }

        let session_dir = config.session_dir.to_string_lossy().to_string();

        Self {
            offsets,
            config,
            game_data: GameData::new(),
            state_detector: GameStateDetector::new(),
            session_manager: SessionManager::new(&session_dir),
            current_playing: None,
        }
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &InfstConfig {
        &self.config
    }

    /// Set score map
    pub fn set_score_map(&mut self, score_map: ScoreMap) {
        self.game_data.score_map = score_map;
    }

    /// Set song database
    pub fn set_song_db(&mut self, song_db: HashMap<u32, SongInfo>) {
        self.game_data.song_db = song_db;
    }

    /// Get a reference to the offsets
    pub fn offsets(&self) -> &OffsetsCollection {
        &self.offsets
    }

    /// Get the offsets version
    pub fn offsets_version(&self) -> &str {
        &self.offsets.version
    }

    /// Update offsets while preserving tracker and game data
    ///
    /// This method updates the offsets without creating a new Infst instance,
    /// preserving the loaded tracker data and game state.
    pub fn update_offsets(&mut self, offsets: OffsetsCollection) {
        if offsets.has_state_detection_offsets() {
            debug!(
                "Updated state detection offsets: judge_data=0x{:X}, play_settings=0x{:X}",
                offsets.judge_data, offsets.play_settings
            );
        }
        self.offsets = offsets;
    }

    /// Export tracker data to TSV file
    pub fn export_tracker_tsv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        crate::export::export_tracker_tsv(
            path,
            &self.game_data.song_db,
            &self.game_data.unlock_state,
            &self.game_data.score_map,
        )
    }
}
