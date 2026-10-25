# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-jupiter-ace-ace` at
  emu198x/emu198x@15e4c8210d, with no `serde` or `thiserror`.
- Decode `.ace` snapshots: RLE stream, the three legal RAM sizes, and the register block.
- Encode snapshots and raw RLE streams.

Independent versions start at 0.1.0.
