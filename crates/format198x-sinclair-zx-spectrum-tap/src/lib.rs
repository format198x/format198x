//! Sinclair ZX Spectrum `.tap` tape-image codec.
//!
//! A TAP file is the simplest archival form of a Spectrum tape: the *data* of
//! each block and nothing else. Leader tone, sync pulses and bit timings are
//! not stored — a loader reconstructs them from the standard ROM pulse rules.
//!
//! Layout facts are authored from
//! `syntheses/sinclair-zx-spectrum/tape-loading-format.md` (§ 2 the block format, § 4
//! the file format; cross-checked there against the Sinclair BASIC manual and
//! fuse's `libspectrum`):
//!
//! - A file is a bare sequence of blocks, each a little-endian `u16` length
//!   followed by that many bytes (§ 4).
//! - A block's bytes are one **flag**, the payload, and one **parity** byte —
//!   so the stored length is the payload's length plus two (§ 2).
//! - The flag is `$00` for a header and `$FF` for data. The ROM loader expects
//!   only those two, and distinguishes them by bit 7 (§ 2).
//! - The parity byte is the XOR of every byte before it in the block,
//!   including the flag (§ 2).
//! - A header block's payload is exactly 17 bytes: type, a 10-byte
//!   space-padded name, the next block's data length, and two type-dependent
//!   parameters — for `Code`, the start address and `$8000` (§ 2).
//!
//! What TAP cannot carry is as much a part of the format as what it can:
//! custom pulse timings, direct recordings, and any metadata at all are out of
//! reach, which is what `.tzx` exists for (§ 4).
//!
//! ```
//! use format198x_sinclair_zx_spectrum_tap::{Header, HeaderKind, TapBlock, encode};
//!
//! let header = Header::new(HeaderKind::Code, "hello", 4, 0x8000, 0x8000);
//! let tape = encode(&[header.block(), TapBlock::data(vec![1, 2, 3, 4])]);
//! assert_eq!(&tape[..2], &[0x13, 0x00]); // a header block is 19 bytes
//! ```

mod error;

pub use error::DecodeError;

/// The flag byte that marks a header block.
pub const HEADER_FLAG: u8 = 0x00;
/// The flag byte that marks a data block.
pub const DATA_FLAG: u8 = 0xFF;
/// A header block's payload is always this long.
pub const HEADER_PAYLOAD: usize = 17;
/// A name is stored in ten bytes, space-padded.
pub const NAME_LEN: usize = 10;

/// One block of a tape, as TAP stores it: the flag and the payload between it
/// and the parity byte.
///
/// The correct parity is a function of the other two, the XOR of the flag and
/// every payload byte, so a block you build leaves [`stored_parity`] as `None`
/// and [`encode`] writes the correct one. A block read by [`decode`] keeps the
/// parity byte the tape actually carries, right or wrong. A real loader
/// rejects a block whose parity does not match ("R Tape loading error"), so
/// dropping a wrong one would turn a damaged tape into a good one.
///
/// [`stored_parity`]: TapBlock::stored_parity
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapBlock {
    /// `$00` for a header, `$FF` for data.
    pub flag: u8,
    /// The bytes between the flag and the parity byte.
    pub data: Vec<u8>,
    /// The parity byte as stored on the tape, or `None` to write the correct
    /// one. [`decode`] fills it for every block that has one.
    pub stored_parity: Option<u8>,
}

impl TapBlock {
    /// A block with this flag and payload, and the correct parity.
    #[must_use]
    pub fn new(flag: u8, data: Vec<u8>) -> Self {
        Self {
            flag,
            data,
            stored_parity: None,
        }
    }

    /// A data block: flag `$FF` and these bytes.
    #[must_use]
    pub fn data(bytes: Vec<u8>) -> Self {
        Self::new(DATA_FLAG, bytes)
    }

    /// Whether this is a header block.
    ///
    /// Read from bit 7 rather than by comparing with `$00`, which is how the
    /// ROM loader tells them apart.
    #[must_use]
    pub fn is_header(&self) -> bool {
        self.flag < 0x80
    }

    /// The correct parity for this block: the XOR of the flag and every
    /// payload byte.
    #[must_use]
    pub fn parity(&self) -> u8 {
        self.data.iter().fold(self.flag, |p, b| p ^ b)
    }

    /// The parity byte the block carries on tape: the stored one if there is
    /// one, otherwise the correct one.
    #[must_use]
    pub fn written_parity(&self) -> u8 {
        self.stored_parity.unwrap_or_else(|| self.parity())
    }

    /// Whether the parity on tape matches the flag and payload, which is the
    /// check the ROM loader makes.
    #[must_use]
    pub fn parity_ok(&self) -> bool {
        self.written_parity() == self.parity()
    }

    /// The block's bytes as they are on tape: the flag, the payload and the
    /// parity byte it carries. This is the stream a loader reads, so it is what
    /// an emulator should play.
    #[must_use]
    pub fn tape_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.data.len() + 2);
        bytes.push(self.flag);
        bytes.extend_from_slice(&self.data);
        bytes.push(self.written_parity());
        bytes
    }
}

