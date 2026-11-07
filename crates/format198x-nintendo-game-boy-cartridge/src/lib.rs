//! Game Boy cartridge headers.
//!
//! The DMG / CGB cartridge header lives at ROM offsets `$0100..=$014F`.
//! This crate decodes it into a [`CartridgeHeader`], and [`set_checksums`]
//! writes the header and global checksums after a ROM is changed. It names
//! the board the header describes without modelling it: which memory bank
//! controllers an emulator supports, and how they behave, is the
//! emulator's business.
//!
//! Header layout (Pan Docs §11):
//!
//! | Range            | Field                                            |
//! |------------------|--------------------------------------------------|
//! | `$0100..=$0103`  | entry point (typically `NOP; JP $0150`)          |
//! | `$0104..=$0133`  | Nintendo logo (48 bytes — boot ROM checks this)  |
//! | `$0134..=$0143`  | title (16 bytes; later carts use 11 + CGB flag)  |
//! | `$0143`          | CGB flag (`$80` = CGB-supported, `$C0` = CGB-only)|
//! | `$0144..=$0145`  | new licensee code (2 ASCII bytes)                |
//! | `$0146`          | SGB flag (`$03` = SGB-aware)                     |
//! | `$0147`          | cartridge type byte → [`CartridgeType`]          |
//! | `$0148`          | ROM size code (`32 KiB << code`)                 |
//! | `$0149`          | RAM size code → external-RAM bytes               |
//! | `$014A`          | destination code (`$00` = Japan, `$01` = world)  |
//! | `$014B`          | old licensee code                                |
//! | `$014C`          | mask ROM version                                 |
//! | `$014D`          | header checksum (sum of `$0134..=$014C` bytes)   |
//! | `$014E..=$014F`  | global checksum (16-bit big-endian)              |

use std::fmt;

/// Smallest legal Game Boy ROM (header + a single 32 KiB bank).
const MIN_ROM_SIZE: usize = 0x8000;

/// Header offsets relative to the start of the ROM image.
mod offset {
    pub const TITLE: usize = 0x0134;
    pub const TITLE_END: usize = 0x0144;
    pub const CGB_FLAG: usize = 0x0143;
    pub const NEW_LICENSEE: usize = 0x0144;
    pub const SGB_FLAG: usize = 0x0146;
    pub const CART_TYPE: usize = 0x0147;
    pub const ROM_SIZE: usize = 0x0148;
    pub const RAM_SIZE: usize = 0x0149;
    pub const DESTINATION: usize = 0x014A;
    pub const OLD_LICENSEE: usize = 0x014B;
    pub const MASK_VERSION: usize = 0x014C;
    pub const HEADER_CHECKSUM: usize = 0x014D;
    pub const GLOBAL_CHECKSUM: usize = 0x014E;
    pub const HEADER_CHECKSUM_RANGE_START: usize = 0x0134;
    pub const HEADER_CHECKSUM_RANGE_END: usize = 0x014C;
}

/// Decoded cartridge header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CartridgeHeader {
    /// Title bytes as read from the ROM, trimmed of trailing zeros.
    /// On post-CGB carts the field shrinks (the last 5 bytes become
    /// licensee + CGB flag); we still expose the trimmed title.
    pub title: String,
    /// CGB-compatibility flag. `$80` = CGB-aware (still runs on
    /// DMG); `$C0` = CGB-only.
    pub cgb_flag: u8,
    /// New licensee code (two ASCII bytes). Only meaningful when the
    /// old licensee code is `$33`.
    pub new_licensee: [u8; 2],
    /// SGB flag. `$03` = SGB-aware.
    pub sgb_flag: u8,
    /// The cartridge-type byte at `$0147`, as stored. [`CartridgeType`]
    /// does not tell `$00` from `$08`, which differ only in RAM the
    /// RAM-size byte also describes.
    pub cart_type_byte: u8,
    /// Decoded cartridge type.
    pub cart_type: CartridgeType,
    /// ROM size in bytes (`32 KiB << code` for codes 0..=8).
    pub rom_size: usize,
    /// External RAM size in bytes (0 / 2 KiB / 8 KiB / 32 KiB / 64 KiB / 128 KiB).
    pub ram_size: usize,
    /// Destination code: `$00` = Japan, `$01` = overseas.
    pub destination: u8,
    /// Old licensee code; `$33` indicates the new code at $0144 is in
    /// use.
    pub old_licensee: u8,
    /// Mask ROM version number (usually `$00`).
    pub mask_version: u8,
    /// Stored header checksum byte at `$014D`.
    pub header_checksum: u8,
    /// Stored global checksum (`$014E..=$014F`, big-endian).
    pub global_checksum: u16,
}

