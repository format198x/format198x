//! ZX Spectrum `.szx` (ZX-State) snapshots: read and write.
//!
//! SZX is Spectaculator's chunked snapshot format:
//!
//! ```text
//! header  "ZXST", u8 major, u8 minor, u8 machine id, u8 flags
//! chunk   char[4] id, u32 size (little-endian), size bytes
//! ```
//!
//! Chunks may come in any order, and readers skip what they do not know, so
//! [`Szx`] keeps every chunk as stored and writes them back in order. Views
//! read the four chunks that describe a machine's core state:
//!
//! | chunk | view | carries |
//! |---|---|---|
//! | `Z80R` | [`Z80Registers`] | CPU registers, `I`, `R`, IFFs, `IM`, T-state position |
//! | `SPCR` | [`SpectrumRegisters`] | border, `$7FFD`, `$1FFD` or `$EFF7`, last `$FE` |
//! | `RAMP` | [`RamPage`] | one 16K RAM bank, optionally zlib-compressed |
//! | `AY\0\0` | [`AyBlock`] | the selected AY register and all sixteen |
//!
//! # Compression is the caller's
//!
//! A `RAMP` chunk's page may be zlib-compressed. This crate has no
//! dependencies, so [`RamPage`] keeps the bytes as stored and says whether
//! they are compressed. Pages it writes are uncompressed, which the format
//! allows.
//!
//! RAM pages are numbered by bank, 0 to 7; a 48K uses banks 5, 2 and 0 for
//! `$4000`, `$8000` and `$C000`. Mapping banks into a machine is the
//! emulator's business.

/// The 4-byte signature.
const MAGIC: &[u8; 4] = b"ZXST";
/// File header length.
const HEADER_LEN: usize = 8;
/// Chunk header: id and size.
const CHUNK_HEADER: usize = 8;
/// One RAM page.
pub const PAGE_LEN: usize = 0x4000;
/// `RAMP` flag: the page is zlib-compressed.
pub const RAMP_FLAG_COMPRESSED: u16 = 0x0001;
/// Full `Z80R` length; version 1.0 files may stop after 29 bytes.
const Z80R_LEN: usize = 37;
/// The part of `Z80R` every version carries.
const Z80R_MIN: usize = 29;

/// One chunk, kept as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    /// Four-character id, such as `*b"Z80R"`.
    pub id: [u8; 4],
    /// The chunk's payload.
    pub data: Vec<u8>,
}

/// An SZX snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Szx {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
    /// `ZXSTMID_*` machine id: 0 16K, 1 48K, 2 128K, 3 +2, 4 +2A, 5 +3,
    /// 6 +3e, 7 Pentagon 128, and so on.
    pub machine_id: u8,
    /// Header flags.
    pub flags: u8,
    /// Chunks in file order.
    pub chunks: Vec<Chunk>,
}

/// The `Z80R` chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Z80Registers {
    /// AF.
    pub af: u16,
    /// BC.
    pub bc: u16,
    /// DE.
    pub de: u16,
    /// HL.
    pub hl: u16,
    /// AF'.
    pub af_alt: u16,
    /// BC'.
    pub bc_alt: u16,
    /// DE'.
    pub de_alt: u16,
    /// HL'.
    pub hl_alt: u16,
    /// IX.
    pub ix: u16,
    /// IY.
    pub iy: u16,
    /// SP.
    pub sp: u16,
    /// PC.
    pub pc: u16,
    /// I.
    pub i: u8,
    /// R.
    pub r: u8,
    /// IFF1.
    pub iff1: bool,
    /// IFF2.
    pub iff2: bool,
    /// Interrupt mode.
    pub im: u8,
    /// T-states into the frame, the hold time of the interrupt request,
    /// flags and `MEMPTR`; `None` in a short version 1.0 chunk.
    pub timing: Option<Z80Timing>,
}

/// The tail of a full `Z80R` chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Z80Timing {
    /// `dwCyclesStart`: T-states since the frame's interrupt.
    pub cycles_start: u32,
    /// `chHoldIntReqCycles`.
    pub hold_int_req_cycles: u8,
    /// `chFlags`: bit 0 last instruction was EI, bit 1 halted.
    pub flags: u8,
    /// `wMemPtr`.
    pub memptr: u16,
}

