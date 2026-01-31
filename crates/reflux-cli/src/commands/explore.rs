//! Explore command implementation.
//!
//! Comprehensive memory structure exploration for understanding the song
//! database layout in INFINITAS. Analyzes entry structure, validates metadata,
//! and searches for specific patterns to reverse-engineer memory layout.
//!
//! This is a debugging tool used when investigating new game versions.

use anyhow::Result;
use reflux_core::{MemoryReader, ProcessHandle, ReadMemory};

/// Run the memory explore command
pub fn run(base_addr: u64, pid: Option<u32>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Reflux-RS {} - Memory Explore Mode", current_version);

    let process = if let Some(pid) = pid {
        ProcessHandle::open(pid)?
    } else {
        ProcessHandle::find_and_open()?
    };

    println!(
        "Found process (PID: {}, Base: 0x{:X})",
        process.pid, process.base_address
    );
    let reader = MemoryReader::new(&process);

    const ENTRY_SIZE: u64 = 0x3F0; // 1008 bytes
    const METADATA_OFFSET: u64 = 0x7E0; // 2016 bytes

    // Analyze entry states
    println!();
    println!("=== Entry State Analysis at 0x{:X} ===", base_addr);
    println!("Entry size: 0x{:X} ({} bytes)", ENTRY_SIZE, ENTRY_SIZE);
    println!(
        "Metadata offset: 0x{:X} ({} bytes)",
        METADATA_OFFSET, METADATA_OFFSET
    );

    let max_entries = 2000u64;
    let mut has_title = 0u32;
    let mut has_valid_meta = 0u32;
    let mut has_both = 0u32;
    let mut title_only = 0u32;
    let mut meta_only = 0u32;
    let mut empty = 0u32;
    let mut read_errors = 0u32;

    let mut found_songs: Vec<(u64, u32, i32, String)> = Vec::new();

    for i in 0..max_entries {
        let text_addr = base_addr + i * ENTRY_SIZE;
        let meta_addr = text_addr + METADATA_OFFSET;

        // Read title
        let title = match reader.read_bytes(text_addr, 64) {
            Ok(bytes) => {
                let len = bytes.iter().position(|&b| b == 0).unwrap_or(64);
                if len > 0 && bytes[0] != 0 {
                    let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes[..len]);
                    let t = decoded.trim();
                    if !t.is_empty()
                        && t.chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_graphic() || !c.is_ascii())
                    {
                        Some(t.to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Err(_) => {
                read_errors += 1;
                continue;
            }
        };

        // Read metadata
        let (song_id, folder) = match reader.read_bytes(meta_addr, 8) {
            Ok(bytes) => {
                let id = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                let folder = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                (id, folder)
            }
            Err(_) => {
                read_errors += 1;
                continue;
            }
        };

        let valid_meta = (1000..=90000).contains(&song_id) && (1..=200).contains(&folder);

        // Debug: Look for song_id=9003 regardless of filter
        if song_id == 9003 {
            println!(
                "*** FOUND song_id=9003 (metadata) at entry={}, folder={}, title={:?}",
                i, folder, title
            );
        }

        // Also check C# style offset (0x270 from entry start for song_id)
        let csharp_id_offset = 624u64; // 256 + 368 = 0x270
        if let Ok(csharp_id) = reader.read_i32(text_addr + csharp_id_offset)
            && csharp_id == 9003
        {
            // Read difficulty levels at offset 288 (0x120)
            let levels = reader.read_bytes(text_addr + 288, 10).unwrap_or_default();
            println!(
                "*** FOUND song_id=9003 (C# style) at entry={}, title={:?}, levels={:?}",
                i, title, levels
            );
        }

        // Debug: Look for title containing "fun"
        if let Some(ref t) = title
            && t.to_lowercase().contains("fun")
        {
            println!(
                "*** FOUND title containing 'fun' at entry={}, id={}, folder={}, title={:?}",
                i, song_id, folder, t
            );
        }

        match (title.is_some(), valid_meta) {
            (true, true) => {
                has_title += 1;
                has_valid_meta += 1;
                has_both += 1;
                found_songs.push((i, song_id as u32, folder, title.unwrap()));
            }
            (true, false) => {
                has_title += 1;
                title_only += 1;
            }
            (false, true) => {
                has_valid_meta += 1;
                meta_only += 1;
            }
            (false, false) => {
                empty += 1;
            }
        }
    }

    println!();
    println!("=== Statistics (first {} entries) ===", max_entries);
    println!("  Entries with valid title:    {:5}", has_title);
    println!("  Entries with valid metadata: {:5}", has_valid_meta);
    println!("  Entries with both:           {:5}", has_both);
    println!("  Title only (no valid meta):  {:5}", title_only);
    println!("  Metadata only (no title):    {:5}", meta_only);
    println!("  Empty entries:               {:5}", empty);
    println!("  Read errors:                 {:5}", read_errors);

    println!();
    println!(
        "=== Found songs with title + valid metadata ({} total) ===",
        found_songs.len()
    );
    for (i, (idx, song_id, folder, title)) in found_songs.iter().take(30).enumerate() {
        println!(
            "  [{:3}] entry={:4}, id={:5}, folder={:3}, title={:?}",
            i, idx, song_id, folder, title
        );
    }
    if found_songs.len() > 30 {
        println!("  ... and {} more", found_songs.len() - 30);
    }

    // Check if entries are contiguous or scattered
    if found_songs.len() >= 2 {
        println!();
        println!("=== Entry distribution ===");
        let indices: Vec<u64> = found_songs.iter().map(|(idx, _, _, _)| *idx).collect();
        let min_idx = *indices.iter().min().unwrap();
        let max_idx = *indices.iter().max().unwrap();
        println!(
            "  Entry range: {} to {} (span: {})",
            min_idx,
            max_idx,
            max_idx - min_idx + 1
        );
        println!(
            "  Density: {:.1}% of entries in range have songs",
            100.0 * found_songs.len() as f64 / (max_idx - min_idx + 1) as f64
        );
    }

    // Check first entry (5.1.1.) with both old and new offsets
    println!();
    println!("=== Analyzing first entry (5.1.1.) structure ===");
    if let Ok(data) = reader.read_bytes(base_addr, 1200) {
        println!("  Reading from 0x{:X}:", base_addr);

        // Check title
        let title_len = data.iter().take(64).position(|&b| b == 0).unwrap_or(64);
        let (title, _, _) = encoding_rs::SHIFT_JIS.decode(&data[..title_len]);
        println!("    title at 0: {:?}", title.trim());

        // Check OLD offsets (C# style)
        let old_levels = &data[288..298];
        let old_song_id = i32::from_le_bytes([data[624], data[625], data[626], data[627]]);
        println!(
            "    OLD: song_id at 624 = {}, levels at 288 = {:?}",
            old_song_id, old_levels
        );

        // Check NEW offsets (discovered from 'fun')
        let new_levels = &data[480..490];
        let new_song_id = i32::from_le_bytes([data[816], data[817], data[818], data[819]]);
        println!(
            "    NEW: song_id at 816 = {}, levels at 480 = {:?}",
            new_song_id, new_levels
        );

        // Dump some key offsets to understand structure
        for offset in [
            256, 320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960, 1024, 1088,
        ]
        .iter()
        {
            if *offset + 64 <= 1200 {
                let str_bytes = &data[*offset..*offset + 64];
                let len = str_bytes.iter().position(|&b| b == 0).unwrap_or(64);
                if len > 0 && str_bytes[0] >= 0x20 && str_bytes[0] < 0x80 {
                    let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&str_bytes[..len]);
                    println!("    offset {}: {:?}", offset, decoded.trim());
                }
            }
        }
    }

    // Try scanning with NEW offsets
    println!();
    println!("=== Scanning with NEW offsets (song_id at 816) ===");
    const NEW_ENTRY_SIZE: u64 = 1200; // Hypothesized new entry size
    let new_max_entries = (0x800000u64 / NEW_ENTRY_SIZE).min(2000);
    let mut found_with_new = Vec::new();

    for i in 0..new_max_entries {
        let entry_addr = base_addr + i * NEW_ENTRY_SIZE;
        if let Ok(data) = reader.read_bytes(entry_addr, 1200) {
            // Check for valid title
            let title_len = data.iter().take(64).position(|&b| b == 0).unwrap_or(64);
            if title_len == 0 || data[0] < 0x20 {
                continue;
            }
            let (title, _, _) = encoding_rs::SHIFT_JIS.decode(&data[..title_len]);
            let title = title.trim();
            if title.is_empty() {
                continue;
            }

            // Read with NEW offsets
            let song_id = i32::from_le_bytes([data[816], data[817], data[818], data[819]]);
            let levels = &data[480..490];

            if (1000..=50000).contains(&song_id) {
                found_with_new.push((i, song_id, title.to_string(), levels.to_vec()));
                if song_id == 9003 {
                    println!(
                        "  *** FOUND song_id=9003: entry={}, title={:?}, levels={:?}",
                        i, title, levels
                    );
                }
            }
        }
    }
    println!("  Found {} songs with new offsets", found_with_new.len());
    for (i, (idx, id, title, levels)) in found_with_new.iter().take(10).enumerate() {
        println!(
            "    [{:2}] entry={:4}, id={:5}, title={:?}, levels={:?}",
            i, idx, id, title, levels
        );
    }

    // Check currentSong offset (0x1428382d0) and surrounding area
    let current_song_addr = 0x1428382d0u64;
    println!();
    println!("=== Current Song Info at 0x{:X} ===", current_song_addr);
    if let Ok(bytes) = reader.read_bytes(current_song_addr, 128) {
        let song_id = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let difficulty = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        println!("  song_id (offset 0): {}", song_id);
        println!("  difficulty (offset 4): {}", difficulty);
        println!("  raw bytes 0-63: {:02X?}", &bytes[..64]);
        println!("  raw bytes 64-127: {:02X?}", &bytes[64..128]);

        // Look for pointers (values that look like addresses)
        for i in (0..120).step_by(8) {
            let val = u64::from_le_bytes([
                bytes[i],
                bytes[i + 1],
                bytes[i + 2],
                bytes[i + 3],
                bytes[i + 4],
                bytes[i + 5],
                bytes[i + 6],
                bytes[i + 7],
            ]);
            if val > 0x140000000 && val < 0x150000000 {
                println!("  Potential pointer at offset {}: 0x{:X}", i, val);
                // Try to read what's at that address
                if let Ok(target_bytes) = reader.read_bytes(val, 64) {
                    let len = target_bytes.iter().position(|&b| b == 0).unwrap_or(64);
                    if len > 0 && target_bytes[0] >= 0x20 {
                        let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&target_bytes[..len]);
                        println!("    -> String: {:?}", decoded.trim());
                    }
                }
            }
        }
    }

    // Search for "fun" string in memory (around song_list area)
    println!();
    println!("=== Searching for 'fun' string in memory ===");
    let search_start = base_addr;
    let search_size = 0x800000u64; // 8MB
    let chunk_size = 0x10000usize; // 64KB chunks

    let fun_pattern = b"fun\x00"; // "fun" followed by null terminator
    let mut found_fun = Vec::new();

    for chunk_start in (0..search_size).step_by(chunk_size) {
        let addr = search_start + chunk_start;
        if let Ok(chunk) = reader.read_bytes(addr, chunk_size) {
            for i in 0..(chunk_size - 4) {
                if &chunk[i..i + 4] == fun_pattern {
                    found_fun.push(addr + i as u64);
                }
            }
        }
    }

    println!("  Found {} occurrences of 'fun\\0'", found_fun.len());
    // Only analyze the first "fun" as it appears to be the title
    if let Some(addr) = found_fun.first() {
        println!("  Analyzing first 'fun' at 0x{:X} as entry start:", addr);

        // Read a larger buffer to analyze the structure
        if let Ok(data) = reader.read_bytes(*addr, 1024) {
            // Dump strings at key offsets
            for (name, offset) in [
                ("title (0)", 0usize),
                ("title_en (64)", 64),
                ("genre (128)", 128),
                ("artist (192)", 192),
                ("unknown (256)", 256),
                ("unknown (320)", 320),
                ("unknown (384)", 384),
                ("unknown (448)", 448),
                ("unknown (512)", 512),
            ]
            .iter()
            {
                let str_bytes = &data[*offset..*offset + 64];
                let len = str_bytes.iter().position(|&b| b == 0).unwrap_or(64);
                if len > 0 && str_bytes[0] >= 0x20 {
                    let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&str_bytes[..len]);
                    println!("      {}: {:?}", name, decoded.trim());
                } else {
                    println!("      {}: (empty or binary)", name);
                }
            }

            // Check for levels-like data (10 consecutive small bytes)
            println!("      Scanning for levels pattern (10 bytes, values 0-12):");
            for offset in (256..900).step_by(8) {
                let slice = &data[offset..offset + 10];
                if slice.iter().all(|&b| b <= 12) && slice.iter().any(|&b| b > 0) {
                    println!("        offset {}: {:?}", offset, slice);
                }
            }
        }

        // Check larger area for song_id
        if let Ok(wide_data) = reader.read_bytes(*addr, 1024) {
            println!("      Scanning for song_id=9003 in entry:");
            let target_bytes: [u8; 4] = 9003u32.to_le_bytes();
            for j in 0..1020 {
                if wide_data[j..j + 4] == target_bytes {
                    println!("        *** song_id=9003 at offset {} ***", j);
                }
            }
        }
    }

    // Search for song_id=9003 (0x232B) as a 4-byte value in memory
    println!();
    println!("=== Searching for song_id=9003 (0x232B) as 4-byte value ===");
    let target_id: u32 = 9003;
    let target_bytes = target_id.to_le_bytes();
    let search_size = 0x800000usize; // 8MB

    if let Ok(data) = reader.read_bytes(base_addr, search_size) {
        let mut found_locations = Vec::new();
        for i in 0..(search_size - 4) {
            if data[i..i + 4] == target_bytes {
                found_locations.push(base_addr + i as u64);
            }
        }

        println!(
            "  Found {} occurrences of 0x{:08X} ({})",
            found_locations.len(),
            target_id,
            target_id
        );
        for (i, addr) in found_locations.iter().take(20).enumerate() {
            let offset_from_base = addr - base_addr;
            println!("  [{}] 0x{:X} (base+0x{:X})", i, addr, offset_from_base);

            // Read context around this location
            let context_start = addr.saturating_sub(64);
            if reader.read_bytes(context_start, 256).is_ok() {
                // Check for readable strings nearby
                let mut strings_found = Vec::new();

                // Check various offsets for strings
                for string_offset in [-624i64, -560, -432, -288, -192, -128, -64, 0, 64].iter() {
                    let check_addr = (*addr as i64 + string_offset) as u64;
                    if let Ok(str_bytes) = reader.read_bytes(check_addr, 64) {
                        let len = str_bytes.iter().position(|&b| b == 0).unwrap_or(64);
                        if len > 2 && str_bytes[0] >= 0x20 && str_bytes[0] < 0x80 {
                            let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&str_bytes[..len]);
                            let s = decoded.trim();
                            if !s.is_empty() && s.len() >= 2 {
                                strings_found.push((string_offset, s.to_string()));
                            }
                        }
                    }
                }

                if !strings_found.is_empty() {
                    for (off, s) in &strings_found {
                        println!("      string at offset {}: {:?}", off, s);
                    }
                }

                // Also check if this might be a song entry
                // If 9003 is at offset 624, entry start would be addr - 624
                let potential_entry_start = addr.saturating_sub(624);
                if let Ok(entry) = reader.read_bytes(potential_entry_start, 1008) {
                    let title_len = entry.iter().take(64).position(|&b| b == 0).unwrap_or(64);
                    if title_len > 0 && entry[0] >= 0x20 {
                        let (title, _, _) = encoding_rs::SHIFT_JIS.decode(&entry[..title_len]);
                        let levels = &entry[288..298];
                        println!(
                            "      -> if at offset 624: entry=0x{:X}, title={:?}, levels={:?}",
                            potential_entry_start,
                            title.trim(),
                            levels
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
