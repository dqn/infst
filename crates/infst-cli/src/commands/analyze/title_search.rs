//! Song title string search.

use infst::{MemoryReader, ReadMemory};

/// Search for known song titles in memory to find where title strings are stored.
pub fn search_for_title_strings(reader: &MemoryReader, base: u64, module_size: u64) {
    println!();
    println!("=== Searching for song title patterns ===");

    // Common ASCII IIDX song titles to search for
    let search_titles: &[(&str, &[u8])] = &[
        ("5.1.1.", b"5.1.1."),
        ("GAMBOL", b"GAMBOL"),
        ("Sleepless", b"Sleepless"),
        ("SLEEPLESS", b"SLEEPLESS"),
        ("piano ambient", b"piano ambient"),
        ("PIANO AMBIENT", b"PIANO AMBIENT"),
        ("R5", b"R5"),
        ("GRADIUSIC CYBER", b"GRADIUSIC CYBER"),
        ("20,november", b"20,november"),
        ("Tangerine Stream", b"Tangerine Stream"),
    ];

    let search_start = base + 0x1000000;
    let search_end = base + module_size.min(0x5000000);
    let chunk_size: usize = 4 * 1024 * 1024;

    for (title, pattern) in search_titles {
        println!("  Searching for \"{}\" ({} bytes)...", title, pattern.len());

        let mut found: Vec<u64> = Vec::new();
        let mut offset = 0u64;

        while search_start + offset < search_end && found.len() < 20 {
            let addr = search_start + offset;
            let read_size = chunk_size.min((search_end - addr) as usize);

            if let Ok(buffer) = reader.read_bytes(addr, read_size) {
                for (i, window) in buffer.windows(pattern.len()).enumerate() {
                    if window == *pattern {
                        let found_addr = addr + i as u64;
                        found.push(found_addr);

                        if found.len() <= 5 {
                            println!("    Found at 0x{:X}", found_addr);

                            // Read some context around the match
                            if let Ok(context) =
                                reader.read_bytes(found_addr.saturating_sub(64), 192)
                            {
                                // Look for song_id nearby (at known offsets from old structure)
                                for check_offset in [0usize, 64, 128, 256, 512, 624, 656, 688] {
                                    if check_offset + 4 <= context.len() {
                                        let potential_id = i32::from_le_bytes([
                                            context[check_offset],
                                            context[check_offset + 1],
                                            context[check_offset + 2],
                                            context[check_offset + 3],
                                        ]);
                                        if (1000..=50000).contains(&potential_id) {
                                            println!(
                                                "      -> Potential song_id={} at relative offset {} (abs: 0x{:X})",
                                                potential_id,
                                                check_offset as i64 - 64,
                                                found_addr.saturating_sub(64) + check_offset as u64
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            offset += chunk_size as u64;
        }

        println!("    Total matches: {}", found.len());
    }
}
