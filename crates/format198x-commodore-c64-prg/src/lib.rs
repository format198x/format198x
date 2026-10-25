//! Commodore `PRG` program files: read and write.
//!
//! A PRG file is the simplest Commodore binary format: a 2-byte little-endian
//! load address followed by the bytes that load there.
//!
//! ```text
//! offset  size  field
//! 0       2     load address (little-endian)
//! 2       N     payload
//! ```
//!
//! This crate covers the bytes only. Loading a PRG into a machine's RAM, and
//! what the KERNAL does afterwards (the `$AE/$AF` end pointer, relinking BASIC
//! text at `$0801`), is machine behaviour and belongs to the emulator.

/// Length of the load-address prefix.
const HEADER_LEN: usize = 2;

/// A parsed PRG file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrgFile {
    /// Load address from the PRG header.
    pub load_address: u16,
    /// Payload bytes after the 2-byte header.
    pub data: Vec<u8>,
}

/// Error surfaced while parsing a PRG file.
#[derive(Debug, PartialEq, Eq)]
pub enum PrgParseError {
    /// The file is shorter than the 2-byte load address.
    TooShort {
        /// The actual byte count.
        actual: usize,
    },
}

impl core::fmt::Display for PrgParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { actual } => write!(
                f,
                "PRG file is {actual} bytes; need at least {HEADER_LEN} for the load address"
            ),
        }
    }
}

impl std::error::Error for PrgParseError {}

impl PrgFile {
    /// Parses one PRG file from raw bytes.
    ///
    /// A file of exactly two bytes is a valid PRG with an empty payload.
    ///
    /// # Errors
    ///
    /// Returns [`PrgParseError::TooShort`] if the file is too short to hold
    /// the load address.
    pub fn parse(data: &[u8]) -> Result<Self, PrgParseError> {
        let [lo, hi, payload @ ..] = data else {
            return Err(PrgParseError::TooShort { actual: data.len() });
        };
        Ok(Self {
            load_address: u16::from_le_bytes([*lo, *hi]),
            data: payload.to_vec(),
        })
    }

    /// Encodes this PRG as file bytes: the load address, then the payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len());
        out.extend_from_slice(&self.load_address.to_le_bytes());
        out.extend_from_slice(&self.data);
        out
    }

    /// One past the last address the payload occupies, wrapping at `$FFFF`
    /// as the KERNAL's load pointer does.
    #[must_use]
    pub fn end_address(&self) -> u16 {
        // Truncation is the point: the address space wraps at 64K.
        #[allow(clippy::cast_possible_truncation)]
        let len = self.data.len() as u16;
        self.load_address.wrapping_add(len)
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
    fn parse_prg() {
        let prg = PrgFile::parse(&[0x00, 0xC0, 0x0A, 0x0B]).expect("parse should succeed");
        assert_eq!(prg.load_address, 0xC000);
        assert_eq!(prg.data, &[0x0A, 0x0B]);
        assert_eq!(prg.end_address(), 0xC002);
    }

    #[test]
    fn empty_payload_is_valid() {
        let prg = PrgFile::parse(&[0x01, 0x08]).expect("two bytes is a PRG");
        assert_eq!(prg.load_address, 0x0801);
        assert!(prg.data.is_empty());
    }

    #[test]
    fn rejects_files_without_a_load_address() {
        assert_eq!(
            PrgFile::parse(&[0x01]),
            Err(PrgParseError::TooShort { actual: 1 })
        );
        assert!(
            PrgParseError::TooShort { actual: 0 }
                .to_string()
                .contains("load address")
        );
    }

    #[test]
    fn encode_round_trips() {
        let raw = [
            0x01, 0x08, 0x0B, 0x08, 0x0A, 0x00, 0x9E, b'2', b'0', b'6', b'1', 0x00, 0x00, 0x00,
        ];
        let prg = PrgFile::parse(&raw).expect("parse");
        assert_eq!(prg.encode(), raw);
    }

    #[test]
    fn end_address_wraps() {
        let prg = PrgFile::parse(&[0xFF, 0xFF, 0x11, 0x22, 0x33]).expect("parse");
        assert_eq!(prg.end_address(), 2);
    }
}
