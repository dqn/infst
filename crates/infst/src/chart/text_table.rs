//! Song titles, levels and score-map IDs used by synchronization.

use crate::process::{ReadMemory, decode_shift_jis_to_string};
use anyhow::{Context, Result};

const MAX_ENTRIES: usize = 5000;
const MAX_CONSECUTIVE_EMPTY: usize = 10;

#[derive(Clone, Copy)]
struct Layout {
    stride: usize,
    title: usize,
    title_size: usize,
    levels: usize,
    game_id: usize,
    utf16: bool,
}

impl Layout {
    const KNOWN: [Self; 2] = [
        Self {
            stride: 0x630,
            title: 0x5b0,
            title_size: 64,
            levels: 0x160,
            game_id: 0x430,
            utf16: false,
        },
        Self {
            stride: 0x730,
            title: 0x630,
            title_size: 128,
            levels: 0x1e0,
            game_id: 0x4b0,
            utf16: true,
        },
    ];

    fn title(self, bytes: &[u8]) -> String {
        let title = if self.utf16 {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .take_while(|&unit| unit != 0)
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            decode_shift_jis_to_string(bytes)
        };
        title.trim().to_string()
    }

    fn matches(self, buffer: &[u8]) -> bool {
        (1..=3).all(|i| {
            let base = i * self.stride;
            let title_start = (i - 1) * self.stride + self.title;
            let title = self.title(&buffer[title_start..title_start + self.title_size]);
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
    let sample = reader.read_bytes(song_list_addr, 4 * 0x730)?;
    let layout = Layout::KNOWN
        .into_iter()
        .find(|layout| layout.matches(&sample))
        .context("Unsupported song text table layout")?;
    let total_size = layout.stride * MAX_ENTRIES;
    let buffer = reader
        .read_bytes(song_list_addr, total_size)
        .context("Failed to read text table from memory")?;

    let mut entries = Vec::new();
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
            layout.title(&buffer[title_start..title_start + layout.title_size])
        } else {
            String::new()
        };

        if title.is_empty() {
            consecutive_empty += 1;
            if consecutive_empty >= MAX_CONSECUTIVE_EMPTY {
                break;
            }
            continue;
        }
        consecutive_empty = 0;

        // Read difficulty levels.
        let mut levels = [0u8; 10];
        levels.copy_from_slice(&entry_buf[layout.levels..layout.levels + 10]);

        // Read the score-map ID (not the internal ID associated with the title).
        let game_id = u32::from_le_bytes(
            entry_buf[layout.game_id..layout.game_id + 4]
                .try_into()
                .unwrap(),
        );

        // Skip entries with invalid game_id (empty slots in the table)
        if game_id == 0 || !(1000..=90000).contains(&game_id) {
            continue;
        }

        // Apply encoding fixes (same as SongInfo::parse_entry for V3 title)
        let title = super::fix_title_encoding(&title)
            .map(|arc| arc.to_string())
            .unwrap_or(title);

        // Skip if title is empty after decoding (V3 title field may be empty for some entries)
        if title.is_empty() {
            continue;
        }

        entries.push(TextTableEntry {
            title,
            levels,
            game_id,
        });
    }

    Ok(entries)
}
