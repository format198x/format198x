# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's
  `format-sinclair-zx-spectrum-z80` at emu198x/emu198x@1d70a0a216.
- File-shaped `Z80Snapshot`, `Registers` and `Version` types in place of
  Emu198x's shared machine-state `Snapshot`; the extension bytes are kept
  as stored, with accessors for the paging, AY and `$1FFD` fields.
- Encoding for all three versions, compressed or not, which Emu198x never had.

### Fixed

- The hardware mode now follows the published table for version 3: mode 3
  is a 48K with an M.G.T. interface, 6 a 128K with an M.G.T. interface, and
  7 and 8 a +3. Emu198x read 3 as a 128K, and 6 and 7 as a +2A.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-sinclair-zx-spectrum-z80-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Lock Spectrum SOLID criteria; extract SNA and snapshot crates
- Fix two parse_z80 over-reads (lib.rs:149, lib.rs:163)
- Add format-sinclair-zx-spectrum-z80 (.z80 + .sna snapshots)
