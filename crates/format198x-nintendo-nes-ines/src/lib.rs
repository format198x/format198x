//! The iNES / NES 2.0 file layout.
//!
//! An iNES file is a 16-byte header, an optional 512-byte trainer, then PRG
//! ROM and CHR ROM back to back. The header names the mapper board, the
//! nametable mirroring, whether the board has a battery, and how much ROM
//! follows. NES 2.0 is the same layout with a signature in byte 7 that
//! widens the mapper number to 12 bits and the ROM sizes to larger counts or
//! an exponent form.
//!
//! This crate reads and writes the layout and nothing more. Which board a
//! mapper number means, and how that board behaves, is cartridge hardware
//! and belongs to the emulator.
//!
//! [`parse`] keeps the header's sixteen bytes and anything after the CHR ROM,
//! so [`InesImage::to_bytes`] writes a file back byte for byte. [`encode`]
//! writes a new file from its parts.
//!
//! Format-author specification: <https://fms.komkon.org/EMUL8/NES.html>.
//! NES 2.0 sizes: `reference/by-system/nintendo-nes/2026-nes2-rom-size-observations.md`
//! in the 198x reference library.

use std::fmt;

/// Length of the header.
pub const HEADER_LEN: usize = 16;
/// Length of the trainer, when the header says there is one.
pub const TRAINER_LEN: usize = 512;
/// The four bytes every file starts with: `NES` and `$1A`.
pub const MAGIC: [u8; 4] = *b"NES\x1a";
const PRG_BANK: usize = 16 * 1024;
const CHR_BANK: usize = 8 * 1024;

/// How the header says the nametables are mirrored. A board with its own
/// mirroring control may override it once running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMirroring {
    /// Flags 6 bit 0 clear: the two horizontal strips share a nametable.
    Horizontal,
    /// Flags 6 bit 0 set: the two vertical strips share a nametable.
    Vertical,
    /// Flags 6 bit 3 set: the board supplies four nametables of its own.
    FourScreen,
}

/// The header's fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InesHeader {
    /// Raw PRG size byte (header byte 4). A 16 KiB bank count for iNES;
    /// NES 2.0 can extend it or use the exponent form.
    pub prg_rom_banks: u8,
    /// Raw CHR size byte (header byte 5). An 8 KiB bank count for iNES;
    /// NES 2.0 can extend it or use the exponent form.
    pub chr_rom_banks: u8,
    /// Mapper number: 8 bits for iNES, 12 for NES 2.0.
    pub mapper_number: u16,
    /// The mirroring the header declares.
    pub mirroring: HeaderMirroring,
    /// Whether the board keeps its PRG RAM alive with a battery.
    pub has_battery: bool,
    /// Whether the file carries the NES 2.0 signature.
    pub is_nes_2_0: bool,
}

/// A parsed file, borrowing its ROM from the bytes it was parsed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InesImage<'a> {
    /// The header's fields.
    pub header: InesHeader,
    /// The header exactly as stored, including the bytes [`InesHeader`]
    /// does not decode: NES 2.0's submapper, RAM sizes, timing and console
    /// type, or whatever an old dumping tool left in bytes 7 to 15.
    pub header_bytes: [u8; HEADER_LEN],
    /// The 512 bytes loaded at `$7000` before the CPU starts, if present.
    pub trainer: Option<&'a [u8]>,
    /// The PRG ROM.
    pub prg_rom: &'a [u8],
    /// The CHR ROM. Empty when the board uses CHR RAM instead.
    pub chr_rom: &'a [u8],
    /// Bytes after the CHR ROM: NES 2.0's miscellaneous ROM, or data an old
    /// tool appended. Kept so the file writes back unchanged.
    pub trailing: &'a [u8],
}

/// The parts of a new file, for [`encode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewImage<'a> {
    /// Mapper number: up to 255 for iNES, up to 4095 for NES 2.0.
    pub mapper_number: u16,
    /// The mirroring the header declares.
    pub mirroring: HeaderMirroring,
    /// Whether the board keeps its PRG RAM alive with a battery.
    pub has_battery: bool,
    /// Write a NES 2.0 header instead of an iNES one.
    pub nes_2_0: bool,
    /// The trainer, which must be 512 bytes.
    pub trainer: Option<&'a [u8]>,
    /// The PRG ROM.
    pub prg_rom: &'a [u8],
    /// The CHR ROM. Empty when the board uses CHR RAM.
    pub chr_rom: &'a [u8],
}

