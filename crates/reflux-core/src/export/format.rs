//! ExportFormat trait definition

use crate::play::PlayData;

/// Trait for export format implementations
///
/// Provides a common interface for different export formats (TSV, JSON, etc.)
pub trait ExportFormat {
    /// Returns the header line for the format (empty for formats without headers)
    fn header(&self) -> Option<String>;

    /// Format a single row of play data
    fn format_row(&self, play_data: &PlayData) -> String;

    /// Format multiple rows of play data
    fn format_rows(&self, play_data: &[PlayData]) -> String {
        let mut output = String::new();
        if let Some(header) = self.header() {
            output.push_str(&header);
            output.push('\n');
        }
        for data in play_data {
            output.push_str(&self.format_row(data));
            output.push('\n');
        }
        output
    }
}
