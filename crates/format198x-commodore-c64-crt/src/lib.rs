//! Commodore 64 `CRT` cartridge images: read and write.
//!
//! A `.crt` file is the standard container for C64 cartridge dumps (the format
//! VICE, CCS64 and Hoxs64 use). It has a fixed 64-byte header carrying the
//! hardware type and the initial state of the `EXROM`/`GAME` control lines,
//! followed by one or more `CHIP` packets — each a ROM/RAM image tagged with a
//! bank number and a load address (`$8000` for ROML, `$A000` or `$E000` for
//! ROMH).
//!
//! This crate is pure layout: it turns the bytes into a [`CrtCartridge`] and back, and
//! leaves the PLA banking (how `EXROM`/`GAME` + the load addresses map into the
//! `$8000-$FFFF` window) to the machine.

/// Fixed CRT header length in bytes. The header's own length field usually
/// reports this, but some dumps pad it larger; we honour the reported value.
const MIN_HEADER_LEN: usize = 0x40;
const MAGIC: &[u8; 16] = b"C64 CARTRIDGE   ";
const CHIP_MAGIC: &[u8; 4] = b"CHIP";
const CHIP_HEADER_LEN: usize = 0x10;

/// One ROM/RAM image packet from a CRT file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrtChip {
    /// Chip type: 0 = ROM, 1 = RAM (no image), 2 = Flash ROM.
    pub chip_type: u16,
    /// Bank number this image belongs to (0 for unbanked carts).
    pub bank: u16,
    /// Load address: `$8000` (ROML), `$A000` or `$E000` (ROMH).
    pub load_address: u16,
    /// The ROM image bytes (its advertised size).
    pub data: Vec<u8>,
}

/// A parsed C64 cartridge image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrtCartridge {
    /// Cartridge hardware type (0 = normal/generic, 1 = Action Replay,
    /// 32 = EasyFlash, …). Only 0 (normal) is mapped by the base machine today.
    pub hardware_type: u16,
    /// `EXROM` line asserted (pulled low) at reset. Active-low in hardware; the
    /// header stores 0 for "asserted", which we normalise to `true` here.
    pub exrom: bool,
    /// `GAME` line asserted (pulled low) at reset. Same convention as `exrom`.
    pub game: bool,
    /// Cartridge name from the header (trimmed of NUL padding).
    pub name: String,
    /// The ROM/RAM image packets, in file order.
    pub chips: Vec<CrtChip>,
}

impl CrtCartridge {
    /// The banking shape implied by the `EXROM`/`GAME` lines at reset.
    #[must_use]
    pub fn mode(&self) -> CrtMode {
        match (self.exrom, self.game) {
            // EXROM low, GAME high → 8K ROM at $8000.
            (true, false) => CrtMode::Rom8k,
            // Both low → 16K ROM at $8000 + $A000.
            (true, true) => CrtMode::Rom16k,
            // EXROM high, GAME low → Ultimax (ROM at $8000 + $E000, RAM hidden).
            (false, true) => CrtMode::Ultimax,
            // Both high → nothing mapped (RAM-only / bank-switched-from-off).
            (false, false) => CrtMode::None,
        }
    }
}

/// The reset-time banking mode a cartridge selects via `EXROM`/`GAME`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrtMode {
    /// No ROM mapped at reset (both lines high — e.g. a bank-switched cart that
    /// powers up disabled).
    None,
    /// 8K ROM visible at `$8000-$9FFF`.
    Rom8k,
    /// 16K ROM visible at `$8000-$9FFF` + `$A000-$BFFF`.
    Rom16k,
    /// Ultimax: ROM at `$8000-$9FFF` + `$E000-$FFFF`, most RAM/ROM hidden.
    Ultimax,
}

