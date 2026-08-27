//! Amiga ADF disk images, in two layers.
//!
//! An ADF is a raw dump of a floppy's decoded sectors, and usually — but not
//! always — an AmigaDOS filesystem written across them. This crate models both,
//! because a consumer needs one or the other and rarely both:
//!
//! - **The raw layer.** [`Image`] and [`ImageMut`] address the sectors
//!   themselves, by cylinder/head/sector the way a drive does or by logical
//!   block the way AmigaDOS does — the same bytes, two names. [`Geometry`] is
//!   the shape they sit in ([`DD`] or [`HD`]). This layer knows nothing about
//!   files: a bootblock-only disk, a copy-protected loader's track data, or a
//!   blank image are all perfectly good [`Image`]s. An emulator's floppy
//!   peripheral wants this layer and nothing above it.
//! - **The filesystem layer.** [`Disk`] reads an OFS/FFS volume — `list`,
//!   `read`, `verify` — and [`Volume`] writes one. This is the layer that knows
//!   what a file is.
//!
//! The layering is one-directional: [`Disk::open`] opens an [`Image`] and then
//! interprets it, and [`Disk::image`] hands the raw view back. Nothing at the
//! raw layer depends on anything above it.
//!
//! Two entry points on the write side. [`Volume`] builds an arbitrary
//! file/directory tree onto a
//! DD floppy image (880 KB) — `add_file`/`add_dir`, then `build`. [`master`]
//! (and [`master_fs`]) is the common special case: a Kickstart-1.x hunk
//! executable plus a `startup-sequence` that runs it, the disk an Amiga boots
//! straight into. It is the mastering half of an Amiga-assembly build — an
//! assembler emits the hunk executable, this crate writes the bootable disk.
//!
//! Both floppy filesystems are supported ([`FileSystem`]): **OFS** (`DOS\0`,
//! the bare A500/KS1.3 default) and **FFS** (`DOS\1`, denser and faster, but
//! bootable only on KS2.0+). They differ only in their data blocks; the volume
//! structure is identical.
//!
//! **Deterministic** (the determinism contract): every date field is zeroed and
//! block allocation is fixed, so the same exe + names always produce identical
//! bytes — unlike xdftool, which stamps creation dates. That makes the committed
//! `.adf` deliverables byte-reproducible.
//!
//! [`Disk`] is the read side: open an image, `list` directories, `read` files,
//! and `verify` every checksum — panic-free on malformed input.
//!
//! **General within the DD-floppy shape** — any tree of files and directories,
//! bootable or a plain data disk. It is correct for *any* input: a file of any
//! size chains into extension blocks (not just the 72 that fit a header), names
//! that hash to the same slot chain through the hash table, nested directories
//! to any depth, and a tree too large for an 880 KB disk is a typed error
//! rather than a corrupt image. The International/Dir-Cache variants, hard-disk
//! (RDB) layouts, and multi-disk sets are the remaining generality frontier —
//! each its own later scope.
//!
//! Layout facts were taken as ground truth from a known-good `xdftool` disk and
//! cross-checked against ADFlib (adflib/ADFlib) and gadf (sphair/gadf, public
//! domain). The block structures used:
//!
//! - **Boot block** (sectors 0–1): the DOS-type byte (`DOS\0` OFS / `DOS\1`
//!   FFS) + the fixed KS1.2+ boot code + `dos.library`, with an add-with-carry
//!   boot checksum. The bootstrap is a constant, volume-independent blob.
//! - **Root block** (block 880): volume name, a 72-slot name-hash table of
//!   top-level entries, the bitmap pointer, dates, and a block checksum.
//! - **Bitmap block** (block 881): one bit per block (1 = free), checksum at
//!   offset 0.
//! - **Directory / file headers**: type `T_HEADER` (2); a directory's 72-slot
//!   table holds child headers hashed by name, a file's holds its data-block
//!   pointers in reverse; secondary type `ST_USERDIR` (2) or `ST_FILE` (−3).
//! - **Data blocks**: OFS wraps each in a 24-byte header (`T_DATA`, header-key,
//!   1-based sequence, data size, next block, checksum) then up to 488 payload
//!   bytes; FFS stores a raw 512-byte sector and relies on the pointer tables.
//!
//! **What a clean [`Image::verify`] does not tell you.** An ADF is decoded
//! sectors with no per-sector check data — that absence is exactly what
//! distinguishes it from a flux-level image such as IPF. So the raw layer can
//! confirm the file is an ADF of a known shape and nothing more; it cannot tell
//! you a sector is intact, because the format does not record enough to know.
//! Soundness is a filesystem question, answered by [`Disk::verify`].
//!
//! Pure byte-layout — `core`/`std` only, no dependencies. Internally organised
//! as small modules — `error`, `fs`, `geometry` and `image` (the raw layer),
//! `layout` (block constants and primitives),
//! `write` ([`Volume`]/[`master`]), and `read` ([`Disk`]) — re-exported here.

mod error;
mod fs;
mod geometry;
mod image;
mod layout;
mod read;
mod write;

#[cfg(test)]
mod tests;

pub use error::Error;
pub use fs::FileSystem;
pub use geometry::{DD, Geometry, HD};
pub use image::{Image, ImageMut};
pub use read::{Disk, Entry, EntryKind};
pub use write::{Volume, master, master_fs};
