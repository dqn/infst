//! Validate command implementation.

use anyhow::Result;
use reflux_core::{MemoryReader, ProcessHandle, ReadMemory};

use super::offset::parse_hex_address;
use crate::cli::ValidateTarget;

/// Run the validate command
pub fn run(target: ValidateTarget) -> Result<()> {
    match target {
        ValidateTarget::SongEntry { address, pid } => {
            let addr = parse_hex_address(&address)?;
            run_validate_song_entry(addr, pid)
        }
    }
}

/// Validate a song entry structure
fn run_validate_song_entry(address: u64, pid: Option<u32>) -> Result<()> {
    let process = if let Some(pid) = pid {
        ProcessHandle::open(pid)?
    } else {
        ProcessHandle::find_and_open()?
    };

    let reader = MemoryReader::new(&process);

    // Read entry data (1200 bytes for new structure)
    const ENTRY_SIZE: usize = 1200;
    let data = reader.read_bytes(address, ENTRY_SIZE)?;

    println!("=== Song Entry Validation ===");
    println!("Address: 0x{:X}", address);
    println!("Entry size: {} bytes (0x{:X})", ENTRY_SIZE, ENTRY_SIZE);
    println!();
    println!("Fields:");

    // Helper to decode Shift-JIS string
    let decode_string = |offset: usize, max_len: usize| -> String {
        let slice = &data[offset..offset + max_len];
        let len = slice.iter().position(|&b| b == 0).unwrap_or(max_len);
        if len == 0 {
            return "(empty)".to_string();
        }
        let (decoded, _, _) = encoding_rs::SHIFT_JIS.decode(&slice[..len]);
        decoded.trim().to_string()
    };

    // Helper to check if field looks valid
    let check_string = |s: &str| -> &str {
        if s == "(empty)" || s.chars().any(|c| c.is_control() && c != '\n' && c != '\r') {
            "?"
        } else {
            "✓"
        }
    };

    // Title at offset 0
    let title = decode_string(0, 64);
    println!("  title     @    0: {:?} {}", title, check_string(&title));

    // Title English at offset 64
    let title_en = decode_string(64, 64);
    println!(
        "  title_en  @   64: {:?} {}",
        title_en,
        check_string(&title_en)
    );

    // Genre at offset 128
    let genre = decode_string(128, 64);
    println!("  genre     @  128: {:?} {}", genre, check_string(&genre));

    // Artist at offset 192
    let artist = decode_string(192, 64);
    println!("  artist    @  192: {:?} {}", artist, check_string(&artist));

    // Levels at offset 480
    let levels: Vec<u8> = data[480..490].to_vec();
    let levels_valid = levels.iter().all(|&l| l <= 12);
    println!(
        "  levels    @  480: {:?} {}",
        levels,
        if levels_valid { "✓" } else { "?" }
    );

    // Song ID at offset 816
    let song_id = i32::from_le_bytes([data[816], data[817], data[818], data[819]]);
    let song_id_valid = (1000..=90000).contains(&song_id);
    println!(
        "  song_id   @  816: {} {}",
        song_id,
        if song_id_valid { "✓" } else { "?" }
    );

    // Folder at offset 820
    let folder = i32::from_le_bytes([data[820], data[821], data[822], data[823]]);
    let folder_valid = (1..=200).contains(&folder);
    println!(
        "  folder    @  820: {} {}",
        folder,
        if folder_valid { "✓" } else { "?" }
    );

    // Total notes at offset 500 (10 x u16)
    let total_notes: Vec<u16> = (0..10)
        .map(|i| {
            let off = 500 + i * 2;
            u16::from_le_bytes([data[off], data[off + 1]])
        })
        .collect();
    println!("  notes     @  500: {:?}", total_notes);

    // BPM at offset 256
    let bpm = decode_string(256, 64);
    println!("  bpm       @  256: {:?}", bpm);

    println!();

    // Overall validation
    let valid = !title.is_empty() && title != "(empty)" && song_id_valid && levels_valid;
    println!(
        "Overall: {}",
        if valid {
            "Valid song entry"
        } else {
            "Invalid or unknown structure"
        }
    );

    Ok(())
}
