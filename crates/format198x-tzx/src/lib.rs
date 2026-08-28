//! Lossless TZX/CDT block-stream decoding and encoding.
//!
//! TZX and CDT use the same bytes. This crate owns that neutral byte grammar:
//! the file signature, version and boundaries of the standard block types. It
//! does not turn pulses into machine time. Spectrum and CPC players interpret
//! the decoded payloads against their respective clocks.

use core::fmt;

const SIGNATURE: &[u8; 8] = b"ZXTape!\x1a";

/// A TZX format version as declared in the file header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

impl Version {
    /// Creates a version declaration.
    #[must_use]
    pub const fn new(major: u8, minor: u8) -> Self {
        Self { major, minor }
    }
}

/// One framed TZX block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    id: u8,
    payload: Vec<u8>,
}

impl Block {
    /// Creates a known block after validating its payload framing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnknownBlock`] for an unsupported ID and
    /// [`Error::Truncated`] when the payload does not have exactly the length
    /// its fields declare.
    pub fn new(id: u8, payload: Vec<u8>) -> Result<Self, Error> {
        let found = framed_len(id, &payload, 0)?;
        if found != payload.len() {
            return Err(Error::TrailingBlockData {
                id,
                expected: found,
                found: payload.len(),
            });
        }
        Ok(Self { id, payload })
    }

    /// Creates block `$10`, standard-speed data with a post-block pause.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BlockTooLong`] when `data` does not fit TZX's 16-bit
    /// standard-speed length field.
    pub fn standard_speed(pause_ms: u16, data: &[u8]) -> Result<Self, Error> {
        let length = u16::try_from(data.len()).map_err(|_| Error::BlockTooLong {
            id: 0x10,
            length: data.len(),
            maximum: usize::from(u16::MAX),
        })?;
        let mut payload = Vec::with_capacity(4 + data.len());
        payload.extend_from_slice(&pause_ms.to_le_bytes());
        payload.extend_from_slice(&length.to_le_bytes());
        payload.extend_from_slice(data);
        Ok(Self { id: 0x10, payload })
    }

    /// The block type byte.
    #[must_use]
    pub const fn id(&self) -> u8 {
        self.id
    }

    /// Bytes following the type byte, including the block's own length fields.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// A complete TZX/CDT image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tzx {
    /// Header version declaration.
    pub version: Version,
    /// Blocks in file order.
    pub blocks: Vec<Block>,
}

impl Tzx {
    /// Creates an image from a version and already-validated blocks.
    #[must_use]
    pub const fn new(version: Version, blocks: Vec<Block>) -> Self {
        Self { version, blocks }
    }
}