impl CartridgeHeader {
    /// Parse the header from a complete ROM image.
    ///
    /// # Errors
    ///
    /// Returns [`HeaderError`] when the ROM is too small, the cart
    /// type byte is not a documented one, the ROM-size code is invalid,
    /// the RAM-size code is unknown, the actual ROM length doesn't match
    /// the declared size, or the header checksum is wrong.
    pub fn parse(rom: &[u8]) -> Result<Self, HeaderError> {
        if rom.len() < MIN_ROM_SIZE {
            return Err(HeaderError::TooShort { len: rom.len() });
        }

        let cart_type = decode_cart_type(rom[offset::CART_TYPE])?;
        let rom_size = decode_rom_size(rom[offset::ROM_SIZE])?;
        let ram_size = decode_ram_size(rom[offset::RAM_SIZE], cart_type)?;

        if rom.len() != rom_size {
            return Err(HeaderError::RomLengthMismatch {
                declared: rom_size,
                actual: rom.len(),
            });
        }

        let header_checksum = rom[offset::HEADER_CHECKSUM];
        let computed = compute_header_checksum(rom);
        if header_checksum != computed {
            return Err(HeaderError::HeaderChecksumMismatch {
                stored: header_checksum,
                computed,
            });
        }

        let cgb_flag = rom[offset::CGB_FLAG];
        let title = decode_title(&rom[offset::TITLE..offset::TITLE_END], cgb_flag);
        let new_licensee = [rom[offset::NEW_LICENSEE], rom[offset::NEW_LICENSEE + 1]];
        let global_checksum = u16::from_be_bytes([
            rom[offset::GLOBAL_CHECKSUM],
            rom[offset::GLOBAL_CHECKSUM + 1],
        ]);

        Ok(Self {
            title,
            cgb_flag,
            new_licensee,
            sgb_flag: rom[offset::SGB_FLAG],
            cart_type_byte: rom[offset::CART_TYPE],
            cart_type,
            rom_size,
            ram_size,
            destination: rom[offset::DESTINATION],
            old_licensee: rom[offset::OLD_LICENSEE],
            mask_version: rom[offset::MASK_VERSION],
            header_checksum,
            global_checksum,
        })
    }

    /// What the cartridge asks of a Game Boy Color, from the CGB flag.
    ///
    /// Bit 7 marks a CGB game. The CGB boot ROM copies the flag to KEY0,
    /// and a flag with bit 2 or 3 also set selects a non-CGB mode, so
    /// such a cartridge runs like a DMG one (Pan Docs, "CGB Flag" and
    /// "CGB Registers: KEY0"; SameBoy `memory.c`, `GB_IO_KEY0`).
    #[must_use]
    pub const fn cgb_support(&self) -> CgbSupport {
        CgbSupport::from_flag(self.cgb_flag)
    }
}

/// How a cartridge uses a Game Boy Color, decoded from the CGB flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CgbSupport {
    /// A DMG game: a CGB runs it in DMG compatibility mode.
    None,
    /// A CGB-enhanced game that also runs on a DMG (flag `$80`).
    Enhanced,
    /// A CGB-only game (flag `$C0`).
    Only,
}

