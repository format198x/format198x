# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-atari-st-msa` at
  emu198x/emu198x@adbc5752f.
- Expand an archive to raw sectors, and compress raw sectors back into one.

### Changed from Emu198x

- `write` returns a `Result`. It refuses other than one or two sides or 1 to
  64 sectors per track, and data that is not 1 to 256 whole tracks. Emu198x's
  writer panicked on zero sides or sectors, dropped a partial last track, and
  wrote a wrong track count past 256 tracks.

Independent versions start at 0.1.0.
