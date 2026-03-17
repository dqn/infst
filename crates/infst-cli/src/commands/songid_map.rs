//! Song ID mapping investigation command.
//!
//! Investigates the relationship between the "internal ID" at offset 0 of
//! the new 0x630 entry table and the "game song_id" reported by
//! CurrentSong/PlayData.
//!
//! The "5.1.1." text table (old format) may still contain the correct
//! game song_id at the old V1/V2 offsets (0x270 or 0x330).

use anyhow::Result;
use infst::offset::{OffsetSearcher, OffsetsCollection, builtin_signatures};
use infst::process::decode_shift_jis_to_string;
use infst::{MemoryReader, ProcessHandle, ReadMemory};

/// Run the song ID mapping investigation
pub fn run(pid: Option<u32>) -> Result<()> {
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

    // Step 1: Find all offsets
    println!();
    println!("=== Step 1: Find Offsets ===");

    let signatures = builtin_signatures();
    let mut searcher = OffsetSearcher::new(&reader);
    let offsets = match searcher.search_all_with_signatures(&signatures) {
        Ok(o) => {
            println!("  song_list:        0x{:X}", o.song_list);
            println!("  song_entry_table: 0x{:X}", o.song_entry_table);
            println!(
                "  song_entry_size:  0x{:X} ({} bytes)",
                o.song_entry_size, o.song_entry_size
            );
            println!("  current_song:     0x{:X}", o.current_song);
            println!("  play_data:        0x{:X}", o.play_data);
            println!("  judge_data:       0x{:X}", o.judge_data);
            o
        }
        Err(e) => {
            println!("  Offset search failed: {}", e);
            return Ok(());
        }
    };

    // Step 2: Read CurrentSong and PlayData
    println!();
    println!("=== Step 2: Game Song ID ===");

    let game_song_id = read_game_song_id(&reader, &offsets);

    // Step 3: Probe the old text table
    println!();
    println!(
        "=== Step 3: Probe Text Table at 0x{:X} ===",
        offsets.song_list
    );
    probe_text_table(&reader, offsets.song_list, game_song_id);

    // Step 4: Probe the new entry table
    let entry_addr = offsets.song_db_address();
    let stride = offsets.entry_stride();
    if entry_addr != offsets.song_list {
        println!();
        println!(
            "=== Step 4: Probe Entry Table at 0x{:X} (stride 0x{:X}) ===",
            entry_addr, stride
        );
        probe_entry_table(&reader, entry_addr, stride, game_song_id);
    }

    // Step 5: Cross-reference by title
    if offsets.song_entry_table != 0 {
        println!();
        println!("=== Step 5: Cross-Reference (title matching) ===");
        cross_reference_tables(
            &reader,
            offsets.song_list,
            offsets.song_entry_table,
            stride,
            game_song_id,
        );
    }

    // Step 6: Search for game song_id in wider memory around entry table
    if game_song_id > 0 {
        println!();
        println!(
            "=== Step 6: Search for game song_id {} near entry table ===",
            game_song_id
        );
        search_game_songid_near_table(&reader, entry_addr, game_song_id);
    }

    Ok(())
}

/// Read the game's song_id from CurrentSong and PlayData
fn read_game_song_id<R: ReadMemory>(reader: &R, offsets: &OffsetsCollection) -> i32 {
    let cs_song_id = reader.read_i32(offsets.current_song).unwrap_or(0);
    let cs_diff = reader.read_i32(offsets.current_song + 4).unwrap_or(0);
    println!(
        "  CurrentSong: song_id={}, difficulty={}",
        cs_song_id, cs_diff
    );

    let pd_song_id = reader
        .read_i32(offsets.play_data + infst::process::layout::play::SONG_ID)
        .unwrap_or(0);
    let pd_diff = reader
        .read_i32(offsets.play_data + infst::process::layout::play::DIFFICULTY)
        .unwrap_or(0);
    println!(
        "  PlayData:    song_id={}, difficulty={}",
        pd_song_id, pd_diff
    );

    if cs_song_id > 0 {
        cs_song_id
    } else {
        pd_song_id
    }
}

