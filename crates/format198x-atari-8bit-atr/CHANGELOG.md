# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-atari-8bit-atr` at
  emu198x/emu198x@1d70a0a216, with no `serde` or `thiserror`.
- Decode ATR images with all three double-density boot-sector layouts, sector reads and writes, and header flags.
- Encode an image back in the layout it was read in, and create blank images.
- Fix: sectors 2 and 3 of a *padded* double-density image now read their own bytes. The Emu198x parser returned boot sector 3 as sector 2.

Independent versions start at 0.1.0.
