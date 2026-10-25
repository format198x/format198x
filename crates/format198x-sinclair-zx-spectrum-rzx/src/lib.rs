//! ZX Spectrum RZX input recordings: read and write.
//!
//! RZX, Ramsoft's replay format, records every `IN` a Spectrum program makes
//! frame by frame, against a starting snapshot, so a player can reproduce a
//! session exactly.
//!
//! ```text
//! header  "RZX!", u8 major, u8 minor, u32 flags
//! block   u8 id, u32 length (header included), payload
//! ```
//!
//! | id | block |
//! |---|---|
//! | `$10` | creator: 20-byte name, major and minor version, custom data |
//! | `$30` | snapshot: flags, 4-byte extension, length, snapshot data |
//! | `$80` | input recording: frame count, reserved byte, T-states, flags, frames |
//!
//! [`Rzx`] keeps every block in file order, so files with several snapshot
//! and recording blocks, and blocks this crate does not know (such as the
//! `$20` security information and `$21` signature), write back as read.
//!
//! # Compression
//!
//! Snapshot data and frame streams may be zlib-compressed (flag bit 1).
//! Blocks keep those bytes as stored, so a file writes back unchanged.
//! [`SnapshotBlock::snapshot`] and [`InputBlock::frames`] inflate them with
//! the family's own [`format198x_deflate`] crate (re-exported as
//! [`deflate`]), and [`SnapshotBlock::compressed`] and
//! [`InputBlock::from_frames_compressed`] write compressed blocks.

pub use format198x_deflate as deflate;
use format198x_deflate::{Level, Options, zlib};
use std::borrow::Cow;

/// The 4-byte signature.
const MAGIC: &[u8; 4] = b"RZX!";
/// File header length.
const HEADER_LEN: usize = 10;
/// Block header: id and length.
const BLOCK_HEADER: usize = 5;
/// Creator block id.
pub const BLOCK_CREATOR: u8 = 0x10;
/// Snapshot block id.
pub const BLOCK_SNAPSHOT: u8 = 0x30;
/// Input recording block id.
pub const BLOCK_INPUT: u8 = 0x80;
/// Snapshot flag: the data names an external file instead of holding it.
pub const SNAPSHOT_FLAG_EXTERNAL: u32 = 0x01;
/// Snapshot or input flag: the data is zlib-compressed.
pub const FLAG_COMPRESSED: u32 = 0x02;
/// Input flag: the frame stream is protected (encrypted).
pub const INPUT_FLAG_PROTECTED: u32 = 0x01;
/// An input count meaning "the same bytes as the previous frame".
const SAME_AS_PREVIOUS: u16 = 0xFFFF;

/// A creator block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creator {
    /// Program name, NUL-padded to 20 bytes on disk.
    pub name: [u8; 20],
    /// Program major version.
    pub version_major: u16,
    /// Program minor version.
    pub version_minor: u16,
    /// Custom data after the fixed fields.
    pub custom: Vec<u8>,
}

impl Creator {
    /// The name, up to its first NUL.
    #[must_use]
    pub fn name(&self) -> String {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(20);
        String::from_utf8_lossy(&self.name[..end]).into_owned()
    }
}

/// A snapshot block, data kept as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotBlock {
    /// Flags: [`SNAPSHOT_FLAG_EXTERNAL`], [`FLAG_COMPRESSED`].
    pub flags: u32,
    /// The snapshot's file extension, NUL-padded, such as `b"z80\0"`.
    pub extension: [u8; 4],
    /// The snapshot's length once uncompressed.
    pub uncompressed_len: u32,
    /// The snapshot data, or an external file descriptor, as stored.
    pub data: Vec<u8>,
}

impl SnapshotBlock {
    /// An embedded, uncompressed snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::TooLong`] for data over 4 GiB.
    pub fn embedded(extension: [u8; 4], data: Vec<u8>) -> Result<Self, RzxError> {
        let uncompressed_len = u32::try_from(data.len()).map_err(|_| RzxError::TooLong)?;
        Ok(Self {
            flags: 0,
            extension,
            uncompressed_len,
            data,
        })
    }

