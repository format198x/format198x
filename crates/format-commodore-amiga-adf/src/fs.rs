use crate::layout::{BSIZE, OFS_DATA};

/// Which Amiga filesystem to write.
///
/// The two differ only in their data blocks: [`Ofs`](Self::Ofs) wraps each in a
/// 24-byte header (type/key/sequence/size/next/checksum), so a block holds 488
/// payload bytes and the file is a self-describing chain; [`Ffs`](Self::Ffs)
/// stores raw 512-byte sectors and relies entirely on the header/extension
/// pointer tables. The root, bitmap, directory, and file-header blocks are
/// identical between them.
///
/// **Boot compatibility:** an FFS floppy boots only on Kickstart 2.0+ — the 1.3
/// ROM's floppy filesystem is OFS-only. Target OFS for a bare A500/KS1.3; FFS
/// is for KS2.0+ machines (and is faster and denser there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum FileSystem {
    /// Old File System (`DOS\0`) — headered data blocks. Boots on KS1.3+.
    #[default]
    Ofs,
    /// Fast File System (`DOS\1`) — raw data sectors. Boots on KS2.0+.
    Ffs,
}

impl FileSystem {
    /// The lowercase short name — `"ofs"` or `"ffs"`. Handy for CLI output and
    /// logging without matching a `#[non_exhaustive]` enum.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ofs => "ofs",
            Self::Ffs => "ffs",
        }
    }

    /// The boot-block DOS-type byte (offset 3): 0 for OFS, 1 for FFS.
    pub(crate) fn dos_type(self) -> u8 {
        match self {
            Self::Ofs => 0,
            Self::Ffs => 1,
        }
    }

    /// Payload bytes per data block: OFS reserves 24 for the block header.
    pub(crate) fn data_capacity(self) -> usize {
        match self {
            Self::Ofs => OFS_DATA,
            Self::Ffs => BSIZE,
        }
    }
}
