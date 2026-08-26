//! Typed errors for [`crate::decode`] and [`crate::encode`].
//!
//! Copied from build198x's shared `format::{DecodeError, EncodeError}`
//! (`crates/build198x/src/format/mod.rs`), keeping only the variants this
//! codec actually constructs — the SCR format has no magic number, no
//! chunked structure, and no out-of-range field values, so it never needs
//! the sibling variants (`Truncated`, `BadMagic`, `Unsupported`,
//! `DimensionsTooLarge`, `MissingChunk`, `Corrupt`, `ValueOutOfRange`) that
//! exist for the other codecs in that shared enum.

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
