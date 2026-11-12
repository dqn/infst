use infst::chart::{EntryLayout, SongInfo};

// Independent fixture offsets from the November 2026 memory inspection.
fn utf16_entries() -> Vec<u8> {
    let mut memory = vec![0; 0x730 * 3];
    for (i, title) in ["R3", "THE EARTH LIGHT", "青龍 Ω"].into_iter().enumerate() {
        let entry = &mut memory[i * 0x730..(i + 1) * 0x730];
        entry[..4].copy_from_slice(&(1001u32 + i as u32).to_le_bytes());
        entry[4..8].copy_from_slice(&43u32.to_le_bytes());
        for (offset, text) in [(0x180, title), (0x2c0, "RAVE"), (0x340, "tiger YAMATO")] {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
            entry[offset..offset + bytes.len()].copy_from_slice(&bytes);
        }
        entry[0x280..0x282].copy_from_slice(b"R3");
        entry[0x460..0x46a].copy_from_slice(&[0, 4, 5, 11, 0, 0, 4, 6, 12, 0]);
        // R3's observed BPM and per-chart notes are separate fields.
        for chart in [1, 2, 3, 6, 7, 8] {
            entry[0x478 + chart * 8..0x47c + chart * 8].copy_from_slice(&157u32.to_le_bytes());
        }
        for (chart, notes) in [
            (1, 330u32),
            (2, 368),
            (3, 454),
            (6, 384),
            (7, 392),
            (8, 475),
        ] {
            entry[0x4f0 + chart * 4..0x4f4 + chart * 4].copy_from_slice(&notes.to_le_bytes());
        }
    }
    memory
}

#[test]
fn reads_utf16_metadata_through_shared_song_parser() {
    let bytes = utf16_entries();
    let layout = EntryLayout::detect(&bytes, 0x730).expect("supported UTF-16 layout");
    let song = SongInfo::parse_from_buffer_with_layout(&bytes, 2 * 0x730, &layout)
        .unwrap()
        .unwrap();
    assert_eq!(song.title.as_ref(), "青龍 Ω");
    assert_eq!(song.title_english.as_ref(), "R3");
    assert_eq!(song.genre.as_ref(), "RAVE");
    assert_eq!(song.artist.as_ref(), "tiger YAMATO");
    assert_eq!(song.levels[3], 11);
    assert_eq!(song.levels[8], 12);
}

#[test]
fn unknown_stride_does_not_fall_back_to_v3_metadata() {
    let mut memory = vec![0; 0x740 * 5000];
    for i in 0..3 {
        let entry = &mut memory[i * 0x740..(i + 1) * 0x740];
        entry[..4].copy_from_slice(&(1001u32 + i as u32).to_le_bytes());
        entry[4..8].copy_from_slice(&43u32.to_le_bytes());
        entry[0x180..0x182].copy_from_slice(b"R3");
        entry[0x363] = 11;
    }
    let reader = infst::process::MockMemoryReader::new(memory);
    assert!(infst::chart::fetch_song_database_bulk(&reader, 0x1000, 0x740).is_err());
}

#[test]
fn offset_detection_resolves_the_utf16_entry_table() {
    let mut memory = vec![0; 12 * 1024 * 1024];
    for i in 0..1100 {
        let base = 0x1000 + i * 0x730;
        memory[base..base + 4].copy_from_slice(&(1001u32 + i as u32).to_le_bytes());
        memory[base + 4..base + 8].copy_from_slice(&43u32.to_le_bytes());
        memory[base + 0x180..base + 0x184].copy_from_slice(b"R\x003\x00");
        memory[base + 0x280..base + 0x286].copy_from_slice(if i == 0 {
            b"5.1.1."
        } else {
            b"SONG  "
        });
        memory[base + 0x463] = 11;
    }
    // The terminal reserved entry supplies the last populated title's score-map ID.
    let terminal = 0x1000 + 1100 * 0x730;
    memory[terminal..terminal + 4].copy_from_slice(&80127u32.to_le_bytes());
    memory[terminal + 4..terminal + 8].copy_from_slice(&76u32.to_le_bytes());
    let reader = infst::process::MockMemoryReader::with_base(memory, 0x140000000);
    let mut searcher = infst::OffsetSearcher::builder(&reader)
        .with_song_list_hint(0x140001280)
        .build();
    let offsets = searcher.search_song_list_only().unwrap();
    assert_eq!(offsets.song_db_address(), 0x140001000);
    assert_eq!(offsets.entry_stride(), 0x730);
    let song = SongInfo::read_from_memory_with_layout(
        &reader,
        offsets.song_db_address(),
        offsets.entry_layout.as_ref().unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(song.title.as_ref(), "R3");
}

#[test]
fn permits_unpopulated_reserved_entry_after_loaded_songs() {
    let mut bytes = utf16_entries();
    bytes.resize(0x730 * 5000, 0);
    // Observed final reserved slot: ID=80127, folder=76, no title or charts.
    bytes[3 * 0x730..3 * 0x730 + 4].copy_from_slice(&80127u32.to_le_bytes());
    bytes[3 * 0x730 + 4..3 * 0x730 + 8].copy_from_slice(&76u32.to_le_bytes());
    let reader = infst::process::MockMemoryReader::new(bytes);
    let db = infst::chart::fetch_song_database_bulk(&reader, 0x1000, 0x730).unwrap();
    assert_eq!(db.len(), 3);
    assert_eq!(db[&1003].title.as_ref(), "青龍 Ω");
}

#[test]
fn utf16_metadata_separates_bpm_from_chart_note_counts() {
    let bytes = utf16_entries();
    let layout = EntryLayout::detect(&bytes, 0x730).unwrap();
    let song = SongInfo::parse_from_buffer_with_layout(&bytes, 0, &layout)
        .unwrap()
        .unwrap();
    assert_eq!(song.bpm.as_ref(), "157");
    assert_eq!(song.total_notes, [0, 330, 368, 454, 0, 0, 384, 392, 475, 0]);
}
