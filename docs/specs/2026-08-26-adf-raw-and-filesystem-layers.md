# ADF: one crate, two layers — raw image and filesystem

**Status:** Proposed 2026-08-26. Not started.

**Goal:** Retire Emu198x's parallel ADF crate by giving
`format198x-commodore-amiga-adf` the raw sector layer the emulator needs,
beneath the filesystem layer it already has — and support HD images in both.

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
- **`ImageMut` is new surface** with no consumer in this repo. It exists because
  Emu198x has `write_sector` and dropping it would make this a downgrade rather
  than a unification. Format198x crates are bidirectional by
  `studio198x-authoring.md` clause 5.

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
3. Emu198x migrates its floppy peripheral and deletes its copy. Its session, its
   timing.

Step 3 is not a precondition for 1 or 2. Nothing here breaks Emu198x while it
carries its own crate — the two coexist exactly as they do today.
