# Accept native bitmap slack

Approved scope: issue #69. Native Easy AMOS Save produces the same bitmap in
FS-UAE and Emu198x. Only nonzero whole words beyond the allocation map cause the
checker rejection. Permit these words; preserve final-word padding, full-block
checksums and real-block ownership checks. No new API, dependency or writer change.

1. Add `src/tests/bitmap_slack.rs` in the Amiga ADF crate, registered from
   `src/tests.rs`. Use a synthetic relocated bitmap with nonzero slack for OFS
   and FFS. Confirm acceptance fails before changing the checker. Cover both
   final padding bits, both allocation disagreement directions, and a slack
   mutation without recomputing its checksum.
2. Bound `src/check.rs`'s tail scan to the final allocation word. Document the
   distinction in `decisions/commodore-amiga-adf.md` and the crate changelog,
   explicitly refining issue #7's former whole-block tail requirement.
3. Run formatting, workspace tests and Clippy. Build the existing standalone
   Build198x ADF CLI in a disposable workspace against the local fixed library;
   verify both private native images and initial images, plus corruption
   controls. Do not change Build198x's tracked manifest or lockfile.
4. Push a focused conventional-commit PR, link the verification evidence to #69
   and Lang198x/amos #6, then remove task-owned build and diagnostic directories.
   No runtime images or tokenised files enter the repository or PR.

## Verification

Before the fix, the acceptance regression failed with
`bitmap block 980: non-existent block 1762 is marked free` (exit 101).
After the fix, workspace tests, `cargo fmt --all --check` and workspace
Clippy with `-D warnings` pass. The existing final-word regression also passes.

The isolated consumer uses unmodified Build198x CLI source, linked to the local
fixed ADF library. `cargo tree` confirms that linkage; the CLI's hardcoded
manifest library label still says 0.3.2 and is not used as version evidence.
All nine retained images verify without byte changes. Five corrupted copies
still fail: live header free, last real block held without an owner, each final
padding bit free, and changed slack with a stale checksum. Exact image hashes,
CLI source/binary identities and findings are in
[`2026-10-22-adf-bitmap-verification.json`](2026-10-22-adf-bitmap-verification.json).

No registry release or permanent Build198x dependency update is included.
