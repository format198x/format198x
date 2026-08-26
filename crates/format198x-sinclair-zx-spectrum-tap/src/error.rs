//! Typed errors for [`crate::decode`] and [`crate::Header::from_payload`].
//!
//! Following the sibling codecs' shape (`format198x-sinclair-zx-spectrum-scr`'s
//! `error.rs`), with the variants this format can actually produce: TAP has no
//! magic number and no fixed total size, so the ways it goes wrong are a block
//! length that overruns the file, a stray byte after the last block, and a
//! header payload that is not what a header's payload has to be.

/// Why a byte stream failed to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A block's length field reaches past the end of the input.
    Truncated {
        /// The offset of the length field that claimed too much.
        at: usize,
        /// What it claimed.
        claimed: usize,
        /// What was left after it.
        available: usize,
    },
    /// One byte is left after the last block — too few for another length
    /// field, and so not the start of a block.
    TrailingByte {
        /// The offset of the odd byte.
        at: usize,
    },
    /// A header block's payload is not seventeen bytes.
    HeaderLength {
        /// The length it was.
        found: usize,
    },
    /// A header's first byte names no kind of block.
    HeaderKind {
        /// The byte.
        found: u8,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated {
                at,
                claimed,
                available,
            } => write!(
                f,
                "TAP block at offset {at} claims {claimed} bytes but only {available} remain"
            ),
            Self::TrailingByte { at } => {
                write!(f, "one byte left over at offset {at}, too few for a block")
            }
            Self::HeaderLength { found } => {
                write!(f, "a header's payload is 17 bytes, not {found}")
            }
            Self::HeaderKind { found } => {
                write!(f, "{found} names no kind of tape block (0..=3 do)")
            }
        }
    }
}

impl std::error::Error for DecodeError {}
