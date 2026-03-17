//! Probe entry command implementation.
//!
//! Automates the manual memory investigation process for INFINITAS song entries.
//! Given a song_id, searches memory for matching entries, detects the entry stride,
//! and pretty-prints the entry structure with annotated fields.

use anyhow::{Result, bail};
use infst::chart::SongInfo;
use infst::process::decode_shift_jis_to_string;
use infst::{MemoryReader, ProcessHandle, ReadMemory};

/// Run the probe-entry command
pub fn run(song_id: i32, pid: Option<u32>) -> Result<()> {
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

    // Search for song_id in memory
    let search_bytes = song_id.to_le_bytes();
    let search_start = process.base_address + 0x1000000;
    let search_end = process.base_address + (process.module_size as u64).min(0x5000000);
    let chunk_size: usize = 4 * 1024 * 1024;

    println!(
        "Searching for song_id={} in range 0x{:X} - 0x{:X}",
        song_id, search_start, search_end
    );

    let mut candidates: Vec<u64> = Vec::new();
    let mut offset = 0u64;

    while search_start + offset < search_end {
        let addr = search_start + offset;
        let read_size = chunk_size.min((search_end - addr) as usize);

        if let Ok(buffer) = reader.read_bytes(addr, read_size) {
            for i in 0..=(buffer.len().saturating_sub(search_bytes.len())) {
                if buffer[i..i + 4] == search_bytes {
                    candidates.push(addr + i as u64);
                }
            }
        }

        offset += chunk_size as u64;
    }

    println!(
        "Found {} raw matches for value {}",
        candidates.len(),
        song_id
    );

    // Filter: check each candidate looks like a valid song entry
    // offset 0 = song_id, offset 4 = folder (1-200)
    let mut valid_entries: Vec<u64> = Vec::new();

    for &addr in &candidates {
        if let Ok(bytes) = reader.read_bytes(addr, 8) {
            let id = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let folder = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

            if id == song_id && (1..=200).contains(&folder) {
                valid_entries.push(addr);
            }
        }
    }

    if valid_entries.is_empty() {
        bail!(
            "No valid song entry found for song_id={}. Found {} raw matches but none had a valid folder (1-200) at offset +4.",
            song_id,
            candidates.len()
        );
    }

    println!(
        "Found {} valid entry candidate(s) with [song_id={}, folder=1..200]",
        valid_entries.len(),
        song_id
    );
    println!();

    let entry_addr = valid_entries[0];
    println!("Found song_id={} at 0x{:X}", song_id, entry_addr);

    // Auto-detect entry stride
    let stride = detect_entry_stride(&reader, entry_addr);

    let stride_value = stride.unwrap_or(SongInfo::MEMORY_SIZE);
    println!();
    println!(
        "=== Entry Structure (detected stride: 0x{:X} = {} bytes) ===",
        stride_value, stride_value
    );

    if stride.is_none() {
        println!(
            "  (stride detection failed, using default SongInfo::MEMORY_SIZE = 0x{:X})",
            SongInfo::MEMORY_SIZE
        );
    }

    // Read full entry
    let entry_size = stride_value.max(0x470); // Read at least enough for all known fields
    if let Ok(data) = reader.read_bytes(entry_addr, entry_size) {
        print_entry_structure(&data, song_id);
    } else {
        println!("  Failed to read entry data at 0x{:X}", entry_addr);
    }

    // Show neighboring entries
    println!();
    println!("=== Neighbors ===");

    // Previous entry
    if entry_addr >= stride_value as u64 {
        let prev_addr = entry_addr - stride_value as u64;
        print_neighbor(&reader, prev_addr, "prev");
    }

    // Next entry
    let next_addr = entry_addr + stride_value as u64;
    print_neighbor(&reader, next_addr, "next");

    // If there are other valid candidates, list them
    if valid_entries.len() > 1 {
        println!();
        println!("=== Other candidates ===");
        for (i, &addr) in valid_entries.iter().enumerate().skip(1).take(5) {
            if let Ok(bytes) = reader.read_bytes(addr, 8) {
                let folder = i32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                println!("  [{}] 0x{:X} folder={}", i, addr, folder);
            }
        }
        if valid_entries.len() > 6 {
            println!("  ... and {} more", valid_entries.len() - 6);
        }
    }

    Ok(())
}

/// Auto-detect entry stride by scanning forward from a known entry address.
///
/// Uses the same algorithm as `OffsetSearcher::detect_entry_stride`:
/// scans for the next valid [song_id(1000-50000), folder(1-200)] pair
/// at 16-byte aligned offsets starting from 0x200 away, then validates
/// with multiple entries.
fn detect_entry_stride<R: ReadMemory>(reader: &R, entry_addr: u64) -> Option<usize> {
    let scan_size = 0x4000; // 16KB
    let buffer = reader.read_bytes(entry_addr, scan_size).ok()?;

    if buffer.len() < 8 {
        return None;
    }

    // Verify first entry has valid song_id and folder
    let first_id = i32::from_le_bytes(buffer[0..4].try_into().ok()?);
    let first_folder = i32::from_le_bytes(buffer[4..8].try_into().ok()?);

    if !(1000..=50000).contains(&first_id) || !(1..=200).contains(&first_folder) {
        return None;
    }

    // Scan for next valid [song_id, folder] pair at 16-byte aligned offsets
    for offset in (0x200..scan_size.saturating_sub(8)).step_by(0x10) {
        let sid = i32::from_le_bytes(buffer[offset..offset + 4].try_into().ok()?);
        let fld = i32::from_le_bytes(buffer[offset + 4..offset + 8].try_into().ok()?);

        if !(1000..=50000).contains(&sid) || !(1..=200).contains(&fld) {
            continue;
        }

        let stride = offset;

        // Validate: check several more entries at this stride
        let mut valid_count = 2; // first entry + this match
        for i in 2..20 {
            let check_off = stride * i;
            if check_off + 8 > buffer.len() {
                break;
            }
            let check_id = i32::from_le_bytes(buffer[check_off..check_off + 4].try_into().ok()?);
            let check_fld =
                i32::from_le_bytes(buffer[check_off + 4..check_off + 8].try_into().ok()?);

            if (1000..=50000).contains(&check_id) && (1..=200).contains(&check_fld) {
                valid_count += 1;
            }
        }

        if valid_count >= 3 {
            return Some(stride);
        }
    }

    None
}

