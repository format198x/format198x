use crate::error::Error;
use crate::fs::FileSystem;

/// Bytes per disk block (sector).
pub(crate) const BSIZE: usize = 512;
/// Blocks on a DD floppy: 80 cylinders × 2 heads × 11 sectors.
pub(crate) const BLOCKS: u32 = 1760;
/// The root block sits at the middle of a DD disk.
pub(crate) const ROOT_BLK: u32 = 880;
/// The bitmap block, immediately after the root.
pub(crate) const BITMAP_BLK: u32 = 881;
/// Hash-table / data-pointer slots per header block.
pub(crate) const HT_SIZE: usize = 72;
/// Payload bytes per OFS data block (512 − the 24-byte OFS data header).
pub(crate) const OFS_DATA: usize = BSIZE - 24;
/// File/dir/data blocks are allocated upward from here (deterministic).
pub(crate) const FIRST_FREE: u32 = 882;

/// Primary block type for headers.
pub(crate) const T_HEADER: u32 = 2;
/// Primary block type for OFS data blocks.
pub(crate) const T_DATA: u32 = 8;
/// Primary block type for file-extension lists (data pointers beyond a header's
/// 72 slots).
pub(crate) const T_LIST: u32 = 16;
/// Secondary type: root.
pub(crate) const ST_ROOT: u32 = 1;
/// Secondary type: user directory.
pub(crate) const ST_USERDIR: u32 = 2;
/// Secondary type: file (−3 as a two's-complement u32).
pub(crate) const ST_FILE: u32 = (-3i32) as u32;

/// AmigaDOS name length limit.
pub(crate) const MAX_NAME: usize = 30;

/// Protection bits for the executable. The low nibble is the RWED set, stored
/// **active-low** — a set bit *revokes* that permission — so `0x00` grants read,
/// write, execute, and delete: a normal, runnable file. The executable must be
/// readable and executable, because the CLI `LoadSeg`s the command named in
/// `startup-sequence`; revoking read breaks that on any Kickstart that enforces
/// protection.
///
/// An earlier `0x0d` (read/write/delete revoked) was copied from an xdftool
/// disk and *looked* fine because KS1.3 ignores protection on LoadSeg — but it
/// fails on KS2.0+ with "file is read protected". See the demand-gate-adf-master
/// decision log (2026-07-10). Fixing it also makes the OFS disks portable to
/// KS2.0+, not just KS1.3.
pub(crate) const EXE_PROTECT: u32 = 0x00;

/// The standard KS1.2+ OFS boot block: `DOS\0`, its checksum, the boot code,
/// and `dos.library`. 49 nonzero bytes; the rest of the 1024-byte boot area is
/// zero. Volume-independent — verified to boot on A500/KS1.3.
pub(crate) const BOOT_PREFIX: [u8; 49] = [
    0x44, 0x4f, 0x53, 0x00, 0xc0, 0x20, 0x0f, 0x19, 0x00, 0x00, 0x03, 0x70, 0x43, 0xfa, 0x00, 0x18,
    0x4e, 0xae, 0xff, 0xa0, 0x4a, 0x80, 0x67, 0x0a, 0x20, 0x40, 0x20, 0x68, 0x00, 0x16, 0x70, 0x00,
    0x4e, 0x75, 0x70, 0xff, 0x60, 0xfa, 0x64, 0x6f, 0x73, 0x2e, 0x6c, 0x69, 0x62, 0x72, 0x61, 0x72,
    0x79,
];

pub(crate) fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_be_bytes());
}

pub(crate) fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// The 512-byte slice for block `n` within the disk image.
pub(crate) fn block_mut(img: &mut [u8], n: u32) -> &mut [u8] {
    let off = n as usize * BSIZE;
    &mut img[off..off + BSIZE]
}

/// The AmigaDOS block checksum: the value that makes the sum of all 128
/// longwords come to zero, with the checksum field (`chk_off`) taken as zero.
/// Headers and data blocks put it at offset 20; the bitmap block at offset 0.
pub(crate) fn checksum(block: &[u8], chk_off: usize) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < BSIZE {
        if i != chk_off {
            sum = sum.wrapping_add(read_u32(block, i));
        }
        i += 4;
    }
    sum.wrapping_neg()
}

/// The boot-block checksum over the 1024-byte boot area: add every longword
/// with end-around carry, then complement. Distinct from [`checksum`] — the
/// bootstrap ROM verifies the boot block with this add-with-carry variant.
/// The caller zeroes the checksum field (offset 4) before calling.
pub(crate) fn boot_checksum(boot: &[u8]) -> u32 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < 1024 {
        let (s, carried) = sum.overflowing_add(read_u32(boot, i));
        sum = if carried { s.wrapping_add(1) } else { s };
        i += 4;
    }
    !sum
}

/// Write the boot block (sectors 0–1): the DOS-type marker, the filesystem's
/// type byte, and a freshly computed boot checksum. A `bootable` disk also
/// carries the fixed bootstrap blob (reproduced byte-for-byte for OFS); a data
/// disk gets only `DOS` + type + checksum — mountable, but the ROM finds no
/// bootstrap to run.
pub(crate) fn write_boot_block(img: &mut [u8], fs: FileSystem, bootable: bool) {
    if bootable {
        img[..BOOT_PREFIX.len()].copy_from_slice(&BOOT_PREFIX);
    } else {
        img[0..4].copy_from_slice(b"DOS\0");
    }
    img[3] = fs.dos_type();
    put_u32(img, 4, 0); // zero the checksum field before computing
    let c = boot_checksum(&img[..1024]);
    put_u32(img, 4, c);
}

/// Whether the boot area carries a bootstrap to run.
///
/// The first twelve bytes are header fields — DOS type, boot checksum, and the
/// root-block pointer — so the bootstrap, if there is one, starts at offset 12.
pub(crate) fn has_boot_code(img: &[u8]) -> bool {
    img[12..1024].iter().any(|&b| b != 0)
}

/// AmigaDOS filename hash → slot in a 72-entry table. `h = len; for each byte
/// h = (h*13 + toupper(c)) & 0x7ff; slot = h % 72`.
pub(crate) fn name_hash(name: &str) -> usize {
    let mut h = name.len() as u32;
    for c in name.bytes() {
        h = h
            .wrapping_mul(13)
            .wrapping_add(c.to_ascii_uppercase() as u32)
            & 0x7ff;
    }
    (h as usize) % HT_SIZE
}

/// Write a `name_len`-prefixed AmigaDOS name into `block` ending at its tail
/// (the name field ends 80 bytes from the block end: len byte at `BSIZE-80`).
pub(crate) fn put_name(block: &mut [u8], name: &str) {
    block[BSIZE - 80] = name.len() as u8;
    block[BSIZE - 79..BSIZE - 79 + name.len()].copy_from_slice(name.as_bytes());
}

pub(crate) fn validate_name(name: &str, what: &'static str) -> Result<(), Error> {
    if name.is_empty() || name.len() > MAX_NAME || !name.is_ascii() {
        return Err(Error::InvalidName {
            what,
            len: name.len(),
        });
    }
    Ok(())
}

/// The immutable 512-byte slice for block `n`.
pub(crate) fn block(img: &[u8], n: u32) -> &[u8] {
    let off = n as usize * BSIZE;
    &img[off..off + BSIZE]
}
