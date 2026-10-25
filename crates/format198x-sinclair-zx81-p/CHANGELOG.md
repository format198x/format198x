# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-sinclair-zx81-p` at
  emu198x/emu198x@1d70a0a216, with no `serde` or `thiserror`.
- Decode and validate `.p` images with trailing slack, build one from a RAM dump, and encode it.
- The cassette pulse train (`to_pulses`) from Emu198x's crate stays in Emu198x, as machine behaviour.

Independent versions start at 0.1.0.
