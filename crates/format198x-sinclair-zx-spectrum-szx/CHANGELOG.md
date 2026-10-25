# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free SZX container graduated from Emu198x's
  `format-sinclair-zx-spectrum-szx` at emu198x/emu198x@1d70a0a216: lossless
  chunk reading and writing, with views and constructors for `Z80R`
  (including its T-state tail), `SPCR`, `RAMP` and `AY`.
- Encoding, which Emu198x never had.

### Changed

- Compressed `RAMP` pages are kept as stored rather than inflated, since the
  crate has no zlib. Machine ids and bank numbers are reported raw; mapping
  them to a machine stays in Emu198x.

Independent versions start at 0.1.0. Emu198x's crate never kept a changelog.