    /// An embedded snapshot, zlib-compressed at `level`.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::TooLong`] for data over 4 GiB.
    pub fn compressed(extension: [u8; 4], data: &[u8], level: Level) -> Result<Self, RzxError> {
        let uncompressed_len = u32::try_from(data.len()).map_err(|_| RzxError::TooLong)?;
        Ok(Self {
            flags: FLAG_COMPRESSED,
            extension,
            uncompressed_len,
            data: zlib::encode(data, level),
        })
    }

    /// Whether the data is zlib-compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    /// The embedded snapshot, inflated if it is compressed.
    ///
    /// A compressed snapshot may not inflate past
    /// [`uncompressed_len`](Self::uncompressed_len), and must fill it.
    /// Bytes after the zlib stream are ignored, as zlib's own reader does.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::External`] for a block that names an external
    /// file, [`RzxError::Inflate`] for a damaged zlib stream, and
    /// [`RzxError::SnapshotLength`] when the inflated length is not the
    /// declared one.
    pub fn snapshot(&self) -> Result<Cow<'_, [u8]>, RzxError> {
        if self.is_external() {
            return Err(RzxError::External);
        }
        if !self.is_compressed() {
            return Ok(Cow::Borrowed(&self.data));
        }
        let expected = usize::try_from(self.uncompressed_len).unwrap_or(usize::MAX);
        let options = Options::default().max_output(expected).allow_trailing(true);
        let data = zlib::decode_with(&self.data, &options).map_err(RzxError::Inflate)?;
        if data.len() != expected {
            return Err(RzxError::SnapshotLength {
                expected: self.uncompressed_len,
                actual: data.len(),
            });
        }
        Ok(Cow::Owned(data))
    }

    /// Whether the block names an external file.
    #[must_use]
    pub fn is_external(&self) -> bool {
        self.flags & SNAPSHOT_FLAG_EXTERNAL != 0
    }
}

/// One recorded frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Opcode fetches the CPU makes during the frame.
    pub fetch_count: u16,
    /// The bytes each `IN` returns, in order.
    pub inputs: Vec<u8>,
}

/// An input recording block, frame stream kept as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBlock {
    /// Frames in the stream.
    pub frame_count: u32,
    /// The reserved byte, kept as stored.
    pub reserved: u8,
    /// T-state counter at the start of the recording.
    pub tstate_counter: u32,
    /// Flags: [`INPUT_FLAG_PROTECTED`], [`FLAG_COMPRESSED`].
    pub flags: u32,
    /// The frame stream as stored.
    pub stream: Vec<u8>,
}

impl InputBlock {
    /// An uncompressed block holding `frames`.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::TooLong`] for more than `u32::MAX` frames, or a
    /// frame with more than 65534 inputs.
    pub fn from_frames(tstate_counter: u32, frames: &[Frame]) -> Result<Self, RzxError> {
        Ok(Self {
            frame_count: u32::try_from(frames.len()).map_err(|_| RzxError::TooLong)?,
            reserved: 0,
            tstate_counter,
            flags: 0,
            stream: encode_frames(frames)?,
        })
    }

    /// A zlib-compressed block holding `frames`.
    ///
    /// # Errors
    ///
    /// As [`InputBlock::from_frames`].
    pub fn from_frames_compressed(
        tstate_counter: u32,
        frames: &[Frame],
        level: Level,
    ) -> Result<Self, RzxError> {
        let mut block = Self::from_frames(tstate_counter, frames)?;
        block.stream = zlib::encode(&block.stream, level);
        block.flags |= FLAG_COMPRESSED;
        Ok(block)
    }

