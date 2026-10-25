# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-commodore-c64-d64` at
  emu198x/emu198x@63fac19e77, with no `serde` or `thiserror`.
- Read the BAM, directory and PRG file chains of standard D64 images, with or without error info.
- Read and write single sectors in place.
- Master a freshly formatted disk (DOS format `A`) holding PRG files.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-commodore-c64-d64-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Tighten 1541 IEC behavior and trace helpers
- Add C64 D64 container import support