/// Probe the old "5.1.1." text table with multiple strides and song_id offsets
fn probe_text_table<R: ReadMemory>(reader: &R, text_base: u64, game_song_id: i32) {
    // Dump first bytes to understand structure
    if let Ok(header) = reader.read_bytes(text_base, 32) {
        let title = decode_shift_jis_to_string(&header);
        println!("  First 32 bytes: {:02X?}", &header);
        println!("  As string: {:?}", title);
    }

    // Try different strides (V1=0x3F0, V2=0x4B0, V3=0x630)
    for &stride in &[0x3F0usize, 0x4B0, 0x630] {
        println!();
        println!("  --- Stride 0x{:X} ({} bytes) ---", stride, stride);

        let mut found_game_id = false;

        for entry_idx in 0..10 {
            let entry_start = text_base + (entry_idx as u64) * (stride as u64);

            // Read title at offset 0 (old format: title-first)
            let title = read_title(reader, entry_start);

            // Try various song_id offsets within entry
            let offsets_to_check = [0x000usize, 0x270, 0x330];
            let mut ids: Vec<(usize, i32)> = Vec::new();
            for offset in offsets_to_check {
                if offset + 4 <= stride
                    && let Ok(val) = reader.read_i32(entry_start + offset as u64)
                {
                    ids.push((offset, val));
                }
            }

            let id_strs: Vec<String> = ids
                .iter()
                .map(|(off, val)| format!("@0x{:03X}={}", off, val))
                .collect();

            let game_match = ids
                .iter()
                .any(|&(_, val)| val == game_song_id && game_song_id > 0);
            let marker = if game_match {
                " *** GAME MATCH ***"
            } else {
                ""
            };
            if game_match {
                found_game_id = true;
            }

            println!(
                "    [{}] 0x{:X}: title={:?} ids=[{}]{}",
                entry_idx,
                entry_start,
                truncate_str(&title, 30),
                id_strs.join(", "),
                marker,
            );
        }

        if found_game_id {
            println!(
                "    >>> Found game song_id={} with stride 0x{:X}!",
                game_song_id, stride
            );
        }
    }

    // Scan text table region for game song_id value
    if game_song_id > 0 {
        println!();
        println!(
            "  --- Scanning text table region for game song_id={} ---",
            game_song_id
        );
        let scan_size = 0x100000; // 1MB
        if let Ok(buffer) = reader.read_bytes(text_base, scan_size) {
            let target = game_song_id.to_le_bytes();
            let mut found = 0;
            for i in 0..buffer.len().saturating_sub(4) {
                if buffer[i..i + 4] == target {
                    println!(
                        "    Found at 0x{:X} (text_base+0x{:X})",
                        text_base + i as u64,
                        i
                    );
                    found += 1;
                    if found >= 10 {
                        println!("    ... (stopping after 10 matches)");
                        break;
                    }
                }
            }
            if found == 0 {
                println!("    NOT FOUND in text table region (1MB)");
            }
        }
    }
}

/// Probe the new entry table (0x630 entries with internal_id at offset 0)
fn probe_entry_table<R: ReadMemory>(reader: &R, entry_base: u64, stride: usize, game_song_id: i32) {
    for entry_idx in 0..10 {
        let entry_start = entry_base + (entry_idx as u64) * (stride as u64);

        let internal_id = reader.read_i32(entry_start).unwrap_or(0);
        let folder = reader.read_i32(entry_start + 4).unwrap_or(0);

        // Title at offset 0x180
        let title = match reader.read_bytes(entry_start + 0x180, 64) {
            Ok(bytes) => decode_shift_jis_to_string(&bytes),
            Err(_) => "(read error)".to_string(),
        };

        let marker = if internal_id == game_song_id && game_song_id > 0 {
            " *** MATCHES GAME ***"
        } else {
            ""
        };

        println!(
            "    [{}] 0x{:X}: internal_id={}, folder={}, title={:?}{}",
            entry_idx,
            entry_start,
            internal_id,
            folder,
            truncate_str(&title, 30),
            marker,
        );
    }

    // Count total entries
    let mut total = 0;
    for i in 0..5000u64 {
        let addr = entry_base + i * stride as u64;
        match reader.read_i32(addr) {
            Ok(id) if (1000..=90000).contains(&id) => total += 1,
            Ok(0) => continue,
            _ => break,
        }
    }
    println!("    Total entries with valid IDs: {}", total);
}