/// Which ROM an error is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rom {
    /// The PRG ROM, counted in 16 KiB banks.
    Prg,
    /// The CHR ROM, counted in 8 KiB banks.
    Chr,
}

impl fmt::Display for Rom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Prg => "PRG",
            Self::Chr => "CHR",
        })
    }
}

/// Why a file could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InesError {
    /// Shorter than the 16-byte header.
    TooShort,
    /// The first four bytes are not `NES` and `$1A`.
    BadMagic,
    /// A NES 2.0 PRG size too large to address.
    PrgSizeOverflow,
    /// A NES 2.0 CHR size too large to address.
    ChrSizeOverflow,
    /// The PRG ROM would end past the address space.
    PrgExtentOverflow,
    /// The CHR ROM would end past the address space.
    ChrExtentOverflow,
    /// The header asks for more bytes than the file holds.
    Truncated {
        /// Bytes the header describes.
        expected: usize,
        /// Bytes the file holds.
        actual: usize,
    },
    /// A trainer that is not 512 bytes.
    TrainerLength(usize),
    /// A mapper number the chosen header cannot hold.
    MapperOutOfRange {
        /// The mapper number asked for.
        mapper: u16,
        /// Whether the header was NES 2.0.
        nes_2_0: bool,
    },
    /// A ROM length the chosen header cannot describe.
    UnencodableSize {
        /// Which ROM.
        rom: Rom,
        /// Its length in bytes.
        len: usize,
        /// Whether the header was NES 2.0.
        nes_2_0: bool,
    },
    /// [`InesImage::header_bytes`] describes a different trainer or ROM
    /// length from the one the image holds.
    HeaderMismatch,
}

impl fmt::Display for InesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("iNES file too short (< 16 bytes)"),
            Self::BadMagic => f.write_str("Invalid iNES magic (expected NES\\x1A)"),
            Self::PrgSizeOverflow => f.write_str("NES 2.0 PRG ROM size exceeds the address space"),
            Self::ChrSizeOverflow => f.write_str("NES 2.0 CHR ROM size exceeds the address space"),
            Self::PrgExtentOverflow => f.write_str("iNES PRG ROM extent exceeds the address space"),
            Self::ChrExtentOverflow => f.write_str("iNES CHR ROM extent exceeds the address space"),
            Self::Truncated { expected, actual } => write!(
                f,
                "iNES file too short: expected {expected} bytes, got {actual}"
            ),
            Self::TrainerLength(len) => {
                write!(f, "a trainer is {TRAINER_LEN} bytes, not {len}")
            }
            Self::MapperOutOfRange { mapper, nes_2_0 } => {
                let max = if *nes_2_0 { 4095 } else { 255 };
                let kind = header_kind(*nes_2_0);
                write!(
                    f,
                    "mapper {mapper} does not fit an {kind} header (max {max})"
                )
            }
            Self::UnencodableSize { rom, len, nes_2_0 } => {
                let kind = header_kind(*nes_2_0);
                write!(
                    f,
                    "an {kind} header cannot describe {len} bytes of {rom} ROM"
                )
            }
            Self::HeaderMismatch => f.write_str(
                "the header bytes describe a different trainer or ROM size from the image's",
            ),
        }
    }
}

fn header_kind(nes_2_0: bool) -> &'static str {
    if nes_2_0 { "NES 2.0" } else { "iNES" }
}

impl std::error::Error for InesError {}

/// What a header says, and the lengths it gives the parts after it.
struct Decoded {
    header: InesHeader,
    has_trainer: bool,
    prg_size: usize,
    chr_size: usize,
}