    /// Whether the frame stream is zlib-compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    /// Whether the frame stream is protected (encrypted).
    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.flags & INPUT_FLAG_PROTECTED != 0
    }

    /// The frame stream, inflated if it is compressed. Bytes after the zlib
    /// stream are ignored, as zlib's own reader does.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::Inflate`] for a damaged zlib stream.
    pub fn frame_stream(&self) -> Result<Cow<'_, [u8]>, RzxError> {
        if !self.is_compressed() {
            return Ok(Cow::Borrowed(&self.stream));
        }
        let options = Options::default().allow_trailing(true);
        zlib::decode_with(&self.stream, &options)
            .map(Cow::Owned)
            .map_err(RzxError::Inflate)
    }

    /// Decode the frames, inflating a compressed stream first.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::Protected`] for a protected stream, the errors
    /// of [`InputBlock::frame_stream`], and those of [`decode_frames`].
    pub fn frames(&self) -> Result<Vec<Frame>, RzxError> {
        if self.is_protected() {
            return Err(RzxError::Protected);
        }
        decode_frames(&self.frame_stream()?, self.frame_count)
    }
}

/// One block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// `$10`.
    Creator(Creator),
    /// `$30`.
    Snapshot(SnapshotBlock),
    /// `$80`.
    Input(InputBlock),
    /// Any other block, payload as stored.
    Other {
        /// Block id.
        id: u8,
        /// Payload after the 5-byte header.
        payload: Vec<u8>,
    },
}

/// An RZX file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rzx {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
    /// File flags.
    pub flags: u32,
    /// Blocks in file order.
    pub blocks: Vec<Block>,
}

/// Why an RZX file could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RzxError {
    /// Shorter than the 10-byte header.
    TooShort,
    /// The `RZX!` magic is missing.
    BadMagic,
    /// A block header is cut off, or claims fewer than 5 bytes.
    BadBlockHeader {
        /// Offset of the block.
        offset: usize,
    },
    /// A block runs past the end of the file.
    TruncatedBlock {
        /// Offset of the block.
        offset: usize,
    },
    /// A known block is shorter than its fixed fields.
    ShortBlock {
        /// Block id.
        id: u8,
    },
    /// A frame stream is cut off or holds fewer frames than its count.
    BadFrames {
        /// Index of the frame that failed.
        frame: usize,
    },
    /// The frame stream is protected (encrypted), which this crate cannot
    /// read.
    Protected,
    /// The snapshot block names an external file instead of holding one.
    External,
    /// A compressed snapshot or frame stream does not inflate.
    Inflate(deflate::Error),
    /// A compressed snapshot does not inflate to its declared length.
    SnapshotLength {
        /// The declared length.
        expected: u32,
        /// The inflated length.
        actual: usize,
    },
    /// On encode: a length does not fit its field.
    TooLong,
}

impl core::fmt::Display for RzxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "RZX file shorter than the 10-byte header"),
            Self::BadMagic => write!(f, "not an RZX file (missing RZX! magic)"),
            Self::BadBlockHeader { offset } => write!(f, "bad RZX block header at offset {offset}"),
            Self::TruncatedBlock { offset } => {
                write!(
                    f,
                    "RZX block at offset {offset} runs past the end of the file"
                )
            }
            Self::ShortBlock { id } => write!(f, "RZX block ${id:02X} is shorter than its fields"),
            Self::BadFrames { frame } => write!(f, "RZX frame {frame} is truncated or missing"),
            Self::Protected => write!(f, "RZX frame stream is protected"),
            Self::External => write!(f, "RZX snapshot is in an external file"),
            Self::Inflate(e) => write!(f, "RZX data does not inflate: {e}"),
            Self::SnapshotLength { expected, actual } => write!(
                f,
                "RZX snapshot inflates to {actual} bytes, not the declared {expected}"
            ),
            Self::TooLong => write!(f, "RZX data is too long for its length field"),
        }
    }
}

impl std::error::Error for RzxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inflate(e) => Some(e),
            _ => None,
        }
    }
}

