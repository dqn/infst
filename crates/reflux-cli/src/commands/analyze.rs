//! Analyze command implementation.

use anyhow::{Result, bail};
use reflux_core::{MemoryReader, OffsetSearcher, ProcessHandle, ReadMemory, SongInfo};

/// Run the memory structure analysis mode
pub fn run(address: Option<String>, pid: Option<u32>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Reflux-RS {} - Memory Analysis Mode", current_version);

    // Open process
    let process = if let Some(pid) = pid {
        println!("Opening process with PID {}...", pid);
        ProcessHandle::open(pid)?
    } else {
        println!("Searching for INFINITAS...");
        ProcessHandle::find_and_open()?
    };

    println!(
        "Found process (Base: 0x{:X}, Size: 0x{:X})",
        process.base_address, process.module_size
    );

    let reader = MemoryReader::new(&process);

    // Parse address or search for it
    let analyze_addr = if let Some(addr_str) = address {
        // Parse hex address
        let addr_str = addr_str.trim_start_matches("0x").trim_start_matches("0X");
        u64::from_str_radix(addr_str, 16)?
    } else {
        // Search for new structure using song_id pattern
        println!("No address specified, searching for song data structures...");
        let mut searcher = OffsetSearcher::new(&reader);

        // Try to find 312-byte structure
        match searcher.search_song_list_comprehensive(process.base_address) {
            Ok(addr) => {
                println!("Found song data at: 0x{:X}", addr);
                addr
            }
            Err(e) => {
                bail!("Failed to find song data: {}", e);
            }
        }
    };

    println!();
    println!("=== Analyzing memory at 0x{:X} ===", analyze_addr);

    let searcher = OffsetSearcher::new(&reader);
    searcher.analyze_new_structure(analyze_addr);

    // Also try to read using old SongInfo structure
    println!();
    println!("=== Attempting old structure read ===");
    match SongInfo::read_from_memory(&reader, analyze_addr) {
        Ok(Some(song)) => {
            println!("  Old structure parsed:");
            println!("    id: {}", song.id);
            println!("    title: {:?}", song.title);
            println!("    artist: {:?}", song.artist);
            println!("    folder: {}", song.folder);
            println!("    levels: {:?}", song.levels);
        }
        Ok(None) => println!("  Old structure: Invalid (first 4 bytes are zero)"),
        Err(e) => println!("  Old structure read failed: {}", e),
    }

    // Count songs with old structure
    println!();
    println!("=== Song count analysis ===");
    let old_count = count_songs_old_structure(&reader, analyze_addr);
    println!("  Old structure (0x3F0): {} songs", old_count);

    // Count songs with new structure
    let new_count = count_songs_new_structure(&reader, analyze_addr);
    println!("  New structure (312 bytes): {} songs", new_count);

    // Comprehensive search for song data in memory
    println!();
    println!("=== Searching for song data patterns in memory ===");
    search_song_patterns(&reader, process.base_address, process.module_size as u64);

    // Search for known song titles
    search_for_title_strings(&reader, process.base_address, process.module_size as u64);

    Ok(())
}

fn count_songs_old_structure(reader: &MemoryReader, start: u64) -> usize {
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

fn count_songs_new_structure(reader: &MemoryReader, start: u64) -> usize {
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

/// Search for various song data patterns in memory
fn search_song_patterns(reader: &MemoryReader, base: u64, module_size: u64) {
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

fn analyze_potential_song_entry(reader: &MemoryReader, addr: u64) {
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

fn search_consecutive_song_ids(reader: &MemoryReader, base: u64, module_size: u64) {
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
    let mut delta_counts: std::collections::HashMap<u64, Vec<u64>> =
        std::collections::HashMap::new();
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

fn count_songs_with_size(reader: &MemoryReader, start: u64, entry_size: u64) -> usize {
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

/// Search for known song titles in memory to find where title strings are stored
fn search_for_title_strings(reader: &MemoryReader, base: u64, module_size: u64) {
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