/// The `SPCR` chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpectrumRegisters {
    /// Border colour.
    pub border: u8,
    /// Last `$7FFD` write.
    pub port_7ffd: u8,
    /// Last `$1FFD` write on Amstrad machines, `$EFF7` on others.
    pub port_1ffd_or_eff7: u8,
    /// Last `$FE` write.
    pub port_fe: u8,
}

/// A `RAMP` chunk, page kept as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RamPage<'a> {
    /// Flags; [`RAMP_FLAG_COMPRESSED`] is the only one defined.
    pub flags: u16,
    /// RAM bank number.
    pub bank: u8,
    /// The page as stored: 16K, or zlib data when compressed.
    pub data: &'a [u8],
}

impl RamPage<'_> {
    /// Whether the page is zlib-compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.flags & RAMP_FLAG_COMPRESSED != 0
    }
}

/// The `AY\0\0` chunk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AyBlock {
    /// Flags: bit 0 Fuller Box, bit 1 Melodik on a 48K.
    pub flags: u8,
    /// The selected register.
    pub current_register: u8,
    /// The sixteen registers.
    pub registers: [u8; 16],
}

/// Why a snapshot or chunk could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SzxError {
    /// Shorter than the 8-byte header.
    TooShort,
    /// The `ZXST` magic is missing.
    BadMagic,
    /// Bytes after the last chunk are too few for a chunk header.
    TrailingBytes {
        /// Offset of the leftover bytes.
        offset: usize,
    },
    /// A chunk runs past the end of the file.
    TruncatedChunk {
        /// Chunk id.
        id: [u8; 4],
        /// Offset of the chunk header.
        offset: usize,
    },
    /// A chunk is shorter than the fields it must hold.
    ShortChunk {
        /// Chunk id.
        id: [u8; 4],
    },
    /// On encode: a chunk is over 4 GiB, or a page is not 16K.
    BadLength,
}

impl core::fmt::Display for SzxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = |id: &[u8; 4]| {
            String::from_utf8_lossy(id)
                .trim_end_matches('\0')
                .to_owned()
        };
        match self {
            Self::TooShort => write!(f, "SZX too short for its 8-byte header"),
            Self::BadMagic => write!(f, "not an SZX snapshot: missing \"ZXST\" magic"),
            Self::TrailingBytes { offset } => write!(
                f,
                "SZX truncated: trailing bytes at offset {offset} are too few for a chunk header"
            ),
            Self::TruncatedChunk { id, offset } => write!(
                f,
                "SZX chunk {} at offset {offset} runs past the end of the file",
                name(id)
            ),
            Self::ShortChunk { id } => {
                write!(f, "SZX chunk {} is shorter than its fields", name(id))
            }
            Self::BadLength => write!(f, "SZX chunk or page has the wrong length"),
        }
    }
}

impl std::error::Error for SzxError {}

