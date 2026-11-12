# Supporting INFINITAS memory layouts

Runtime song metadata is defined by `EntryLayout::supported()` in
`crates/infst/src/chart/song/layout.rs`. Synchronization derives its text-table
positions from these same profiles; export and tracking use the entry profiles
directly. Localized text encoding is part of the profile. Romanized titles remain
64-byte Shift-JIS fields.

| Profile | Entry size | Localized encoding | Title | Romanized title | Levels |
| --- | --- | --- | --- | --- | --- |
| V3 | `0x630` | Shift-JIS | `0x180` | `0x200` | `0x360` |
| UTF-16 | `0x730` | UTF-16LE | `0x180` | `0x280` | `0x460` |

A text-table block starts at the romanized title. Its score-map ID belongs to the
next physical entry, while its localized title comes from the preceding block.
Preserve this relationship: internal IDs and score-map IDs are not interchangeable.

## Runtime validation

- Verify three independent populated entries against a supported profile.
- Revalidate cached profiles against live memory. Missing or inconsistent profiles
  trigger offset detection instead of defaulting to V3.
- Validate every populated sync tuple before creating upload entries: strict text
  decoding, ID range and uniqueness, and levels in `0..=12`. Incomplete tuples
  abort the read, rather than producing a partial upload.
- Metadata-only reserved slots with a valid ID/folder but no title or levels are
  allowed at the end of the entry table. They are not exported as songs.
- Unknown entry sizes and unsupported field layouts return errors. Heuristic
  results are diagnostic candidates and are never automatically promoted to
  runtime profiles.

The UTF-16 profile intentionally leaves embedded score/lamp offsets unset because
those fields have not been verified. Scores continue to come from `ScoreMap`. The UTF-16 profile reads chart note
counts at `0x4f0` independently of BPM at `0x478`; it does not infer notes from BPM.
Gameplay-state offsets and the ScoreMap node layout are separate structures;
validating song metadata does not establish compatibility of those structures.
Plausible but semantically changed fields still require comparison with game UI.

## Investigating an update

1. Reach the song selection screen, then run `infst probe-entry --song-id 1001`.
   The command reports every candidate's stride and whether three entries match
   a supported profile. It also reports unverified field candidates for unknown
   structures; unrelated tables can contain the same song ID.
2. Compare candidate metadata with several songs in the game. Verify localized
   titles, chart levels, and the relationship between internal and score-map IDs.
   Use `hexdump` for further inspection; do not treat heuristic output as proof.
3. Add a regression fixture using explicit observed offsets, independently of
   production profile constants. Keep only the metadata needed by the test;
   exclude credentials, personal scores and unrelated memory.
4. Confirm the existing code fails the regression, then add or correct the shared
   profile. Keep unknown-layout and malformed-record tests failing closed.
5. Run `cargo test` and `cargo clippy --all-targets -- -D warnings` on Windows,
   then check `probe-entry`, export, and sync with the game running. Validate
   tracking separately when changing gameplay-state offsets.

Regression coverage lives in `layout_compatibility_tests.rs`,
`text_table_tests.rs`, and `searcher_tests.rs`. It includes both text encodings,
UTF-16 song parsing, unknown strides, corruption after the initial sample,
duplicate/missing IDs, missing titles, and the observed reserved terminal slot.
