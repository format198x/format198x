# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-nintendo-snes-sfc`
  at emu198x/emu198x@84b5031a2.
- Find and read the internal header (LoROM, HiROM and ExHiROM), skip a
  copier header, and compute the checksum the way the cartridge mirrors
  its ROM.
- `SfcImage::to_bytes` writes a parsed file back byte for byte, copier
  header included.
- `set_checksum` sets the checksum and complement to match the ROM.

### Changed from Emu198x

- `SfcImage::copier_header` holds the copier header's 512 bytes, in place of
  Emu198x's `had_copier_header` flag, so the file can be written back.

Independent versions start at 0.1.0.
