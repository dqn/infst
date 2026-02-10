//! Song counting functions for memory analysis.

use infst::{MemoryReader, ReadMemory, SongInfo};

/// Count songs using old structure (0x3F0 bytes per entry).
pub fn count_songs_old_structure(reader: &MemoryReader, start: u64) -> usize {
    let mut count = 0;
    let mut addr = start;
    while count < 5000 {
        match SongInfo::read_from_memory(reader, addr) {
            Ok(Some(song)) if !song.title.is_empty() => {
                count += 1;
            }
            _ => break,
        }
        addr += SongInfo::MEMORY_SIZE as u64;
    }
    count
}

/// Count songs using new structure (312 bytes per entry).
pub fn count_songs_new_structure(reader: &MemoryReader, start: u64) -> usize {
    const NEW_SIZE: u64 = 312;
    let mut count = 0;
    let mut addr = start;
    while count < 5000 {
        let song_id = match reader.read_i32(addr) {
            Ok(id) => id,
            Err(_) => break,
        };
        if !(1000..=50000).contains(&song_id) {
            break;
        }
        count += 1;
        addr += NEW_SIZE;
    }
    count
}

/// Count songs with a specified entry size.
pub fn count_songs_with_size(reader: &MemoryReader, start: u64, entry_size: u64) -> usize {
    let mut count = 0;
    let mut addr = start;
    let mut prev_id = 0i32;

    while count < 5000 {
        match reader.read_i32(addr) {
            Ok(id) => {
                if !(1000..=50000).contains(&id) {
                    break;
                }
                // Allow some gaps/out-of-order but not too much
                if count > 0 && (id < prev_id - 500 || id > prev_id + 500) {
                    break;
                }
                prev_id = id;
                count += 1;
                addr += entry_size;
            }
            Err(_) => break,
        }
    }

    count
}
