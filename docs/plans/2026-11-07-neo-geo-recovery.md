# Recover the approved Neo Geo format graduation

Continue PR C of `2026-11-07-remaining-emu198x-layouts.md`. Named ROM files are the agreed boundary; ZIP and machine mapping stay in Emu198x.

1. Import the existing parser and its non-ZIP tests from Emu198x commit cab1b24fc5092264937ee145fb314f7a785b536c into `crates/format198x-snk-neo-geo-romset/`. Keep the inherited Claude scratch work untouched. Replace thiserror with standard-library error implementations.
2. Add `to_files(&RomSet)` for canonical named files. Swap P words back, split C byte lanes, keep both V regions. Preserve region bytes rather than original names/chip boundaries; document first S/M selection and P1 half normalisation. Reject empty P and odd C regions. Use two P parts for a 2 MiB region so reparsing cannot reinterpret upper-half header data.
3. Retain source tests, test writer byte lanes independently of the parser, and round-trip all regions including odd P lengths and ambiguous upper-half headers. Verify known C pairs reject even/odd numbering in the wrong order, with a red control before fixing it.
4. Add manifest, doctested README, changelog, root catalogue entry and binding boundary record. Exercise all 17 extracted real sets without committing ROMs; require a positive set count and compare canonical output regions with original-file decoding.
5. Run formatting, workspace tests/Clippy, package dry-run and full CI. Import commit names source SHA and history command. Merge by rebase when qualified, then follow the existing release/bootstrap plan and record the Emu198x consumer follow-up. Remove task worktree/build cache after remote retention is verified.

## Verification

- C2/C3 regression first failed against the imported parser: it returned an accepted interleaved region. The odd-start check makes it reject; the original pair tests still pass.
- All 13 crate tests and the README doctest pass. Direct writer assertions cover P word order, odd-length P, separate V regions and sprite byte lanes; a 2 MiB upper-half-header regression guards against reinterpretation.
- All 17 extracted sets preserve P/S/M/VA/VB/C regions through canonical named output, including Metal Slug and separate ADPCM-B sets. No ROM bytes are committed. This establishes region preservation, not reproduction of original chip filenames or boundaries.
- Workspace tests and all-target Clippy pass. Package dry-run includes nine files and verifies the dependency-free crate.