fn u16_at(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// Decode `count` frames from an uncompressed frame stream, expanding the
/// "same as previous" marker.
///
/// # Errors
///
/// Returns [`RzxError::BadFrames`] when the stream is cut off or holds fewer
/// frames than `count`.
pub fn decode_frames(stream: &[u8], count: u32) -> Result<Vec<Frame>, RzxError> {
    let count = usize::try_from(count).unwrap_or(usize::MAX);
    let mut frames: Vec<Frame> = Vec::with_capacity(count.min(stream.len() / 4));
    let mut cursor = 0;
    let mut last: Vec<u8> = Vec::new();
    while frames.len() < count {
        let frame = frames.len();
        let header = stream
            .get(cursor..cursor + 4)
            .ok_or(RzxError::BadFrames { frame })?;
        let fetch_count = u16_at(header, 0);
        let in_count = u16_at(header, 2);
        cursor += 4;
        let inputs = if in_count == SAME_AS_PREVIOUS {
            last.clone()
        } else {
            let n = usize::from(in_count);
            let bytes = stream
                .get(cursor..cursor + n)
                .ok_or(RzxError::BadFrames { frame })?
                .to_vec();
            cursor += n;
            last.clone_from(&bytes);
            bytes
        };
        frames.push(Frame {
            fetch_count,
            inputs,
        });
    }
    Ok(frames)
}

/// Encode frames as an uncompressed stream, writing the "same as previous"
/// marker for a frame whose inputs repeat the last ones written.
///
/// # Errors
///
/// Returns [`RzxError::TooLong`] for a frame with more than 65534 inputs.
pub fn encode_frames(frames: &[Frame]) -> Result<Vec<u8>, RzxError> {
    let mut out = Vec::new();
    let mut last: &[u8] = &[];
    for frame in frames {
        out.extend_from_slice(&frame.fetch_count.to_le_bytes());
        if frame.inputs == last {
            out.extend_from_slice(&SAME_AS_PREVIOUS.to_le_bytes());
        } else {
            let n = u16::try_from(frame.inputs.len())
                .ok()
                .filter(|&n| n != SAME_AS_PREVIOUS)
                .ok_or(RzxError::TooLong)?;
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&frame.inputs);
            last = &frame.inputs;
        }
    }
    Ok(out)
}

fn parse_block(id: u8, payload: &[u8]) -> Result<Block, RzxError> {
    let short = RzxError::ShortBlock { id };
    Ok(match id {
        BLOCK_CREATOR => {
            if payload.len() < 24 {
                return Err(short);
            }
            let mut name = [0u8; 20];
            name.copy_from_slice(&payload[..20]);
            Block::Creator(Creator {
                name,
                version_major: u16_at(payload, 20),
                version_minor: u16_at(payload, 22),
                custom: payload[24..].to_vec(),
            })
        }
        BLOCK_SNAPSHOT => {
            if payload.len() < 12 {
                return Err(short);
            }
            let mut extension = [0u8; 4];
            extension.copy_from_slice(&payload[4..8]);
            Block::Snapshot(SnapshotBlock {
                flags: u32_at(payload, 0),
                extension,
                uncompressed_len: u32_at(payload, 8),
                data: payload[12..].to_vec(),
            })
        }
        BLOCK_INPUT => {
            if payload.len() < 13 {
                return Err(short);
            }
            Block::Input(InputBlock {
                frame_count: u32_at(payload, 0),
                reserved: payload[4],
                tstate_counter: u32_at(payload, 5),
                flags: u32_at(payload, 9),
                stream: payload[13..].to_vec(),
            })
        }
        id => Block::Other {
            id,
            payload: payload.to_vec(),
        },
    })
}

