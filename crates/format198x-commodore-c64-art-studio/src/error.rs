//! Typed errors for [`crate::decode`] and [`crate::encode`].
//!
//! Copied from build198x's shared `format::{DecodeError, EncodeError}`
//! (`crates/build198x/src/format/mod.rs`), keeping only the variants this
//! codec actually constructs — Art Studio validates its length range and
//! its $2000 load address, so it needs `WrongLength` and `BadMagic`, but
//! never the sibling variants (`Truncated`, `Unsupported`,
//! `DimensionsTooLarge`, `MissingChunk`, `Corrupt` on the decode side;
//! `ValueOutOfRange` on the encode side) that exist for the other codecs in
//! that shared enum.

/// Why a byte stream failed to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input is not a size the fixed-layout format allows.
    WrongLength {
        /// Which format/section was being decoded.
        what: &'static str,
        /// Human-readable statement of the allowed size(s).
        expected: &'static str,
        /// The size actually supplied.
        actual: usize,
    },
    /// A magic number / signature did not match the format.
    BadMagic {
        /// The signature that failed to match.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected} bytes, got {actual}")
            }
            Self::BadMagic { what } => write!(f, "bad magic: {what} did not match"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why an encode input was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    /// A buffer in the input struct is not the length the format requires.
    WrongLength {
        /// Which buffer was the wrong length.
        what: &'static str,
        /// The required length.
        expected: usize,
        /// The length actually supplied.
        actual: usize,
    },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLength {
                what,
                expected,
                actual,
            } => {
                write!(f, "{what}: expected {expected} bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}
