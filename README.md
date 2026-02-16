# infst

[[日本語](README.ja.md)]

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Release](https://img.shields.io/github/v/release/dqn/reflux-rs)](https://github.com/dqn/reflux-rs/releases)

A real-time score tracker for beatmania IIDX INFINITAS.

## Features

- Automatically tracks play data in real-time
- Exports scores in TSV/JSON format
- Syncs play data directly to the web service

## Requirements

- Windows only
- beatmania IIDX INFINITAS installed

## Installation

1. Download `infst.exe` from [GitHub Releases](https://github.com/dqn/reflux-rs/releases)
2. Place the executable anywhere you like

## Usage

### Tracking

Run with INFINITAS open:

```bash
infst
```

Your plays are automatically recorded while the tracker is running.

### Export Data

Export all your play data (scores, lamps, miss counts, DJ points, etc.):

```bash
# Export to TSV (default)
infst export -o scores.tsv

# Export to JSON
infst export -o scores.json -f json

# Output to stdout
infst export
```

#### Options

| Option | Description |
|--------|-------------|
| `-o, --output` | Output file path (stdout if omitted) |
| `-f, --format` | Output format: `tsv` (default) / `json` |

### Sync Data

Sync all play data directly from game memory to the web service:

```bash
# Login first (one-time setup)
infst login

# Sync all play data
infst sync
```

#### Options

| Option | Description |
|--------|-------------|
| `--endpoint` | API endpoint URL (env: `INFST_API_ENDPOINT`) |
| `--token` | API token (env: `INFST_API_TOKEN`) |
| `--pid` | Process ID (auto-detected if omitted) |

## License

[MIT License](LICENSE)
