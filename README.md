# Reflux-RS

A Rust reimplementation of [Reflux](https://github.com/olji/Reflux), a score tracker for beatmania IIDX INFINITAS.

[日本語版はこちら](README.ja.md)

## Features

- **Memory Reading**: Reads game data directly from the INFINITAS process
- **Score Tracking**: Records play results including judgments, scores, and clear lamps
- **Signature-based Offset Detection**: AOB scan + pattern search for remaining offsets
- **Local Storage**: TSV sessions (default), tracker.db, tracker.tsv, unlockdb
- **Optional Outputs**: JSON sessions, latest-*.txt, and full song info files (config)
- **Remote Sync (optional)**: Custom server API (config)
- **Support Files**: Optional `encodingfixes.txt` / `customtypes.txt`

## Feature Comparison with Original Reflux

| Category | Feature | Original (C#) | This (Rust) | Notes |
|----------|---------|---------------|-------------|-------|
| **Core** | Memory reading | ✅ | ✅ | |
| | Game state detection | ✅ | ✅ | |
| | Auto offset search | ✅ | ✅ | Signature (AOB) + pattern search |
| | Version detection | ✅ | ✅ | |
| **Data** | Play data | ✅ | ✅ | Score, lamp, grade |
| | Judge data | ✅ | ✅ | P1/P2, Fast/Slow |
| | Settings | ✅ | ✅ | Style, gauge, assist, H-RAN |
| | Unlock tracking | ✅ | ✅ | |
| **Storage** | TSV session | ✅ | ✅ | |
| | JSON session | ✅ | ✅ | Optional (config) |
| | Tracker DB | ✅ | ✅ | Best scores |
| | Unlock DB | ✅ | ✅ | |
| **Remote** | Server sync | ✅ | ✅ | |
| | File updates | ✅ | ⚠️ | API exists, CLI does not call it |
| | Kamaitachi | ⚠️ | ⚠️ | Song search helper only (library) |
| **Stream** | playstate/marquee | ✅ | ✅ | |
| | latest-*.txt | ✅ | ✅ | Optional (config) |
| | Song info files | ✅ | ✅ | Optional (config) |
| **Config** | INI config | ✅ | ⚠️ | Parser exists, CLI uses defaults |
| **Extra** | GitHub update check | ❌ | ❌ | Not wired |

✅ = Implemented, ⚠️ = Partial, ❌ = Not implemented

## Requirements

- Windows (uses ReadProcessMemory API)
- Rust 1.85+ (Edition 2024)
- beatmania IIDX INFINITAS

## Installation

### From Source

```bash
git clone https://github.com/dqn/reflux-rs.git
cd reflux-rs
cargo build --release
```

The binary will be at `target/release/reflux.exe`.

## Usage

```bash
# Run with default settings
reflux

# Show help
reflux --help
```

Current CLI notes:
- No CLI flags are defined yet; the binary always uses built-in defaults.
- `config.ini` is **not** read by the CLI (parser exists in the core library).
- Offsets are resolved via built-in signatures; `offsets.txt` is not loaded.

Default file paths used by the CLI:
- Tracker DB: `tracker.db`
- Tracker export: `tracker.tsv` (exported on song select and on disconnect)
- Unlock DB: `unlockdb`
- Sessions: `sessions/Session_YYYY_MM_DD_HH_MM_SS.tsv`
- Optional support files: `encodingfixes.txt`, `customtypes.txt`
- Optional debug output: `songs.tsv` (requires `debug.outputdb = true` in config)

## Configuration (parser exists, CLI does not load it yet)

You can parse this format via `Config::load`, but the current CLI always uses
`Config::default()` and never reads `config.ini`.

```ini
[Update]
updatefiles = true
updateserver = https://raw.githubusercontent.com/olji/Reflux/master/Reflux

[Record]
saveremote = false
savelocal = true
savejson = false
savelatestjson = false
savelatesttxt = false

[RemoteRecord]
serveraddress =
apikey = your-api-key

[LocalRecord]
songinfo = false
chartdetails = false
resultdetails = false
judge = false
settings = false

[Livestream]
playstate = false
marquee = false
fullsonginfo = false
marqueeidletext = INFINITAS

[Debug]
outputdb = false
```

## Offsets

The CLI resolves offsets using built-in code signatures (AOB scan) and pattern
search. It does **not** load `offsets.txt` at runtime. The core library can
parse/save the file if you wire it in.

`offsets.txt` format (first line is version):

```
P2D:J:B:A:2025010100
songList = 0x12345678
dataMap = 0x12345678
judgeData = 0x12345678
playData = 0x12345678
playSettings = 0x12345678
unlockData = 0x12345678
currentSong = 0x12345678
```

On startup, the CLI attempts signature-based detection whenever offsets are invalid.

## Project Structure

```
reflux-rs/
├── Cargo.toml              # Workspace configuration
├── crates/
│   ├── reflux-core/        # Core library
│   │   └── src/
│   │       ├── config/     # INI configuration parser
│   │       ├── game/       # Game data structures
│   │       ├── memory/     # Windows API wrappers
│   │       ├── network/    # HTTP client, Kamaitachi API
│   │       ├── offset/     # Offset management
│   │       ├── storage/    # Local persistence
│   │       ├── stream/     # OBS streaming output
│   │       ├── reflux/     # Main tracker logic
│   │       └── error.rs    # Error types
│   │
│   └── reflux-cli/         # CLI application
│       └── src/main.rs
```

## Output Files

### Session Files

Play data is saved to `sessions/Session_YYYY_MM_DD_HH_MM_SS.tsv`.
When `savejson = true`, a JSON session is also written to
`sessions/Session_YYYY_MM_DD_HH_MM_SS.json`.

### Streaming/OBS Files (only when enabled in config)

| File | Description |
|------|-------------|
| `playstate.txt` | Current state: `menu`, `play`, or `off` |
| `marquee.txt` | Current song title or status |
| `latest.json` | Latest play result (post form JSON) |
| `latest.txt` | Latest play result (3 lines: title/grade/lamp) |
| `latest-grade.txt` | Latest grade (AAA, AA, etc.) |
| `latest-lamp.txt` | Latest clear lamp (expanded name) |
| `latest-difficulty.txt` | Latest difficulty short name |
| `latest-difficulty-color.txt` | Difficulty color code |
| `latest-titleenglish.txt` | English title |
| `title.txt` | Song title |
| `artist.txt` | Artist |
| `englishtitle.txt` | English title |
| `genre.txt` | Genre |
| `folder.txt` | Folder number |
| `level.txt` | Difficulty level |

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=reflux=debug cargo run

# Check code quality
cargo clippy
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Credits

- Original [Reflux](https://github.com/olji/Reflux) by olji
- [Kamaitachi/Tachi](https://github.com/zkrising/Tachi) for score tracking platform
