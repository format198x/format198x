//! Commodore VIC-20 cartridge images: read and write.
//!
//! VICE `.crt` files carry a 64-byte `VIC20 CARTRIDGE ` header followed by
//! address-tagged `CHIP` packets. Raw 2/4/8 KiB images have no address metadata;
//! this parser treats those as the ordinary BLK5 game-cartridge window at
//! `$A000`, while multi-block images must use CRT so their wiring is explicit.

const MAGIC: &[u8; 16] = b"VIC20 CARTRIDGE ";
const CHIP_MAGIC: &[u8; 4] = b"CHIP";
const HEADER_LEN: usize = 0x40;
const CHIP_HEADER_LEN: usize = 0x10;

/// One address-tagged ROM image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartridgeBlock {
    /// CPU address at which the image is decoded.
    pub load_address: u16,
    /// ROM bytes in address order.
    pub data: Vec<u8>,
}

/// A parsed generic VIC-20 cartridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cartridge {
    /// VICE hardware type (`0` is a generic cartridge).
    pub hardware_type: u16,
    /// Header name, without trailing NUL or space padding.
    pub name: String,
    /// Address-tagged ROM blocks in file order.
    pub blocks: Vec<CartridgeBlock>,
}

impl Cartridge {
    /// Whether BLK5 carries the VIC-20 KERNAL's `A0` + high-bit `CBM` signature.
    #[must_use]
    pub fn is_autostart(&self) -> bool {
        self.blocks.iter().any(|block| {
            let start = usize::from(0xA004u16.saturating_sub(block.load_address));
            block.load_address <= 0xA004
                && start + 5 <= block.data.len()
                && block.data[start..start + 5] == [0x41, 0x30, 0xC3, 0xC2, 0xCD]
        })
    }
}

