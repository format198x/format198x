# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-dragon-disk` at
  emu198x/emu198x@63fac19e77, with no `serde` or `thiserror`.
- Decode and encode VDK images, with sector access and DragonDOS directory lookup.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-dragon-disk-v0.2.0) - 2026-06-04

### Other

- *(release)* independent per-machine versioning, baseline 0.2.0
- Open Emu198x for public release
- Add Dragon runtime snapshots
- Share DragonDOS directory entry lookup
- Add Dragon VDK sidecar export
- Support DragonDOS sector writes
- Add initial DragonDOS VDK disk support
