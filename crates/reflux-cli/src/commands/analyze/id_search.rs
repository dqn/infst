//! Consecutive song ID search.

use std::collections::HashMap;

use reflux_core::{MemoryReader, ReadMemory};

use super::song_counter::count_songs_with_size;

/// Search for consecutive song IDs to find potential song lists.
pub fn search_consecutive_song_ids(reader: &MemoryReader, base: u64, module_size: u64) {
    let search_start = base + 0x1000000;
    let search_end = base + module_size.min(0x5000000);
    let chunk_size: usize = 4 * 1024 * 1024;

    let pattern_1001 = [0xE9u8, 0x03, 0x00, 0x00];
    let pattern_1002 = [0xEAu8, 0x03, 0x00, 0x00];

    let mut offset = 0u64;
    let mut found_pairs: Vec<(u64, u64, u64)> = Vec::new(); // (addr_1001, addr_1002, delta)

    while search_start + offset < search_end {
        let addr = search_start + offset;
        let read_size = chunk_size.min((search_end - addr) as usize);

        if let Ok(buffer) = reader.read_bytes(addr, read_size) {
            // Find all 1001 patterns
            let mut addr_1001s: Vec<u64> = Vec::new();
            let mut addr_1002s: Vec<u64> = Vec::new();

            for (i, window) in buffer.windows(4).enumerate() {
                if window == pattern_1001 {
                    addr_1001s.push(addr + i as u64);
                } else if window == pattern_1002 {
                    addr_1002s.push(addr + i as u64);
                }
            }

            // Find pairs
            for &a1001 in &addr_1001s {
                for &a1002 in &addr_1002s {
                    if a1002 > a1001 {
                        let delta = a1002 - a1001;
                        // Look for reasonable entry sizes
                        if (32..=2048).contains(&delta) && delta % 4 == 0 {
                            found_pairs.push((a1001, a1002, delta));
                        }
                    }
                }
            }
        }

        offset += chunk_size as u64;
    }

    // Group by delta to find likely entry sizes
    let mut delta_counts: HashMap<u64, Vec<u64>> = HashMap::new();
    for (addr_1001, _, delta) in &found_pairs {
        delta_counts.entry(*delta).or_default().push(*addr_1001);
    }

    // Sort by count
    let mut sorted: Vec<_> = delta_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    println!("    Top entry size candidates:");
    for (delta, addresses) in sorted.iter().take(5) {
        println!(
            "      Delta={} bytes: {} occurrences",
            delta,
            addresses.len()
        );
        if let Some(&first_addr) = addresses.first()
            && addresses.len() >= 10
        {
            // Try to count songs with this structure size
            let count = count_songs_with_size(reader, first_addr, *delta);
            println!(
                "        -> Starting at 0x{:X}: {} consecutive songs",
                first_addr, count
            );
        }
    }
}
