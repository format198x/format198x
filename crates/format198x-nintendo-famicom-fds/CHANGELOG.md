# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's
  `format-nintendo-famicom-fds` at emu198x/emu198x@6ecd5455b.
- Read and write `.fds` images, with or without the fwNES header, and scan
  the disk info, file amount, file header and file data blocks on a side.

### Changed from Emu198x

- `write` returns a `Result`. It refuses a side longer than 65,500 bytes
  (`FdsError::SideTooLong`), and more than 255 sides with a header
  (`FdsError::TooManySides`). Emu198x's writer cut the side and clamped the
  count without saying so.

Independent versions start at 0.1.0.
