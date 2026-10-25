# Emu198x's format layouts graduate together

**Status:** Active

**Decided:** 2026-10-25

## Decision

Every on-disk layout in Emu198x's `format-*` crates moves here, whether or
not anything besides Emu198x reads it yet. That is the umbrella's ruling in
[`emu198x-format-layouts-move-to-format198x.md`](https://github.com/stevehill1981/198x/blob/main/decisions/emu198x-format-layouts-move-to-format198x.md),
amended on 2026-10-25. This record keeps the choices made while moving them.

1. **The bytes move; machine behaviour stays.** A crate here reads and writes
   a file. It does not load a file into a machine or turn it into a signal.
   So the PRG crate has no RAM loader or BASIC relinking, and the ZX80 `.o`
   and ZX81 `.p` crates have no cassette pulse trains. Those stay in Emu198x,
   over the published crates. The Oric TAP record drew the same line.
2. **Both directions.** A layout Emu198x only read gains its writer here,
   with round-trip tests. Where a writer cannot be lossless, its docs say what
   it drops: the Dragon CAS writer does not keep junk a tolerant read
   skipped, and the PC-Dragon writer zeroes the info-block bytes nobody has
   decoded.
3. **No dependencies.** `serde` derives and `thiserror` errors are dropped
   in favour of hand-written `Display` and `Error` impls. Emu198x needs
   `serde` for save states, so its adapters own that.
4. **One import commit per crate.** The history stays in Emu198x, and each
   import commit names the source commit and the `git log` command that
   shows it. Replaying it would break the rule that every commit subject here
   is a conventional commit, which several of those subjects are not.
5. **Names follow Emu198x's system segment** so the phase-2 switch is a
   mechanical rename, with one exception: Memotech's crate gains a format
   segment, `format198x-memotech-mtx-mtx`, and carries `.RUN` too.

## Known differences from the Emu198x crates

- ATR: sectors 2 and 3 of a *padded* double-density image now read their own
  bytes. The Emu198x parser read boot sector 3 as sector 2.
- PRG: a two-byte file is a valid PRG with an empty payload. Emu198x
  required three bytes.
- Jupiter Ace and Memotech MTX return typed errors instead of strings, with
  the same messages.

## Drift triggers

- Moving a loader, pulse encoder, mapper or drive controller in here. Move
  the layout and leave the behaviour in Emu198x.
- Adding `serde` to a crate here for Emu198x's save states. The adapter
  wraps the type instead.
- Dropping a writer because Emu198x does not use it.
