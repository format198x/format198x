# Changelog

## [0.1.1](https://github.com/format198x/format198x/compare/format198x-snk-neo-geo-romset-v0.1.0...format198x-snk-neo-geo-romset-v0.1.1) - 2026-11-10

### Fixed

- Program ROMs that MAME names `pgN` are read as P-ROM N. Samurai Shodown's
  second program ROM, `045-pg2.sp2`, was skipped, leaving its program region
  1 MiB short. A `.spN` extension alone does not count, because the system
  BIOS is `sp-s2.sp1`.

## [0.1.0] - 2026-11-07

Read Neo Geo named ROM files into their program, fix-layer, sound, sample and sprite regions. Write canonical named files that preserve those region bytes. ZIP handling stays with callers; the crate has no dependencies. Reject incorrectly numbered sprite pairs and return typed errors for regions that cannot be written.