fn decode_header(data: &[u8; HEADER_LEN]) -> Result<Decoded, InesError> {
    if data[0..4] != MAGIC {
        return Err(InesError::BadMagic);
    }

    let prg_banks = data[4];
    let chr_banks = data[5];
    let flags6 = data[6];
    let flags7 = data[7];
    let mapper_lo = (flags6 >> 4) & 0x0F;
    let mapper_hi = flags7 & 0xF0;

    // NES 2.0 is signalled by bits 3-2 of flags 7 reading 0b10.
    let is_nes_2_0 = (flags7 & 0x0C) == 0x08;

    let (mapper_number, prg_size, chr_size) = if is_nes_2_0 {
        let mapper_number =
            u16::from(mapper_lo) | u16::from(mapper_hi) | (u16::from(data[8] & 0x0F) << 8);
        let prg_size =
            nes2_rom_size(prg_banks, data[9] & 0x0F, PRG_BANK).ok_or(InesError::PrgSizeOverflow)?;
        let chr_size =
            nes2_rom_size(chr_banks, data[9] >> 4, CHR_BANK).ok_or(InesError::ChrSizeOverflow)?;
        (mapper_number, prg_size, chr_size)
    } else {
        (
            u16::from(mapper_hi | mapper_lo),
            usize::from(prg_banks) * PRG_BANK,
            usize::from(chr_banks) * CHR_BANK,
        )
    };

    let mirroring = if flags6 & 0x08 != 0 {
        HeaderMirroring::FourScreen
    } else if flags6 & 0x01 != 0 {
        HeaderMirroring::Vertical
    } else {
        HeaderMirroring::Horizontal
    };

    Ok(Decoded {
        header: InesHeader {
            prg_rom_banks: prg_banks,
            chr_rom_banks: chr_banks,
            mapper_number,
            mirroring,
            has_battery: flags6 & 0x02 != 0,
            is_nes_2_0,
        },
        has_trainer: flags6 & 0x04 != 0,
        prg_size,
        chr_size,
    })
}

/// Parse an iNES or NES 2.0 file.
///
/// # Errors
///
/// [`InesError`] when the file has no header, lacks the magic, or describes
/// more ROM than it holds or than the address space can.
pub fn parse(data: &[u8]) -> Result<InesImage<'_>, InesError> {
    let header_bytes: [u8; HEADER_LEN] = data
        .get(..HEADER_LEN)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(InesError::TooShort)?;
    let decoded = decode_header(&header_bytes)?;

    let prg_start = if decoded.has_trainer {
        HEADER_LEN + TRAINER_LEN
    } else {
        HEADER_LEN
    };
    let chr_start = prg_start
        .checked_add(decoded.prg_size)
        .ok_or(InesError::PrgExtentOverflow)?;
    let rom_end = chr_start
        .checked_add(decoded.chr_size)
        .ok_or(InesError::ChrExtentOverflow)?;
    if data.len() < rom_end {
        return Err(InesError::Truncated {
            expected: rom_end,
            actual: data.len(),
        });
    }

    Ok(InesImage {
        header: decoded.header,
        header_bytes,
        trainer: decoded.has_trainer.then(|| &data[HEADER_LEN..prg_start]),
        prg_rom: &data[prg_start..chr_start],
        chr_rom: &data[chr_start..rom_end],
        trailing: &data[rom_end..],
    })
}

impl InesImage<'_> {
    /// The file: the stored header, then the trainer, PRG ROM, CHR ROM and
    /// trailing bytes. A parsed image writes back byte for byte.
    ///
    /// [`InesImage::header`] is not consulted; the header written is
    /// [`InesImage::header_bytes`].
    ///
    /// # Errors
    ///
    /// [`InesError::BadMagic`] or a size overflow if the header bytes are
    /// not a valid header, and [`InesError::HeaderMismatch`] if they
    /// describe a different trainer or ROM length from the image's.
    pub fn to_bytes(&self) -> Result<Vec<u8>, InesError> {
        let decoded = decode_header(&self.header_bytes)?;
        if decoded.has_trainer != self.trainer.is_some()
            || self.trainer.is_some_and(|t| t.len() != TRAINER_LEN)
            || decoded.prg_size != self.prg_rom.len()
            || decoded.chr_size != self.chr_rom.len()
        {
            return Err(InesError::HeaderMismatch);
        }
        let mut out = Vec::with_capacity(
            HEADER_LEN
                + self.trainer.map_or(0, <[u8]>::len)
                + self.prg_rom.len()
                + self.chr_rom.len()
                + self.trailing.len(),
        );
        out.extend_from_slice(&self.header_bytes);
        out.extend_from_slice(self.trainer.unwrap_or_default());
        out.extend_from_slice(self.prg_rom);
        out.extend_from_slice(self.chr_rom);
        out.extend_from_slice(self.trailing);
        Ok(out)
    }
}