/// Cartridge parse failure.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// A non-CRT image is not one of the unambiguous BLK5 sizes.
    UnsupportedRawSize(usize),
    /// The CRT fixed header is missing or malformed.
    InvalidHeader,
    /// The container is for a different machine.
    InvalidSignature,
    /// Only generic, statically mapped cartridges are supported.
    UnsupportedHardware(u16),
    /// A CHIP packet is malformed.
    InvalidChip(usize),
    /// A CHIP packet maps outside BLK1/2/3/5.
    UnsupportedMapping { address: u16, size: usize },
    /// The CRT carries no ROM packets.
    NoBlocks,
}
impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedRawSize(v0) => write!(
                f,
                "raw VIC-20 cartridge must be 2, 4, or 8 KiB; got {v0} bytes"
            ),
            Self::InvalidHeader => write!(f, "VIC-20 CRT header is truncated or malformed"),
            Self::InvalidSignature => {
                write!(f, "CRT image does not have the VIC20 CARTRIDGE signature")
            }
            Self::UnsupportedHardware(v0) => {
                write!(f, "unsupported VIC-20 cartridge hardware type {v0}")
            }
            Self::InvalidChip(v0) => write!(
                f,
                "CRT CHIP packet at offset {v0} is truncated or malformed"
            ),
            Self::UnsupportedMapping { address, size } => write!(
                f,
                "unsupported VIC-20 cartridge mapping ${address:04X} + {size} bytes"
            ),
            Self::NoBlocks => write!(f, "VIC-20 CRT contains no ROM CHIP packets"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Cartridge encode failure.
#[derive(Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// Only generic, statically mapped cartridges can be written.
    UnsupportedHardware(u16),
    /// The name is longer than the 32-byte header field.
    NameTooLong(usize),
    /// A block maps outside BLK1/2/3/5, which [`parse`] would reject.
    UnsupportedMapping {
        /// Block load address.
        address: u16,
        /// Block size in bytes.
        size: usize,
    },
    /// The cartridge has no blocks.
    NoBlocks,
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedHardware(v0) => {
                write!(f, "unsupported VIC-20 cartridge hardware type {v0}")
            }
            Self::NameTooLong(v0) => write!(f, "VIC-20 CRT name is {v0} bytes; the field holds 32"),
            Self::UnsupportedMapping { address, size } => write!(
                f,
                "unsupported VIC-20 cartridge mapping ${address:04X} + {size} bytes"
            ),
            Self::NoBlocks => write!(f, "VIC-20 CRT needs at least one ROM block"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Encode a cartridge as a VICE `.crt` image.
///
/// Writes a 64-byte header (version `$0100`), then one ROM `CHIP` packet per
/// block. Raw BLK5 images carry no header at all; to write one, take the
/// block's `data` directly.
///
/// # Errors
///
/// Returns an error for non-generic hardware, a name over 32 bytes, a block
/// [`parse`] would reject, or an empty cartridge.
pub fn encode(cart: &Cartridge) -> Result<Vec<u8>, EncodeError> {
    if cart.hardware_type != 0 {
        return Err(EncodeError::UnsupportedHardware(cart.hardware_type));
    }
    let name = cart.name.as_bytes();
    if name.len() > 32 {
        return Err(EncodeError::NameTooLong(name.len()));
    }
    if cart.blocks.is_empty() {
        return Err(EncodeError::NoBlocks);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(HEADER_LEN as u32).to_be_bytes());
    out.extend_from_slice(&0x0100u16.to_be_bytes());
    out.extend_from_slice(&cart.hardware_type.to_be_bytes());
    out.resize(0x20, 0);
    out.extend_from_slice(name);
    out.resize(HEADER_LEN, 0);
    for block in &cart.blocks {
        let size = block.data.len();
        let unsupported = EncodeError::UnsupportedMapping {
            address: block.load_address,
            size,
        };
        if !supported_mapping(block.load_address, size) {
            return Err(unsupported);
        }
        let size16 = u16::try_from(size).map_err(|_| unsupported)?;
        out.extend_from_slice(CHIP_MAGIC);
        out.extend_from_slice(&(CHIP_HEADER_LEN as u32 + u32::from(size16)).to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&block.load_address.to_be_bytes());
        out.extend_from_slice(&size16.to_be_bytes());
        out.extend_from_slice(&block.data);
    }
    Ok(out)
}

fn be_u16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn supported_mapping(address: u16, size: usize) -> bool {
    let Some(end) = address.checked_add(u16::try_from(size).unwrap_or(u16::MAX)) else {
        return false;
    };
    matches!(address, 0x2000..=0x7FFF | 0xA000..=0xBFFF)
        && matches!(end.saturating_sub(1), 0x2000..=0x7FFF | 0xA000..=0xBFFF)
        && !(address < 0x8000 && end > 0x8000)
}

/// Parse a VICE CRT or an ordinary raw BLK5 ROM.
///
/// # Errors
///
/// Returns an error for malformed CRT data, bank-switching hardware, mappings
/// outside BLK1/2/3/5, or ambiguous raw image sizes.
pub fn parse(bytes: &[u8]) -> Result<Cartridge, ParseError> {
    if !bytes.starts_with(MAGIC) {
        return match bytes.len() {
            0x0800 | 0x1000 | 0x2000 => Ok(Cartridge {
                hardware_type: 0,
                name: "raw BLK5 cartridge".to_owned(),
                blocks: vec![CartridgeBlock {
                    load_address: 0xA000,
                    data: bytes.to_vec(),
                }],
            }),
            size if size >= MAGIC.len() && bytes[..MAGIC.len()].contains(&b' ') => {
                Err(ParseError::InvalidSignature)
            }
            size => Err(ParseError::UnsupportedRawSize(size)),
        };
    }
    if bytes.len() < HEADER_LEN {
        return Err(ParseError::InvalidHeader);
    }
    let header_len = be_u32(&bytes[0x10..0x14]) as usize;
    if header_len < HEADER_LEN || header_len > bytes.len() {
        return Err(ParseError::InvalidHeader);
    }
    let hardware_type = be_u16(&bytes[0x16..0x18]);
    if hardware_type != 0 {
        return Err(ParseError::UnsupportedHardware(hardware_type));
    }
    let name = String::from_utf8_lossy(&bytes[0x20..0x40])
        .trim_end_matches('\0')
        .trim_end()
        .to_owned();

    let mut blocks = Vec::new();
    let mut offset = header_len;
    while offset < bytes.len() {
        if offset + CHIP_HEADER_LEN > bytes.len() || &bytes[offset..offset + 4] != CHIP_MAGIC {
            return Err(ParseError::InvalidChip(offset));
        }
        let packet_len = be_u32(&bytes[offset + 4..offset + 8]) as usize;
        let chip_type = be_u16(&bytes[offset + 8..offset + 10]);
        let load_address = be_u16(&bytes[offset + 12..offset + 14]);
        let image_size = be_u16(&bytes[offset + 14..offset + 16]) as usize;
        let data_start = offset + CHIP_HEADER_LEN;
        let data_end = data_start.saturating_add(image_size);
        if packet_len < CHIP_HEADER_LEN + image_size
            || offset.saturating_add(packet_len) > bytes.len()
            || data_end > bytes.len()
        {
            return Err(ParseError::InvalidChip(offset));
        }
        if chip_type == 0 || chip_type == 2 {
            if !supported_mapping(load_address, image_size) {
                return Err(ParseError::UnsupportedMapping {
                    address: load_address,
                    size: image_size,
                });
            }
            blocks.push(CartridgeBlock {
                load_address,
                data: bytes[data_start..data_end].to_vec(),
            });
        }
        offset += packet_len;
    }
    if blocks.is_empty() {
        return Err(ParseError::NoBlocks);
    }
    Ok(Cartridge {
        hardware_type,
        name,
        blocks,
    })
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn crt(chips: &[(u16, &[u8])]) -> Vec<u8> {
        let mut bytes = Vec::from(*MAGIC);
        bytes.extend_from_slice(&0x40u32.to_be_bytes());
        bytes.extend_from_slice(&0x0100u16.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&[0; 32]);
        for (address, data) in chips {
            bytes.extend_from_slice(CHIP_MAGIC);
            bytes.extend_from_slice(&((CHIP_HEADER_LEN + data.len()) as u32).to_be_bytes());
            bytes.extend_from_slice(&0u16.to_be_bytes());
            bytes.extend_from_slice(&0u16.to_be_bytes());
            bytes.extend_from_slice(&address.to_be_bytes());
            bytes.extend_from_slice(&(data.len() as u16).to_be_bytes());
            bytes.extend_from_slice(data);
        }
        bytes
    }

    #[test]
    fn parses_raw_blk5_and_detects_autostart() {
        let mut raw = vec![0; 0x2000];
        raw[4..9].copy_from_slice(&[0x41, 0x30, 0xC3, 0xC2, 0xCD]);
        let cart = parse(&raw).expect("raw BLK5 cart");
        assert_eq!(cart.blocks[0].load_address, 0xA000);
        assert!(cart.is_autostart());
    }

    #[test]
    fn parses_addressed_multi_block_crt() {
        let bytes = crt(&[(0x2000, &[1; 0x2000]), (0xA000, &[2; 0x2000])]);
        let cart = parse(&bytes).expect("generic multi-block cart");
        assert_eq!(cart.blocks.len(), 2);
        assert_eq!(cart.blocks[0].load_address, 0x2000);
        assert_eq!(cart.blocks[1].load_address, 0xA000);
    }

    #[test]
    fn rejects_ambiguous_raw_and_out_of_range_crt() {
        assert_eq!(
            parse(&[0; 0x3000]),
            Err(ParseError::UnsupportedRawSize(0x3000))
        );
        assert!(matches!(
            parse(&crt(&[(0x8000, &[0; 0x2000])])),
            Err(ParseError::UnsupportedMapping { .. })
        ));
    }

    #[test]
    fn encode_round_trips_multi_block_carts() {
        let cart = Cartridge {
            hardware_type: 0,
            name: "TWO BLOCKS".to_owned(),
            blocks: vec![
                CartridgeBlock {
                    load_address: 0x2000,
                    data: vec![0x11; 0x2000],
                },
                CartridgeBlock {
                    load_address: 0xA000,
                    data: vec![0x22; 0x2000],
                },
            ],
        };
        assert_eq!(parse(&encode(&cart).expect("encodes")), Ok(cart));
    }

    #[test]
    fn encode_rejects_what_parse_cannot_read() {
        let mut cart = Cartridge {
            hardware_type: 1,
            name: String::new(),
            blocks: Vec::new(),
        };
        assert_eq!(encode(&cart), Err(EncodeError::UnsupportedHardware(1)));
        cart.hardware_type = 0;
        assert_eq!(encode(&cart), Err(EncodeError::NoBlocks));
        cart.name = "N".repeat(33);
        assert_eq!(encode(&cart), Err(EncodeError::NameTooLong(33)));
        cart.name.clear();
        cart.blocks.push(CartridgeBlock {
            load_address: 0x8000,
            data: vec![0; 0x1000],
        });
        assert_eq!(
            encode(&cart),
            Err(EncodeError::UnsupportedMapping {
                address: 0x8000,
                size: 0x1000
            })
        );
        for e in [EncodeError::NoBlocks, EncodeError::NameTooLong(1)] {
            assert!(!e.to_string().is_empty());
        }
        assert!(!ParseError::NoBlocks.to_string().is_empty());
    }
}