impl Rzx {
    /// Parse an RZX file.
    ///
    /// # Errors
    ///
    /// Returns an error when the header is missing or a block is cut off or
    /// shorter than its fixed fields.
    pub fn parse(data: &[u8]) -> Result<Self, RzxError> {
        if data.len() < HEADER_LEN {
            return Err(RzxError::TooShort);
        }
        if &data[..4] != MAGIC {
            return Err(RzxError::BadMagic);
        }
        let mut rzx = Self {
            major: data[4],
            minor: data[5],
            flags: u32_at(data, 6),
            blocks: Vec::new(),
        };
        let mut offset = HEADER_LEN;
        while offset < data.len() {
            if offset + BLOCK_HEADER > data.len() {
                return Err(RzxError::BadBlockHeader { offset });
            }
            let id = data[offset];
            let len = usize::try_from(u32_at(data, offset + 1)).unwrap_or(usize::MAX);
            if len < BLOCK_HEADER {
                return Err(RzxError::BadBlockHeader { offset });
            }
            let end = offset
                .checked_add(len)
                .filter(|&end| end <= data.len())
                .ok_or(RzxError::TruncatedBlock { offset })?;
            rzx.blocks
                .push(parse_block(id, &data[offset + BLOCK_HEADER..end])?);
            offset = end;
        }
        Ok(rzx)
    }

    /// Encode the file, blocks in order and data as stored.
    ///
    /// # Errors
    ///
    /// Returns [`RzxError::TooLong`] when a block is over 4 GiB.
    pub fn encode(&self) -> Result<Vec<u8>, RzxError> {
        let mut out = MAGIC.to_vec();
        out.push(self.major);
        out.push(self.minor);
        out.extend_from_slice(&self.flags.to_le_bytes());
        for block in &self.blocks {
            let mut payload = Vec::new();
            let id = match block {
                Block::Creator(c) => {
                    payload.extend_from_slice(&c.name);
                    payload.extend_from_slice(&c.version_major.to_le_bytes());
                    payload.extend_from_slice(&c.version_minor.to_le_bytes());
                    payload.extend_from_slice(&c.custom);
                    BLOCK_CREATOR
                }
                Block::Snapshot(s) => {
                    payload.extend_from_slice(&s.flags.to_le_bytes());
                    payload.extend_from_slice(&s.extension);
                    payload.extend_from_slice(&s.uncompressed_len.to_le_bytes());
                    payload.extend_from_slice(&s.data);
                    BLOCK_SNAPSHOT
                }
                Block::Input(i) => {
                    payload.extend_from_slice(&i.frame_count.to_le_bytes());
                    payload.push(i.reserved);
                    payload.extend_from_slice(&i.tstate_counter.to_le_bytes());
                    payload.extend_from_slice(&i.flags.to_le_bytes());
                    payload.extend_from_slice(&i.stream);
                    BLOCK_INPUT
                }
                Block::Other { id, payload: p } => {
                    payload.extend_from_slice(p);
                    *id
                }
            };
            let len = u32::try_from(payload.len() + BLOCK_HEADER).map_err(|_| RzxError::TooLong)?;
            out.push(id);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&payload);
        }
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

    fn frames() -> Vec<Frame> {
        vec![
            Frame {
                fetch_count: 100,
                inputs: vec![0xBF, 0xFF],
            },
            Frame {
                fetch_count: 101,
                inputs: vec![0xBF, 0xFF],
            },
            Frame {
                fetch_count: 102,
                inputs: Vec::new(),
            },
        ]
    }

    fn sample() -> Rzx {
        let mut name = [0u8; 20];
        name[..7].copy_from_slice(b"Emu198x");
        Rzx {
            major: 0,
            minor: 13,
            flags: 0,
            blocks: vec![
                Block::Creator(Creator {
                    name,
                    version_major: 1,
                    version_minor: 2,
                    custom: vec![9],
                }),
                Block::Snapshot(SnapshotBlock::embedded(*b"sna\0", vec![0x55; 64]).expect("fits")),
                Block::Input(InputBlock::from_frames(42, &frames()).expect("fits")),
                Block::Snapshot(SnapshotBlock {
                    flags: FLAG_COMPRESSED,
                    extension: *b"z80\0",
                    uncompressed_len: 1000,
                    data: vec![0x78, 0x9C, 1, 2, 3],
                }),
                Block::Other {
                    id: 0x21,
                    payload: vec![1, 2, 3, 4],
                },
            ],
        }
    }

