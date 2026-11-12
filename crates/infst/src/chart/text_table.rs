//! Song titles, levels and score-map IDs used by synchronization.

use super::{EntryLayout, TextEncoding};
use crate::process::ReadMemory;
use anyhow::{Context, Result, ensure};

const MAX_ENTRIES: usize = 5000;
const MAX_CONSECUTIVE_EMPTY: usize = 10;

#[derive(Clone, Copy)]
struct Layout {
    stride: usize,
    title: usize,
    title_size: usize,
    levels: usize,
    game_id: usize,
    encoding: TextEncoding,
}

impl Layout {
    fn from_entry(entry: EntryLayout) -> Self {
        let anchor = entry
            .title_english
            .expect("supported profiles have romanized titles");
        Self {
            stride: entry.entry_size,
            title: entry.entry_size - anchor + entry.title,
            title_size: entry.text_encoding.field_size(),
            levels: entry.levels - anchor,
            game_id: entry.entry_size - anchor + entry.song_id,
            encoding: entry.text_encoding,
        }
    }

    fn title(self, bytes: &[u8]) -> Result<String> {
        Ok(self.encoding.decode(bytes)?)
    }

    fn matches(self, buffer: &[u8]) -> bool {
        (1..=3).all(|i| {
            let base = i * self.stride;
            let title_start = (i - 1) * self.stride + self.title;
            let Ok(title) = self.title(&buffer[title_start..title_start + self.title_size]) else {
                return false;
            };
            let levels = &buffer[base + self.levels..base + self.levels + 10];
            let id = u32::from_le_bytes(
                buffer[base + self.game_id..base + self.game_id + 4]
                    .try_into()
                    .unwrap(),
            );
            !title.is_empty()
                && !title.chars().any(char::is_control)
                && levels.iter().all(|&level| level <= 12)
                && levels.iter().any(|&level| level > 0)
                && (1000..=90000).contains(&id)
        })
    }
}

/// Data extracted from a single text table entry.
pub struct TextTableEntry {
    pub title: String,
    pub levels: [u8; 10],
    pub game_id: u32,
}

/// Read sync metadata from a detected "5.1.1." text table.
///
/// Supports the 0x630-byte Shift-JIS and 0x730-byte UTF-16 layouts.
/// Titles belong to the next block's game ID; levels belong to the current block.
/// Rejects unknown or uninitialized layouts before exposing data for upload.
pub fn read_text_table<R: ReadMemory>(
    reader: &R,
    song_list_addr: u64,
) -> Result<Vec<TextTableEntry>> {
    // Bulk read the entire table to minimize process memory reads.
    // Detect the layout from three title/ID/level tuples before reading the table.
    let sample = reader.read_bytes(
        song_list_addr,
        4 * EntryLayout::supported()
            .iter()
            .map(|layout| layout.entry_size)
            .max()
            .unwrap(),
    )?;
    let layout = EntryLayout::supported()
        .into_iter()
        .map(Layout::from_entry)
        .find(|layout| layout.matches(&sample))
        .context("Unsupported song text table layout")?;
    let total_size = layout.stride * MAX_ENTRIES;
    let buffer = reader
        .read_bytes(song_list_addr, total_size)
        .context("Failed to read text table from memory")?;

    let mut entries = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut consecutive_empty = 0;

    // Start from entry 1: entry 0 has no previous entry for V3 title lookup
    // The first block has no preceding title available in this buffer.
    for i in 1..MAX_ENTRIES {
        let base = i * layout.stride;
        if base + layout.stride > buffer.len() {
            break;
        }

        let entry_buf = &buffer[base..base + layout.stride];

        // The title in the previous block matches this block's score-map ID.
        let prev_base = (i - 1) * layout.stride;
        let title_start = prev_base + layout.title;
        let title = if title_start + layout.title_size <= buffer.len() {
            layout.title(&buffer[title_start..title_start + layout.title_size])?
        } else {
            String::new()
        };

        let mut levels = [0u8; 10];
        levels.copy_from_slice(&entry_buf[layout.levels..layout.levels + 10]);
        let game_id = u32::from_le_bytes(
            entry_buf[layout.game_id..layout.game_id + 4]
                .try_into()
                .unwrap(),
        );

        if title.is_empty() {
            ensure!(
                game_id == 0 && levels.iter().all(|&level| level == 0),
                "Unsupported song text table: entry {i} has metadata without a title"
            );
            consecutive_empty += 1;
            if consecutive_empty >= MAX_CONSECUTIVE_EMPTY {
                break;
            }
            continue;
        }
        consecutive_empty = 0;

        ensure!(
            levels.iter().all(|&level| level <= 12),
            "Unsupported song text table: entry {i} has invalid difficulty levels"
        );
        ensure!(
            (1000..=90000).contains(&game_id),
            "Unsupported song text table: entry {i} has invalid score-map ID {game_id}"
        );
        ensure!(
            seen_ids.insert(game_id),
            "Unsupported song text table: entry {i} repeats score-map ID {game_id}"
        );

        // Apply encoding fixes (same as SongInfo::parse_entry for V3 title)
        let title = super::fix_title_encoding(&title)
            .map(|arc| arc.to_string())
            .unwrap_or(title);

        entries.push(TextTableEntry {
            title,
            levels,
            game_id,
        });
    }

    Ok(entries)
}
