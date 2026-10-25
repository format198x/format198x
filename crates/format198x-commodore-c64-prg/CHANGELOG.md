# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-commodore-c64-prg` at
  emu198x/emu198x@b5f8fe59ce, with no `serde` or `thiserror`.
- Decode and encode PRG files. A two-byte file is a valid PRG with an empty payload; the Emu198x parser required three bytes.
- The RAM loader and BASIC relinking from Emu198x's crate stay in Emu198x, as machine behaviour.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-commodore-c64-prg-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Add C64 host-side program import