fn word(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

impl Szx {
    /// An empty snapshot.
    #[must_use]
    pub fn new(major: u8, minor: u8, machine_id: u8, flags: u8) -> Self {
        Self {
            major,
            minor,
            machine_id,
            flags,
            chunks: Vec::new(),
        }
    }

    /// Parse an SZX file.
    ///
    /// # Errors
    ///
    /// Returns an error when the header is missing or a chunk is cut off.
    pub fn parse(data: &[u8]) -> Result<Self, SzxError> {
        if data.len() < HEADER_LEN {
            return Err(SzxError::TooShort);
        }
        if &data[..4] != MAGIC {
            return Err(SzxError::BadMagic);
        }
        let mut szx = Self::new(data[4], data[5], data[6], data[7]);
        let mut offset = HEADER_LEN;
        while offset < data.len() {
            if offset + CHUNK_HEADER > data.len() {
                return Err(SzxError::TrailingBytes { offset });
            }
            let mut id = [0u8; 4];
            id.copy_from_slice(&data[offset..offset + 4]);
            let size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let start = offset + CHUNK_HEADER;
            let end = usize::try_from(size)
                .ok()
                .and_then(|size| start.checked_add(size))
                .filter(|&end| end <= data.len())
                .ok_or(SzxError::TruncatedChunk { id, offset })?;
            szx.chunks.push(Chunk {
                id,
                data: data[start..end].to_vec(),
            });
            offset = end;
        }
        Ok(szx)
    }

    /// Encode the snapshot, chunks in order and as stored.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::BadLength`] for a chunk over 4 GiB.
    pub fn encode(&self) -> Result<Vec<u8>, SzxError> {
        let mut out = MAGIC.to_vec();
        out.extend_from_slice(&[self.major, self.minor, self.machine_id, self.flags]);
        for chunk in &self.chunks {
            let size = u32::try_from(chunk.data.len()).map_err(|_| SzxError::BadLength)?;
            out.extend_from_slice(&chunk.id);
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&chunk.data);
        }
        Ok(out)
    }

    /// The first chunk with this id.
    #[must_use]
    pub fn chunk(&self, id: [u8; 4]) -> Option<&Chunk> {
        self.chunks.iter().find(|c| c.id == id)
    }

    /// The `Z80R` chunk, if any.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] when it is too short.
    pub fn z80_registers(&self) -> Result<Option<Z80Registers>, SzxError> {
        self.chunk(*b"Z80R").map(Chunk::z80_registers).transpose()
    }

    /// The `SPCR` chunk, if any.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] when it is too short.
    pub fn spectrum_registers(&self) -> Result<Option<SpectrumRegisters>, SzxError> {
        self.chunk(*b"SPCR")
            .map(Chunk::spectrum_registers)
            .transpose()
    }

    /// Every `RAMP` chunk, in file order.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] when one is too short.
    pub fn ram_pages(&self) -> Result<Vec<RamPage<'_>>, SzxError> {
        self.chunks
            .iter()
            .filter(|c| &c.id == b"RAMP")
            .map(Chunk::ram_page)
            .collect()
    }

    /// The `AY\0\0` chunk, if any.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] when it is too short.
    pub fn ay(&self) -> Result<Option<AyBlock>, SzxError> {
        self.chunk(*b"AY\0\0").map(Chunk::ay).transpose()
    }
}

impl Chunk {
    /// Read as `Z80R`.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] for fewer than 29 bytes.
    pub fn z80_registers(&self) -> Result<Z80Registers, SzxError> {
        let d = &self.data;
        if d.len() < Z80R_MIN {
            return Err(SzxError::ShortChunk { id: self.id });
        }
        let timing = (d.len() >= Z80R_LEN).then(|| Z80Timing {
            cycles_start: u32::from_le_bytes([d[29], d[30], d[31], d[32]]),
            hold_int_req_cycles: d[33],
            flags: d[34],
            memptr: word(d, 35),
        });
        Ok(Z80Registers {
            af: word(d, 0),
            bc: word(d, 2),
            de: word(d, 4),
            hl: word(d, 6),
            af_alt: word(d, 8),
            bc_alt: word(d, 10),
            de_alt: word(d, 12),
            hl_alt: word(d, 14),
            ix: word(d, 16),
            iy: word(d, 18),
            sp: word(d, 20),
            pc: word(d, 22),
            i: d[24],
            r: d[25],
            iff1: d[26] != 0,
            iff2: d[27] != 0,
            im: d[28],
            timing,
        })
    }

    /// Read as `SPCR`.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] for fewer than 4 bytes.
    pub fn spectrum_registers(&self) -> Result<SpectrumRegisters, SzxError> {
        let [border, port_7ffd, port_1ffd_or_eff7, port_fe, ..] = self.data[..] else {
            return Err(SzxError::ShortChunk { id: self.id });
        };
        Ok(SpectrumRegisters {
            border,
            port_7ffd,
            port_1ffd_or_eff7,
            port_fe,
        })
    }

