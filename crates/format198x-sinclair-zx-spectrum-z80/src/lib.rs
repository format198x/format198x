//! ZX Spectrum `.z80` snapshots: read and write.
//!
//! Versions 1, 2 and 3 are covered, with the `ED ED` run-length compression
//! all three use. See
//! <https://worldofspectrum.org/faq/reference/z80format.htm>.
//!
//! # Layout
//!
//! Every version starts with the same 30-byte header. A non-zero PC at
//! offset 6 means version 1: one 48K block follows, compressed when bit 5 of
//! byte 12 is set and then ended by `00 ED ED 00`. A zero PC means version 2
//! or 3: a little-endian extension length at offset 30 (23 for version 2,
//! 54 or 55 for version 3), the real PC, the rest of the extension, and then
//! memory blocks. Each block is a length word (`$FFFF` for an uncompressed
//! 16K), a page number and the data.
//!
//! # Compression
//!
//! A run of five or more equal bytes, or of two or more `$ED`s, becomes
//! `ED ED count value`. A byte directly after a single literal `$ED` is never
//! the start of a run, so `ED 00 00 00 00 00` is written as `ED 00` and then
//! a run of four zeros, which itself stays literal.
//!
//! # What this crate does not decide
//!
//! The hardware mode's meaning depends on the version; [`Z80Snapshot::machine`]
//! applies the published table. How a machine loads a snapshot is the
//! emulator's business.

/// One 16K memory page.
pub const PAGE_LEN: usize = 0x4000;
/// Bytes in a version 1 memory image.
const V1_RAM_LEN: usize = 3 * PAGE_LEN;
/// The common header.
const HEADER_LEN: usize = 30;
/// Version 1's end-of-data marker.
const V1_END_MARKER: [u8; 4] = [0x00, 0xED, 0xED, 0x00];
/// Pages that hold a version 1 image, in address order: `$4000`, `$8000`,
/// `$C000`.
const V1_PAGES: [u8; 3] = [8, 4, 5];

/// Which version of the format a snapshot uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    /// Version 1: 48K only, a single memory block.
    V1,
    /// Version 2: a 23-byte extension.
    V2,
    /// Version 3: a 54-byte extension.
    V3,
    /// Version 3 with the 55-byte extension that adds the `$1FFD` byte.
    V3Plus3,
}

impl Version {
    /// The extension length the version writes, or `None` for version 1.
    #[must_use]
    pub fn extension_len(self) -> Option<usize> {
        match self {
            Self::V1 => None,
            Self::V2 => Some(23),
            Self::V3 => Some(54),
            Self::V3Plus3 => Some(55),
        }
    }
}

/// The machine a hardware mode names, per the published table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Machine {
    /// 48K Spectrum.
    Spectrum48K,
    /// 48K with Interface 1.
    Spectrum48KInterface1,
    /// 48K with a SamRam.
    SamRam,
    /// 48K with an M.G.T. disk interface (version 3 only).
    Spectrum48KMgt,
    /// 128K Spectrum.
    Spectrum128K,
    /// 128K with Interface 1.
    Spectrum128KInterface1,
    /// 128K with an M.G.T. disk interface (version 3 only).
    Spectrum128KMgt,
    /// Spectrum +3 (mode 7, and mode 8 as some emulators wrongly wrote it).
    SpectrumPlus3,
    /// Pentagon 128K.
    Pentagon128,
    /// Scorpion 256K.
    Scorpion256,
    /// Didaktik Kompakt.
    DidaktikKompakt,
    /// Spectrum +2.
    SpectrumPlus2,
    /// Spectrum +2A.
    SpectrumPlus2A,
    /// Timex Computer 2048.
    Tc2048,
    /// Timex Computer 2068.
    Tc2068,
    /// Timex Sinclair 2068.
    Ts2068,
    /// A mode the table does not list.
    Unknown(u8),
}

