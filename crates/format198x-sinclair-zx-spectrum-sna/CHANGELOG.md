# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's
  `format-sinclair-zx-spectrum-sna` at emu198x/emu198x@1d70a0a216.
- File-shaped `Sna`, `Registers` and `Extension128` types in place of
  Emu198x's shared machine-state `Snapshot`, so 48K and 128K snapshots
  (five or six trailing banks) write back byte for byte.
- `Sna::new_48k` pushes the PC as `.sna` writers do; `Sna::pc` and
  `Sna::bank` read the PC and banks back.
- Encoding, which Emu198x never had.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-sinclair-zx-spectrum-sna-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Lock Spectrum SOLID criteria; extract SNA and snapshot crates
