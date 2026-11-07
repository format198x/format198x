# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-nintendo-nes-ines`
  at emu198x/emu198x@456ae57b7.
- `InesImage::to_bytes` writes a parsed file back byte for byte. It keeps the
  header bytes the parser does not decode (`header_bytes`) and anything after
  the CHR ROM (`trailing`), and refuses to write a header that no longer
  matches the image's trainer or ROM lengths.
- `encode` writes a new iNES or NES 2.0 file from its parts. NES 2.0 sizes
  use a bank count where one fits and the exponent form otherwise.

Independent versions start at 0.1.0.