impl CgbSupport {
    /// Decodes a CGB flag byte (ROM offset `$0143`).
    #[must_use]
    pub const fn from_flag(flag: u8) -> Self {
        if flag & 0x80 == 0 || flag & 0x0C != 0 {
            Self::None
        } else if flag & 0x40 != 0 {
            Self::Only
        } else {
            Self::Enhanced
        }
    }

    /// Whether a CGB runs the cartridge in CGB mode.
    #[must_use]
    pub const fn uses_cgb_mode(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// The board the cartridge-type byte at `$0147` describes: which memory
/// bank controller, and whether it carries RAM, a battery, a clock or a
/// rumble motor. The byte describes hardware; this crate only names it.
///
/// Every type Pan Docs documents decodes, whether or not any emulator
/// supports it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CartridgeType {
    /// `$00`, `$08`, `$09`: no controller, any RAM wired directly.
    RomOnly { battery: bool },
    /// `$01..=$03`.
    Mbc1 { ram: bool, battery: bool },
    /// `$05..=$06`. MBC2 has 512 four-bit cells of RAM inside it.
    Mbc2 { battery: bool },
    /// `$0F..=$13`. The clock and the battery are independent.
    Mbc3 { ram: bool, battery: bool, rtc: bool },
    /// `$19..=$1E`.
    Mbc5 {
        ram: bool,
        battery: bool,
        rumble: bool,
    },
    /// `$0B..=$0D`: the MMM01 multicart controller.
    Mmm01 { ram: bool, battery: bool },
    /// `$20`.
    Mbc6,
    /// `$22`: MBC7 with its accelerometer, rumble, RAM and battery.
    Mbc7,
    /// `$FC`: the Game Boy Camera.
    PocketCamera,
    /// `$FD`: Bandai TAMA5.
    BandaiTama5,
    /// `$FE`: Hudson HuC3.
    HuC3,
    /// `$FF`: Hudson HuC1 with RAM and a battery.
    HuC1,
}

impl CartridgeType {
    /// The controller's name, for logs and messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RomOnly { .. } => "ROM only",
            Self::Mbc1 { .. } => "MBC1",
            Self::Mbc2 { .. } => "MBC2",
            Self::Mbc3 { .. } => "MBC3",
            Self::Mbc5 { .. } => "MBC5",
            Self::Mmm01 { .. } => "MMM01",
            Self::Mbc6 => "MBC6",
            Self::Mbc7 => "MBC7",
            Self::PocketCamera => "Pocket Camera",
            Self::BandaiTama5 => "Bandai TAMA5",
            Self::HuC3 => "HuC3",
            Self::HuC1 => "HuC1",
        }
    }

    /// Whether the type byte says the board keeps anything alive with a
    /// battery. Types whose documented name lists no battery report
    /// `false`.
    #[must_use]
    pub const fn has_battery(self) -> bool {
        match self {
            Self::RomOnly { battery }
            | Self::Mbc1 { battery, .. }
            | Self::Mbc2 { battery }
            | Self::Mbc3 { battery, .. }
            | Self::Mbc5 { battery, .. }
            | Self::Mmm01 { battery, .. } => battery,
            Self::Mbc7 | Self::HuC1 => true,
            Self::Mbc6 | Self::PocketCamera | Self::BandaiTama5 | Self::HuC3 => false,
        }
    }
}