    /// Read as `RAMP`.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] for fewer than 3 bytes.
    pub fn ram_page(&self) -> Result<RamPage<'_>, SzxError> {
        let [lo, hi, bank, data @ ..] = &self.data[..] else {
            return Err(SzxError::ShortChunk { id: self.id });
        };
        Ok(RamPage {
            flags: u16::from_le_bytes([*lo, *hi]),
            bank: *bank,
            data,
        })
    }

    /// Read as `AY\0\0`.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::ShortChunk`] for fewer than 18 bytes.
    pub fn ay(&self) -> Result<AyBlock, SzxError> {
        let d = &self.data;
        if d.len() < 18 {
            return Err(SzxError::ShortChunk { id: self.id });
        }
        let mut registers = [0u8; 16];
        registers.copy_from_slice(&d[2..18]);
        Ok(AyBlock {
            flags: d[0],
            current_register: d[1],
            registers,
        })
    }

    /// A full 37-byte `Z80R` chunk. Missing timing is written as zeros.
    #[must_use]
    pub fn from_z80_registers(r: &Z80Registers) -> Self {
        let mut d = Vec::with_capacity(Z80R_LEN);
        for w in [
            r.af, r.bc, r.de, r.hl, r.af_alt, r.bc_alt, r.de_alt, r.hl_alt, r.ix, r.iy, r.sp, r.pc,
        ] {
            d.extend_from_slice(&w.to_le_bytes());
        }
        d.extend_from_slice(&[r.i, r.r, u8::from(r.iff1), u8::from(r.iff2), r.im]);
        let t = r.timing.unwrap_or_default();
        d.extend_from_slice(&t.cycles_start.to_le_bytes());
        d.extend_from_slice(&[t.hold_int_req_cycles, t.flags]);
        d.extend_from_slice(&t.memptr.to_le_bytes());
        Self {
            id: *b"Z80R",
            data: d,
        }
    }

    /// An 8-byte `SPCR` chunk; the four reserved bytes are zero.
    #[must_use]
    pub fn from_spectrum_registers(s: &SpectrumRegisters) -> Self {
        Self {
            id: *b"SPCR",
            data: vec![
                s.border,
                s.port_7ffd,
                s.port_1ffd_or_eff7,
                s.port_fe,
                0,
                0,
                0,
                0,
            ],
        }
    }

    /// An uncompressed `RAMP` chunk.
    ///
    /// # Errors
    ///
    /// Returns [`SzxError::BadLength`] unless the page is 16K.
    pub fn uncompressed_ram_page(bank: u8, page: &[u8]) -> Result<Self, SzxError> {
        if page.len() != PAGE_LEN {
            return Err(SzxError::BadLength);
        }
        let mut data = vec![0, 0, bank];
        data.extend_from_slice(page);
        Ok(Self { id: *b"RAMP", data })
    }

    /// An `AY\0\0` chunk.
    #[must_use]
    pub fn from_ay(ay: &AyBlock) -> Self {
        let mut data = vec![ay.flags, ay.current_register];
        data.extend_from_slice(&ay.registers);
        Self {
            id: *b"AY\0\0",
            data,
        }
    }
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn registers() -> Z80Registers {
        Z80Registers {
            af: 1,
            bc: 2,
            de: 3,
            hl: 4,
            af_alt: 5,
            bc_alt: 6,
            de_alt: 7,
            hl_alt: 8,
            ix: 9,
            iy: 10,
            sp: 0x7FF0,
            pc: 0x8000,
            i: 0x3F,
            r: 0x81,
            iff1: true,
            iff2: true,
            im: 1,
            timing: Some(Z80Timing {
                cycles_start: 12345,
                hold_int_req_cycles: 32,
                flags: 2,
                memptr: 0x1234,
            }),
        }
    }

    fn sample() -> Szx {
        let mut szx = Szx::new(1, 4, 2, 0);
        szx.chunks.push(Chunk::from_z80_registers(&registers()));
        szx.chunks
            .push(Chunk::from_spectrum_registers(&SpectrumRegisters {
                border: 3,
                port_7ffd: 0x10,
                port_1ffd_or_eff7: 0,
                port_fe: 0x13,
            }));
        for bank in 0..8u8 {
            szx.chunks
                .push(Chunk::uncompressed_ram_page(bank, &[bank; PAGE_LEN]).expect("16K"));
        }
        szx.chunks.push(Chunk::from_ay(&AyBlock {
            flags: 0,
            current_register: 7,
            registers: [0xAA; 16],
        }));
        szx.chunks.push(Chunk {
            id: *b"PLTT",
            data: vec![1, 2, 3],
        });
        szx
    }

    #[test]
    fn chunks_round_trip_byte_for_byte() {
        let szx = sample();
        let bytes = szx.encode().expect("encodes");
        assert!(bytes.starts_with(b"ZXST\x01\x04\x02\x00Z80R\x25\0\0\0"));
        let back = Szx::parse(&bytes).expect("parses");
        assert_eq!(back, szx);
        assert_eq!(back.encode().expect("encodes"), bytes);
    }

    #[test]
    fn views_read_back_what_was_written() {
        let szx = sample();
        assert_eq!(szx.z80_registers(), Ok(Some(registers())));
        let spcr = szx.spectrum_registers().expect("ok").expect("present");
        assert_eq!((spcr.border, spcr.port_7ffd, spcr.port_fe), (3, 0x10, 0x13));
        let pages = szx.ram_pages().expect("ok");
        assert_eq!(pages.len(), 8);
        assert!(!pages[3].is_compressed());
        assert_eq!((pages[3].bank, pages[3].data[0]), (3, 3));
        assert_eq!(szx.ay().expect("ok").map(|a| a.current_register), Some(7));
    }

    #[test]
    fn a_short_version_one_z80r_has_no_timing() {
        let mut chunk = Chunk::from_z80_registers(&registers());
        chunk.data.truncate(29);
        let regs = chunk.z80_registers().expect("29 bytes is enough");
        assert_eq!(regs.timing, None);
        assert_eq!(regs.pc, 0x8000);
        chunk.data.truncate(28);
        assert!(matches!(
            chunk.z80_registers(),
            Err(SzxError::ShortChunk { .. })
        ));
    }

    #[test]
    fn compressed_pages_are_kept_as_stored() {
        let chunk = Chunk {
            id: *b"RAMP",
            data: vec![1, 0, 5, 0x78, 0x9C, 0xAB],
        };
        let page = chunk.ram_page().expect("ok");
        assert!(page.is_compressed());
        assert_eq!(page.data, [0x78, 0x9C, 0xAB]);
    }

    #[test]
    fn missing_and_short_chunks() {
        let empty = Szx::new(1, 4, 1, 0);
        assert_eq!(empty.z80_registers(), Ok(None));
        assert_eq!(empty.spectrum_registers(), Ok(None));
        assert_eq!(empty.ay(), Ok(None));
        assert!(empty.ram_pages().expect("ok").is_empty());
        for id in [*b"SPCR", *b"RAMP", *b"AY\0\0"] {
            let chunk = Chunk { id, data: vec![0] };
            let short = Err::<(), _>(SzxError::ShortChunk { id });
            match &id {
                b"SPCR" => assert_eq!(chunk.spectrum_registers().map(|_| ()), short),
                b"RAMP" => assert_eq!(chunk.ram_page().map(|_| ()), short),
                _ => assert_eq!(chunk.ay().map(|_| ()), short),
            }
        }
        assert_eq!(
            Chunk::uncompressed_ram_page(0, &[0; 10]),
            Err(SzxError::BadLength)
        );
    }

    #[test]
    fn malformed_files_are_rejected() {
        assert_eq!(Szx::parse(b"ZXST"), Err(SzxError::TooShort));
        assert_eq!(Szx::parse(b"NOPE\x01\x04\x01\x00"), Err(SzxError::BadMagic));
        let mut bytes = Szx::new(1, 4, 1, 0).encode().expect("encodes");
        bytes.extend_from_slice(b"Z80");
        assert_eq!(
            Szx::parse(&bytes),
            Err(SzxError::TrailingBytes { offset: 8 })
        );
        let mut bytes = Szx::new(1, 4, 1, 0).encode().expect("encodes");
        bytes.extend_from_slice(b"RAMP\x10\0\0\0\0");
        assert_eq!(
            Szx::parse(&bytes),
            Err(SzxError::TruncatedChunk {
                id: *b"RAMP",
                offset: 8
            })
        );
        for e in [
            SzxError::TooShort,
            SzxError::BadMagic,
            SzxError::TrailingBytes { offset: 1 },
            SzxError::TruncatedChunk {
                id: *b"AY\0\0",
                offset: 1,
            },
            SzxError::ShortChunk { id: *b"Z80R" },
            SzxError::BadLength,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
