# ADF: one crate, two layers — raw image and filesystem

**Status:** Steps 1–4 landed 2026-08-27. Step 5 (Emu198x migrating its five
consumers and deleting its copy) is the Emu198x session's, and outstanding. See
**Outcome** at the end for what the work found that this spec did not predict.

**Goal:** Retire Emu198x's parallel ADF crate by giving
`format198x-commodore-amiga-adf` the raw sector layer the emulator needs,
beneath the filesystem layer it already has — with **read, write and verify
reachable at both layers, for both DD and HD**.

Two of those six capabilities do not exist anywhere today: writing to a
filesystem that already exists, and verifying one exhaustively rather than
stopping at the first bad checksum. They are the bulk of the work, not the
layering.

---

## Why there are two crates today

They collided on a name and were assumed to be duplicates. They are not: the
public API overlap is **zero**.

| | `Emu198x/emu198x/crates/format-commodore-amiga-adf` | `format198x-commodore-amiga-adf` |
|---|---|---|
| Origin | `Emu198x-archive` | build198x's `format::adf` |
| Is | a raw CHS sector-dump container | an OFS/FFS filesystem |
| Exposes | `Adf::from_bytes`, `read_sector`, `write_sector`, `read_track_sectors`, `sectors_per_track` | `Disk`, `Volume`, `master`, `master_fs` |
| Geometry | DD **and** HD | DD only, `BLOCKS` hardcoded at 1760 |
| Bounds | none — panics on an out-of-range cylinder | checked throughout |
| Consumers | five Emu198x crates by path; `peripheral-commodore-amiga-floppy` feeds `read_track_sectors` to the MFM encoder | Build198x, Play198x, Asm198x |

Neither is wrong. They are **two layers of the same thing, built twice.**

## The observation that makes this cheap

They already address identical bytes.

```
Emu198x, CHS:   offset = ((cyl * HEADS + head) * sectors_per_track + sector) * 512
Canonical, LBA: offset = n * 512
```

So `lba = (cyl * heads + head) * sectors_per_track + sector`, and that is the
whole conversion. The canonical crate is already doing raw-layer work — it just
keeps it private in `cblock` and fixes the geometry at DD.

An ADF track is **contiguous** under that layout, and Emu198x already returns one
as a single `&[u8]` rather than a `Vec` of sector references. The MFM encoder's
zero-copy path survives unchanged.

## The shape

```rust
/// Geometry a disk image implies. Not a filesystem concept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors_per_track: u8,
}

pub const DD: Geometry = Geometry { cylinders: 80, heads: 2, sectors_per_track: 11 };
pub const HD: Geometry = Geometry { cylinders: 80, heads: 2, sectors_per_track: 22 };

impl Geometry {
    pub const fn blocks(self) -> u32;      // cylinders * heads * sectors_per_track
    pub const fn len(self) -> usize;       // blocks() * 512
    /// Commodore's own formula; see **HD** below.
    pub const fn root_block(self) -> u32;
}

/// A raw ADF image: bytes, plus the geometry their length implies.
/// Knows nothing about filesystems — a bootblock disk is a perfectly good `Image`.
pub struct Image<'a> { /* &'a [u8] + Geometry */ }

impl<'a> Image<'a> {
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error>;
    pub fn geometry(&self) -> Geometry;
    pub fn sector(&self, cyl: u16, head: u8, sector: u8) -> Result<&'a [u8], Error>;
    pub fn track(&self, cyl: u16, head: u8) -> Result<&'a [u8], Error>;
    pub fn block(&self, lba: u32) -> Result<&'a [u8], Error>;
    pub fn bytes(&self) -> &'a [u8];
}

/// The writable counterpart. A real Amiga writes to floppies.
pub struct ImageMut<'a> { /* &'a mut [u8] + Geometry */ }

impl<'a> ImageMut<'a> {
    pub fn open(bytes: &'a mut [u8]) -> Result<Self, Error>;
    pub fn blank(geometry: Geometry) -> Vec<u8>;
    pub fn sector_mut(&mut self, cyl: u16, head: u8, sector: u8) -> Result<&mut [u8], Error>;
    pub fn write_sector(&mut self, cyl: u16, head: u8, sector: u8, data: &[u8]) -> Result<(), Error>;
    pub fn as_image(&self) -> Image<'_>;
}

impl<'a> Disk<'a> {
    /// Unchanged. Opens an `Image`, then interprets it.
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error>;
    pub fn from_image(image: Image<'a>) -> Result<Self, Error>;
    pub fn image(&self) -> Image<'a>;
}
```

