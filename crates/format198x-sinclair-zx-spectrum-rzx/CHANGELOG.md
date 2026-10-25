# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's
  `format-sinclair-zx-spectrum-rzx` at emu198x/emu198x@63fac19e77.
- Blocks are kept in file order, so files with several snapshot and input
  blocks write back as read. Emu198x kept only the last snapshot and wrote
  blocks back grouped by kind.
- Creator custom data and the input block's reserved byte are kept.

### Changed

- Compressed snapshot data and frame streams are kept as stored rather than
  inflated, since the crate has no zlib. `InputBlock::frames` decodes an
  uncompressed stream and `decode_frames` an inflated one.

Independent versions start at 0.1.0; older entries below record Emu198x releases.

## [0.2.0](https://github.com/emu198x/emu198x/releases/tag/format-sinclair-zx-spectrum-rzx-v0.2.0) - 2026-06-04

### Added

- pull RZX format from Older + close out Older
