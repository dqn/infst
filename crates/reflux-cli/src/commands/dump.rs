//! Dump command implementation.

use anyhow::Result;
use reflux_core::{
    DumpInfo, MemoryReader, OffsetSearcher, ProcessHandle, builtin_signatures, load_offsets,
};

/// Run the dump command
pub fn run(offsets_file: Option<&str>, pid: Option<u32>, output: Option<&str>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Reflux-RS {} - Dump Mode", current_version);

    // Open process
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

    // Load or search for offsets
    let offsets = if let Some(path) = offsets_file {
        load_offsets(path)?
    } else {
        let signatures = builtin_signatures();
        let mut searcher = OffsetSearcher::new(&reader);
        searcher.search_all_with_signatures(&signatures)?
    };

    // Collect dump
    let dump = DumpInfo::collect(&reader, &offsets);

    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&dump)?;
        std::fs::write(output_path, json)?;
        println!("Dump saved to: {}", output_path);
    } else {
        // Print summary to stdout
        println!();
        println!("=== Offsets ===");
        println!("{}", serde_json::to_string_pretty(&dump.offsets)?);

        println!();
        println!("=== Song Entries (first {}) ===", dump.song_entries.len());
        for entry in &dump.song_entries {
            println!(
                "  [{}] 0x{:X}: id={}, folder={}, title={:?}",
                entry.index, entry.address, entry.song_id, entry.folder, entry.title
            );
            if let (Some(meta_id), Some(meta_folder)) =
                (entry.metadata_song_id, entry.metadata_folder)
            {
                println!("       metadata: id={}, folder={}", meta_id, meta_folder);
            }
        }

        if let Some(ref song_list_dump) = dump.song_list_dump {
            println!();
            println!("=== SongList Memory Dump (first 256 bytes) ===");
            for line in song_list_dump.hex_dump.iter().take(16) {
                println!("  {}", line);
            }
        }

        println!();
        println!(
            "=== Detected Songs ({} total) ===",
            dump.detected_songs.len()
        );
        for (i, song) in dump.detected_songs.iter().take(20).enumerate() {
            println!(
                "  [{}] id={}, folder={}, title={:?} ({})",
                i, song.song_id, song.folder, song.title, song.source
            );
        }
        if dump.detected_songs.len() > 20 {
            println!("  ... and {} more", dump.detected_songs.len() - 20);
        }
    }

    Ok(())
}