`Disk::open` keeps its signature and behaviour exactly, so no existing consumer
changes. The layering is additive.

## HD

Supported in **both** layers, not just the raw one.

The filesystem's only real dependency on geometry is where the root block sits,
and Commodore published the formula. From the *AmigaDOS Manual, 3rd edition*
(Baker, Jesup et al., 1991), `rootblock.c` — Commodore-Amiga's own code:

```c
blocksPerCyl  = de->de_BlocksPerTrack * de->de_Surfaces;
blocksPerDisk = blocksPerCyl * (de->de_HighCyl - de->de_LowCyl + 1);
root          = (blocksPerDisk - 1 + de->de_Reserved) >> 1;
```

It validates against a known answer, which is why it can be trusted rather than
merely quoted. DD, with `Reserved = 2`:

```
(1760 - 1 + 2) >> 1 = 880
```

880 is the root block every 1980s AmigaDOS manual in `reference/` names for a
DD floppy. The same formula gives HD:

```
(3520 - 1 + 2) >> 1 = 1760
```

So `BLOCKS` stops being a constant and becomes `geometry.blocks()`, and the
hardcoded `880` becomes `geometry.root_block()`.

**One bitmap block still suffices.** A bitmap block holds `512 - 4 = 508` bytes
of allocation bits — 4064 blocks' worth. HD needs 3518. This is arithmetic from
the block size rather than a fact needing a citation, and it is stated because a
reader would otherwise reasonably assume HD needs a second bitmap block and a
bitmap-extension chain.

**Verify HD against a real image before claiming support.** Every fact above is
derived, and derived facts about on-disk layout have been wrong in this family
before. The acceptance test is an `#[ignore]`d one that reads a real HD ADF from
a path in an environment variable — no media in the repository.

## Read, write and verify — universal across both layers and both geometries

This is the acceptance bar, not a wish list. Every cell below must be reachable
for DD **and** HD.

| | Raw image | Filesystem |
|---|---|---|
| **Read** | `sector`, `track`, `block` | `list`, `read` — *exists* |
| **Write** | `write_sector`, `sector_mut` | create, replace, delete, in place |
| **Verify** | geometry and container consistency | checksums, chains, bitmap agreement |

Two of those six are real gaps today, and neither is a rename away.

### Writing to a disk that already exists

`Volume` is a **builder**: `new`, `add_file`, `add_dir`, `build` — it produces a
whole image from nothing. There is no way to open an ADF and change it. An
emulator writing a save file, an authoring tool replacing one asset, or a
learner dropping a binary onto a working disk all need the thing that does not
exist.

```rust
pub struct DiskMut<'a> { /* ImageMut<'a> */ }

impl<'a> DiskMut<'a> {
    pub fn open(bytes: &'a mut [u8]) -> Result<Self, Error>;
    pub fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error>;
    pub fn delete(&mut self, path: &str) -> Result<(), Error>;
    pub fn create_dir(&mut self, path: &str) -> Result<(), Error>;
    pub fn as_disk(&self) -> Disk<'_>;
}
```

Each operation allocates or frees bitmap blocks, splices the directory hash
chain, and recomputes every checksum it disturbed — root, header, bitmap, and
for OFS the per-block data checksums that FFS does not carry.

**`Volume::build` is then re-expressed over `DiskMut`**: blank the image, then
apply the same writes. That is the point of doing it this way round. Today the
builder is the only code that knows how to place a file on an Amiga disk; adding
a mutator alongside it would make two, and they would drift. One implementation,
two entry points.

`Volume`'s public API does not change.

### Verifying more than three checksums

