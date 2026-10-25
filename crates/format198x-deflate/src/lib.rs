//! DEFLATE compression ([RFC 1951]) with its zlib ([RFC 1950]) and gzip
//! ([RFC 1952]) wrappers: inflate and deflate, with no dependencies.
//!
//! Many retro formats store their payloads compressed with DEFLATE. A UEF
//! tape is usually gzipped whole, and SZX and RZX snapshots hold zlib
//! streams. This crate lets the Format198x crates read and write those files
//! without reaching outside the family.
//!
//! # Decoding
//!
//! [`inflate`] decodes a raw DEFLATE stream. [`zlib::decode`] and
//! [`gzip::decode`] also check the wrapper's header and its checksum:
//! Adler-32 for zlib, CRC-32 and length for gzip. Every failure is a typed
//! [`Error`]; no input, however broken, makes the decoder panic.
//!
//! Memory is bounded. [`Options::max_output`] caps the decoded size, so a
//! small hostile stream cannot expand without limit. Without a cap, output
//! can be at most about 1032 times the input, which is DEFLATE's own limit.
//!
//! # Encoding
//!
//! [`deflate`] compresses at a [`Level`] from 0 (stored blocks only) to 9.
//! It finds repeats with hash chains over the full 32K window, and for each
//! block it writes whichever of stored, fixed-Huffman and dynamic-Huffman
//! coding is shortest. Output is deterministic: the same input and level
//! always give the same bytes. [`zlib::encode`] and [`gzip::encode`] add the
//! wrappers.
//!
//! ```
//! use format198x_deflate::{Level, gzip, zlib};
//!
//! let text = b"UEF File!\0UEF File!\0UEF File!\0";
//! let packed = zlib::encode(text, Level::DEFAULT);
//! assert_eq!(zlib::decode(&packed)?, text);
//!
//! let header = gzip::Header { name: Some(b"tape.uef".to_vec()), ..Default::default() };
//! let gz = gzip::encode(&header, text, Level::BEST)?;
//! let member = gzip::decode(&gz)?;
//! assert_eq!(member.header.name.as_deref(), Some(&b"tape.uef"[..]));
//! assert_eq!(member.data, text);
//! # Ok::<(), format198x_deflate::Error>(())
//! ```
//!
//! # `no_std`
//!
//! The crate is `no_std` and needs only `alloc`.
//!
//! [RFC 1950]: https://www.rfc-editor.org/rfc/rfc1950
//! [RFC 1951]: https://www.rfc-editor.org/rfc/rfc1951
//! [RFC 1952]: https://www.rfc-editor.org/rfc/rfc1952

#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

mod bits;
mod checksum;
mod encode;
mod error;
mod huffman;
mod inflate;
mod tables;

pub mod gzip;
pub mod zlib;

pub use checksum::{adler32, crc32};
pub use encode::deflate;
pub use error::Error;
pub use inflate::{inflate, inflate_prefix, inflate_with};

/// How hard [`deflate`] tries to find repeats: 0 to 9.
///
/// Level 0 writes stored blocks only. Levels 1 to 9 search longer hash
/// chains, and from level 4 they also try a match one byte later before
/// taking the current one. Values above 9 act as 9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(u8);

impl Level {
    /// Level 0: stored blocks, no compression.
    pub const NONE: Self = Self(0);
    /// Level 1: the fastest compression.
    pub const FASTEST: Self = Self(1);
    /// Level 6: the usual balance, as zlib's default.
    pub const DEFAULT: Self = Self(6);
    /// Level 9: the smallest output.
    pub const BEST: Self = Self(9);

    /// A level from 0 to 9. Values above 9 become 9.
    #[must_use]
    pub const fn new(level: u8) -> Self {
        if level > 9 { Self(9) } else { Self(level) }
    }

    /// The level as a number from 0 to 9.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Limits and leniency for decoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Options {
    /// The most bytes the decoder may produce. Going past it is
    /// [`Error::OutputLimit`]. The default is `usize::MAX`.
    pub max_output: usize,
    /// Whether bytes after the end of the stream are allowed. When `false`,
    /// the default, they are [`Error::TrailingBytes`]. Some writers pad their
    /// streams, and some readers, such as zlib's own, ignore what follows.
    pub allow_trailing: bool,
}

impl Options {
    /// No output limit, and no trailing bytes allowed.
    pub const DEFAULT: Self = Self {
        max_output: usize::MAX,
        allow_trailing: false,
    };

    /// These options with a new output limit.
    #[must_use]
    pub const fn max_output(self, max_output: usize) -> Self {
        Self { max_output, ..self }
    }

    /// These options with trailing bytes allowed or not.
    #[must_use]
    pub const fn allow_trailing(self, allow_trailing: bool) -> Self {
        Self {
            allow_trailing,
            ..self
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_clamp_and_report() {
        assert_eq!(Level::new(12), Level::BEST);
        assert_eq!(Level::new(3).get(), 3);
        assert_eq!(Level::default(), Level::DEFAULT);
    }

    #[test]
    fn options_build() {
        let o = Options::default().max_output(10).allow_trailing(true);
        assert_eq!(o.max_output, 10);
        assert!(o.allow_trailing);
    }
}
