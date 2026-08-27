/// The physical shape a disk image implies: how many cylinders, how many heads,
/// and how many sectors each track holds.
///
/// This is a property of the *media*, not of any filesystem written onto it. A
/// bootblock-only disk with no filesystem at all still has a geometry, which is
/// why this type lives at the raw layer and [`Disk`](crate::Disk) is built on
/// top of it rather than the other way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// Cylinders (tracks per side).
    pub cylinders: u16,
    /// Recording surfaces — 2 for every Amiga floppy.
    pub heads: u8,
    /// Sectors per track: 11 for DD, 22 for HD.
    pub sectors_per_track: u8,
}

/// A double-density Amiga floppy: 80 × 2 × 11 × 512 = 901,120 bytes.
pub const DD: Geometry = Geometry {
    cylinders: 80,
    heads: 2,
    sectors_per_track: 11,
};

/// A high-density Amiga floppy: 80 × 2 × 22 × 512 = 1,802,240 bytes. The drive
/// spins at half speed and writes twice as many sectors per track, so the
/// cylinder count is unchanged.
pub const HD: Geometry = Geometry {
    cylinders: 80,
    heads: 2,
    sectors_per_track: 22,
};

impl Geometry {
    /// Total 512-byte blocks — `cylinders × heads × sectors_per_track`.
    pub const fn blocks(self) -> u32 {
        self.cylinders as u32 * self.heads as u32 * self.sectors_per_track as u32
    }

    /// The image length in bytes this geometry implies.
    ///
    /// There is deliberately no `is_empty`: a geometry describes media, and
    /// media of no size is not a case this crate has.
    #[allow(clippy::len_without_is_empty)]
    pub const fn len(self) -> usize {
        self.blocks() as usize * crate::layout::BSIZE
    }

    /// Where AmigaDOS puts the root block on media of this shape — 880 on a DD
    /// floppy, 1760 on HD.
    ///
    /// Commodore published the arithmetic. From the *AmigaDOS Manual, 3rd
    /// edition* (Baker, Jesup et al., 1991), `rootblock.c`:
    ///
    /// ```c
    /// blocksPerCyl  = de->de_BlocksPerTrack * de->de_Surfaces;
    /// blocksPerDisk = blocksPerCyl * (de->de_HighCyl - de->de_LowCyl + 1);
    /// root          = (blocksPerDisk - 1 + de->de_Reserved) >> 1;
    /// ```
    ///
    /// `de_Reserved` is 2 for a floppy — the two boot sectors. So the root sits
    /// halfway across the disk, which is where a seeking head spends least time
    /// getting to it from anywhere.
    pub const fn root_block(self) -> u32 {
        (self.blocks() - 1 + RESERVED) >> 1
    }

    /// The bitmap block, immediately after the root.
    pub(crate) const fn bitmap_block(self) -> u32 {
        self.root_block() + 1
    }

    /// The geometry an image of `len` bytes must have, if this crate knows one
    /// that size. Only DD and HD share a length, so the mapping is
    /// unambiguous.
    pub(crate) const fn for_len(len: usize) -> Option<Geometry> {
        if len == DD.len() {
            Some(DD)
        } else if len == HD.len() {
            Some(HD)
        } else {
            None
        }
    }

    /// The logical block number of a CHS address, or `None` if the address
    /// falls outside this geometry.
    ///
    /// This is the whole of the conversion between the two ways of naming the
    /// same bytes: an ADF stores its sectors in exactly this order, so a track
    /// is a contiguous run of blocks and needs no gathering.
    pub(crate) const fn lba(self, cyl: u16, head: u8, sector: u8) -> Option<u32> {
        if cyl >= self.cylinders || head >= self.heads || sector >= self.sectors_per_track {
            return None;
        }
        Some(
            (cyl as u32 * self.heads as u32 + head as u32) * self.sectors_per_track as u32
                + sector as u32,
        )
    }
}

/// Blocks AmigaDOS reserves at the front of a floppy — the two boot sectors.
/// `de_Reserved` in Commodore's `rootblock.c`; see [`Geometry::root_block`].
pub(crate) const RESERVED: u32 = 2;
