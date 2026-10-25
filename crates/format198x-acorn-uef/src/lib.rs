//! Acorn UEF (Unified Emulator Format) images: read and write.
//!
//! UEF is the tape (and occasionally disc and state) container used for the
//! BBC Micro, Acorn Electron and Acorn Atom. A file is the 10-byte magic
//! `UEF File!\0`, a minor and a major version byte, then chunks: a
//! little-endian `u16` id, a `u32` length and that many bytes.
//!
//! [`Uef`] reads and writes the container losslessly, keeping every chunk
//! whether or not this crate understands it. [`Chunk::tape`] reads the
//! common tape chunks, and the [`Chunk`] constructors write them.
//!
//! # Not here
//!
//! - **The gzip layer.** UEF files are often gzip-compressed as a whole.
//!   That wrapper is gzip, not UEF, and this crate has no dependencies, so
//!   [`Uef::parse`] reports [`UefError::Gzipped`] and leaves inflating to
//!   the caller.
//! - **The waveform.** Turning chunks into Kansas City cycles at a machine's
//!   clock is the emulator's business.
//!
//! Chunk semantics follow the UEF specification and MAME's
//! `src/lib/formats/uef_cas.cpp`.

/// The 10-byte signature.
pub const MAGIC: [u8; 10] = *b"UEF File!\0";
/// The gzip signature, which a compressed UEF starts with.
const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];
/// Offset of the first chunk: the magic and two version bytes.
const FIRST_CHUNK: usize = MAGIC.len() + 2;
/// Chunk header: id and length.
const CHUNK_HEADER: usize = 6;

/// One chunk, kept as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Chunk id, such as `0x0100` for implicit-format data.
    pub id: u16,
    /// The chunk's payload.
    pub data: Vec<u8>,
}

/// A UEF image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uef {
    /// Minor version byte.
    pub minor: u8,
    /// Major version byte.
    pub major: u8,
    /// Chunks in file order.
    pub chunks: Vec<Chunk>,
    /// Bytes after the last whole chunk header, too few to be a chunk. Kept
    /// so that a file writes back as it was read.
    pub trailing: Vec<u8>,
}

/// The tape chunks this crate reads, borrowed from a [`Chunk`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TapeChunk<'a> {
    /// `&0100`: data bytes framed 8N1.
    ImplicitData(&'a [u8]),
    /// `&0104`: data bytes with their own framing.
    DefinedFormat {
        /// Data bits per byte.
        data_bits: u8,
        /// Parity as stored: `b'N'`, `b'E'` or `b'O'`.
        parity: u8,
        /// Stop bits; negative means an extra short high-tone cycle.
        stop_bits: i8,
        /// The data bytes.
        data: &'a [u8],
    },
    /// `&0110`: a carrier tone of so many high-frequency cycles.
    Carrier {
        /// Cycle count.
        cycles: u16,
    },
    /// `&0111`: carrier, a dummy `&AA` byte, then more carrier.
    CarrierWithDummy {
        /// Cycles before the dummy byte.
        before: u16,
        /// Cycles after it.
        after: u16,
    },
    /// `&0112`: a gap in units of half a base-frequency cycle.
    IntegerGap {
        /// Gap length in half-cycles.
        units: u16,
    },
    /// `&0113`: a new base frequency in hertz.
    BaseFrequency(f32),
    /// `&0116`: a gap in seconds.
    FloatGap(f32),
    /// `&0117`: a new baud rate.
    BaudRate(u16),
}

/// Why a UEF image or chunk could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UefError {
    /// Shorter than the magic and version bytes.
    TooSmall(usize),
    /// The image is gzip-compressed; inflate it first.
    Gzipped,
    /// The `UEF File!\0` magic is missing.
    BadMagic,
    /// A chunk claims more bytes than remain.
    TruncatedChunk {
        /// Chunk id.
        id: u16,
        /// Offset of the chunk header.
        offset: usize,
        /// Declared payload length.
        length: usize,
        /// Bytes left after the header.
        available: usize,
    },
    /// A tape chunk's payload is shorter than its fields.
    MalformedChunk {
        /// Chunk id.
        id: u16,
        /// Bytes the chunk needs.
        needed: usize,
    },
    /// On encode: a chunk is longer than a `u32` length can describe.
    ChunkTooLong {
        /// Chunk id.
        id: u16,
    },
}

