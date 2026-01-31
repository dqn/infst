//! Analyze command implementation.
//!
//! Memory structure analysis mode for debugging and reverse engineering
//! the INFINITAS memory layout.

mod id_search;
mod pattern_search;
mod song_counter;
mod title_search;

use anyhow::{Result, bail};
use reflux_core::{MemoryReader, OffsetSearcher, ProcessHandle, SongInfo};

use super::hex_utils::parse_hex_address;
use pattern_search::search_song_patterns;
use song_counter::{count_songs_new_structure, count_songs_old_structure};
use title_search::search_for_title_strings;

/// Run the memory structure analysis mode.
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
        parse_hex_address(&addr_str)?
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