/// The register set, with every bit the format keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    /// AF; the file stores A first.
    pub af: u16,
    /// BC.
    pub bc: u16,
    /// DE.
    pub de: u16,
    /// HL.
    pub hl: u16,
    /// AF'; the file stores A' first.
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
    /// Interrupt vector register.
    pub i: u8,
    /// Refresh register, all eight bits.
    pub r: u8,
    /// IFF1.
    pub iff1: bool,
    /// IFF2.
    pub iff2: bool,
    /// Interrupt mode, 0 to 2.
    pub im: u8,
}

/// A `.z80` snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Z80Snapshot {
    /// Format version.
    pub version: Version,
    /// The registers.
    pub registers: Registers,
    /// Border colour, 0 to 7.
    pub border: u8,
    /// Byte 12, bit 4: the SamRom BASIC switched in.
    pub samrom: bool,
    /// Byte 29, bits 2 to 7: issue 2 emulation, double interrupt frequency,
    /// video synchronisation and joystick, kept as stored (bits 0 and 1 are
    /// zero here; they are the interrupt mode).
    pub emulator_flags: u8,
    /// For versions 2 and 3, the extension bytes after the PC (from the
    /// hardware mode on), kept as stored. Empty for version 1.
    pub extension: Vec<u8>,
    /// Memory pages in file order, each 16K, decompressed. A version 1
    /// image is pages 8, 4 and 5, the order of their addresses.
    pub pages: Vec<(u8, Vec<u8>)>,
}

/// Why a snapshot could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Z80Error {
    /// Shorter than the 30-byte header.
    TooShort,
    /// Version 2 or 3 data is shorter than its extension says.
    TruncatedExtension,
    /// An uncompressed version 1 image is shorter than 48K.
    TruncatedV1 {
        /// Bytes present after the header.
        len: usize,
    },
    /// On encode: the extension is not as long as the version requires.
    BadExtension,
    /// On encode: a page is not 16K, or a version 1 snapshot does not hold
    /// exactly pages 8, 4 and 5.
    BadPages,
    /// On encode: a version 1 snapshot has a PC of zero, which a reader
    /// would take for version 2 or 3.
    ZeroPcInV1,
}

impl core::fmt::Display for Z80Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort => write!(f, "file too short for a .z80 header"),
            Self::TruncatedExtension => write!(f, "file too short for its .z80 extended header"),
            Self::TruncatedV1 { len } => {
                write!(f, ".z80 v1 uncompressed data too short: {len} bytes")
            }
            Self::BadExtension => write!(f, ".z80 extension length does not match its version"),
            Self::BadPages => write!(
                f,
                ".z80 pages must be 16K, and a v1 snapshot holds exactly pages 8, 4 and 5"
            ),
            Self::ZeroPcInV1 => write!(f, "a .z80 v1 snapshot cannot store a PC of zero"),
        }
    }
}

impl std::error::Error for Z80Error {}

fn read_u16(data: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([data[pos], data[pos + 1]])
}

