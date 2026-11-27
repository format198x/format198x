//! Atari 8-bit segmented executable (`.xex`) codec.
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

/// Why ordered load segments could not be written as XEX.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// No load segment was supplied.
    NoSegments,
    /// A segment starts with the reserved $FFFF file marker.
    MarkerAddress { index: usize },
    /// An inclusive range ended before it started.
    ReversedRange { index: usize, start: u16, end: u16 },
    /// The payload length differs from the inclusive address range.
    LengthMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSegments => f.write_str("XEX requires at least one load segment"),
            Self::MarkerAddress { index } => {
                write!(f, "XEX segment {index} starts at the reserved $FFFF marker")
            }
            Self::ReversedRange { index, start, end } => write!(
                f,
                "XEX segment {index} ends at ${end:04X} before ${start:04X}"
            ),
            Self::LengthMismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "XEX segment {index} needs {expected} payload bytes; got {actual}"
            ),
        }
    }
}

impl core::error::Error for EncodeError {}

/// Write one leading `$FFFF` marker followed by the supplied load segments.
///
/// Order and overlaps are preserved, including explicit writes to `RUNAD` and
/// `INITAD`. A range may end at `$FFFF`. A start at `$FFFF` is reserved for
/// the marker.
///
/// # Errors
/// Returns [`EncodeError`] for an empty segment list, a reserved start address,
/// a reversed range, or a payload whose length does not match its range.
pub fn encode(segments: &[Segment<'_>]) -> Result<Vec<u8>, EncodeError> {
    if segments.is_empty() {
        return Err(EncodeError::NoSegments);
    }

    let mut bytes = vec![0xff, 0xff];
    for (index, segment) in segments.iter().enumerate() {
        let Segment { start, end, data } = *segment;
        if start == u16::MAX {
            return Err(EncodeError::MarkerAddress { index });
        }
        let span =
            end.checked_sub(start)
                .ok_or(EncodeError::ReversedRange { index, start, end })?;
        let expected = usize::from(span) + 1;
        if data.len() != expected {
            return Err(EncodeError::LengthMismatch {
                index,
                expected,
                actual: data.len(),
            });
        }
        bytes.extend_from_slice(&start.to_le_bytes());
        bytes.extend_from_slice(&end.to_le_bytes());
        bytes.extend_from_slice(data);
    }
    Ok(bytes)
}

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
    fn encodes_the_same_order_and_vectors_as_the_ld65_atari_writer() {
        // ld65 2fa92f9, FILES { %O: format = atari; }, with INITAD for the
        // $2000 area (RTS) and RUNAD at $3000 (LDA #$2A; STA $0600; RTS).
        let native = [
            0xff, 0xff, 0x00, 0x20, 0x00, 0x20, 0x60, 0xe2, 0x02, 0xe3, 0x02, 0x00, 0x20, 0x00,
            0x30, 0x05, 0x30, 0xa9, 0x2a, 0x8d, 0x00, 0x06, 0x60, 0xe0, 0x02, 0xe1, 0x02, 0x00,
            0x30,
        ];
        let segments = [
            Segment {
                start: 0x2000,
                end: 0x2000,
                data: &[0x60],
            },
            Segment {
                start: 0x02e2,
                end: 0x02e3,
                data: &[0x00, 0x20],
            },
            Segment {
                start: 0x3000,
                end: 0x3005,
                data: &[0xa9, 0x2a, 0x8d, 0x00, 0x06, 0x60],
            },
            Segment {
                start: 0x02e0,
                end: 0x02e1,
                data: &[0x00, 0x30],
            },
        ];
        assert_eq!(encode(&segments).expect("encode segments"), native);
    }

    #[test]
    fn encodes_overlaps_in_order_and_allows_the_top_address_as_an_end() {
        let segments = [
            Segment {
                start: 0xfffd,
                end: 0xffff,
                data: &[1, 2, 3],
            },
            Segment {
                start: 0xfffe,
                end: 0xffff,
                data: &[4, 5],
            },
        ];
        let bytes = encode(&segments).expect("encode overlapping segments");
        assert_eq!(parse(&bytes).expect("parse segments").segments, segments);
    }

    #[test]
    fn rejects_unencodable_ranges_and_payloads() {
        assert_eq!(encode(&[]), Err(EncodeError::NoSegments));
        let segment = Segment {
            start: 0xfffe,
            end: 0xffff,
            data: &[1, 2],
        };
        for data in [&[][..], &[1, 2, 3][..]] {
            assert_eq!(
                encode(&[Segment { data, ..segment }]),
                Err(EncodeError::LengthMismatch {
                    index: 0,
                    expected: 2,
                    actual: data.len()
                })
            );
        }
        assert_eq!(
            encode(&[Segment { end: 0, ..segment }]),
            Err(EncodeError::ReversedRange {
                index: 0,
                start: 0xfffe,
                end: 0
            })
        );
        assert_eq!(
            encode(&[Segment {
                start: 0xffff,
                ..segment
            }]),
            Err(EncodeError::MarkerAddress { index: 0 })
        );
    }

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
