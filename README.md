# reflux-rs

A score tracker for beatmania IIDX INFINITAS. This is a Rust port of the original [Reflux](https://github.com/olji/Reflux) (C#).

## Features

- Automatic best score tracking (tracker.db)
- Session-based play records (TSV/JSON)
- OBS streaming integration via text file output

## Requirements

- Windows only
- beatmania IIDX INFINITAS must be running

## Installation

Download the latest release from [Releases](https://github.com/dqn/reflux-rs/releases).

## Usage

1. Start beatmania IIDX INFINITAS
2. Run `reflux.exe`
3. The tracker will automatically detect the game and start tracking

### Logging

Set `RUST_LOG` environment variable to change log level:

```
RUST_LOG=debug reflux.exe
```

## Output Files

### Best Score Tracking

| File | Description |
|------|-------------|
| `tracker.db` | Best scores in custom format (auto-saved) |
| `tracker.tsv` | Best scores exported as TSV |

### Session Records

Session files are created in the current directory:

| File | Description |
|------|-------------|
| `Session_YYYY_MM_DD_HH_MM_SS.tsv` | Play records for the session |
| `Session_YYYY_MM_DD_HH_MM_SS.json` | Play records in JSON format |

## OBS Integration

The following text files are output for OBS overlays:

### Play State

| File | Description |
|------|-------------|
| `playstate.txt` | Current state: `menu`, `play`, or `off` |

### Current Song Info

| File | Description |
|------|-------------|
| `title.txt` | Song title (Japanese) |
| `englishtitle.txt` | Song title (English) |
| `artist.txt` | Artist name |
| `genre.txt` | Genre |
| `level.txt` | Level number |
| `folder.txt` | Folder name |
| `currentsong.txt` | Combined format: `Title [DifficultyLevel]` |

### Latest Result

| File | Description |
|------|-------------|
| `latest.txt` | Title, grade, and lamp (multi-line) |
| `latest-grade.txt` | Grade (e.g., AAA, AA, A) |
| `latest-lamp.txt` | Clear lamp (e.g., FULL COMBO, HARD CLEAR) |
| `latest-difficulty.txt` | Difficulty (e.g., SPA, SPH) |
| `latest-difficulty-color.txt` | Difficulty color code |
| `latest-titleenglish.txt` | English title |
| `latest.json` | Full result data in JSON |

## Optional Support Files

Place these files in the same directory as `reflux.exe`:

### encodingfixes.txt

Fixes character encoding issues in song titles. Format:

```
wrong_string	correct_string
```

### customtypes.txt

Custom song type classifications. Format:

```
song_id	type_name
```

## License

MIT
