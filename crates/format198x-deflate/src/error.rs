//! The one error type for decoding and encoding.

use core::fmt;

/// Why a stream could not be decoded or encoded.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The input ended before the stream did.
    UnexpectedEof,
    /// A block header names block type 3, which is reserved.
    ReservedBlockType,
    /// A stored block's length does not match its one's complement.
    StoredLengthMismatch {
        /// `LEN`.
        len: u16,
        /// `NLEN`.
        nlen: u16,
    },
    /// A dynamic block declares more than 286 literal/length codes or more
    /// than 30 distance codes.
    TooManyCodes,
    /// A dynamic block's code lengths do not form a usable prefix code:
    /// too many codes of some length, too few to cover every bit pattern,
    /// or no end-of-block code.
    BadCodeLengths,
    /// A dynamic block repeats a code length before any was given, or its
    /// lengths run past the number of codes it declared.
    BadLengthRepeat,
    /// The bits do not spell any code, or spell length code 286 or 287, or
    /// distance code 30 or 31.
    InvalidCode,
    /// A match reaches back further than the output so far.
    DistanceTooFar {
        /// The distance asked for.
        distance: usize,
        /// The bytes written so far.
        available: usize,
    },
    /// Decoding would produce more than [`Options::max_output`] bytes.
    ///
    /// [`Options::max_output`]: crate::Options::max_output
    OutputLimit {
        /// The limit.
        limit: usize,
    },
    /// Bytes follow the end of the stream and the options do not allow
    /// them.
    TrailingBytes {
        /// Offset of the first byte after the stream.
        offset: usize,
    },
    /// A zlib header fails its check bits or declares a window over 32K.
    BadZlibHeader,
    /// The wrapper names a compression method other than 8 (DEFLATE).
    UnsupportedMethod(u8),
    /// The zlib stream needs a preset dictionary, which this crate does not
    /// support.
    PresetDictionary,
    /// The zlib Adler-32 trailer does not match the decoded data.
    Adler32Mismatch {
        /// The checksum in the trailer.
        expected: u32,
        /// The checksum of the decoded data.
        actual: u32,
    },
    /// The input does not start with the gzip signature `1F 8B`.
    BadGzipMagic,
    /// The gzip header sets one of the reserved flag bits 5 to 7.
    ReservedGzipFlags(u8),
    /// The gzip header's own CRC-16 does not match.
    HeaderCrcMismatch {
        /// The CRC-16 in the header.
        expected: u16,
        /// The CRC-16 of the header bytes.
        actual: u16,
    },
    /// The gzip CRC-32 trailer does not match the decoded data.
    Crc32Mismatch {
        /// The checksum in the trailer.
        expected: u32,
        /// The checksum of the decoded data.
        actual: u32,
    },
    /// The gzip `ISIZE` trailer does not match the decoded length, modulo
    /// 2^32.
    SizeMismatch {
        /// The size in the trailer.
        expected: u32,
        /// The decoded length, modulo 2^32.
        actual: u32,
    },
    /// On encode: a gzip file name or comment contains a NUL byte, which
    /// would end it early.
    NulInHeaderField,
    /// On encode: a gzip extra field is longer than 65535 bytes.
    ExtraFieldTooLong,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "compressed data ends before the stream does"),
            Self::ReservedBlockType => write!(f, "DEFLATE block uses the reserved type 3"),
            Self::StoredLengthMismatch { len, nlen } => write!(
                f,
                "stored block length {len:#06x} does not match its complement {nlen:#06x}"
            ),
            Self::TooManyCodes => write!(f, "dynamic block declares too many codes"),
            Self::BadCodeLengths => {
                write!(f, "dynamic block code lengths do not form a prefix code")
            }
            Self::BadLengthRepeat => write!(f, "dynamic block code-length repeat is out of place"),
            Self::InvalidCode => write!(f, "invalid DEFLATE code"),
            Self::DistanceTooFar {
                distance,
                available,
            } => write!(
                f,
                "match distance {distance} reaches before the start of the output ({available} bytes)"
            ),
            Self::OutputLimit { limit } => {
                write!(f, "decoded data would exceed the {limit}-byte limit")
            }
            Self::TrailingBytes { offset } => {
                write!(f, "unexpected bytes after the stream at offset {offset}")
            }
            Self::BadZlibHeader => write!(f, "invalid zlib header"),
            Self::UnsupportedMethod(m) => write!(f, "unsupported compression method {m}"),
            Self::PresetDictionary => write!(f, "zlib stream needs a preset dictionary"),
            Self::Adler32Mismatch { expected, actual } => write!(
                f,
                "zlib Adler-32 mismatch: trailer {expected:#010x}, data {actual:#010x}"
            ),
            Self::BadGzipMagic => write!(f, "not gzip data: missing 1F 8B signature"),
            Self::ReservedGzipFlags(flags) => {
                write!(f, "gzip header sets reserved flags ({flags:#04x})")
            }
            Self::HeaderCrcMismatch { expected, actual } => write!(
                f,
                "gzip header CRC mismatch: stored {expected:#06x}, computed {actual:#06x}"
            ),
            Self::Crc32Mismatch { expected, actual } => write!(
                f,
                "gzip CRC-32 mismatch: trailer {expected:#010x}, data {actual:#010x}"
            ),
            Self::SizeMismatch { expected, actual } => {
                write!(f, "gzip size mismatch: trailer {expected}, data {actual}")
            }
            Self::NulInHeaderField => write!(f, "gzip name or comment contains a NUL byte"),
            Self::ExtraFieldTooLong => write!(f, "gzip extra field is longer than 65535 bytes"),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn every_error_has_a_message() {
        for e in [
            Error::UnexpectedEof,
            Error::ReservedBlockType,
            Error::StoredLengthMismatch { len: 1, nlen: 2 },
            Error::TooManyCodes,
            Error::BadCodeLengths,
            Error::BadLengthRepeat,
            Error::InvalidCode,
            Error::DistanceTooFar {
                distance: 2,
                available: 1,
            },
            Error::OutputLimit { limit: 1 },
            Error::TrailingBytes { offset: 1 },
            Error::BadZlibHeader,
            Error::UnsupportedMethod(7),
            Error::PresetDictionary,
            Error::Adler32Mismatch {
                expected: 1,
                actual: 2,
            },
            Error::BadGzipMagic,
            Error::ReservedGzipFlags(0x20),
            Error::HeaderCrcMismatch {
                expected: 1,
                actual: 2,
            },
            Error::Crc32Mismatch {
                expected: 1,
                actual: 2,
            },
            Error::SizeMismatch {
                expected: 1,
                actual: 2,
            },
            Error::NulInHeaderField,
            Error::ExtraFieldTooLong,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
