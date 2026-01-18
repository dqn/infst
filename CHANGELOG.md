# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Current behavior (0.1.19)

- CLI has no flags; it always uses built-in defaults (`Config::default()`).
- Offsets are detected via built-in signatures (AOB) plus pattern fallback; `offsets.txt` is not loaded.
- Default outputs: `sessions/Session_*.tsv`, `tracker.db`, `tracker.tsv`, `unlockdb`.
- Optional outputs behind config flags: JSON sessions, OBS/latest files, remote sync.
- Optional support files: `encodingfixes.txt`, `customtypes.txt`.

## [0.1.0] - 2025-01-14

### Added

- Initial public release.

[Unreleased]: https://github.com/dqn/reflux-rs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/dqn/reflux-rs/releases/tag/v0.1.0