/// Pretty-print the entry structure with annotated fields.
fn print_entry_structure(data: &[u8], song_id: i32) {
    // Header
    println!();
    println!("Header:");
    let folder = read_i32(data, 0x004);
    println!("  0x000 song_id:     {}", song_id);
    println!("  0x004 folder:      {}", folder);

    // Identifier (raw bytes at 0x008, 22 bytes)
    if data.len() >= 0x008 + 22 {
        let id_bytes = &data[0x008..0x008 + 22];
        let hex: Vec<String> = id_bytes.iter().map(|b| format!("{:02X}", b)).collect();
        println!("  0x008 identifier:  {}", hex.join(" "));
    }

    // Text fields
    println!();
    println!("Text fields:");
    print_shift_jis_field(data, 0x180, 64, "title");
    print_shift_jis_field(data, 0x1C0, 64, "unknown_1");
    print_shift_jis_field(data, 0x200, 64, "title_en");
    print_shift_jis_field(data, 0x240, 64, "genre");
    print_shift_jis_field(data, 0x280, 64, "unknown_2");
    print_shift_jis_field(data, 0x2C0, 64, "artist");

    // Metadata
    println!();
    println!("Metadata:");

    // Levels at 0x360 (10 bytes)
    if data.len() >= 0x360 + 10 {
        let levels: Vec<String> = data[0x360..0x360 + 10]
            .iter()
            .map(|&b| {
                if b == 0 {
                    "_".to_string()
                } else {
                    b.to_string()
                }
            })
            .collect();
        println!("  0x360 levels:      [{}]", levels.join(","));
    }

    // Total notes at 0x378 (10 x u32, 8-byte stride)
    if data.len() >= 0x378 + 10 * 8 {
        let mut notes = Vec::new();
        for i in 0..10 {
            let off = 0x378 + i * 8;
            notes.push(read_u32(data, off));
        }
        let notes_str: Vec<String> = notes.iter().map(|n| n.to_string()).collect();
        println!("  0x378 notes:       [{}]", notes_str.join(","));
    }

    // Score data
    println!();
    println!("Score data:");

    // EX scores at 0x3F0 (10 x u32)
    if data.len() >= 0x3F0 + 10 * 4 {
        let mut scores = Vec::new();
        for i in 0..10 {
            let off = 0x3F0 + i * 4;
            scores.push(read_u32(data, off));
        }
        let scores_str: Vec<String> = scores.iter().map(|s| s.to_string()).collect();
        println!("  0x3F0 ex_scores:   [{}]", scores_str.join(","));
    }

    // Clear lamps at 0x430 (first 10 u32 values)
    if data.len() >= 0x430 + 10 * 4 {
        let mut lamps = Vec::new();
        for i in 0..10 {
            let off = 0x430 + i * 4;
            lamps.push(read_u32(data, off));
        }
        let lamps_str: Vec<String> = lamps.iter().map(|l| l.to_string()).collect();
        println!("  0x430 lamps:       [{}]", lamps_str.join(","));
    }
}

/// Print a neighboring entry (just song_id and title).
fn print_neighbor<R: ReadMemory>(reader: &R, addr: u64, label: &str) {
    if let Ok(bytes) = reader.read_bytes(addr, 0x200) {
        let id = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        // Read title at 0x180
        if bytes.len() >= 0x180 + 64 {
            let title = decode_shift_jis_to_string(&bytes[0x180..0x180 + 64]);
            println!("  {}: 0x{:X} song_id={} {:?}", label, addr, id, title);
        } else {
            println!("  {}: 0x{:X} song_id={}", label, addr, id);
        }
    } else {
        println!("  {}: 0x{:X} (read failed)", label, addr);
    }
}

/// Print a Shift-JIS string field at the given offset.
fn print_shift_jis_field(data: &[u8], offset: usize, size: usize, name: &str) {
    if data.len() >= offset + size {
        let s = decode_shift_jis_to_string(&data[offset..offset + size]);
        println!("  0x{:03X} {:12}{:?}", offset, format!("{}:", name), s);
    }
}

/// Read a little-endian i32 from a byte slice at the given offset.
fn read_i32(data: &[u8], offset: usize) -> i32 {
    if data.len() >= offset + 4 {
        i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    } else {
        0
    }
}

/// Read a little-endian u32 from a byte slice at the given offset.
fn read_u32(data: &[u8], offset: usize) -> u32 {
    if data.len() >= offset + 4 {
        u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ])
    } else {
        0
    }
}
