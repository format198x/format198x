# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-commodore-c64-d71` at
  emu198x/emu198x@63fac19e77, with no `serde` or `thiserror`.
- Named `format198x-commodore-c128-d71`, for the machine whose drive the
  1571 is, rather than following Emu198x's `c64` segment.
- Read the directory and PRG file chains of D71 images, and read and write single sectors.

Independent versions start at 0.1.0.