/// What a header says the block after it holds.
///
/// The four kinds are the tape's own; the numbering is the header's first
/// byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HeaderKind {
    /// A BASIC program.
    Program = 0,
    /// A numeric array.
    NumberArray = 1,
    /// A character array.
    CharacterArray = 2,
    /// Machine code.
    Code = 3,
}

impl HeaderKind {
    /// The kind a header's first byte names, or `None` for a byte that names
    /// no kind.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Program),
            1 => Some(Self::NumberArray),
            2 => Some(Self::CharacterArray),
            3 => Some(Self::Code),
            _ => None,
        }
    }
}

/// A header block's seventeen bytes, read as what they mean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    /// What the next block holds.
    pub kind: HeaderKind,
    /// The name, as stored: ten bytes, space-padded and space-trimmed here.
    pub name: String,
    /// The length of the next block's payload.
    pub length: u16,
    /// The first type-dependent parameter — a start address for `Code`, an
    /// auto-start line for `Program`.
    pub param1: u16,
    /// The second — `$8000` for `Code`, the program length without variables
    /// for `Program`.
    pub param2: u16,
}

impl Header {
    /// A header. A name longer than ten bytes is truncated and a shorter one
    /// space-padded, which is what the ten-byte field can hold.
    #[must_use]
    pub fn new(kind: HeaderKind, name: &str, length: u16, param1: u16, param2: u16) -> Self {
        Self {
            kind,
            name: name.into(),
            length,
            param1,
            param2,
        }
    }

    /// The header as a block, ready for [`encode`].
    #[must_use]
    pub fn block(&self) -> TapBlock {
        let mut data = Vec::with_capacity(HEADER_PAYLOAD);
        data.push(self.kind as u8);
        let name = self.name.as_bytes();
        for i in 0..NAME_LEN {
            data.push(name.get(i).copied().unwrap_or(b' '));
        }
        data.extend_from_slice(&self.length.to_le_bytes());
        data.extend_from_slice(&self.param1.to_le_bytes());
        data.extend_from_slice(&self.param2.to_le_bytes());
        TapBlock::new(HEADER_FLAG, data)
    }

    /// Read a header out of a header block's payload.
    ///
    /// # Errors
    ///
    /// [`DecodeError::HeaderLength`] when the payload is not seventeen bytes,
    /// and [`DecodeError::HeaderKind`] when its first byte names no kind.
    pub fn from_payload(payload: &[u8]) -> Result<Self, DecodeError> {
        let bytes: [u8; HEADER_PAYLOAD] =
            payload.try_into().map_err(|_| DecodeError::HeaderLength {
                found: payload.len(),
            })?;
        let kind =
            HeaderKind::from_byte(bytes[0]).ok_or(DecodeError::HeaderKind { found: bytes[0] })?;
        let name = String::from_utf8_lossy(&bytes[1..1 + NAME_LEN])
            .trim_end()
            .into();
        let word = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        Ok(Self {
            kind,
            name,
            length: word(11),
            param1: word(13),
            param2: word(15),
        })
    }
}

/// Encode blocks into the bytes of a TAP file.
///
/// Each block becomes a little-endian `u16` length — the flag, the payload and
/// the parity byte — followed by those bytes. `decode(&encode(b)) == Ok(b)` for
/// any blocks `b` whose payloads fit the length field.
#[must_use]
pub fn encode(blocks: &[TapBlock]) -> Vec<u8> {
    let mut out = Vec::new();
    for block in blocks {
        // The stored length covers the flag, the payload and the parity byte.
        let len = block.data.len().saturating_add(2);
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(len as u16).to_le_bytes());
        out.extend_from_slice(&block.tape_bytes());
    }
    out
}

/// Decode the bytes of a TAP file into its blocks.
///
/// # Errors
///
/// [`DecodeError::Truncated`] when a block's length runs past the end of the
/// input, and [`DecodeError::TrailingByte`] when a single byte is left over —
/// too few for another length field, and so not a block.
pub fn decode(bytes: &[u8]) -> Result<Vec<TapBlock>, DecodeError> {
    let mut blocks = Vec::new();
    let mut pos = 0usize;
    while pos + 2 <= bytes.len() {
        let len = usize::from(u16::from_le_bytes([bytes[pos], bytes[pos + 1]]));
        let at = pos;
        pos += 2;
        // A zero-length block carries no flag and no parity. Real tapes do not
        // have them; a file that does is skipping nothing, so skip it too.
        if len == 0 {
            continue;
        }
        if pos + len > bytes.len() {
            return Err(DecodeError::Truncated {
                at,
                claimed: len,
                available: bytes.len() - pos,
            });
        }
        let block = &bytes[pos..pos + len];
        pos += len;
        // The last byte is the parity, kept as stored; anything between it
        // and the flag is the payload. A one-byte block is a flag alone, which
        // leaves neither.
        let (data, stored_parity) = if len >= 2 {
            (block[1..len - 1].to_vec(), Some(block[len - 1]))
        } else {
            (Vec::new(), None)
        };
        blocks.push(TapBlock {
            flag: block[0],
            data,
            stored_parity,
        });
    }
    if pos < bytes.len() {
        return Err(DecodeError::TrailingByte { at: pos });
    }
    Ok(blocks)
}
