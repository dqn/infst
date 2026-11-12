use infst::chart::read_text_table;
use infst::process::MockMemoryReader;

#[test]
fn reads_utf16_titles_with_matching_levels_and_score_map_ids() {
    let mut memory = vec![0; 0x730 * 5000];
    // Measured from the November 2026 table. Each ID belongs to the title
    // in the previous block, while levels are in the current block.
    for (i, (title, id)) in [
        ("GRADIUSIC CYBER", 1001u32),
        ("R3", 1003),
        ("青龍 Ω ", 1005),
    ]
    .into_iter()
    .enumerate()
    {
        let previous = i * 0x730;
        let current = previous + 0x730;
        let encoded: Vec<u8> = title.encode_utf16().flat_map(u16::to_le_bytes).collect();
        memory[previous + 0x630..previous + 0x630 + encoded.len()].copy_from_slice(&encoded);
        memory[current..current + 4].copy_from_slice(b"Song");
        memory[current + 0x1e0..current + 0x1ea]
            .copy_from_slice(&[0, 4, 7, 11, 12, 0, 5, 10, 12, 0]);
        memory[current + 0x4b0..current + 0x4b4].copy_from_slice(&id.to_le_bytes());
    }
    let entries = read_text_table(&MockMemoryReader::new(memory), 0x1000).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].title, "GRADIUSIC CYBER");
    assert_eq!(entries[0].game_id, 1001);
    assert_eq!(entries[1].title, "R3");
    assert_eq!(entries[1].game_id, 1003);
    assert_eq!(entries[2].title, "青龍 Ω");
    assert_eq!(entries[2].game_id, 1005);
    assert_eq!(entries[2].levels, [0, 4, 7, 11, 12, 0, 5, 10, 12, 0]);
}

#[test]
fn preserves_shift_jis_table_support() {
    let mut memory = vec![0; 0x630 * 5000];
    for (i, title) in ["GRADIUSIC CYBER", "R3", "THE EARTH LIGHT"]
        .into_iter()
        .enumerate()
    {
        let previous = i * 0x630;
        let current = previous + 0x630;
        memory[previous + 0x5b0..previous + 0x5b0 + title.len()].copy_from_slice(title.as_bytes());
        memory[current + 0x160..current + 0x16a]
            .copy_from_slice(&[0, 4, 7, 11, 0, 0, 5, 10, 12, 0]);
        memory[current + 0x430..current + 0x434]
            .copy_from_slice(&(1001u32 + i as u32).to_le_bytes());
    }
    let entries = read_text_table(&MockMemoryReader::new(memory), 0x1000).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].title, "GRADIUSIC CYBER");
    assert_eq!(entries[0].game_id, 1001);
    assert_eq!(entries[2].title, "THE EARTH LIGHT");
    assert_eq!(entries[2].levels[8], 12);
}

#[test]
fn rejects_uninitialized_table() {
    let reader = MockMemoryReader::new(vec![0; 0x730 * 5000]);
    assert!(read_text_table(&reader, 0x1000).is_err());
}
