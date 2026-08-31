//! Atari 8-bit segmented executable (`.xex`) parser.
//!
//! A file is an optional/repeatable `$FFFF` marker followed by ordered
//! segments. Each segment stores an inclusive little-endian start/end address
//! and exactly `end - start + 1` payload bytes. The marker may appear between
//! segments, as accepted by Atari loaders.
//!
//! Execution is deliberately outside this crate. `RUNAD` (`$02E0`) and
//! `INITAD` (`$02E2`) are ordinary memory locations that segments may write;
//! a machine or tool decides how and when to honour them. Altirra's executable
//! loader (`ATDevices/source/exeloader.cpp`) is the implementation cross-check.

use core::fmt;

/// One ordered XEX load segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment<'a> {
    /// First destination address.
    pub start: u16,
    /// Last destination address, inclusive.
    pub end: u16,
    /// Bytes copied to `start..=end`.
    pub data: &'a [u8],
}

/// A validated XEX image borrowing its payloads from the source bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Xex<'a> {
    /// Segments in file/load order.
    pub segments: Vec<Segment<'a>>,
}

/// Why an XEX byte stream could not be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    /// No load segment was present.
    NoSegments,
    /// Fewer than four bytes remained for a segment header.
    TruncatedHeader { at: usize, available: usize },
    /// An inclusive range ended before it started.
    ReversedRange { at: usize, start: u16, end: u16 },
    /// A segment payload ended before its declared inclusive range.
    TruncatedSegment {
        at: usize,
        start: u16,
        end: u16,
        expected: usize,
        available: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSegments => f.write_str("XEX contains no load segments"),
            Self::TruncatedHeader { at, available } => write!(
                f,
                "XEX segment header at byte {at} has {available} bytes; expected 4"
            ),
            Self::ReversedRange { at, start, end } => write!(
                f,
                "XEX segment at byte {at} ends at ${end:04X} before ${start:04X}"
            ),
            Self::TruncatedSegment {
                at,
                start,
                end,
                expected,
                available,
            } => write!(
                f,
                "XEX segment at byte {at} (${start:04X}-${end:04X}) needs {expected} bytes; {available} remain"
            ),
        }
    }
}

impl core::error::Error for ParseError {}

/// Parse a segmented Atari executable.
///
/// # Errors
///
/// Returns [`ParseError`] for an empty image, incomplete header or payload, or
/// a segment whose end precedes its start.
pub fn parse(mut bytes: &[u8]) -> Result<Xex<'_>, ParseError> {
    let total = bytes.len();
    let mut segments = Vec::new();
    while !bytes.is_empty() {
        while bytes.starts_with(&[0xFF, 0xFF]) {
            bytes = &bytes[2..];
        }
        if bytes.is_empty() {
            break;
        }
        let at = total - bytes.len();
        if bytes.len() < 4 {
            return Err(ParseError::TruncatedHeader {
                at,
                available: bytes.len(),
            });
        }
        let start = u16::from_le_bytes([bytes[0], bytes[1]]);
        let end = u16::from_le_bytes([bytes[2], bytes[3]]);
        if end < start {
            return Err(ParseError::ReversedRange { at, start, end });
        }
        bytes = &bytes[4..];
        let len = usize::from(end - start) + 1;
        if bytes.len() < len {
            return Err(ParseError::TruncatedSegment {
                at,
                start,
                end,
                expected: len,
                available: bytes.len(),
            });
        }
        let (data, rest) = bytes.split_at(len);
        segments.push(Segment { start, end, data });
        bytes = rest;
    }
    if segments.is_empty() {
        return Err(ParseError::NoSegments);
    }
    Ok(Xex { segments })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ordered_segments_and_repeated_markers() {
        let bytes = [
            0xFF, 0xFF, 0x00, 0x20, 0x02, 0x20, 1, 2, 3, 0xFF, 0xFF, 0xE0, 0x02, 0xE1, 0x02, 0x00,
            0x20,
        ];
        let xex = parse(&bytes).expect("valid XEX");
        assert_eq!(xex.segments.len(), 2);
        assert_eq!(xex.segments[0].start, 0x2000);
        assert_eq!(xex.segments[0].data, &[1, 2, 3]);
        assert_eq!(xex.segments[1].start, 0x02E0);
        assert_eq!(xex.segments[1].data, &[0x00, 0x20]);
    }

    #[test]
    fn rejects_truncated_and_reversed_segments() {
        assert!(matches!(
            parse(&[0x00, 0x20, 0x02]),
            Err(ParseError::TruncatedHeader { .. })
        ));
        assert!(matches!(
            parse(&[0x01, 0x20, 0x00, 0x20]),
            Err(ParseError::ReversedRange { .. })
        ));
        assert!(matches!(
            parse(&[0x00, 0x20, 0x02, 0x20, 1]),
            Err(ParseError::TruncatedSegment { .. })
        ));
    }
}