    #[test]
    fn blocks_round_trip_in_order() {
        let rzx = sample();
        let bytes = rzx.encode().expect("encodes");
        assert!(bytes.starts_with(b"RZX!\x00\x0d"));
        let back = Rzx::parse(&bytes).expect("parses");
        assert_eq!(back, rzx);
        assert_eq!(back.encode().expect("encodes"), bytes);
    }

    #[test]
    fn frames_round_trip_with_the_repeat_marker() {
        let stream = encode_frames(&frames()).expect("encodes");
        assert_eq!(&stream[6..10], &[101, 0, 0xFF, 0xFF], "repeat marker");
        assert_eq!(decode_frames(&stream, 3), Ok(frames()));
        let Some(Block::Input(input)) = sample().blocks.get(2).cloned() else {
            panic!("input block");
        };
        assert_eq!(input.frames(), Ok(frames()));
        assert_eq!(input.tstate_counter, 42);
    }

    #[test]
    fn the_first_empty_frame_uses_the_marker() {
        let empty = vec![Frame {
            fetch_count: 1,
            inputs: Vec::new(),
        }];
        let stream = encode_frames(&empty).expect("encodes");
        assert_eq!(stream, [1, 0, 0xFF, 0xFF]);
        assert_eq!(decode_frames(&stream, 1), Ok(empty));
    }

    #[test]
    fn compressed_blocks_inflate_and_write_back_as_stored() {
        let input = InputBlock::from_frames_compressed(7, &frames(), Level::BEST).expect("fits");
        assert!(input.is_compressed());
        assert_eq!(&input.stream[..2], [0x78, 0xDA]);
        assert_eq!(input.frames(), Ok(frames()));
        let snapshot = vec![0xAA; 49_179];
        let snap = SnapshotBlock::compressed(*b"sna\0", &snapshot, Level::DEFAULT).expect("fits");
        assert!(snap.is_compressed() && snap.data.len() < 1000);
        assert_eq!(snap.snapshot().expect("inflates")[..], snapshot[..]);
        let rzx = Rzx {
            major: 0,
            minor: 13,
            flags: 0,
            blocks: vec![Block::Snapshot(snap), Block::Input(input)],
        };
        let bytes = rzx.encode().expect("encodes");
        let back = Rzx::parse(&bytes).expect("parses");
        assert_eq!(back, rzx);
        assert_eq!(back.encode().expect("encodes"), bytes);
    }

    /// Streams compressed by Python's `zlib.compress`: the sample frames,
    /// and a 64-byte snapshot of `0x55`.
    #[test]
    fn reads_blocks_compressed_by_another_tool() {
        let mut input = InputBlock::from_frames(0, &frames()).expect("fits");
        input.flags = FLAG_COMPRESSED;
        input.stream = vec![
            0x78, 0x9C, 0x4B, 0x61, 0x60, 0x62, 0xD8, 0xFF, 0x3F, 0x95, 0xE1, 0xFF, 0xFF, 0x34,
            0x06, 0x06, 0x06, 0x00, 0x25, 0xC0, 0x04, 0xEE,
        ];
        assert_eq!(input.frames(), Ok(frames()));
        let snap = SnapshotBlock {
            flags: FLAG_COMPRESSED,
            extension: *b"z80\0",
            uncompressed_len: 64,
            data: vec![
                0x78, 0x9C, 0x0B, 0x0D, 0xA5, 0x0C, 0x00, 0x00, 0xB2, 0xFE, 0x15, 0x41,
            ],
        };
        assert_eq!(snap.snapshot().expect("inflates")[..], [0x55; 64]);
    }

