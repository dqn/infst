//! Stream output for OBS integration.
//!
//! This module provides file-based output for streaming overlays:
//!
//! - Current song info (title, artist, level)
//! - Play state (menu, play, off)
//! - Latest result (grade, lamp, EX score)
//!
//! Files are written to a configurable directory and can be read
//! by OBS text sources for live display.
//!
//! **Note**: This feature is not yet fully integrated into the main
//! tracking loop. The `stream_output` field in `Reflux` is reserved
//! for future OBS integration.

mod output;

pub use output::*;
