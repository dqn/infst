//! Prelude module for convenient imports
//!
//! This module re-exports the most commonly used types and traits from infst.
//!
//! # Usage
//!
//! ```ignore
//! use infst::prelude::*;
//! ```
//!
//! This brings the following into scope:
//!
//! - Core types: `Infst`, `InfstConfig`, `GameData`
//! - Play data: `PlayData`, `Judge`, `Settings`, `PlayType`
//! - Chart data: `ChartInfo`, `SongInfo`, `Difficulty`
//! - Score data: `Grade`, `Lamp`, `ScoreData`, `ScoreMap`
//! - Error handling: `Error`, `Result`

// Core application types
pub use crate::infst::{GameData, Infst, InfstConfig, InfstConfigBuilder};

// Error handling
pub use crate::error::{Error, Result};

// Play data types
pub use crate::play::{PlayData, PlayType, Settings};

// Score types
pub use crate::score::{Grade, Judge, Lamp, ScoreData, ScoreMap};

// Chart types
pub use crate::chart::{ChartInfo, Difficulty, SongInfo};

// Export format trait
pub use crate::export::ExportFormat;
