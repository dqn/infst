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

    // Step 7: Investigate DataMap node unknown fields
    println!();
    println!("=== Step 7: DataMap Node Unknown Fields ===");
    investigate_datamap_nodes(&reader, &offsets, game_song_id);

    // Step 8: Full hexdump of entry for internal_id = game_song_id - 1
    if game_song_id > 1000 {
        println!();
        println!("=== Step 8: Full Entry Hexdump (neighboring internal_ids) ===");
        for delta in [-1i32, 0, 1] {
            let target_iid = game_song_id + delta;
            println!();
            println!(
                "--- Entry for internal_id={} (game_id{:+}) ---",
                target_iid, delta
            );
            dump_full_entry(&reader, entry_addr, stride, target_iid, game_song_id);
        }
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

    // Scan full entry table region for game song_id value
    // 1809 entries * 0x630 = ~2.8MB, so scan 4MB to be safe
    if game_song_id > 0 {
        println!();
        println!(
            "  --- Scanning entry table region for game song_id={} ---",
            game_song_id
        );
        let scan_size = 0x400000; // 4MB
        if let Ok(buffer) = reader.read_bytes(text_base, scan_size) {
            let target = game_song_id.to_le_bytes();
            let mut found = 0;
            for i in 0..buffer.len().saturating_sub(4) {
                if buffer[i..i + 4] == target {
                    let abs_addr = text_base + i as u64;
                    println!("    Found at 0x{:X} (text_base+0x{:X})", abs_addr, i);
                    found += 1;
                    if found >= 10 {
                        println!("    ... (stopping after 10 matches)");
                        break;
                    }
                }
            }
            if found == 0 {
                println!("    NOT FOUND in entry table region (4MB)");
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
    // Search 2MB before and 4MB after to cover full entry table
    let search_start = entry_base.saturating_sub(0x200000);
    let search_size = 0x600000usize; // 6MB total

    if let Ok(buffer) = reader.read_bytes(search_start, search_size) {
        let mut found_addrs: Vec<u64> = Vec::new();
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
                found_addrs.push(abs_addr);
                if found_addrs.len() >= 20 {
                    println!("    ... (stopping after 20 matches)");
                    break;
                }
            }
        }
        if found_addrs.is_empty() {
            println!("    NOT FOUND within search range");
        } else {
            println!("    Total: {} matches", found_addrs.len());

            // Hexdump around first match for context
            if let Some(&first_addr) = found_addrs.first() {
                println!();
                println!("  --- Hexdump around first match (0x{:X}) ---", first_addr);
                hexdump_region(reader, first_addr.saturating_sub(64), 192);
            }
        }
    }

    // Also check: find the closest internal_id to game_song_id in entry table
    println!();
    println!(
        "  --- Searching entry table for internal_id closest to {} ---",
        game_song_id
    );
    find_closest_internal_id(reader, entry_base, 0x630, game_song_id);
}

/// Find the entry with internal_id closest to the target game song_id
fn find_closest_internal_id<R: ReadMemory>(
    reader: &R,
    entry_base: u64,
    stride: usize,
    target_id: i32,
) {
    let mut closest: Option<(i32, u64, String)> = None;
    let mut closest_diff = i32::MAX;

    for i in 0..2000u64 {
        let addr = entry_base + i * stride as u64;
        let id = match reader.read_i32(addr) {
            Ok(id) if (1000..=90000).contains(&id) => id,
            Ok(0) => continue,
            _ => break,
        };

        let diff = (id - target_id).abs();
        if diff < closest_diff {
            closest_diff = diff;
            let title = match reader.read_bytes(addr + 0x180, 64) {
                Ok(bytes) => decode_shift_jis_to_string(&bytes),
                Err(_) => String::new(),
            };
            closest = Some((id, addr, title));

            if diff == 0 {
                break; // Exact match
            }
        }
    }

    if let Some((id, addr, title)) = closest {
        let diff = id - target_id;
        println!(
            "    Closest: internal_id={} at 0x{:X}, title={:?} (diff={})",
            id, addr, title, diff
        );

        // Also show neighbors
        if diff != 0 {
            // Show entries around the closest match
            let idx = ((addr - entry_base) / stride as u64) as i64;
            for delta in [-2i64, -1, 0, 1, 2] {
                let check_idx = idx + delta;
                if check_idx < 0 {
                    continue;
                }
                let check_addr = entry_base + (check_idx as u64) * stride as u64;
                let check_id = reader.read_i32(check_addr).unwrap_or(0);
                let check_title = match reader.read_bytes(check_addr + 0x180, 64) {
                    Ok(bytes) => decode_shift_jis_to_string(&bytes),
                    Err(_) => String::new(),
                };
                let marker = if delta == 0 { " <-- closest" } else { "" };
                println!(
                    "      [idx {}] internal_id={}, title={:?}{}",
                    check_idx, check_id, check_title, marker
                );
            }
        }
    }
}

/// Print a hex dump of a memory region
fn hexdump_region<R: ReadMemory>(reader: &R, addr: u64, size: usize) {
    let Ok(buffer) = reader.read_bytes(addr, size) else {
        println!("    (read failed)");
        return;
    };

    for row in (0..buffer.len()).step_by(16) {
        let row_end = (row + 16).min(buffer.len());
        let hex: Vec<String> = buffer[row..row_end]
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect();
        let ascii: String = buffer[row..row_end]
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!(
            "    {:012X}: {:48} {}",
            addr + row as u64,
            hex.join(" "),
            ascii,
        );
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

/// Investigate DataMap linked list nodes for unknown fields that might contain internal_id.
///
/// The ListNode structure (64 bytes) has several unknown fields:
///   0-7: next (u64), 8-15: prev (u64), 16-19: diff (i32), 20-23: song/game_id (i32),
///   24-27: playtype (i32), 28-31: uk2 (?), 32-35: score (u32), 36-39: miss_count (u32),
///   40-43: uk3 (?), 44-47: uk4 (?), 48-51: lamp (i32), 52-63: unknown (12 bytes)
fn investigate_datamap_nodes<R: ReadMemory>(
    reader: &R,
    offsets: &OffsetsCollection,
    target_game_id: i32,
) {
    use infst::process::ByteBuffer;

    let data_map_addr = offsets.data_map;
    if data_map_addr == 0 {
        println!("  DataMap address not available");
        return;
    }

    // Read DataMap hash table boundaries
    let Ok(null_obj) = reader.read_u64(data_map_addr.wrapping_sub(16)) else {
        println!("  Failed to read DataMap null_obj");
        return;
    };
    let Ok(start_addr) = reader.read_u64(data_map_addr) else {
        println!("  Failed to read DataMap start");
        return;
    };
    let Ok(end_addr) = reader.read_u64(data_map_addr + 8) else {
        println!("  Failed to read DataMap end");
        return;
    };

    if end_addr <= start_addr {
        println!("  DataMap is empty");
        return;
    }

    let buf_size = (end_addr - start_addr) as usize;
    let Ok(hash_buf) = reader.read_bytes(start_addr, buf_size) else {
        println!("  Failed to read DataMap hash table");
        return;
    };

    // Collect entry points
    let buf = ByteBuffer::new(&hash_buf);
    let mut entry_points = Vec::new();
    for i in 0..(buf_size / 8) {
        let addr = buf.read_u64_at(i * 8).unwrap_or(0);
        if addr != 0 && addr != null_obj && addr != 0x494fdce0 {
            entry_points.push(addr);
        }
    }

    println!(
        "  DataMap: {} buckets, null_obj=0x{:X}",
        entry_points.len(),
        null_obj
    );

    // Follow linked lists and collect raw node data
    let mut target_nodes: Vec<(u64, [u8; 64])> = Vec::new();
    let mut sample_nodes: Vec<(i32, u64, [u8; 64])> = Vec::new();
    let mut visited = std::collections::HashSet::new();

    for &ep in &entry_points {
        let mut current = ep;
        for _ in 0..1000 {
            if visited.contains(&current) || current == 0 || current == null_obj {
                break;
            }
            visited.insert(current);

            let Ok(node_bytes) = reader.read_bytes(current, 64) else {
                break;
            };
            let mut raw = [0u8; 64];
            raw.copy_from_slice(&node_bytes);

            let song_id = i32::from_le_bytes(raw[20..24].try_into().unwrap());

            if song_id == target_game_id {
                target_nodes.push((current, raw));
            } else if sample_nodes.len() < 10
                && (1000..=90000).contains(&song_id)
                && raw[32..36] != [0, 0, 0, 0]
            {
                // Sample some nodes with non-zero scores for comparison
                sample_nodes.push((song_id, current, raw));
            }

            let next = u64::from_le_bytes(raw[0..8].try_into().unwrap());
            if next == 0 || next == null_obj {
                break;
            }
            current = next;
        }
    }

    // Print target nodes
    if target_nodes.is_empty() {
        println!("  No DataMap nodes found for game_id={}", target_game_id);
    } else {
        println!(
            "  Found {} nodes for game_id={}:",
            target_nodes.len(),
            target_game_id
        );
        for (addr, raw) in &target_nodes {
            print_raw_node(*addr, raw, target_game_id);
        }
    }

    // Print sample nodes for comparison
    if !sample_nodes.is_empty() {
        println!();
        println!("  Sample nodes (for comparison):");
        for (song_id, addr, raw) in &sample_nodes {
            print_raw_node(*addr, raw, *song_id);
        }
    }
}

/// Print a raw 64-byte DataMap node with all fields annotated.
fn print_raw_node(addr: u64, raw: &[u8; 64], game_id: i32) {
    let diff = i32::from_le_bytes(raw[16..20].try_into().unwrap());
    let playtype = i32::from_le_bytes(raw[24..28].try_into().unwrap());
    let uk2 = i32::from_le_bytes(raw[28..32].try_into().unwrap());
    let score = u32::from_le_bytes(raw[32..36].try_into().unwrap());
    let miss = u32::from_le_bytes(raw[36..40].try_into().unwrap());
    let uk3 = i32::from_le_bytes(raw[40..44].try_into().unwrap());
    let uk4 = i32::from_le_bytes(raw[44..48].try_into().unwrap());
    let lamp = i32::from_le_bytes(raw[48..52].try_into().unwrap());
    let uk5 = i32::from_le_bytes(raw[52..56].try_into().unwrap());
    let uk6 = i32::from_le_bytes(raw[56..60].try_into().unwrap());
    let uk7 = i32::from_le_bytes(raw[60..64].try_into().unwrap());

    println!(
        "    0x{:X}: gid={} diff={} pt={} | uk2={} score={} miss={} uk3={} uk4={} lamp={} | uk5={} uk6={} uk7={}",
        addr, game_id, diff, playtype, uk2, score, miss, uk3, uk4, lamp, uk5, uk6, uk7
    );

    // Hex dump of the full 64 bytes
    let hex: Vec<String> = raw.iter().map(|b| format!("{:02X}", b)).collect();
    println!("      hex: {}", hex.join(" "));
}

/// Dump a full entry from the entry table for a given internal_id.
fn dump_full_entry<R: ReadMemory>(
    reader: &R,
    entry_base: u64,
    stride: usize,
    target_iid: i32,
    game_id: i32,
) {
    // Find the entry index for this internal_id
    for i in 0..2000u64 {
        let addr = entry_base + i * stride as u64;
        let id = match reader.read_i32(addr) {
            Ok(id) => id,
            Err(_) => break,
        };

        if id == target_iid {
            println!(
                "  Found internal_id={} at index {} (0x{:X})",
                target_iid, i, addr
            );

            // Read full entry
            let Ok(data) = reader.read_bytes(addr, stride) else {
                println!("  Failed to read entry");
                return;
            };

            // Title
            if data.len() >= 0x180 + 64 {
                let title = decode_shift_jis_to_string(&data[0x180..0x180 + 64]);
                println!("  Title: {:?}", title);
            }

            // Search for game_id value within this entry
            let game_id_bytes = game_id.to_le_bytes();
            let mut found_offsets = Vec::new();
            for off in 0..data.len().saturating_sub(4) {
                if data[off..off + 4] == game_id_bytes {
                    found_offsets.push(off);
                }
            }
            if found_offsets.is_empty() {
                println!(
                    "  game_id={} NOT found within this entry's {} bytes",
                    game_id,
                    data.len()
                );
            } else {
                println!(
                    "  game_id={} found at offsets: {:?}",
                    game_id,
                    found_offsets
                        .iter()
                        .map(|o| format!("0x{:03X}", o))
                        .collect::<Vec<_>>()
                );
            }

            // Hexdump the interesting regions (non-zero areas)
            println!("  Non-zero regions:");
            let mut in_nonzero = false;
            let mut region_start = 0;
            for off in (0..data.len()).step_by(4) {
                let is_nonzero = data[off..off.min(data.len()).max(off).min(off + 4).max(off)]
                    .iter()
                    .chain(data.get(off..off + 4).unwrap_or(&[]).iter())
                    .any(|&b| b != 0);
                let chunk = &data[off..(off + 4).min(data.len())];
                let is_nz = chunk.iter().any(|&b| b != 0);
                if is_nz && !in_nonzero {
                    region_start = off;
                    in_nonzero = true;
                } else if !is_nz && in_nonzero {
                    // Dump this non-zero region
                    let region_end = off;
                    hexdump_region_inline(
                        &data[region_start..region_end],
                        addr + region_start as u64,
                        region_start,
                    );
                    in_nonzero = false;
                }
                let _ = is_nonzero; // suppress warning
            }
            if in_nonzero {
                hexdump_region_inline(
                    &data[region_start..],
                    addr + region_start as u64,
                    region_start,
                );
            }

            return;
        }

        if id == 0 {
            continue;
        }
    }
    println!("  internal_id={} not found in entry table", target_iid);
}

/// Hexdump a region with offset annotations
fn hexdump_region_inline(data: &[u8], addr: u64, entry_offset: usize) {
    for row in (0..data.len()).step_by(16) {
        let row_end = (row + 16).min(data.len());
        let hex: Vec<String> = data[row..row_end]
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect();
        let ascii: String = data[row..row_end]
            .iter()
            .map(|&b| {
                if (0x20..0x7F).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!(
            "    0x{:03X} {:012X}: {:48} {}",
            entry_offset + row,
            addr + row as u64,
            hex.join(" "),
            ascii,
        );
    }
}