impl Z80Snapshot {
    /// Parse a `.z80` file.
    ///
    /// Reading is tolerant where preserved files are known to be untidy: a
    /// compressed block that decodes short is padded with zeros, and a
    /// truncated memory block at the end is ignored, as the Emu198x reader
    /// did.
    ///
    /// # Errors
    ///
    /// Returns an error when the header or extension is truncated, or an
    /// uncompressed version 1 image is shorter than 48K.
    pub fn parse(data: &[u8]) -> Result<Self, Z80Error> {
        if data.len() < HEADER_LEN {
            return Err(Z80Error::TooShort);
        }
        // Byte 12 of $FF means 1, for compatibility with old writers.
        let byte12 = if data[12] == 0xFF { 1 } else { data[12] };
        let mut registers = Registers {
            af: u16::from_be_bytes([data[0], data[1]]),
            bc: read_u16(data, 2),
            hl: read_u16(data, 4),
            pc: read_u16(data, 6),
            sp: read_u16(data, 8),
            i: data[10],
            r: (data[11] & 0x7F) | ((byte12 & 0x01) << 7),
            de: read_u16(data, 13),
            bc_alt: read_u16(data, 15),
            de_alt: read_u16(data, 17),
            hl_alt: read_u16(data, 19),
            af_alt: u16::from_be_bytes([data[21], data[22]]),
            iy: read_u16(data, 23),
            ix: read_u16(data, 25),
            iff1: data[27] != 0,
            iff2: data[28] != 0,
            im: data[29] & 0x03,
        };
        let border = (byte12 >> 1) & 0x07;
        let samrom = byte12 & 0x10 != 0;
        let emulator_flags = data[29] & 0xFC;

        if registers.pc != 0 {
            let raw = &data[HEADER_LEN..];
            let mem = if byte12 & 0x20 != 0 {
                decompress(raw, V1_RAM_LEN, true)
            } else {
                raw.get(..V1_RAM_LEN)
                    .ok_or(Z80Error::TruncatedV1 { len: raw.len() })?
                    .to_vec()
            };
            let pages = V1_PAGES
                .iter()
                .zip(mem.chunks(PAGE_LEN))
                .map(|(&page, chunk)| (page, chunk.to_vec()))
                .collect();
            return Ok(Self {
                version: Version::V1,
                registers,
                border,
                samrom,
                emulator_flags,
                extension: Vec::new(),
                pages,
            });
        }

        if data.len() < HEADER_LEN + 4 {
            return Err(Z80Error::TruncatedExtension);
        }
        let ext_len = usize::from(read_u16(data, 30));
        let ext_end = 32 + ext_len;
        if ext_len < 2 || data.len() < ext_end {
            return Err(Z80Error::TruncatedExtension);
        }
        registers.pc = read_u16(data, 32);
        let version = match ext_len {
            23 => Version::V2,
            55.. => Version::V3Plus3,
            _ => Version::V3,
        };
        let extension = data[34..ext_end].to_vec();

        let mut offset = ext_end;
        let mut pages = Vec::new();
        while offset + 3 <= data.len() {
            let block_len = usize::from(read_u16(data, offset));
            let page = data[offset + 2];
            offset += 3;
            if block_len == 0xFFFF {
                let Some(block) = data.get(offset..offset + PAGE_LEN) else {
                    break;
                };
                pages.push((page, block.to_vec()));
                offset += PAGE_LEN;
            } else {
                let Some(block) = data.get(offset..offset + block_len) else {
                    break;
                };
                pages.push((page, decompress(block, PAGE_LEN, false)));
                offset += block_len;
            }
        }
        Ok(Self {
            version,
            registers,
            border,
            samrom,
            emulator_flags,
            extension,
            pages,
        })
    }

    /// The raw hardware mode, or `None` for version 1 (always a 48K).
    #[must_use]
    pub fn hardware_mode(&self) -> Option<u8> {
        self.extension.first().copied()
    }

    /// The machine the hardware mode names, read with the table for this
    /// snapshot's version.
    #[must_use]
    pub fn machine(&self) -> Machine {
        let Some(mode) = self.hardware_mode() else {
            return Machine::Spectrum48K;
        };
        let common = match mode {
            0 => Some(Machine::Spectrum48K),
            1 => Some(Machine::Spectrum48KInterface1),
            2 => Some(Machine::SamRam),
            128 => Some(Machine::Ts2068),
            _ => None,
        };
        if let Some(machine) = common {
            return machine;
        }
        if self.version == Version::V2 {
            return match mode {
                3 => Machine::Spectrum128K,
                4 => Machine::Spectrum128KInterface1,
                other => Machine::Unknown(other),
            };
        }
        match mode {
            3 => Machine::Spectrum48KMgt,
            4 => Machine::Spectrum128K,
            5 => Machine::Spectrum128KInterface1,
            6 => Machine::Spectrum128KMgt,
            7 | 8 => Machine::SpectrumPlus3,
            9 => Machine::Pentagon128,
            10 => Machine::Scorpion256,
            11 => Machine::DidaktikKompakt,
            12 => Machine::SpectrumPlus2,
            13 => Machine::SpectrumPlus2A,
            14 => Machine::Tc2048,
            15 => Machine::Tc2068,
            other => Machine::Unknown(other),
        }
    }