`Disk::verify` today checks the boot, root and bitmap checksums and returns at
the first failure. That answers "is this disk obviously broken" and not "what is
wrong with this disk".

```rust
pub struct Report { pub problems: Vec<Problem> }   // empty == sound

impl<'a> Disk<'a> {
    pub fn verify(&self) -> Result<(), Error>;   // unchanged: fast, first failure
    pub fn check(&self) -> Report;               // exhaustive, never stops early
}
```

`check` walks every directory hash chain and file header chain, follows each
file's data blocks to its declared length, verifies OFS data-block checksums,
and confirms the bitmap agrees with what is actually allocated — a disagreement
being the classic symptom of a disk written by something that got the bitmap
wrong. It reports **every** problem, because a tool that shows one fault per run
is a tool you run many times.

`verify`'s signature and behaviour are untouched, so no consumer changes.

> **Deviation, 2026-08-27.** Its signature is untouched; its behaviour is not,
> and deliberately. `verify` was rejecting disks that are not faulty — anything
> with a zero boot-checksum field, which is what AmigaDOS `Format` leaves until
> `Install` writes a bootstrap, and so what nearly every data disk carries. It
> now accepts that state and refuses everything else. Two commits, both with the
> reasoning; see **Outcome**. Keeping the stated behaviour would have meant
> keeping a bug because the spec had promised it.

### Verifying a raw image

At the raw layer there is nothing to checksum — an ADF is decoded sectors with
no per-sector check data, which is exactly what distinguishes it from a flux
image. So `Image::verify` answers the only questions the layer can: does the
length match a geometry it knows, and is this file actually an ADF rather than
an IPF, DMS, zip or gzip wearing the extension. That second half is the `#1192`
guard, which becomes a verification step rather than only an `open` guard.

Saying this plainly matters more than implementing it. A caller who assumes a
clean `Image::verify` means the sectors are intact has misunderstood the format,
and the documentation is the only place to stop them.

## What this fixes on the way

- **Bounds.** Emu198x's `read_sector` indexes without checking and panics on an
  out-of-range cylinder, head or sector. The unified layer returns `Result`
  everywhere. That matters more than it looks: this crate is destined for an FFI
  boundary where unwinding is undefined behaviour.
- **Container identification.** The `#1192` fix — say the file is an IPF, do not
  say its size is wrong — currently exists in the Emu198x copy and is being
  ported to the canonical one. After unification there is one copy of it.
- **One fewer crate to rename.** Emu198x's copy goes away before the Emu198x
  naming sweep rather than being renamed and then deleted.

## Costs, stated plainly

- **Emu198x's call sites change from panicking indexing to `Result`.** Chiefly
  `peripheral-commodore-amiga-floppy/src/lib.rs:64`. Small, but it is in the
  Emu198x session's hands, not this repo's.
- **Emu198x takes a crates.io dependency** where it currently has a path
  dependency. That is the normal direction of travel for this family and the
  crate is already published.
- **`DiskMut` is the largest piece of new work here**, and it is not a port of
  anything — nothing in either crate mutates an existing filesystem today.
  Bitmap allocation, hash-chain splicing and OFS data checksums are where the
  bugs will be, which is why step 4's `check` is sequenced right behind it: an
  exhaustive verifier is the natural test oracle for a mutator, and each makes
  the other worth trusting.

## What this is NOT

- **Not a change to `Disk`'s public API.** `open`, `list`, `read`, `verify`,
  `Volume` and `master` are untouched.
- **Not MFM.** Track encoding stays in Emu198x's floppy peripheral. This crate
  hands out bytes; what a drive does with them is the emulator's business.
- **Not a new crate.** The raw layer is modules in the existing one, so the
  bounds checking, the container guard and the error type are shared rather than
  reimplemented.

## Sequencing

1. `Geometry`, `Image`, `ImageMut` added; `Disk` re-expressed over `Image` with
   its API unchanged. Ships as a minor bump.
2. HD accepted by `Image`, and by `Disk` once verified against a real HD image.
   Until then `Disk::from_image` declines HD with a typed error rather than
   guessing.