    #[test]
    fn unreadable_payloads_are_errors() {
        let mut input = InputBlock::from_frames(0, &frames()).expect("fits");
        input.flags = INPUT_FLAG_PROTECTED;
        assert!(input.is_protected());
        assert_eq!(input.frames(), Err(RzxError::Protected));
        input.flags = FLAG_COMPRESSED;
        assert!(matches!(input.frames(), Err(RzxError::Inflate(_))));
        let Some(Block::Snapshot(mut snap)) = sample().blocks.get(3).cloned() else {
            panic!("snapshot block");
        };
        assert!(snap.is_compressed());
        assert!(!snap.is_external());
        assert!(matches!(snap.snapshot(), Err(RzxError::Inflate(_))));
        snap.data = zlib::encode(&[1; 999], Level::DEFAULT);
        assert_eq!(
            snap.snapshot(),
            Err(RzxError::SnapshotLength {
                expected: 1000,
                actual: 999
            })
        );
        snap.data = zlib::encode(&[1; 1001], Level::DEFAULT);
        assert!(matches!(snap.snapshot(), Err(RzxError::Inflate(_))));
        snap.flags = SNAPSHOT_FLAG_EXTERNAL;
        assert_eq!(snap.snapshot(), Err(RzxError::External));
        let plain = SnapshotBlock::embedded(*b"sna\0", vec![1, 2]).expect("fits");
        assert_eq!(plain.snapshot().expect("plain")[..], [1, 2]);
        let Some(Block::Creator(creator)) = sample().blocks.first().cloned() else {
            panic!("creator block");
        };
        assert_eq!(creator.name(), "Emu198x");
    }

    #[test]
    fn malformed_files_are_rejected() {
        assert_eq!(Rzx::parse(b"RZX!"), Err(RzxError::TooShort));
        assert_eq!(Rzx::parse(b"NOPE\0\0\0\0\0\0"), Err(RzxError::BadMagic));
        let header = Rzx {
            major: 0,
            minor: 13,
            flags: 0,
            blocks: Vec::new(),
        }
        .encode()
        .expect("encodes");
        let mut bytes = header.clone();
        bytes.extend_from_slice(&[0x10, 3, 0, 0, 0]);
        assert_eq!(
            Rzx::parse(&bytes),
            Err(RzxError::BadBlockHeader { offset: 10 })
        );
        let mut bytes = header.clone();
        bytes.extend_from_slice(&[0x10, 0, 0]);
        assert_eq!(
            Rzx::parse(&bytes),
            Err(RzxError::BadBlockHeader { offset: 10 })
        );
        let mut bytes = header.clone();
        bytes.extend_from_slice(&[0x21, 50, 0, 0, 0]);
        assert_eq!(
            Rzx::parse(&bytes),
            Err(RzxError::TruncatedBlock { offset: 10 })
        );
        for id in [BLOCK_CREATOR, BLOCK_SNAPSHOT, BLOCK_INPUT] {
            let mut bytes = header.clone();
            bytes.extend_from_slice(&[id, 6, 0, 0, 0, 0]);
            assert_eq!(Rzx::parse(&bytes), Err(RzxError::ShortBlock { id }));
        }
        assert_eq!(
            decode_frames(&[1, 0, 2, 0, 0xBF], 1),
            Err(RzxError::BadFrames { frame: 0 })
        );
        assert_eq!(
            decode_frames(&[1, 0], 1),
            Err(RzxError::BadFrames { frame: 0 })
        );
        let huge = vec![Frame {
            fetch_count: 0,
            inputs: vec![0; 0xFFFF],
        }];
        assert_eq!(encode_frames(&huge), Err(RzxError::TooLong));
        for e in [
            RzxError::TooShort,
            RzxError::BadMagic,
            RzxError::BadBlockHeader { offset: 1 },
            RzxError::TruncatedBlock { offset: 1 },
            RzxError::ShortBlock { id: 1 },
            RzxError::BadFrames { frame: 1 },
            RzxError::Protected,
            RzxError::External,
            RzxError::Inflate(deflate::Error::UnexpectedEof),
            RzxError::SnapshotLength {
                expected: 1,
                actual: 2,
            },
            RzxError::TooLong,
        ] {
            use std::error::Error as _;
            assert_eq!(e.source().is_some(), matches!(e, RzxError::Inflate(_)));
            assert!(!e.to_string().is_empty());
        }
    }
}