impl core::fmt::Display for UefError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooSmall(v0) => write!(f, "file is too small to be a UEF image ({v0} bytes)"),
            Self::Gzipped => write!(f, "UEF image is gzip-compressed; inflate it first"),
            Self::BadMagic => write!(f, "missing UEF magic header"),
            Self::TruncatedChunk {
                id,
                offset,
                length,
                available,
            } => write!(
                f,
                "chunk {id:#06x} at offset {offset} claims {length} payload bytes but only {available} remain"
            ),
            Self::MalformedChunk { id, needed } => {
                write!(f, "chunk {id:#06x} needs at least {needed} payload bytes")
            }
            Self::ChunkTooLong { id } => {
                write!(f, "chunk {id:#06x} is too long for a 32-bit length")
            }
        }
    }
}

impl std::error::Error for UefError {}

/// Whether `data` starts with the gzip signature, as a compressed UEF does.
#[must_use]
pub fn is_gzipped(data: &[u8]) -> bool {
    data.starts_with(&GZIP_MAGIC)
}

fn u16_at(data: &[u8], id: u16, at: usize) -> Result<u16, UefError> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or(UefError::MalformedChunk { id, needed: at + 2 })
}

fn f32_at(data: &[u8], id: u16) -> Result<f32, UefError> {
    data.get(..4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(UefError::MalformedChunk { id, needed: 4 })
}

impl Chunk {
    /// A chunk with the given id and payload.
    #[must_use]
    pub fn new(id: u16, data: Vec<u8>) -> Self {
        Self { id, data }
    }

    /// `&0100`: implicit-format (8N1) data.
    #[must_use]
    pub fn implicit_data(data: &[u8]) -> Self {
        Self::new(0x0100, data.to_vec())
    }

    /// `&0104`: defined-format data.
    #[must_use]
    pub fn defined_format(data_bits: u8, parity: u8, stop_bits: i8, data: &[u8]) -> Self {
        let mut payload = vec![data_bits, parity, stop_bits.to_le_bytes()[0]];
        payload.extend_from_slice(data);
        Self::new(0x0104, payload)
    }

    /// `&0110`: carrier tone.
    #[must_use]
    pub fn carrier(cycles: u16) -> Self {
        Self::new(0x0110, cycles.to_le_bytes().to_vec())
    }

    /// `&0111`: carrier with a dummy byte.
    #[must_use]
    pub fn carrier_with_dummy(before: u16, after: u16) -> Self {
        let mut payload = before.to_le_bytes().to_vec();
        payload.extend_from_slice(&after.to_le_bytes());
        Self::new(0x0111, payload)
    }

    /// `&0112`: integer gap.
    #[must_use]
    pub fn integer_gap(units: u16) -> Self {
        Self::new(0x0112, units.to_le_bytes().to_vec())
    }

    /// `&0113`: base-frequency change.
    #[must_use]
    pub fn base_frequency(hz: f32) -> Self {
        Self::new(0x0113, hz.to_le_bytes().to_vec())
    }

    /// `&0116`: floating-point gap.
    #[must_use]
    pub fn float_gap(seconds: f32) -> Self {
        Self::new(0x0116, seconds.to_le_bytes().to_vec())
    }

    /// `&0117`: baud-rate change.
    #[must_use]
    pub fn baud_rate(baud: u16) -> Self {
        Self::new(0x0117, baud.to_le_bytes().to_vec())
    }

    /// Read this chunk as one of the common tape chunks. `Ok(None)` for any
    /// other id.
    ///
    /// # Errors
    ///
    /// Returns [`UefError::MalformedChunk`] when the payload is shorter than
    /// the chunk's fields.
    pub fn tape(&self) -> Result<Option<TapeChunk<'_>>, UefError> {
        let id = self.id;
        let d = &self.data;
        Ok(Some(match id {
            0x0100 => TapeChunk::ImplicitData(d),
            0x0104 => {
                let [data_bits, parity, stop, data @ ..] = d.as_slice() else {
                    return Err(UefError::MalformedChunk { id, needed: 3 });
                };
                TapeChunk::DefinedFormat {
                    data_bits: *data_bits,
                    parity: *parity,
                    stop_bits: i8::from_le_bytes([*stop]),
                    data,
                }
            }
            0x0110 => TapeChunk::Carrier {
                cycles: u16_at(d, id, 0)?,
            },
            0x0111 => TapeChunk::CarrierWithDummy {
                before: u16_at(d, id, 0)?,
                after: u16_at(d, id, 2)?,
            },
            0x0112 => TapeChunk::IntegerGap {
                units: u16_at(d, id, 0)?,
            },
            0x0113 => TapeChunk::BaseFrequency(f32_at(d, id)?),
            0x0116 => TapeChunk::FloatGap(f32_at(d, id)?),
            0x0117 => TapeChunk::BaudRate(u16_at(d, id, 0)?),
            _ => return Ok(None),
        }))
    }
}

