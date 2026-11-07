# Changelog

## [0.1.0] - 2026-11-07

Read Neo Geo named ROM files into their program, fix-layer, sound, sample and sprite regions. Write canonical named files that preserve those region bytes. ZIP handling stays with callers; the crate has no dependencies. Reject incorrectly numbered sprite pairs and return typed errors for regions that cannot be written.