/// Why a TZX/CDT byte stream or block could not be represented.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The file is shorter than its ten-byte header or has the wrong signature.
    InvalidHeader,
    /// This decoder supports the 1.x block grammar, not a later major version.
    UnsupportedVersion(Version),
    /// A block header or body ends before the declared length.
    Truncated {
        /// Block ID being decoded.
        id: u8,
        /// Absolute byte offset at which bytes were needed.
        at: usize,
        /// Number of bytes required there.
        need: usize,
        /// Number available there.
        available: usize,
    },
    /// The parser does not know how to frame this block type.
    UnknownBlock {
        /// Unsupported type byte.
        id: u8,
        /// Absolute offset of the type byte.
        at: usize,
    },
    /// A caller-provided block has bytes after its declared body.
    TrailingBlockData {
        /// Block type.
        id: u8,
        /// Length declared by its fields.
        expected: usize,
        /// Payload length supplied by the caller.
        found: usize,
    },
    /// A payload is too long for its block type's length field.
    BlockTooLong {
        /// Block type.
        id: u8,
        /// Supplied payload length.
        length: usize,
        /// Largest representable payload length.
        maximum: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "invalid TZX signature or truncated header"),
            Self::UnsupportedVersion(version) => write!(
                f,
                "unsupported TZX version {}.{}",
                version.major, version.minor
            ),
            Self::Truncated {
                id,
                at,
                need,
                available,
            } => write!(
                f,
                "TZX block ${id:02X} needs {need} bytes at offset {at}, but only {available} remain"
            ),
            Self::UnknownBlock { id, at } => {
                write!(f, "unknown TZX block ${id:02X} at offset {at}")
            }
            Self::TrailingBlockData {
                id,
                expected,
                found,
            } => write!(
                f,
                "TZX block ${id:02X} declares {expected} payload bytes, not {found}"
            ),
            Self::BlockTooLong {
                id,
                length,
                maximum,
            } => write!(
                f,
                "TZX block ${id:02X} payload is {length} bytes; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Decodes a complete TZX/CDT image without interpreting machine timings.
///
/// # Errors
///
/// Returns a typed [`Error`] for invalid headers, unsupported versions,
/// unknown blocks and any block whose declared body overruns the input.
pub fn decode(bytes: &[u8]) -> Result<Tzx, Error> {
    if bytes.len() < 10 || &bytes[..8] != SIGNATURE {
        return Err(Error::InvalidHeader);
    }
    let version = Version::new(bytes[8], bytes[9]);
    if version.major > 1 {
        return Err(Error::UnsupportedVersion(version));
    }

    let mut blocks = Vec::new();
    let mut pos = 10;
    while pos < bytes.len() {
        let at = pos;
        let id = bytes[pos];
        pos += 1;
        let length = framed_len(id, &bytes[pos..], pos)?;
        blocks.push(Block {
            id,
            payload: bytes[pos..pos + length].to_vec(),
        });
        pos += length;
        debug_assert!(pos > at);
    }
    Ok(Tzx { version, blocks })
}

/// Encodes a complete, already-validated TZX/CDT image.
#[must_use]
pub fn encode(image: &Tzx) -> Vec<u8> {
    let capacity = 10
        + image
            .blocks
            .iter()
            .map(|block| 1 + block.payload.len())
            .sum::<usize>();
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(SIGNATURE);
    bytes.push(image.version.major);
    bytes.push(image.version.minor);
    for block in &image.blocks {
        bytes.push(block.id);
        bytes.extend_from_slice(&block.payload);
    }
    bytes
}

fn framed_len(id: u8, bytes: &[u8], at: usize) -> Result<usize, Error> {
    let fixed = |need| require(id, bytes, at, need).map(|()| need);
    match id {
        0x10 => variable_u16(id, bytes, at, 4, 2),
        0x11 => variable_u24(id, bytes, at, 0x12, 0x0f),
        0x12 => fixed(4),
        0x13 => {
            require(id, bytes, at, 1)?;
            let length = 1 + usize::from(bytes[0]) * 2;
            require(id, bytes, at, length)?;
            Ok(length)
        }
        0x14 => variable_u24(id, bytes, at, 0x0a, 7),
        0x15 => variable_u24(id, bytes, at, 8, 5),
        0x20 | 0x23 | 0x24 => fixed(2),
        0x21 | 0x30 => variable_u8(id, bytes, at, 1, 0),
        0x22 | 0x25 | 0x27 => Ok(0),
        0x26 => {
            require(id, bytes, at, 2)?;
            let length = 2 + usize::from(read_u16(bytes, 0)) * 2;
            require(id, bytes, at, length)?;
            Ok(length)
        }
        0x28 | 0x32 => variable_u16(id, bytes, at, 2, 0),
        0x2a | 0x2b => variable_u32(id, bytes, at, 4, 0),
        0x31 => variable_u8(id, bytes, at, 2, 1),
        0x33 => {
            require(id, bytes, at, 1)?;
            let length = 1 + usize::from(bytes[0]) * 3;
            require(id, bytes, at, length)?;
            Ok(length)
        }
        0x35 => variable_u32(id, bytes, at, 20, 16),
        0x5a => fixed(9),
        _ => Err(Error::UnknownBlock {
            id,
            at: at.saturating_sub(1),
        }),
    }
}

fn variable_u8(
    id: u8,
    bytes: &[u8],
    at: usize,
    header: usize,
    offset: usize,
) -> Result<usize, Error> {
    require(id, bytes, at, header)?;
    finish_variable(id, bytes, at, header, usize::from(bytes[offset]))
}

fn variable_u16(
    id: u8,
    bytes: &[u8],
    at: usize,
    header: usize,
    offset: usize,
) -> Result<usize, Error> {
    require(id, bytes, at, header)?;
    finish_variable(id, bytes, at, header, usize::from(read_u16(bytes, offset)))
}

fn variable_u24(
    id: u8,
    bytes: &[u8],
    at: usize,
    header: usize,
    offset: usize,
) -> Result<usize, Error> {
    require(id, bytes, at, header)?;
    finish_variable(id, bytes, at, header, read_u24(bytes, offset) as usize)
}

fn variable_u32(
    id: u8,
    bytes: &[u8],
    at: usize,
    header: usize,
    offset: usize,
) -> Result<usize, Error> {
    require(id, bytes, at, header)?;
    finish_variable(id, bytes, at, header, read_u32(bytes, offset) as usize)
}

fn finish_variable(
    id: u8,
    bytes: &[u8],
    at: usize,
    header: usize,
    body: usize,
) -> Result<usize, Error> {
    let length = header.checked_add(body).ok_or(Error::Truncated {
        id,
        at,
        need: usize::MAX,
        available: bytes.len(),
    })?;
    require(id, bytes, at, length)?;
    Ok(length)
}

fn require(id: u8, bytes: &[u8], at: usize, need: usize) -> Result<(), Error> {
    if bytes.len() < need {
        Err(Error::Truncated {
            id,
            at,
            need,
            available: bytes.len(),
        })
    } else {
        Ok(())
    }
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u24(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], 0])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_speed_round_trips_losslessly() {
        let block = Block::standard_speed(1_000, &[0xff, 1, 2, 0xfc]).expect("small block");
        let image = Tzx::new(Version::new(1, 13), vec![block]);
        let bytes = encode(&image);
        assert_eq!(decode(&bytes), Ok(image));
    }

    #[test]
    fn every_supported_block_shape_is_framed() {
        let payloads = [
            (0x10, vec![0, 0, 1, 0, 0xaa]),
            (0x11, vec![0; 0x12]),
            (0x12, vec![0; 4]),
            (0x13, vec![2, 1, 0, 2, 0]),
            (0x14, vec![0; 0x0a]),
            (0x15, vec![0; 8]),
            (0x20, vec![0; 2]),
            (0x21, vec![2, b'o', b'k']),
            (0x22, vec![]),
            (0x23, vec![0; 2]),
            (0x24, vec![0; 2]),
            (0x25, vec![]),
            (0x26, vec![1, 0, 0, 0]),
            (0x27, vec![]),
            (0x28, vec![1, 0, 0]),
            (0x2a, vec![0; 4]),
            (0x2b, vec![0; 4]),
            (0x30, vec![1, b'x']),
            (0x31, vec![1, 1, b'x']),
            (0x32, vec![1, 0, 0]),
            (0x33, vec![1, 0, 0, 0]),
            (0x35, vec![0; 20]),
            (0x5a, vec![0; 9]),
        ];
        let blocks = payloads
            .into_iter()
            .map(|(id, payload)| Block::new(id, payload).expect("valid block"))
            .collect();
        let image = Tzx::new(Version::new(1, 20), blocks);
        assert_eq!(decode(&encode(&image)), Ok(image));
    }

    #[test]
    fn malformed_and_unknown_blocks_are_typed_errors() {
        assert_eq!(decode(b"not tzx"), Err(Error::InvalidHeader));

        let mut unknown = b"ZXTape!\x1a\x01\x14".to_vec();
        unknown.push(0xff);
        assert!(matches!(
            decode(&unknown),
            Err(Error::UnknownBlock { id: 0xff, at: 10 })
        ));

        let mut short = b"ZXTape!\x1a\x01\x14".to_vec();
        short.extend_from_slice(&[0x10, 0, 0, 2, 0, 0xaa]);
        assert!(matches!(
            decode(&short),
            Err(Error::Truncated { id: 0x10, .. })
        ));
    }
}
