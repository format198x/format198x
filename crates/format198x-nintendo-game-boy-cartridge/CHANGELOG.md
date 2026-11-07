# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's
  `format-nintendo-game-boy-cartridge` at emu198x/emu198x@42257c106, with no
  `serde` or `thiserror`.
- `set_checksums` writes the header checksum and then the global checksum,
  and `global_checksum` computes the latter.
- `CartridgeHeader::cart_type_byte` keeps the type byte as stored, so `$00`
  and `$08` can be told apart.

### Changed from Emu198x

- Every documented cartridge type decodes: MMM01, MBC6, MBC7, Pocket
  Camera, Bandai TAMA5, HuC3 and HuC1 are new `CartridgeType` variants.
  Emu198x refused them with `UnsupportedCartridgeType`, which is gone. Which
  boards an emulator supports is the emulator's decision.

Independent versions start at 0.1.0.