    /// The last `$7FFD` write, or `None` for version 1.
    #[must_use]
    pub fn port_7ffd(&self) -> Option<u8> {
        self.extension.get(1).copied()
    }

    /// The selected AY register, when the extension holds it.
    #[must_use]
    pub fn ay_register(&self) -> Option<u8> {
        self.extension.get(4).copied()
    }

    /// The sixteen AY registers, when the extension holds them.
    #[must_use]
    pub fn ay_registers(&self) -> Option<[u8; 16]> {
        let bytes = self.extension.get(5..21)?;
        let mut regs = [0u8; 16];
        regs.copy_from_slice(bytes);
        Some(regs)
    }

    /// The last `$1FFD` write, in a 55-byte version 3 extension.
    #[must_use]
    pub fn port_1ffd(&self) -> Option<u8> {
        self.extension.get(52).copied()
    }

    /// The page with the given number.
    #[must_use]
    pub fn page(&self, number: u8) -> Option<&[u8]> {
        self.pages
            .iter()
            .find(|(n, _)| *n == number)
            .map(|(_, data)| data.as_slice())
    }

    /// Encode the snapshot, compressing memory when `compress` is set.
    ///
    /// For versions 2 and 3, a compressed page that would not be smaller is
    /// written uncompressed, as the format allows.
    ///
    /// # Errors
    ///
    /// Returns an error when the extension's length does not match the
    /// version, a page is not 16K, a version 1 snapshot does not hold pages
    /// 8, 4 and 5, or its PC is zero.
    pub fn encode(&self, compress: bool) -> Result<Vec<u8>, Z80Error> {
        if self.pages.iter().any(|(_, p)| p.len() != PAGE_LEN) {
            return Err(Z80Error::BadPages);
        }
        let r = &self.registers;
        let v1 = self.version == Version::V1;
        if v1 && r.pc == 0 {
            return Err(Z80Error::ZeroPcInV1);
        }
        let mut out = Vec::with_capacity(HEADER_LEN + V1_RAM_LEN);
        out.extend_from_slice(&r.af.to_be_bytes());
        out.extend_from_slice(&r.bc.to_le_bytes());
        out.extend_from_slice(&r.hl.to_le_bytes());
        out.extend_from_slice(&(if v1 { r.pc } else { 0 }).to_le_bytes());
        out.extend_from_slice(&r.sp.to_le_bytes());
        out.push(r.i);
        out.push(r.r & 0x7F);
        let byte12 = (r.r >> 7)
            | ((self.border & 0x07) << 1)
            | (u8::from(self.samrom) << 4)
            | (u8::from(v1 && compress) << 5);
        out.push(byte12);
        for word in [r.de, r.bc_alt, r.de_alt, r.hl_alt] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&r.af_alt.to_be_bytes());
        out.extend_from_slice(&r.iy.to_le_bytes());
        out.extend_from_slice(&r.ix.to_le_bytes());
        out.push(u8::from(r.iff1));
        out.push(u8::from(r.iff2));
        out.push((self.emulator_flags & 0xFC) | (r.im & 0x03));

        if v1 {
            let numbers: Vec<u8> = self.pages.iter().map(|(n, _)| *n).collect();
            if numbers != V1_PAGES {
                return Err(Z80Error::BadPages);
            }
            let mem: Vec<u8> = self
                .pages
                .iter()
                .flat_map(|(_, p)| p.iter().copied())
                .collect();
            if compress {
                out.extend_from_slice(&compress_block(&mem));
                out.extend_from_slice(&V1_END_MARKER);
            } else {
                out.extend_from_slice(&mem);
            }
            return Ok(out);
        }

