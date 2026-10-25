# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Crate graduated from Emu198x's
  `format-sinclair-zx-spectrum-rzx` at emu198x/emu198x@63fac19e77.
- Blocks are kept in file order, so files with several snapshot and input
  blocks write back as read. Emu198x kept only the last snapshot and wrote
  blocks back grouped by kind.
- Creator custom data and the input block's reserved byte are kept.
- Compressed snapshots and frame streams inflate with `format198x-deflate`,
  the crate's one dependency: `SnapshotBlock::snapshot`,
  `InputBlock::frame_stream` and `InputBlock::frames`. They are written by
  `SnapshotBlock::compressed` and `InputBlock::from_frames_compressed`.

### Changed

- Compressed snapshot data and frame streams are kept as stored and inflated
  on request, so files write back byte for byte. A compressed snapshot must
  inflate to its declared length.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-sinclair-zx-spectrum-rzx-v0.2.0) - 2026-06-04

### Added

- pull RZX format from Older + close out Older