impl Uef {
    /// An empty image with the given version.
    #[must_use]
    pub fn new(minor: u8, major: u8) -> Self {
        Self {
            minor,
            major,
            chunks: Vec::new(),
            trailing: Vec::new(),
        }
    }

    /// Parse an uncompressed UEF image.
    ///
    /// # Errors
    ///
    /// Returns [`UefError::Gzipped`] for a compressed image, and an error
    /// when the magic is missing or a chunk runs past the end.
    pub fn parse(data: &[u8]) -> Result<Self, UefError> {
        if is_gzipped(data) {
            return Err(UefError::Gzipped);
        }
        if data.len() < FIRST_CHUNK {
            return Err(UefError::TooSmall(data.len()));
        }
        if data[..MAGIC.len()] != MAGIC {
            return Err(UefError::BadMagic);
        }
        let mut uef = Self::new(data[10], data[11]);
        let mut pos = FIRST_CHUNK;
        while pos + CHUNK_HEADER <= data.len() {
            let id = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let length =
                u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]]);
            let start = pos + CHUNK_HEADER;
            let available = data.len() - start;
            let length = usize::try_from(length).unwrap_or(usize::MAX);
            if length > available {
                return Err(UefError::TruncatedChunk {
                    id,
                    offset: pos,
                    length,
                    available,
                });
            }
            uef.chunks
                .push(Chunk::new(id, data[start..start + length].to_vec()));
            pos = start + length;
        }
        uef.trailing = data[pos..].to_vec();
        Ok(uef)
    }

    /// Encode the image, uncompressed.
    ///
    /// # Errors
    ///
    /// Returns [`UefError::ChunkTooLong`] for a chunk over 4 GiB.
    pub fn encode(&self) -> Result<Vec<u8>, UefError> {
        let mut out = MAGIC.to_vec();
        out.push(self.minor);
        out.push(self.major);
        for chunk in &self.chunks {
            let len = u32::try_from(chunk.data.len())
                .map_err(|_| UefError::ChunkTooLong { id: chunk.id })?;
            out.extend_from_slice(&chunk.id.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&chunk.data);
        }
        out.extend_from_slice(&self.trailing);
        Ok(out)
    }
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Uef {
        let mut uef = Uef::new(10, 0);
        uef.chunks = vec![
            Chunk::baud_rate(300),
            Chunk::carrier(2400),
            Chunk::implicit_data(&[0x2A, 0x2A, 0x2A]),
            Chunk::integer_gap(720),
            Chunk::carrier_with_dummy(10, 20),
            Chunk::defined_format(7, b'E', -1, &[1, 2]),
            Chunk::base_frequency(1200.0),
            Chunk::float_gap(0.5),
            Chunk::new(0x0000, b"origin\0".to_vec()),
        ];
        uef
    }

    #[test]
    fn the_container_round_trips_byte_for_byte() {
        let uef = sample();
        let bytes = uef.encode().expect("encodes");
        assert!(bytes.starts_with(b"UEF File!\0\x0a\x00"));
        assert_eq!(&bytes[12..20], &[0x17, 0x01, 2, 0, 0, 0, 0x2C, 0x01]);
        let back = Uef::parse(&bytes).expect("parses");
        assert_eq!(back, uef);
        assert_eq!(back.encode().expect("encodes"), bytes);
    }

    #[test]
    fn tape_chunks_read_back_as_written() {
        let uef = sample();
        let tape: Vec<_> = uef
            .chunks
            .iter()
            .map(|c| c.tape().expect("well formed"))
            .collect();
        assert_eq!(tape[0], Some(TapeChunk::BaudRate(300)));
        assert_eq!(tape[1], Some(TapeChunk::Carrier { cycles: 2400 }));
        assert_eq!(tape[2], Some(TapeChunk::ImplicitData(&[0x2A; 3])));
        assert_eq!(tape[3], Some(TapeChunk::IntegerGap { units: 720 }));
        assert_eq!(
            tape[4],
            Some(TapeChunk::CarrierWithDummy {
                before: 10,
                after: 20
            })
        );
        assert_eq!(
            tape[5],
            Some(TapeChunk::DefinedFormat {
                data_bits: 7,
                parity: b'E',
                stop_bits: -1,
                data: &[1, 2]
            })
        );
        assert_eq!(tape[6], Some(TapeChunk::BaseFrequency(1200.0)));
        assert_eq!(tape[7], Some(TapeChunk::FloatGap(0.5)));
        assert_eq!(tape[8], None);
    }

    /// The minimal tape from Emu198x's crate docs: a 4-cycle carrier.
    #[test]
    fn reads_the_minimal_carrier_tape() {
        let mut image = b"UEF File!\0\x0a\x00".to_vec();
        image.extend_from_slice(&[0x10, 0x01, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00]);
        let uef = Uef::parse(&image).expect("parses");
        assert_eq!(
            uef.chunks[0].tape(),
            Ok(Some(TapeChunk::Carrier { cycles: 4 }))
        );
    }

    #[test]
    fn short_chunks_and_trailing_bytes() {
        let mut bytes = Uef::new(10, 0).encode().expect("encodes");
        bytes.extend_from_slice(&[1, 2, 3]);
        let uef = Uef::parse(&bytes).expect("parses");
        assert_eq!(uef.trailing, [1, 2, 3]);
        assert_eq!(uef.encode().expect("encodes"), bytes);
        for id in [0x0104u16, 0x0110, 0x0111, 0x0112, 0x0113, 0x0116, 0x0117] {
            let chunk = Chunk::new(id, vec![0]);
            assert!(matches!(chunk.tape(), Err(UefError::MalformedChunk { .. })));
        }
    }

    #[test]
    fn bad_images_are_rejected() {
        assert_eq!(Uef::parse(&[0x1F, 0x8B, 8, 0]), Err(UefError::Gzipped));
        assert!(is_gzipped(&[0x1F, 0x8B]));
        assert_eq!(Uef::parse(b"UEF"), Err(UefError::TooSmall(3)));
        assert_eq!(Uef::parse(b"NOT A UEF!\0\0"), Err(UefError::BadMagic));
        let mut bytes = Uef::new(10, 0).encode().expect("encodes");
        bytes.extend_from_slice(&[0x00, 0x01, 9, 0, 0, 0, 1]);
        assert_eq!(
            Uef::parse(&bytes),
            Err(UefError::TruncatedChunk {
                id: 0x0100,
                offset: 12,
                length: 9,
                available: 1
            })
        );
        for e in [
            UefError::TooSmall(1),
            UefError::Gzipped,
            UefError::BadMagic,
            UefError::MalformedChunk { id: 1, needed: 2 },
            UefError::ChunkTooLong { id: 1 },
            UefError::TruncatedChunk {
                id: 1,
                offset: 2,
                length: 3,
                available: 4,
            },
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
