//! Typed errors for [`crate::decrunch`].
//!
//! PP20 sits behind an FFI boundary in this family (Play198x consumes it from
//! outside Rust), where a panic is undefined behaviour rather than a caught
//! exception. Every field this crate reads from untrusted input — lengths,
//! bit-widths, back-reference offsets — is checked before use, and a
//! violation returns one of these variants instead of indexing past a slice
//! or overflowing a shift.

/// Why a byte stream failed to decrunch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input is shorter than a PP20 stream can be (12 bytes: 4-byte
    /// magic, 4-byte offset-length table, 4-byte trailer), or the backward
    /// bitstream ran out of source bytes before decompression finished.
    Truncated {
        /// What was being read when the input ran out.
        what: &'static str,
    },
    /// The input does not start with the `"PP20"` magic.
    BadMagic,
    /// A header or bitstream field held a value the format does not allow —
    /// an initial bit-skip or offset bit-width too wide to read safely, or a
    /// back-reference offset that would land at or past the end of the
    /// (not-yet-written) output buffer.
    Corrupt {
        /// What was found to be invalid.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { what } => {
                write!(f, "truncated PP20 stream: ran out of input reading {what}")
            }
            Self::BadMagic => {
                write!(f, "not a PowerPacker file: missing \"PP20\" magic")
            }
            Self::Corrupt { what } => write!(f, "corrupt PP20 stream: {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}
