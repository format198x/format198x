/// Why an ADF operation failed.
///
/// The write path validates its inputs and the read path validates the image,
/// rather than panicking. Marked `#[non_exhaustive]` so later filesystem
/// variants can add error kinds without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A file or volume name is empty, longer than 30 bytes, or not
    /// AmigaDOS-legal (ASCII only).
    InvalidName {
        /// Which name was rejected — e.g. `"file name"`, `"volume name"`.
        what: &'static str,
        /// The length supplied.
        len: usize,
    },
    /// The content does not fit on a double-density floppy. Counts are in
    /// 512-byte blocks.
    DiskFull {
        /// Blocks the content requires.
        needed: u32,
        /// Blocks a DD floppy leaves free for the file tree.
        available: u32,
    },
    /// A path could not be used — on the write side it is empty, already
    /// exists, or routes a directory through a file; on the read side it names
    /// the wrong kind (a file where a directory was expected, or vice versa).
    BadPath {
        /// The offending path.
        path: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// The image is not a valid ADF — wrong size, unknown filesystem, a block
    /// pointer out of range, a structural loop, or a bad checksum ([`Disk`](crate::Disk)).
    Corrupt {
        /// What was malformed.
        what: &'static str,
    },
    /// A path was not found in the volume ([`Disk::read`](crate::Disk::read), [`Disk::list`](crate::Disk::list)).
    NotFound {
        /// The path that did not resolve.
        path: String,
    },
    /// A CHS or block address lies outside the image's geometry
    /// ([`Image`](crate::Image), [`ImageMut`](crate::ImageMut)).
    ///
    /// The raw layer returns this rather than indexing and panicking, because
    /// this crate is destined for an FFI boundary where unwinding is undefined
    /// behaviour.
    OutOfBounds {
        /// Which coordinate was out of range — `"cylinder"`, `"head"`,
        /// `"sector"` or `"block"`.
        what: &'static str,
        /// The value asked for.
        got: u32,
        /// One past the last valid value.
        limit: u32,
    },
    /// A sector write supplied something other than a whole 512-byte sector
    /// ([`ImageMut::write_sector`](crate::ImageMut::write_sector)).
    BadSectorLength {
        /// The length supplied.
        got: usize,
    },
    /// The image is a shape this crate reads at the raw layer but whose
    /// filesystem layout it does not claim ([`Disk::from_image`](crate::Disk::from_image)).
    ///
    /// Sectors, tracks and blocks are still reachable through
    /// [`Image`](crate::Image); it is only the AmigaDOS interpretation of them
    /// that is withheld.
    UnsupportedGeometry {
        /// The shape, named — e.g. `"high-density"`.
        shape: &'static str,
    },
    /// The image is a disk image in a container this crate does not read
    /// ([`Disk::open`](crate::Disk::open)). Named rather than measured: an IPF
    /// or a `.adz` told its *size* is wrong sends the reader hunting a
    /// truncated ADF that never existed.
    UnsupportedContainer {
        /// Short name of the format the leading bytes identify — e.g.
        /// `"IPF"`, `"gzip"`.
        format: &'static str,
        /// What it is, in a clause that finishes "…, which this crate does
        /// not read".
        detail: &'static str,
    },
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName { what, len } => {
                write!(f, "{what}: must be 1..=30 ASCII bytes (got {len})")
            }
            Self::DiskFull { needed, available } => write!(
                f,
                "disk full: {needed} blocks needed, {available} free on an 880K floppy"
            ),
            Self::BadPath { path, reason } => write!(f, "bad path {path:?}: {reason}"),
            Self::Corrupt { what } => write!(f, "corrupt ADF: {what}"),
            Self::NotFound { path } => write!(f, "not found: {path:?}"),
            Self::OutOfBounds { what, got, limit } => {
                write!(f, "{what} {got} is out of range (0..{limit})")
            }
            Self::BadSectorLength { got } => {
                write!(f, "a sector is 512 bytes (got {got})")
            }
            Self::UnsupportedGeometry { shape } => write!(
                f,
                "{shape} media: this crate reads its sectors but does not read its filesystem"
            ),
            Self::UnsupportedContainer { format, detail } => write!(
                f,
                "not an ADF: the file is {format} — {detail}, which this crate does not read"
            ),
        }
    }
}

impl std::error::Error for Error {}
