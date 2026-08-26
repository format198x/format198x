//! Typed errors for [`crate::decode`] and [`crate::encode`].
//!
//! This crate sits behind an FFI boundary in the wider Play198x player,
//! where unwinding across the boundary is undefined behaviour. Every field
//! [`decode`](crate::decode) reads from untrusted bytes — a song length, an
//! order-table entry, a sample length — is range-checked before use, and a
//! violation returns one of [`DecodeError`]'s variants instead of indexing
//! past a slice or panicking on a bad length. [`encode`](crate::encode)
//! takes a well-typed [`Module`](crate::Module) rather than raw bytes, but
//! still rejects a shape the file format cannot represent (the wrong sample
//! count, a pattern that isn't 64 rows, a note field too wide for its
//! nibble) with [`EncodeError`] rather than silently truncating it.

/// Why a byte stream failed to decode as a ProTracker module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The input is shorter than the fixed header this format requires (20-byte
    /// title, 31×30-byte sample headers, song length, restart byte, 128-byte
    /// order table, 4-byte magic — 1084 bytes), or shorter than the pattern or
    /// sample data the header declares.
    Truncated {
        /// What was being read when the input ran out.
        what: &'static str,
    },
    /// The 4 bytes at offset 1080 do not match any recognised ProTracker
    /// magic (`M.K.`, `M!K!`, `FLT4`, `4CHN`, `6CHN`, `8CHN`, `FLT8`).
    BadMagic,
    /// The magic identifies a 6- or 8-channel module (`6CHN`, `8CHN`, or
    /// Startrekker's `FLT8`).
    /// [`is_module`](crate::is_module) recognises these as ProTracker
    /// modules — the sniff is not a promise of decodability — but
    /// [`Module`](crate::Module)'s pattern rows are fixed at 4 channels, so
    /// this crate cannot represent them without corrupting the pattern
    /// layout. Decoding a wider module is unimplemented, not silently wrong.
    UnsupportedChannelCount {
        /// The magic bytes that were found.
        magic: [u8; 4],
    },
    /// A header field held a value the format does not allow — a song
    /// length longer than the 128-entry order table can hold, or an
    /// arithmetic overflow while computing pattern or sample data extents.
    Corrupt {
        /// What was found to be invalid.
        what: &'static str,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Truncated { what } => {
                write!(f, "truncated MOD module: ran out of input reading {what}")
            }
            Self::BadMagic => {
                write!(
                    f,
                    "not a ProTracker module: no recognised magic at offset 1080"
                )
            }
            Self::UnsupportedChannelCount { magic } => write!(
                f,
                "unsupported channel count: magic {:?} is not a 4-channel module",
                String::from_utf8_lossy(magic)
            ),
            Self::Corrupt { what } => write!(f, "corrupt MOD module: {what}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why a [`Module`](crate::Module) could not be encoded as ProTracker MOD
/// bytes.
///
/// Most header fields are raw bytes/words by the time they reach `encode`
/// (see the crate documentation's Losslessness section), so there is
/// nothing to validate for them — they write back unconditionally. Only
/// values `encode` still has to *compute* (a sample's length in words from
/// `data.len()`, the pattern byte count) can fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// A ProTracker module always has exactly 31 sample slots.
    WrongSampleCount {
        /// How many samples the module actually had.
        found: usize,
    },
    /// Every pattern is exactly 64 rows.
    WrongPatternRows {
        /// The offending pattern's index.
        pattern: usize,
        /// How many rows it actually had.
        found: usize,
    },
    /// A sample's data could not be represented: its length is odd (sample
    /// lengths are stored in 16-bit words) or too long for the 16-bit word
    /// count the header field holds.
    SampleDataInvalid {
        /// The offending sample's index (0..31).
        index: usize,
    },
    /// A note's period or effect number does not fit the nibble widths a
    /// pattern cell has for them (period: 12 bits, effect: 4 bits).
    NoteOutOfRange {
        /// The pattern index the note is in.
        pattern: usize,
        /// The row index within the pattern.
        row: usize,
        /// The channel index within the row.
        channel: usize,
    },
    /// The pattern data would be too large to address (`patterns.len() *
    /// 1024` overflowed).
    PatternDataTooLarge,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongSampleCount { found } => {
                write!(f, "a MOD module needs exactly 31 samples, found {found}")
            }
            Self::WrongPatternRows { pattern, found } => {
                write!(f, "pattern {pattern} has {found} rows, expected exactly 64")
            }
            Self::SampleDataInvalid { index } => write!(
                f,
                "sample {index}'s data length cannot be represented as a 16-bit word count"
            ),
            Self::NoteOutOfRange {
                pattern,
                row,
                channel,
            } => write!(
                f,
                "pattern {pattern} row {row} channel {channel}: period or effect out of range"
            ),
            Self::PatternDataTooLarge => write!(f, "pattern data is too large to address"),
        }
    }
}

impl std::error::Error for EncodeError {}