        let ext_len = self.version.extension_len().unwrap_or(0);
        if self.extension.len() + 2 != ext_len {
            return Err(Z80Error::BadExtension);
        }
        // The extension length is 23, 54 or 55.
        #[allow(clippy::cast_possible_truncation)]
        out.extend_from_slice(&(ext_len as u16).to_le_bytes());
        out.extend_from_slice(&r.pc.to_le_bytes());
        out.extend_from_slice(&self.extension);
        for (number, page) in &self.pages {
            let packed = compress.then(|| compress_block(page));
            match packed {
                Some(packed) if packed.len() < PAGE_LEN => {
                    // Shorter than a page, so it fits a u16.
                    #[allow(clippy::cast_possible_truncation)]
                    out.extend_from_slice(&(packed.len() as u16).to_le_bytes());
                    out.push(*number);
                    out.extend_from_slice(&packed);
                }
                _ => {
                    out.extend_from_slice(&0xFFFFu16.to_le_bytes());
                    out.push(*number);
                    out.extend_from_slice(page);
                }
            }
        }
        Ok(out)
    }
}

/// Expand `ED ED count value` runs to at most `limit` bytes, padding with
/// zeros if the data runs out. Version 1 data stops at its end marker.
fn decompress(data: &[u8], limit: usize, v1: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(limit);
    let mut i = 0;
    while i < data.len() && out.len() < limit {
        if v1 && data[i..].starts_with(&V1_END_MARKER) {
            break;
        }
        if i + 3 < data.len() && data[i] == 0xED && data[i + 1] == 0xED {
            let count = usize::from(data[i + 2]).min(limit - out.len());
            out.extend(std::iter::repeat_n(data[i + 3], count));
            i += 4;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out.resize(limit, 0);
    out
}

/// Compress with `ED ED count value` runs.
fn compress_block(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut after_literal_ed = false;
    while i < data.len() {
        let value = data[i];
        let run = data[i..]
            .iter()
            .take(255)
            .take_while(|&&b| b == value)
            .count();
        if !after_literal_ed && (run >= 5 || (value == 0xED && run >= 2)) {
            // `run` is at most 255 by the `take` above.
            #[allow(clippy::cast_possible_truncation)]
            out.extend_from_slice(&[0xED, 0xED, run as u8, value]);
            i += run;
            after_literal_ed = false;
        } else {
            out.push(value);
            i += 1;
            after_literal_ed = value == 0xED;
        }
    }
    out
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn registers() -> Registers {
        Registers {
            af: 0x1234,
            bc: 0x5678,
            de: 0x9ABC,
            hl: 0xDEF0,
            af_alt: 0x1122,
            bc_alt: 0x3344,
            de_alt: 0x5566,
            hl_alt: 0x7788,
            ix: 0x99AA,
            iy: 0xBBCC,
            sp: 0xFF00,
            pc: 0x8000,
            i: 0x3F,
            r: 0xC5,
            iff1: true,
            iff2: false,
            im: 2,
        }
    }

    fn patterned_page(seed: u8) -> Vec<u8> {
        let mut page = vec![0u8; PAGE_LEN];
        for (i, b) in page.iter_mut().enumerate().take(300) {
            *b = (i as u8).wrapping_mul(seed);
        }
        page[400..410].fill(0xED);
        page[500] = 0xED;
        page[501..520].fill(0x00);
        page
    }

    fn v1() -> Z80Snapshot {
        Z80Snapshot {
            version: Version::V1,
            registers: registers(),
            border: 5,
            samrom: false,
            emulator_flags: 0x04,
            extension: Vec::new(),
            pages: V1_PAGES.iter().map(|&n| (n, patterned_page(n))).collect(),
        }
    }

    fn v3(paged: u8) -> Z80Snapshot {
        let mut extension = vec![0u8; 53];
        extension[0] = 4; // 128K in version 3
        extension[1] = paged;
        extension[4] = 7;
        extension[5..21].copy_from_slice(&[1; 16]);
        extension[52] = 0x04;
        Z80Snapshot {
            version: Version::V3Plus3,
            registers: registers(),
            border: 2,
            samrom: true,
            emulator_flags: 0,
            extension,
            pages: (3..11).map(|n| (n, patterned_page(n))).collect(),
        }
    }

    #[test]
    fn version_1_round_trips_compressed_and_not() {
        let snap = v1();
        for compress in [true, false] {
            let bytes = snap.encode(compress).expect("encodes");
            assert_eq!(bytes[12] & 0x20 != 0, compress);
            assert_eq!(Z80Snapshot::parse(&bytes), Ok(snap.clone()), "{compress}");
        }
        assert_eq!(snap.machine(), Machine::Spectrum48K);
        assert_eq!(snap.hardware_mode(), None);
        assert_eq!(snap.port_7ffd(), None);
        assert_eq!(snap.page(4).map(|p| p[1]), Some(4));
    }

    #[test]
    fn version_3_round_trips_with_the_extension_and_pages() {
        let snap = v3(0x10);
        for compress in [true, false] {
            let bytes = snap.encode(compress).expect("encodes");
            assert_eq!(Z80Snapshot::parse(&bytes), Ok(snap.clone()));
        }
        assert_eq!(snap.machine(), Machine::Spectrum128K);
        assert_eq!(snap.port_7ffd(), Some(0x10));
        assert_eq!(snap.ay_register(), Some(7));
        assert_eq!(snap.ay_registers(), Some([1; 16]));
        assert_eq!(snap.port_1ffd(), Some(0x04));
        let smaller = snap.encode(true).expect("encodes");
        assert!(smaller.len() < snap.encode(false).expect("encodes").len());
    }

    #[test]
    fn version_2_round_trips() {
        let mut snap = v3(0);
        snap.version = Version::V2;
        snap.extension.truncate(21);
        snap.extension[0] = 3;
        let bytes = snap.encode(true).expect("encodes");
        assert_eq!(Z80Snapshot::parse(&bytes), Ok(snap.clone()));
        assert_eq!(snap.machine(), Machine::Spectrum128K);
        assert_eq!(snap.port_1ffd(), None);
    }

    #[test]
    fn the_r_register_keeps_bit_seven_in_byte_twelve() {
        let bytes = v1().encode(false).expect("encodes");
        assert_eq!(bytes[11], 0x45);
        assert_eq!(bytes[12] & 0x01, 1);
        assert_eq!((bytes[12] >> 1) & 0x07, 5);
        assert_eq!(&bytes[0..2], &[0x12, 0x34], "A is stored first");
    }

    #[test]
    fn byte_twelve_of_ff_reads_as_one() {
        let mut bytes = v1().encode(false).expect("encodes");
        bytes[12] = 0xFF;
        let snap = Z80Snapshot::parse(&bytes).expect("parses");
        assert_eq!(snap.border, 0);
        assert_eq!(snap.registers.r & 0x80, 0x80);
    }

    #[test]
    fn compression_follows_the_ed_rules() {
        assert_eq!(compress_block(&[0xED, 0xED]), [0xED, 0xED, 2, 0xED]);
        assert_eq!(compress_block(&[1, 1, 1, 1]), [1, 1, 1, 1]);
        assert_eq!(compress_block(&[2; 5]), [0xED, 0xED, 5, 2]);
        // The byte after a lone ED is never the start of a run.
        assert_eq!(
            compress_block(&[0xED, 0, 0, 0, 0, 0]),
            [0xED, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            compress_block(&[0xED, 0, 0, 0, 0, 0, 0]),
            [0xED, 0, 0xED, 0xED, 5, 0]
        );
        // A run straight after an ED run block may itself be a block.
        assert_eq!(
            compress_block(&[0xED, 0xED, 0, 0, 0, 0, 0]),
            [0xED, 0xED, 2, 0xED, 0xED, 0xED, 5, 0]
        );
        let long = vec![9u8; 600];
        assert_eq!(decompress(&compress_block(&long), 600, false), long);
    }

    #[test]
    fn the_hardware_table_depends_on_the_version() {
        let mut snap = v3(0);
        let cases = [
            (0, Machine::Spectrum48K),
            (1, Machine::Spectrum48KInterface1),
            (2, Machine::SamRam),
            (3, Machine::Spectrum48KMgt),
            (5, Machine::Spectrum128KInterface1),
            (6, Machine::Spectrum128KMgt),
            (7, Machine::SpectrumPlus3),
            (8, Machine::SpectrumPlus3),
            (9, Machine::Pentagon128),
            (10, Machine::Scorpion256),
            (11, Machine::DidaktikKompakt),
            (12, Machine::SpectrumPlus2),
            (13, Machine::SpectrumPlus2A),
            (14, Machine::Tc2048),
            (15, Machine::Tc2068),
            (128, Machine::Ts2068),
            (77, Machine::Unknown(77)),
        ];
        for (mode, machine) in cases {
            snap.extension[0] = mode;
            assert_eq!(snap.machine(), machine, "v3 mode {mode}");
        }
        snap.version = Version::V2;
        snap.extension[0] = 4;
        assert_eq!(snap.machine(), Machine::Spectrum128KInterface1);
        snap.extension[0] = 9;
        assert_eq!(snap.machine(), Machine::Unknown(9));
    }

    #[test]
    fn truncated_files_are_rejected_or_tolerated() {
        assert_eq!(Z80Snapshot::parse(&[0; 10]), Err(Z80Error::TooShort));
        let mut header = vec![0u8; 32];
        assert_eq!(
            Z80Snapshot::parse(&header),
            Err(Z80Error::TruncatedExtension)
        );
        header.extend_from_slice(&[23, 0]);
        assert_eq!(
            Z80Snapshot::parse(&header),
            Err(Z80Error::TruncatedExtension)
        );
        let mut v1_raw = v1().encode(false).expect("encodes");
        v1_raw.truncate(1000);
        assert_eq!(
            Z80Snapshot::parse(&v1_raw),
            Err(Z80Error::TruncatedV1 { len: 970 })
        );
        // A memory block cut short at the end of the file is skipped.
        let mut bytes = v3(0).encode(false).expect("encodes");
        bytes.truncate(bytes.len() - 10);
        assert_eq!(Z80Snapshot::parse(&bytes).expect("parses").pages.len(), 7);
        let mut bytes = v3(0).encode(true).expect("encodes");
        bytes.truncate(bytes.len() - 10);
        assert_eq!(Z80Snapshot::parse(&bytes).expect("parses").pages.len(), 7);
    }

    #[test]
    fn short_compressed_data_pads_with_zeros() {
        let mut bytes = v1().encode(false).expect("encodes")[..HEADER_LEN].to_vec();
        bytes[12] |= 0x20;
        bytes.extend_from_slice(&[0xED, 0xED, 3, 0x42, 0x07]);
        let snap = Z80Snapshot::parse(&bytes).expect("parses");
        assert_eq!(&snap.pages[0].1[..5], &[0x42, 0x42, 0x42, 0x07, 0x00]);
    }

    #[test]
    fn encode_rejects_inconsistent_snapshots() {
        let mut snap = v1();
        snap.registers.pc = 0;
        assert_eq!(snap.encode(true), Err(Z80Error::ZeroPcInV1));
        let mut snap = v1();
        snap.pages.swap(0, 1);
        assert_eq!(snap.encode(true), Err(Z80Error::BadPages));
        let mut snap = v3(0);
        snap.pages[0].1.pop();
        assert_eq!(snap.encode(true), Err(Z80Error::BadPages));
        let mut snap = v3(0);
        snap.extension.pop();
        assert_eq!(snap.encode(true), Err(Z80Error::BadExtension));
        for e in [
            Z80Error::TooShort,
            Z80Error::TruncatedExtension,
            Z80Error::TruncatedV1 { len: 1 },
            Z80Error::BadExtension,
            Z80Error::BadPages,
            Z80Error::ZeroPcInV1,
        ] {
            assert!(!e.to_string().is_empty());
        }
        assert_eq!(Version::V1.extension_len(), None);
    }
}
