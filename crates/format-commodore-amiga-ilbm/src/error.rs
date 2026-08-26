//! Typed errors for [`crate::decode`] and [`crate::encode`].
//!
//! Copied from build198x's shared `format::{DecodeError, EncodeError}`
//! (`crates/build198x/src/format/mod.rs`), keeping only the variants this
//! codec actually constructs. ILBM is a chunked container rather than a
//! fixed-size dump, so its needs differ from the other three graduated
//! codecs: `DecodeError` keeps `Truncated`, `BadMagic`, `Unsupported`,
//! `DimensionsTooLarge`, `MissingChunk`, and `Corrupt`, but never
//! `WrongLength` — a chunked format has no single fixed length to check,
//! so every size failure is `Truncated` or `Corrupt` instead.
//! `EncodeError` keeps both `WrongLength` (the pixel buffer must match
//! `width × height`) and `ValueOutOfRange` (dimensions, plane count,
//! palette size, and pixel indices are all range-checked).

/// Why a byte stream failed to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended before the named structure was complete.
    Truncated {
        /// The structure that ran past the end of the input.
        what: &'static str,
    },
    /// A magic number / signature did not match the format.
    BadMagic {
        /// The signature that failed to match.
        what: &'static str,
    },
    /// A header field holds a value this codec does not support.
    Unsupported {
        /// The field in question.
        what: &'static str,
        /// The value found.
        value: u32,
    },
    /// Declared dimensions exceed the sanity cap ([`crate::MAX_DIMENSION`]).
    DimensionsTooLarge {
        /// Declared width in pixels.
        width: u16,
        /// Declared height in pixels.
        height: u16,
    },
    /// A chunk the format requires never appeared.
    MissingChunk {
        /// The four-character chunk ID, as ASCII.
        id: &'static str,
    },
    /// The input is structurally inconsistent — e.g. a compressed run that
    /// overruns its scanline, or a chunk whose payload contradicts its size.
    Corrupt {
        /// What was inconsistent.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { what } => write!(f, "input truncated inside {what}"),
            Self::BadMagic { what } => write!(f, "bad magic: {what} did not match"),
            Self::Unsupported { what, value } => {
                write!(f, "unsupported {what}: {value}")
            }
            Self::DimensionsTooLarge { width, height } => {
                write!(
                    f,
                    "declared dimensions {width}x{height} exceed the sanity cap"
                )
            }
            Self::MissingChunk { id } => write!(f, "required chunk {id} missing"),
            Self::Corrupt { what } => write!(f, "corrupt input: {what}"),
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
    /// A field or pixel value is outside the format's representable range.
    ValueOutOfRange {
        /// The field or value in question.
        what: &'static str,
        /// The offending value.
        value: u32,
        /// The smallest allowed value.
        min: u32,
        /// The largest allowed value.
        max: u32,
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
            Self::ValueOutOfRange {
                what,
                value,
                min,
                max,
            } => {
                write!(f, "{what} = {value} outside allowed range {min}..={max}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}
