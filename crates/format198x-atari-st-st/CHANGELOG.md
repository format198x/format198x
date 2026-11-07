# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Dependency-free crate graduated from Emu198x's `format-atari-st-st` at
  emu198x/emu198x@adbc5752f.
- Work out an image's geometry from its boot sector or its size, and make
  blank images.
- `StImage::to_bytes` writes an image back byte for byte, and
  `StImage::trailing` keeps any bytes past the last whole track.

### Fixed (from Emu198x)

- The boot sector no longer wins over the image size when they disagree.
  Emu198x's reader trusted any boot sector that fitted, so an 82-track
  double-sided image whose boot sector described one side lost half its
  sectors. The size now decides unless the boot sector describes the whole
  image, as in Hatari.
- Images of 79 tracks or fewer, or 85 and 86, are recognised by size.
- Bytes past the geometry are kept rather than dropped.

Independent versions start at 0.1.0.
