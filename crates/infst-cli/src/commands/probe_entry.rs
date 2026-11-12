//! Probe entry command implementation.
//!
//! Automates the manual memory investigation process for INFINITAS song entries.
//! Given a song_id, searches memory for matching entries, detects the entry stride,
//! and pretty-prints the entry structure with annotated fields.

use anyhow::{Result, bail};
use infst::chart::{EntryLayout, SongInfo};
use infst::{MemoryReader, OffsetSearcher, ProcessHandle, ReadMemory};

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

    let searcher = OffsetSearcher::new(&reader);
    for entry_addr in valid_entries.into_iter().take(10) {
        println!("\n=== Candidate at {entry_addr:#X} ===");
        let Some(stride) = searcher.detect_entry_stride(entry_addr) else {
            println!("Entry stride could not be determined; no layout assumed.");
            continue;
        };
        println!("Observed entry stride: {stride:#X} ({stride} bytes)");
        let sample = reader.read_bytes(entry_addr, stride * 3)?;
        if let Some(layout) = EntryLayout::detect_supported(&sample, stride) {
            println!("Supported profile; three song entries validated.");
            println!("Encoding: {:?}", layout.text_encoding);
            println!(
                "Fields: ID={:#X}, title={:#X}, levels={:#X}",
                layout.song_id, layout.title, layout.levels
            );
            for (index, entry) in sample.chunks_exact(stride).enumerate() {
                if let Some(song) = SongInfo::parse_entry_with_layout(entry, &layout)? {
                    println!(
                        "  [{index}] ID={} title={:?} levels={:?}",
                        song.id, song.title, song.levels
                    );
                }
            }
        } else {
            println!("Unsupported profile: runtime reads will be refused.");
            for known in EntryLayout::supported() {
                println!(
                    "  Known {:?}: stride={:#X}, title={:#X}, levels={:#X}",
                    known.text_encoding, known.entry_size, known.title, known.levels
                );
            }
            if let Some(candidate) = EntryLayout::detect(&sample, stride) {
                println!(
                    "  Unverified field candidates (diagnostics only): ID={:#X}, title={:#X}, levels={:#X}",
                    candidate.song_id, candidate.title, candidate.levels
                );
            }
        }
    }
    Ok(())
}
