//! Score storage and session management.
//!
//! This module handles persistent storage of play data:
//!
//! - **Score map**: In-memory cache of scores read from game
//! - **Session management**: TSV/JSON session file output
//! - **Export formats**: TSV and JSON formatting for tracker data
//!
//! ## Session Files
//!
//! Sessions are stored in timestamped files under `sessions/`:
//! - TSV format for human-readable logs
//! - JSON format for programmatic access

mod format;
mod score_map;
mod session;

pub use format::{
    ChartDataJson, ExportDataJson, JudgeJson, PlayDataJson, SongDataJson, TsvRowData,
    export_song_list, export_tracker_json, export_tracker_tsv, format_full_tsv_header,
    format_full_tsv_row, format_json_entry, format_play_data_console, format_play_summary,
    format_tracker_tsv_header, format_tsv_header, format_tsv_row, generate_tracker_json,
    generate_tracker_tsv,
};
pub use score_map::*;
pub use session::*;
