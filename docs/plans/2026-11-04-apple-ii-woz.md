# WOZ 2 format

Part of the approved Apple II Disk II plan in Emu198x/apple-ii/docs/plans/.
Add crates/format198x-apple-ii-woz/{Cargo.toml,README.md,src/lib.rs} and
 decisions/apple-ii-woz.md. Implement bounded parsing, lossless writing and a
canonical builder. Check malformed/truncated input, mappings, CRC and round
trips, including an external real image. Run format, Clippy, tests, package
verification and owning-workspace CI; commit, push, PR, release and verify the
published package before the emulator consumes it. Preserve unrelated work.
