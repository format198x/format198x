# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-memotech-mtx` at
  emu198x/emu198x@c7b3834297, with no `serde` or `thiserror`.
- Decode and encode `.mtx` tape images in both the full and headerless forms, and `.RUN` programs.
- Errors are now a typed `MtxError`; the Emu198x crate returned strings.

Independent versions start at 0.1.0.