/// Header-decoding errors.
#[derive(Debug, Eq, PartialEq)]
pub enum HeaderError {
    /// ROM is shorter than the minimum 32 KiB header + bank-0 image.
    TooShort { len: usize },
    /// Cart-type byte isn't in any documented Pan Docs slot.
    UnknownCartridgeType { byte: u8 },
    /// ROM-size code is outside the documented 0..=8 range.
    InvalidRomSize { code: u8 },
    /// RAM-size code isn't one of the known values.
    InvalidRamSize { code: u8 },
    /// File length doesn't match the declared ROM-size code.
    RomLengthMismatch { declared: usize, actual: usize },
    /// Header checksum byte at $014D is wrong. The boot ROM will
    /// refuse to start a cart that fails this — we surface the
    /// failure rather than silently masking it.
    HeaderChecksumMismatch { stored: u8, computed: u8 },
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(
                f,
                "ROM too short — got {len} bytes, need at least {MIN_ROM_SIZE}"
            ),
            Self::UnknownCartridgeType { byte } => {
                write!(f, "unknown cartridge type byte ${byte:02X}")
            }
            Self::InvalidRomSize { code } => write!(f, "invalid ROM size code ${code:02X}"),
            Self::InvalidRamSize { code } => write!(f, "invalid RAM size code ${code:02X}"),
            Self::RomLengthMismatch { declared, actual } => write!(
                f,
                "ROM length mismatch — header says {declared} bytes, file has {actual}"
            ),
            Self::HeaderChecksumMismatch { stored, computed } => write!(
                f,
                "header checksum mismatch — stored ${stored:02X}, computed ${computed:02X}"
            ),
        }
    }
}

impl std::error::Error for HeaderError {}

fn decode_cart_type(byte: u8) -> Result<CartridgeType, HeaderError> {
    match byte {
        0x00 => Ok(CartridgeType::RomOnly { battery: false }),
        0x01 => Ok(CartridgeType::Mbc1 {
            ram: false,
            battery: false,
        }),
        0x02 => Ok(CartridgeType::Mbc1 {
            ram: true,
            battery: false,
        }),
        0x03 => Ok(CartridgeType::Mbc1 {
            ram: true,
            battery: true,
        }),
        0x05 => Ok(CartridgeType::Mbc2 { battery: false }),
        0x06 => Ok(CartridgeType::Mbc2 { battery: true }),
        // ROM+RAM (rare, 32 KiB ROMs). No MBC — the cartridge wires RAM
        // directly and the ram-size byte drives the allocation. $09 adds a
        // battery, so its RAM persists to the `.sav` sidecar; $08 does not.
        0x08 => Ok(CartridgeType::RomOnly { battery: false }),
        0x09 => Ok(CartridgeType::RomOnly { battery: true }),
        0x0F => Ok(CartridgeType::Mbc3 {
            ram: false,
            battery: true,
            rtc: true,
        }),
        0x10 => Ok(CartridgeType::Mbc3 {
            ram: true,
            battery: true,
            rtc: true,
        }),
        0x11 => Ok(CartridgeType::Mbc3 {
            ram: false,
            battery: false,
            rtc: false,
        }),
        0x12 => Ok(CartridgeType::Mbc3 {
            ram: true,
            battery: false,
            rtc: false,
        }),
        0x13 => Ok(CartridgeType::Mbc3 {
            ram: true,
            battery: true,
            rtc: false,
        }),
        0x19 => Ok(CartridgeType::Mbc5 {
            ram: false,
            battery: false,
            rumble: false,
        }),
        0x1A => Ok(CartridgeType::Mbc5 {
            ram: true,
            battery: false,
            rumble: false,
        }),
        0x1B => Ok(CartridgeType::Mbc5 {
            ram: true,
            battery: true,
            rumble: false,
        }),
        0x1C => Ok(CartridgeType::Mbc5 {
            ram: false,
            battery: false,
            rumble: true,
        }),
        0x1D => Ok(CartridgeType::Mbc5 {
            ram: true,
            battery: false,
            rumble: true,
        }),
        0x1E => Ok(CartridgeType::Mbc5 {
            ram: true,
            battery: true,
            rumble: true,
        }),
        0x0B => Ok(CartridgeType::Mmm01 {
            ram: false,
            battery: false,
        }),
        0x0C => Ok(CartridgeType::Mmm01 {
            ram: true,
            battery: false,
        }),
        0x0D => Ok(CartridgeType::Mmm01 {
            ram: true,
            battery: true,
        }),
        0x20 => Ok(CartridgeType::Mbc6),
        0x22 => Ok(CartridgeType::Mbc7),
        0xFC => Ok(CartridgeType::PocketCamera),
        0xFD => Ok(CartridgeType::BandaiTama5),
        0xFE => Ok(CartridgeType::HuC3),
        0xFF => Ok(CartridgeType::HuC1),
        _ => Err(HeaderError::UnknownCartridgeType { byte }),
    }
}

