# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/format198x/format198x/compare/format198x-amstrad-cpc-dsk-v0.1.1...format198x-amstrad-cpc-dsk-v0.1.2) - 2026-10-28

### Added

- expose a checked DSK container with physical track metadata

## [0.1.1](https://github.com/format198x/format198x/compare/format198x-amstrad-cpc-dsk-v0.1.0...format198x-amstrad-cpc-dsk-v0.1.1) - 2026-10-16

### Added

- Encode a sector image as Extended DSK while preserving physical order,
  CHRN, status flags and exact stored data lengths, including zero-length sectors.
- Reject inconsistent geometry and overflowing tables or tracks instead of
  truncating data. A conversion example creates a new output file only.

The sector model is unchanged. Original gap/filler, recording mode/rate and
container padding are not preserved; output uses documented defaults. Runtime
export and fresh-machine reopening require separate consumer verification.

## [0.1.0] - 2026-10-14

### Added

- Dependency-free DSK/EDSK reader graduated from Emu198x with its history.
- Standard 180 KiB +3DOS mastering, BASIC/CODE headers and multi-extent files.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-amstrad-dsk-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Restore CI: cargo fmt + clippy --all-targets clean across the workspace
- Carry per-sector ST1/ST2 + DDAM through the EDSK pipeline
- Chase H.Q. (+3) title screen now loads end-to-end
- commit mechanical cleanup across diagnostics
- Run rustfmt across the workspace
- Add ZX Spectrum +2A/+2B/+3 + Amstrad 40077 + NEC µPD765A + DSK
