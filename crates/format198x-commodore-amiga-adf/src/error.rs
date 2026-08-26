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
        }
    }
}

impl std::error::Error for Error {}