/// Error surfaced while parsing a CRT image.
#[derive(Debug, PartialEq, Eq)]
pub enum CrtParseError {
    /// The image is shorter than the fixed header.
    TooShort { expected: usize, actual: usize },
    /// The 16-byte signature is not `C64 CARTRIDGE   `.
    InvalidSignature,
    /// The header's length field is smaller than the fixed minimum.
    InvalidHeaderLength(usize),
    /// A CHIP packet is malformed or extends past the end of the image.
    TruncatedChip { offset: usize },
    /// No CHIP packets were present.
    NoChips,
}
impl core::fmt::Display for CrtParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { expected, actual } => write!(
                f,
                "CRT image is too short: expected at least {expected} bytes, got {actual}"
            ),
            Self::InvalidSignature => write!(f, "CRT image has an invalid header signature"),
            Self::InvalidHeaderLength(v0) => write!(
                f,
                "CRT header length {v0} is below the {MIN_HEADER_LEN}-byte minimum"
            ),
            Self::TruncatedChip { offset } => write!(
                f,
                "CRT CHIP packet at offset {offset} is truncated or malformed"
            ),
            Self::NoChips => write!(f, "CRT image contains no CHIP packets"),
        }
    }
}

impl std::error::Error for CrtParseError {}

/// Error surfaced while encoding a CRT image.
#[derive(Debug, PartialEq, Eq)]
pub enum CrtEncodeError {
    /// The name is longer than the 32-byte header field.
    NameTooLong {
        /// The name's byte count.
        actual: usize,
    },
    /// A chip image is larger than the 16-bit size field allows.
    ChipTooLarge {
        /// Index of the chip in [`CrtCartridge::chips`].
        index: usize,
    },
    /// The cartridge has no chips, which [`parse`] would reject.
    NoChips,
}

impl core::fmt::Display for CrtEncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NameTooLong { actual } => {
                write!(f, "CRT name is {actual} bytes; the header field holds 32")
            }
            Self::ChipTooLarge { index } => {
                write!(
                    f,
                    "CRT chip {index} exceeds the 65535-byte image size field"
                )
            }
            Self::NoChips => write!(f, "CRT image needs at least one CHIP packet"),
        }
    }
}

impl std::error::Error for CrtEncodeError {}

/// Encode a cartridge as a `.crt` image.
///
/// Writes a 64-byte header with version `$0100` and subtype 0, then one
/// `CHIP` packet per chip with no padding. `parse(&encode(cart)?)` returns the
/// same cartridge when the name has no trailing spaces.
///
/// # Errors
///
/// Returns an error if the name exceeds 32 bytes, a chip image exceeds 65535
/// bytes, or there are no chips.
pub fn encode(cart: &CrtCartridge) -> Result<Vec<u8>, CrtEncodeError> {
    let name = cart.name.as_bytes();
    if name.len() > 32 {
        return Err(CrtEncodeError::NameTooLong { actual: name.len() });
    }
    if cart.chips.is_empty() {
        return Err(CrtEncodeError::NoChips);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&0x40u32.to_be_bytes());
    out.extend_from_slice(&0x0100u16.to_be_bytes());
    out.extend_from_slice(&cart.hardware_type.to_be_bytes());
    out.push(u8::from(!cart.exrom));
    out.push(u8::from(!cart.game));
    out.extend_from_slice(&[0u8; 6]);
    out.extend_from_slice(name);
    out.resize(MIN_HEADER_LEN, 0);
    for (index, chip) in cart.chips.iter().enumerate() {
        let size =
            u16::try_from(chip.data.len()).map_err(|_| CrtEncodeError::ChipTooLarge { index })?;
        out.extend_from_slice(CHIP_MAGIC);
        out.extend_from_slice(&(CHIP_HEADER_LEN as u32 + u32::from(size)).to_be_bytes());
        out.extend_from_slice(&chip.chip_type.to_be_bytes());
        out.extend_from_slice(&chip.bank.to_be_bytes());
        out.extend_from_slice(&chip.load_address.to_be_bytes());
        out.extend_from_slice(&size.to_be_bytes());
        out.extend_from_slice(&chip.data);
    }
    Ok(out)
}

