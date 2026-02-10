//! Pattern-based song data search.

use infst::{MemoryReader, ReadMemory};

use super::id_search::search_consecutive_song_ids;

/// Search for various song data patterns in memory.
pub fn search_song_patterns(reader: &MemoryReader, base: u64, module_size: u64) {
    // Search for song_id=1001 followed by folder=43 pattern
    let pattern_1001_43: [u8; 8] = [0xE9, 0x03, 0x00, 0x00, 0x2B, 0x00, 0x00, 0x00];

    println!("  Searching for song_id=1001 + folder=43 pattern...");

    // Read large chunks of memory and search
    let search_start = base + 0x1000000; // Start 16MB into the module
    let search_end = base + module_size.min(0x5000000); // Up to 80MB
    let chunk_size: usize = 4 * 1024 * 1024; // 4MB chunks

    let mut found_addresses: Vec<u64> = Vec::new();
    let mut offset = 0u64;

    while search_start + offset < search_end {
        let addr = search_start + offset;
        let read_size = chunk_size.min((search_end - addr) as usize);

        match reader.read_bytes(addr, read_size) {
            Ok(buffer) => {
                // Search for pattern
                for (i, window) in buffer.windows(8).enumerate() {
                    if window == pattern_1001_43 {
                        let found_addr = addr + i as u64;
                        found_addresses.push(found_addr);

                        if found_addresses.len() <= 10 {
                            println!("    Found at 0x{:X}", found_addr);

                            // Try to analyze structure at this location
                            analyze_potential_song_entry(reader, found_addr);
                        }
                    }
                }
            }
            Err(_) => {
                // Skip unreadable regions
            }
        }

        offset += chunk_size as u64;
    }

    println!("  Total matches found: {}", found_addresses.len());

    // Also search for consecutive song IDs to find potential song list
    println!();
    println!("  Searching for consecutive song IDs (1001, 1002, 1003)...");
    search_consecutive_song_ids(reader, base, module_size);
}

/// Analyze a potential song entry at the given address.
pub fn analyze_potential_song_entry(reader: &MemoryReader, addr: u64) {
    // Read 64 bytes around the address
    if let Ok(buffer) = reader.read_bytes(addr, 64) {
        let song_id = i32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        let folder = i32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

        // Check if offset 8 has ASCII data (difficulty levels)
        let has_ascii = buffer[8..18]
            .iter()
            .all(|&b| (0x30..=0x39).contains(&b) || b == 0);

        if has_ascii {
            let diff_str: String = buffer[8..18]
                .iter()
                .take_while(|&&b| (0x30..=0x39).contains(&b))
                .map(|&b| b as char)
                .collect();
            println!(
                "      song_id={}, folder={}, difficulty=\"{}\"",
                song_id, folder, diff_str
            );
        }

        // Check for next entry at various offsets
        for entry_size in [32u64, 48, 64, 80, 96, 128] {
            let next_addr = addr + entry_size;
            if let Ok(next_buf) = reader.read_bytes(next_addr, 8) {
                let next_id =
                    i32::from_le_bytes([next_buf[0], next_buf[1], next_buf[2], next_buf[3]]);
                let next_folder =
                    i32::from_le_bytes([next_buf[4], next_buf[5], next_buf[6], next_buf[7]]);

                // Check if next entry looks valid (song_id 1001-50000, folder 1-50)
                if (1000..=50000).contains(&next_id) && (1..=50).contains(&next_folder) {
                    println!(
                        "        -> Entry size {} works: next song_id={}",
                        entry_size, next_id
                    );
                }
            }
        }
    }
}