/// Write a new file from its parts.
///
/// An iNES header counts whole banks, at most 255 of each. A NES 2.0 header
/// counts banks up to `$EFF`, and otherwise uses the exponent form,
/// 2^E × (2M + 1) bytes, so any length of that shape fits. The NES 2.0
/// fields this crate does not model — submapper, RAM sizes, timing, console
/// type and miscellaneous ROM count — are written as zero.
///
/// # Errors
///
/// [`InesError::TrainerLength`], [`InesError::MapperOutOfRange`] or
/// [`InesError::UnencodableSize`] when the parts do not fit the header asked
/// for.
pub fn encode(image: &NewImage<'_>) -> Result<Vec<u8>, InesError> {
    let NewImage {
        mapper_number,
        mirroring,
        has_battery,
        nes_2_0,
        trainer,
        prg_rom,
        chr_rom,
    } = *image;

    if let Some(trainer) = trainer
        && trainer.len() != TRAINER_LEN
    {
        return Err(InesError::TrainerLength(trainer.len()));
    }
    let mapper_max = if nes_2_0 { 0x0FFF } else { 0x00FF };
    if mapper_number > mapper_max {
        return Err(InesError::MapperOutOfRange {
            mapper: mapper_number,
            nes_2_0,
        });
    }

    let size = |rom: Rom, len: usize, bank: usize| {
        let fitted = if nes_2_0 {
            nes2_size_fields(len, bank)
        } else {
            len.is_multiple_of(bank)
                .then(|| u8::try_from(len / bank).ok())
                .flatten()
                .map(|count| (count, 0))
        };
        fitted.ok_or(InesError::UnencodableSize { rom, len, nes_2_0 })
    };
    let (prg_low, prg_high) = size(Rom::Prg, prg_rom.len(), PRG_BANK)?;
    let (chr_low, chr_high) = size(Rom::Chr, chr_rom.len(), CHR_BANK)?;

    let [mapper_hi_byte, mapper_lo_byte] = mapper_number.to_be_bytes();
    let mut header = [0u8; HEADER_LEN];
    header[0..4].copy_from_slice(&MAGIC);
    header[4] = prg_low;
    header[5] = chr_low;
    header[6] = (mapper_lo_byte << 4)
        | match mirroring {
            HeaderMirroring::Horizontal => 0,
            HeaderMirroring::Vertical => 0x01,
            HeaderMirroring::FourScreen => 0x08,
        }
        | if has_battery { 0x02 } else { 0 }
        | if trainer.is_some() { 0x04 } else { 0 };
    header[7] = (mapper_lo_byte & 0xF0) | if nes_2_0 { 0x08 } else { 0 };
    if nes_2_0 {
        header[8] = mapper_hi_byte & 0x0F;
        header[9] = (chr_high << 4) | prg_high;
    }

    let image = InesImage {
        header: decode_header(&header)?.header,
        header_bytes: header,
        trainer,
        prg_rom,
        chr_rom,
        trailing: &[],
    };
    image.to_bytes()
}

/// A NES 2.0 ROM size. An MSB nibble of `$F` makes the low byte `EEEEEEMM`,
/// meaning 2^E × (2M + 1) bytes; otherwise the nibble and byte together
/// count banks.
fn nes2_rom_size(low: u8, high: u8, bank_size: usize) -> Option<usize> {
    if high == 0x0F {
        let power = 1usize.checked_shl(u32::from(low >> 2))?;
        power.checked_mul(usize::from((low & 3) * 2 + 1))
    } else {
        ((usize::from(high) << 8) | usize::from(low)).checked_mul(bank_size)
    }
}

/// The `(low byte, MSB nibble)` that [`nes2_rom_size`] reads back as `len`:
/// a bank count when one fits, else the exponent form.
fn nes2_size_fields(len: usize, bank_size: usize) -> Option<(u8, u8)> {
    if len.is_multiple_of(bank_size) {
        let count = len / bank_size;
        if count <= 0x0EFF {
            let low = u8::try_from(count & 0xFF).ok()?;
            let high = u8::try_from(count >> 8).ok()?;
            return Some((low, high));
        }
    }
    (0u8..=3).find_map(|m| {
        let multiplier = usize::from(m) * 2 + 1;
        let power = len / multiplier;
        (len.is_multiple_of(multiplier) && power.is_power_of_two())
            .then(|| u8::try_from(power.trailing_zeros()).ok())
            .flatten()
            .filter(|&exponent| exponent <= 63)
            .map(|exponent| ((exponent << 2) | m, 0x0F))
    })
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