fn be_u16(b: &[u8]) -> u16 {
    u16::from_be_bytes([b[0], b[1]])
}

fn be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Parse a `.crt` cartridge image.
///
/// # Errors
///
/// Returns an error if the signature is wrong, the header is malformed, or a
/// CHIP packet is truncated.
pub fn parse(bytes: &[u8]) -> Result<CrtCartridge, CrtParseError> {
    if bytes.len() < MIN_HEADER_LEN {
        return Err(CrtParseError::TooShort {
            expected: MIN_HEADER_LEN,
            actual: bytes.len(),
        });
    }
    if &bytes[0..16] != MAGIC {
        return Err(CrtParseError::InvalidSignature);
    }

    let header_len = be_u32(&bytes[0x10..0x14]) as usize;
    if header_len < MIN_HEADER_LEN || header_len > bytes.len() {
        return Err(CrtParseError::InvalidHeaderLength(header_len));
    }

    // The line bytes are 0 = asserted (low). Normalise to "asserted = true".
    let exrom = bytes[0x18] == 0;
    let game = bytes[0x19] == 0;
    let hardware_type = be_u16(&bytes[0x16..0x18]);
    let name = String::from_utf8_lossy(&bytes[0x20..0x40])
        .trim_end_matches('\0')
        .trim_end()
        .to_string();

    let mut chips = Vec::new();
    let mut offset = header_len;
    while offset + CHIP_HEADER_LEN <= bytes.len() {
        if &bytes[offset..offset + 4] != CHIP_MAGIC {
            return Err(CrtParseError::TruncatedChip { offset });
        }
        let packet_len = be_u32(&bytes[offset + 4..offset + 8]) as usize;
        let chip_type = be_u16(&bytes[offset + 8..offset + 10]);
        let bank = be_u16(&bytes[offset + 10..offset + 12]);
        let load_address = be_u16(&bytes[offset + 12..offset + 14]);
        let image_size = be_u16(&bytes[offset + 14..offset + 16]) as usize;

        let data_start = offset + CHIP_HEADER_LEN;
        let data_end = data_start + image_size;
        // `packet_len` covers the 0x10 header + the image; sanity-check both it
        // and the advertised image size against the buffer.
        if packet_len < CHIP_HEADER_LEN
            || data_end > bytes.len()
            || offset + packet_len > bytes.len()
        {
            return Err(CrtParseError::TruncatedChip { offset });
        }
        chips.push(CrtChip {
            chip_type,
            bank,
            load_address,
            data: bytes[data_start..data_end].to_vec(),
        });
        offset += packet_len.max(CHIP_HEADER_LEN + image_size);
    }

    if chips.is_empty() {
        return Err(CrtParseError::NoChips);
    }

    Ok(CrtCartridge {
        hardware_type,
        exrom,
        game,
        name,
        chips,
    })
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal CRT image: header + one CHIP packet.
    fn build_crt(exrom: u8, game: u8, hw: u16, load: u16, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(MAGIC);
        v.extend_from_slice(&0x40u32.to_be_bytes()); // header length
        v.extend_from_slice(&0x0100u16.to_be_bytes()); // version
        v.extend_from_slice(&hw.to_be_bytes());
        v.push(exrom);
        v.push(game);
        v.extend_from_slice(&[0u8; 6]); // reserved
        let mut name = *b"TESTCART\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
        name[0..8].copy_from_slice(b"TESTCART");
        v.extend_from_slice(&name); // 32-byte name
        assert_eq!(v.len(), 0x40);
        // CHIP packet.
        v.extend_from_slice(CHIP_MAGIC);
        v.extend_from_slice(&((CHIP_HEADER_LEN + data.len()) as u32).to_be_bytes());
        v.extend_from_slice(&0u16.to_be_bytes()); // ROM
        v.extend_from_slice(&0u16.to_be_bytes()); // bank 0
        v.extend_from_slice(&load.to_be_bytes());
        v.extend_from_slice(&(data.len() as u16).to_be_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn parses_plain_8k_rom() {
        let img = build_crt(0, 1, 0, 0x8000, &[0xAA; 0x2000]);
        let cart = parse(&img).expect("valid 8K CRT");
        assert_eq!(cart.hardware_type, 0);
        assert!(cart.exrom, "EXROM asserted");
        assert!(!cart.game, "GAME not asserted");
        assert_eq!(cart.mode(), CrtMode::Rom8k);
        assert_eq!(cart.name, "TESTCART");
        assert_eq!(cart.chips.len(), 1);
        assert_eq!(cart.chips[0].load_address, 0x8000);
        assert_eq!(cart.chips[0].data.len(), 0x2000);
    }

    #[test]
    fn mode_reflects_exrom_game_lines() {
        assert_eq!(
            parse(&build_crt(0, 0, 0, 0x8000, &[0; 0x4000]))
                .expect("valid 16K CRT")
                .mode(),
            CrtMode::Rom16k
        );
        assert_eq!(
            parse(&build_crt(1, 0, 0, 0x8000, &[0; 0x2000]))
                .expect("valid Ultimax CRT")
                .mode(),
            CrtMode::Ultimax
        );
    }

    #[test]
    fn rejects_bad_signature() {
        let mut img = build_crt(0, 1, 0, 0x8000, &[0; 0x2000]);
        img[0] = b'X';
        assert_eq!(parse(&img), Err(CrtParseError::InvalidSignature));
    }

    #[test]
    fn rejects_truncated_image() {
        assert!(matches!(
            parse(&[0u8; 8]),
            Err(CrtParseError::TooShort { .. })
        ));
    }

    #[test]
    fn rejects_truncated_chip_data() {
        let mut img = build_crt(0, 1, 0, 0x8000, &[0xAA; 0x2000]);
        img.truncate(0x40 + CHIP_HEADER_LEN + 0x100); // cut the ROM image short
        assert!(matches!(
            parse(&img),
            Err(CrtParseError::TruncatedChip { .. })
        ));
    }

    #[test]
    fn encode_matches_the_hand_built_image_and_round_trips() {
        let img = build_crt(0, 1, 0, 0x8000, &[0xAA; 0x2000]);
        let cart = parse(&img).expect("valid");
        assert_eq!(encode(&cart).expect("encodes"), img);
    }

    #[test]
    fn encode_round_trips_banked_carts() {
        let cart = CrtCartridge {
            hardware_type: 32,
            exrom: false,
            game: true,
            name: "EASY".to_owned(),
            chips: (0..3)
                .map(|bank| CrtChip {
                    chip_type: 2,
                    bank,
                    load_address: if bank % 2 == 0 { 0x8000 } else { 0xA000 },
                    data: vec![bank as u8; 0x2000],
                })
                .collect(),
        };
        assert_eq!(parse(&encode(&cart).expect("encodes")), Ok(cart));
    }

    #[test]
    fn encode_rejects_what_parse_cannot_read() {
        let mut cart = CrtCartridge {
            hardware_type: 0,
            exrom: true,
            game: false,
            name: "X".repeat(33),
            chips: Vec::new(),
        };
        assert_eq!(
            encode(&cart),
            Err(CrtEncodeError::NameTooLong { actual: 33 })
        );
        cart.name.clear();
        assert_eq!(encode(&cart), Err(CrtEncodeError::NoChips));
        cart.chips.push(CrtChip {
            chip_type: 0,
            bank: 0,
            load_address: 0x8000,
            data: vec![0; 0x1_0000],
        });
        assert_eq!(
            encode(&cart),
            Err(CrtEncodeError::ChipTooLarge { index: 0 })
        );
        assert!(!CrtEncodeError::NoChips.to_string().is_empty());
        assert!(!CrtParseError::InvalidHeaderLength(3).to_string().is_empty());
    }
}