fn decode_rom_size(code: u8) -> Result<usize, HeaderError> {
    if code > 8 {
        return Err(HeaderError::InvalidRomSize { code });
    }
    Ok(0x8000usize << code)
}

fn decode_ram_size(code: u8, cart_type: CartridgeType) -> Result<usize, HeaderError> {
    // MBC2's "RAM" lives on the mapper chip (512×4 bits = 512 bytes).
    // The header byte is conventionally `$00`.
    if matches!(cart_type, CartridgeType::Mbc2 { .. }) {
        return Ok(0x200);
    }
    Ok(match code {
        0 => 0,
        1 => 0x800,   // 2 KiB (rare unofficial use)
        2 => 0x2000,  // 8 KiB
        3 => 0x8000,  // 32 KiB (4 banks of 8 KiB)
        4 => 0x20000, // 128 KiB (16 banks)
        5 => 0x10000, // 64 KiB (8 banks)
        _ => return Err(HeaderError::InvalidRamSize { code }),
    })
}

/// The header checksum and global checksum [`set_checksums`] wrote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Checksums {
    /// The byte at `$014D`.
    pub header: u8,
    /// The big-endian word at `$014E..=$014F`.
    pub global: u16,
}

/// The global checksum a ROM should store at `$014E..=$014F`: the sum of
/// every byte except those two. Hardware does not check it, so real ROMs
/// sometimes carry a wrong one.
#[must_use]
pub fn global_checksum(rom: &[u8]) -> u16 {
    rom.iter()
        .enumerate()
        .filter(|&(i, _)| i != offset::GLOBAL_CHECKSUM && i != offset::GLOBAL_CHECKSUM + 1)
        .fold(0u16, |sum, (_, &byte)| sum.wrapping_add(u16::from(byte)))
}

/// Write the header checksum and then the global checksum, which covers
/// it, so both match the ROM. A ROM whose checksums were already right is
/// left unchanged.
///
/// # Errors
///
/// [`HeaderError::TooShort`] for a ROM shorter than 32 KiB.
pub fn set_checksums(rom: &mut [u8]) -> Result<Checksums, HeaderError> {
    if rom.len() < MIN_ROM_SIZE {
        return Err(HeaderError::TooShort { len: rom.len() });
    }
    let header = compute_header_checksum(rom);
    rom[offset::HEADER_CHECKSUM] = header;
    let global = global_checksum(rom);
    rom[offset::GLOBAL_CHECKSUM..=offset::GLOBAL_CHECKSUM + 1]
        .copy_from_slice(&global.to_be_bytes());
    Ok(Checksums { header, global })
}

fn compute_header_checksum(rom: &[u8]) -> u8 {
    let mut checksum: u8 = 0;
    for &byte in &rom[offset::HEADER_CHECKSUM_RANGE_START..=offset::HEADER_CHECKSUM_RANGE_END] {
        checksum = checksum.wrapping_sub(byte).wrapping_sub(1);
    }
    checksum
}

fn decode_title(bytes: &[u8], cgb_flag: u8) -> String {
    // CGB-aware carts moved the last 4-5 bytes for the manufacturer
    // code + CGB flag. If the CGB flag is set, only the first 11
    // bytes are title.
    let effective_len = if cgb_flag == 0x80 || cgb_flag == 0xC0 {
        11
    } else {
        bytes.len()
    };
    let trimmed: Vec<u8> = bytes[..effective_len.min(bytes.len())]
        .iter()
        .copied()
        .take_while(|&b| b != 0)
        .filter(|&b| (0x20..=0x7E).contains(&b))
        .collect();
    String::from_utf8(trimmed).unwrap_or_default()
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests;