/// Cross-reference old text table and new entry table by matching titles
fn cross_reference_tables<R: ReadMemory>(
    reader: &R,
    text_base: u64,
    entry_base: u64,
    entry_stride: usize,
    game_song_id: i32,
) {
    // Read entries from the new table (internal_id + title)
    let mut new_entries: Vec<(i32, String)> = Vec::new();
    for i in 0..100u64 {
        let addr = entry_base + i * entry_stride as u64;
        let id = match reader.read_i32(addr) {
            Ok(id) if (1000..=90000).contains(&id) => id,
            _ => continue,
        };
        let title = match reader.read_bytes(addr + 0x180, 64) {
            Ok(bytes) => {
                let s = decode_shift_jis_to_string(&bytes);
                if s.is_empty() {
                    continue;
                }
                s
            }
            Err(_) => continue,
        };
        new_entries.push((id, title));
    }

    println!("  Loaded {} entries from new table", new_entries.len());

    // Try each combination of old stride and song_id offset
    for &old_stride in &[0x3F0usize, 0x4B0] {
        for &sid_offset in &[0x270usize, 0x330] {
            if sid_offset + 4 > old_stride {
                continue;
            }

            println!();
            println!(
                "  --- Matching: old stride=0x{:X}, song_id@0x{:X} ---",
                old_stride, sid_offset
            );

            // Build old table map: title -> old_song_id
            let mut old_map: Vec<(String, i32)> = Vec::new();
            for i in 0..2000u64 {
                let addr = text_base + i * old_stride as u64;
                let title = read_title(reader, addr);
                if title == "(empty)" || title == "(read error)" {
                    continue;
                }

                let old_song_id = reader.read_i32(addr + sid_offset as u64).unwrap_or(0);
                if (1000..=90000).contains(&old_song_id) {
                    old_map.push((title, old_song_id));
                }
            }

            println!("    Found {} entries in old table", old_map.len());

            // Match by title
            let mut matched = 0;
            let mut mismatched = 0;

            for (new_id, new_title) in &new_entries {
                let normalized_new = normalize(new_title);
                for (old_title, old_id) in &old_map {
                    let normalized_old = normalize(old_title);
                    if normalized_new == normalized_old {
                        if new_id != old_id {
                            let marker = if *old_id == game_song_id {
                                " *** OLD = GAME ***"
                            } else {
                                ""
                            };
                            println!(
                                "    {:?}: internal={} -> old={}  (diff={}){}",
                                truncate_str(new_title, 25),
                                new_id,
                                old_id,
                                old_id - new_id,
                                marker,
                            );
                            mismatched += 1;
                        } else {
                            matched += 1;
                        }
                        break;
                    }
                }
            }

            println!(
                "    Results: {} matched (same ID), {} mismatched (different ID)",
                matched, mismatched
            );
        }
    }
}

/// Search for the game's song_id value in memory around the entry table
fn search_game_songid_near_table<R: ReadMemory>(reader: &R, entry_base: u64, game_song_id: i32) {
    let target = game_song_id.to_le_bytes();
    let search_start = entry_base.saturating_sub(0x100000); // 1MB before
    let search_size = 0x200000usize; // 2MB total

    if let Ok(buffer) = reader.read_bytes(search_start, search_size) {
        let mut found = 0;
        for i in 0..buffer.len().saturating_sub(4) {
            if buffer[i..i + 4] == target {
                let abs_addr = search_start + i as u64;
                let rel_to_entry = abs_addr as i64 - entry_base as i64;

                let context_str = if i >= 4 {
                    let prev = i32::from_le_bytes(buffer[i - 4..i].try_into().unwrap());
                    format!("prev_i32={}", prev)
                } else {
                    String::new()
                };

                let next_str = if i + 8 <= buffer.len() {
                    let next = i32::from_le_bytes(buffer[i + 4..i + 8].try_into().unwrap());
                    format!("next_i32={}", next)
                } else {
                    String::new()
                };

                println!(
                    "    0x{:X} (entry_base{:+}) {} {}",
                    abs_addr, rel_to_entry, context_str, next_str,
                );
                found += 1;
                if found >= 20 {
                    println!("    ... (stopping after 20 matches)");
                    break;
                }
            }
        }
        if found == 0 {
            println!("    NOT FOUND within 1MB of entry table");
        } else {
            println!("    Total: {} matches", found);
        }
    }
}

fn read_title<R: ReadMemory>(reader: &R, addr: u64) -> String {
    match reader.read_bytes(addr, 64) {
        Ok(bytes) => {
            let s = decode_shift_jis_to_string(&bytes);
            if s.is_empty() || s.bytes().next().is_none_or(|b| b < 0x20) {
                "(empty)".to_string()
            } else {
                s
            }
        }
        Err(_) => "(read error)".to_string(),
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}
