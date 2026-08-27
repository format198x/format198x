# ADF raw + filesystem layers — handoff from the Emu198x session

**Written:** 2026-08-27, from the Emu198x session that found the duplication.
**Scope agreed with Steve:** all four Format198x steps, then hand back.
**Status:** steps 1–4 done the same day. Kept as the record of why the work was
picked up when it was; the spec's **Outcome** section carries what it found.

## What this is

[`docs/specs/2026-08-26-adf-raw-and-filesystem-layers.md`](../specs/2026-08-26-adf-raw-and-filesystem-layers.md)
is the authority. It is detailed and current; read it first and follow it.

This brief adds only what the spec cannot know: the state of the other side of
the migration, the exact API surface a real consumer needs, and why the work is
being picked up now.

## Why now

Emu198x published its first six crates on 2026-08-26 —
`emu198x-mos-6502`, `emu198x-zilog-z80`, `emu198x-mos-sid-6581`,
`emu198x-gi-ay-3-8910`, `emu198x-ricoh-apu-2a03`,
`emu198x-commodore-paula-8364` — to unblock Play198x's code-driven formats
(emu198x#1214). Publishing stopped at six because widening it drags in
`format-commodore-amiga-adf`, and putting a second family ADF crate on a
registry that never releases a name is not a decision anyone had taken.

So this work is the gate on two things in Emu198x: publishing the remaining
chip crates, and its naming sweep. The spec's own note — *"one fewer crate to
rename: Emu198x's copy goes away before the Emu198x naming sweep rather than
being renamed and then deleted"* — is the reason it is worth doing before,
not after.

**No deadline.** Nothing breaks while both crates coexist, exactly as today.

## Verified starting state

Checked on 2026-08-27, not assumed:

- `format198x/main` is clean and idle at `2cfa893` (release v0.2.3).
- `Geometry`, `Image`, `ImageMut`, `DiskMut` and `Disk::check` **do not exist**.
  Steps 1–4 are all unstarted.
- The crate's public surface today is `Error`, `FileSystem`, `EntryKind`,
  `Entry`, `Disk` (`open`, `filesystem`, `label`, `list`, `read`, `verify`),
  `master`, `master_fs`, `Volume` (`new`, `set_bootable`, `add_file`,
  `add_file_with_protection`, `add_dir`, `build`).
- Geometry is private and DD-only: `BSIZE`, `BLOCKS = 1760`, `ROOT_BLK = 880`
  are all `pub(crate)` in `layout.rs`.

## What Emu198x actually consumes

Step 1's API should be checked against this, not only against the spec's
sketch. Taken from every call site in Emu198x at `635cec3a`:

| Symbol | Uses | Notes |
|---|---|---|
| `ADF_SIZE_DD` | 20 | the constant, used for sizing and validation |
| `Adf` | 15 | the owned container type |
| `sectors_per_track()` | 6 | |
| `data()` | ~5 | whole-image byte access |
| `write_sector()` | 3 | **in-place mutation of an existing image** |
| `read_track_sectors()` | 2 | returns a contiguous `&[u8]`, not a `Vec` of sectors |
| `read_sector()` | 1 | |
| `AdfError` | 8 | |
| `identify_container()` | 2 | the #1192 container guard |
| `from_bytes()` | — | constructor |

HD constants (`ADF_SIZE_HD`, `SECTORS_PER_TRACK_HD`) exist in the Emu198x
crate but **no consumer uses them**. HD is internal capability there, so
step 2 is not gated on an Emu198x requirement — but dropping HD would be a
behaviour regression for anyone loading an HD image.

**Nothing in Emu198x touches a filesystem-level API.** No `Disk`, no
`Volume`, no `list`/`read`. The emulator wants sectors and geometry only.

### The call site that matters most

`crates/peripheral-commodore-amiga-floppy/src/lib.rs:64` — the MFM encoder's
zero-copy path:

```rust
fn encode_mfm_track(&self, cyl: u32, head: u32) -> Option<Vec<u8>> {
    let track_num = (cyl * 2 + head) as u8;
    let sectors = self.adf.read_track_sectors(cyl, head);
    Some(encode_mfm_track(sectors, track_num, self.adf.sectors_per_track()))
}
```

`read_track_sectors` hands the encoder one contiguous slice. The spec notes an
ADF track is contiguous under the LBA layout, so `Image::track` preserves this
— please keep it a `&[u8]` rather than a `Vec<&[u8]>`, or that path grows an
allocation per track.

### The five consumers

`machine-commodore-amiga-a1200`, `machine-commodore-amiga-ecs`,
`machine-commodore-amiga-ocs`, `peripheral-commodore-amiga-floppy`,
`runtime-commodore-amiga` — all by path dependency.

## Sequencing and acceptance

Follow the spec's five steps. Steps 1–4 here; step 5 is Emu198x's.

Worth knowing while planning: **steps 1–2 alone unblock Emu198x.** Everything
in the table above lands in `Geometry`/`Image`/`ImageMut` plus HD. `DiskMut`
and `check` serve authoring and verification, not the emulator. That is not a
reason to stop after 2 — Steve has asked for all four — but if the work needs
to pause, 2 is the point where the other side can proceed.

Per-step acceptance, from the spec:

1. **`Geometry` / `Image` / `ImageMut`; `Disk` re-expressed over `Image`.**
   `Disk::open`, `list`, `read`, `verify`, `Volume`, `master` unchanged —
   no existing consumer edits. Minor bump.
2. **HD in both layers.** `blocks()` replaces `BLOCKS`, `root_block()`
   replaces the hardcoded `880`, per Commodore's `rootblock.c` formula. The
   spec is explicit that HD is derived and must be **verified against a real
   HD image** before support is claimed — an `#[ignore]`d test reading a path
   from an environment variable, no media committed.
3. **`DiskMut`**, with `Volume::build` re-expressed over it so there is one
   implementation of placing a file on a disk. The spec flags this as the
   largest and riskiest piece: bitmap allocation, hash-chain splicing, OFS
   data checksums.
4. **`Disk::check`** beside the unchanged `verify`. Sequenced behind 3
   deliberately — an exhaustive verifier is the natural oracle for a mutator.

## Constraints

- **Dependency-free.** This is the stated reason Format198x exists; the raw
  layer needs nothing. Do not add a dependency for it.
- **`Disk`'s public API does not change.** Additive layering only.
- **Bounds-checked everywhere.** Emu198x's `read_sector` indexes and panics on
  an out-of-range cylinder. The spec wants `Result` throughout, and notes this
  crate is destined for an FFI boundary where unwinding is undefined behaviour.
- **No MFM here.** Track encoding stays in Emu198x's floppy peripheral. This
  crate hands out bytes.
- **No media in the repository.**
- **Do not rename or touch Emu198x's crate.** It is deleted in step 5 by the
  Emu198x session, not renamed.

## Handing back

When steps 1–4 are released, the Emu198x side is:

- migrate the five consumers to the published crate,
- move call sites from panicking indexing to `Result` (chiefly the floppy
  peripheral above),
- delete `crates/format-commodore-amiga-adf`,
- swap a path dependency for a crates.io one,
- reopen the chip-crate publishing scope in
  `Emu198x/emu198x/knowledge/decisions/crate-naming.md`, where this
  duplication is currently recorded as the gate.

Tell the Emu198x session the released version and it will pick step 5 up.
Its session, its timing — per the spec.

## One correction to carry across

The Emu198x naming record currently describes the two crates as a
duplication. The spec is right and that wording is loose: the public API
overlap is zero, and they are two layers of one thing built twice. That
record will be corrected when step 5 lands.

---

## Outcome

Steps 1–4 landed on 2026-08-27, one commit each, plus three commits for faults
found on the way. The full account is in the spec's **Outcome** section; the
parts that bear on the handover:

- **The consumed surface is covered.** Everything in the table above has a home:
  `ADF_SIZE_DD`/`ADF_SIZE_HD` as `DD.len()`/`HD.len()` (both `const fn`),
  `sectors_per_track()` as `geometry().sectors_per_track`, `data()` as
  `bytes()`, `read_sector`/`write_sector`/`read_track_sectors` as
  `Image::sector`/`ImageMut::write_sector`/`Image::track`, `AdfError` as
  `Error`, and `identify_container` now public. A test named
  `the_raw_layer_serves_an_emulators_floppy_peripheral` stands in for the call
  sites so the layer cannot drift from them.
- **`Image::track` stays a contiguous `&[u8]`**, as asked. The test asserts
  pointer identity with the track's first sector, not merely equal content, so
  the encoder's zero-copy path cannot regress into an allocation.
- **The owned/borrowed split is the one migration cost.** Emu198x's `Adf` owns
  its `Vec<u8>`; `Image`/`ImageMut` borrow. The five consumers will hold the
  `Vec` themselves and open a view over it. Not difficult, but it is a shape
  change rather than a rename, and worth knowing before step 5 starts.
- **Call sites move from panicking indexing to `Result`**, as the spec said.
  Out-of-range addresses now name the coordinate at fault — cylinder, head,
  sector or block.
- **HD verification is cross-implementation, not a period dump.** There is no HD
  ADF in the Amiga TOSEC sets. See the spec's Outcome for what was done instead.

Emu198x's crate was not touched, per the brief. Step 5 deletes it.
