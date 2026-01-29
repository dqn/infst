//! Export command for exporting play data.

use std::collections::HashMap;

use anyhow::Result;
use reflux_core::{
    CustomTypes, EncodingFixes, MemoryReader, OffsetSearcher, ProcessHandle, ScoreMap,
    builtin_signatures, fetch_song_database_with_fixes, generate_tracker_json,
    generate_tracker_tsv, get_unlock_states,
};

use crate::cli::ExportFormat;

/// Export all play data
pub fn run(output: Option<&str>, format: ExportFormat, pid: Option<u32>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    eprintln!("Reflux-RS {} - Export Mode", current_version);

    // Open process
    let process = if let Some(pid) = pid {
        ProcessHandle::open(pid)?
    } else {
        ProcessHandle::find_and_open()?
    };

    eprintln!(
        "Found process (PID: {}, Base: 0x{:X})",
        process.pid, process.base_address
    );

    let reader = MemoryReader::new(&process);

    // Search for offsets using builtin signatures
    let signatures = builtin_signatures();
    let mut searcher = OffsetSearcher::new(&reader);
    let offsets = searcher.search_all_with_signatures(&signatures)?;

    eprintln!("Offsets detected");

    // Load encoding fixes (optional)
    let encoding_fixes = match EncodingFixes::load("encodingfixes.txt") {
        Ok(ef) => {
            eprintln!("Loaded {} encoding fixes", ef.len());
            Some(ef)
        }
        Err(_) => None,
    };

    // Load song database
    eprintln!("Loading song database...");
    let song_db =
        fetch_song_database_with_fixes(&reader, offsets.song_list, encoding_fixes.as_ref())?;
    eprintln!("Loaded {} songs", song_db.len());

    // Load unlock data
    eprintln!("Loading unlock data...");
    let unlock_db = get_unlock_states(&reader, offsets.unlock_data, &song_db)?;
    eprintln!("Loaded {} unlock entries", unlock_db.len());

    // Load score map
    eprintln!("Loading score data...");
    let score_map = ScoreMap::load_from_memory(&reader, offsets.data_map, &song_db)?;
    eprintln!("Loaded {} score entries", score_map.len());

    // Load custom types (optional, for TSV format)
    let custom_types: HashMap<u32, String> = match CustomTypes::load("customtypes.txt") {
        Ok(ct) => {
            let mut types = HashMap::new();
            for (k, v) in ct.iter() {
                if let Ok(id) = k.parse::<u32>() {
                    types.insert(id, v.clone());
                }
            }
            eprintln!("Loaded {} custom types", types.len());
            types
        }
        Err(_) => HashMap::new(),
    };

    // Generate output based on format
    let content = match format {
        ExportFormat::Tsv => generate_tracker_tsv(&song_db, &unlock_db, &score_map, &custom_types),
        ExportFormat::Json => generate_tracker_json(&song_db, &unlock_db, &score_map)?,
    };

    // Write output
    if let Some(output_path) = output {
        std::fs::write(output_path, &content)?;
        eprintln!("Exported to: {}", output_path);
    } else {
        println!("{}", content);
    }

    Ok(())
}