3. `DiskMut` lands, and `Volume::build` is re-expressed over it so there is one
   implementation of placing a file on a disk rather than two.
4. `Disk::check` lands beside the unchanged `verify`.
5. Emu198x migrates its floppy peripheral and deletes its copy. Its session, its
   timing.

Step 5 is not a precondition for 1-4. Nothing here breaks Emu198x while it
carries its own crate — the two coexist exactly as they do today.

---

## Outcome

Steps 1–4 landed on 2026-08-27 in this order, one commit each:

1. `Geometry`, `Image`, `ImageMut`; `Disk` re-expressed over `Image`.
2. HD in both layers.
3. `DiskMut`, with `Volume::build` re-expressed over it.
4. `Disk::check`.

The shape held. What follows is what the plan could not know.

### HD was verified against another implementation, not a period disk

The spec asks for verification against "a real HD ADF", and is right to: every
HD fact in it is derived. There is no HD ADF in the Amiga TOSEC sets — a search
of the whole tree for a 1,802,240-byte file returned nothing, which fits, since
Amiga HD floppies were rare.

So the oracle is amitools' `xdftool`, an independent implementation and the one
this crate's DD layout facts were originally taken from. Images it wrote put the
root block at 1760, the bitmap at 1761, `bm_pages[0]` set with the rest empty
and no bitmap-extension chain — every derived fact confirmed. Both filesystems
on both densities were read back, walked and checksummed against known contents,
and disks this crate had written and churned were handed back to `xdftool`,
which agreed with the bitmap block for block.

That is strong, but it is cross-implementation agreement rather than a
period-original dump. Anyone who later finds a real HD disk should run
`hd_image_from_the_wild` against it: `ADF_HD_IMAGE=<path> cargo test -- --ignored`.

### Three faults the work found, each fixed on its own commit

None was in scope. Each was found by reading disks this crate had not written —
which is the method worth keeping, since the crate had only ever verified its
own stricter output.

- **Every uninstalled disk read as corrupt.** `verify` rejected any disk with a
  zero boot-checksum field. AmigaDOS `Format` leaves it zero until `Install`
  writes the bootstrap, so ordinary data disks — every one `xdftool` produces —
  failed. The ROM only checksums the boot block when about to run it.
- **A bootstrap was looked for in the wrong place.** The first fix asked whether
  any of the 1024-byte boot area was non-zero. The ROM executes from offset 12,
  in sector 0; bytes living only in sector 1 are not a bootstrap. A real disk
  (`WheelDriverAkiko.adf`) carries exactly that filler and was called corrupt.
- **A volume could fill only half its disk.** The block planner walked upward
  from the root and never revisited what lay below, so an 880 KB DD floppy
  topped out near 432 KB. Not introduced by HD — it had always been there, and
  HD only made it visible. Fixed by step 3, where allocation moved onto the
  bitmap: DD now reaches 886 KB and HD 1.78 MB, 98% of the media rather than 49%.

### Byte-for-byte output is unchanged

Re-expressing `Volume::build` over `DiskMut` put the crate's determinism
contract at risk — build198x has committed `.adf` deliverables. The allocator
therefore takes the lowest free block *above the root* before considering
anything below, which is exactly the order the old planner used. Every image
that built before builds to the same bytes; only content that previously failed
now succeeds.

Checked against a corpus of eight images — `master`, `master_fs`, and nested
trees with colliding names, empty files, deep directories and extension blocks,
across both filesystems and both geometries — hashed before the change and
unchanged after. Freed blocks are zeroed for the same reason, so a disk written,
emptied and rewritten matches one written straight.

### Two small additions beyond the sketch

- `Volume::set_geometry`, without which the acceptance table's write-to-HD cell
  is unreachable. Additive; the default is still `DD`.
- `DiskMut::format`, which turns a blank image into an empty volume. Needed by
  `Volume::build` and the only way to make a fresh HD disk from nothing.
- `identify_container` and `DiskMut::free_blocks` made public.

`Error` gained `OutOfBounds` and `BadSectorLength`; `DiskFull` and the
wrong-size `Corrupt` message stopped naming DD specifically. `Disk`'s existing
methods are untouched.
