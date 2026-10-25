//! `.o` / `.80` tape images for the Sinclair ZX80: read and write.
//!
//! # The container is a memory image
//!
//! There is no header, no block structure and no checksum. A `.o` file is
//! the bytes the ROM's `SAVE` puts on tape, which is RAM from `$4000` up to
//! the address held in `$400A`:
//!
//! ```text
//! $01F8   INC HL
//!         EX DE,HL
//!         LD HL,($400A)
//!         SCF
//!         SBC HL,DE        ; ($400A) - pointer - 1
//!         EX DE,HL
//!         RET NC           ; keep going while the pointer is below it
//! ```
//!
//! `$400A` is inside the saved region, at offset `$0A`, so a well-formed
//! image describes its own length: the little-endian word there must equal
//! `$4000 + len`. That is the only integrity check the format admits, and
//! this parser applies it — a truncated or padded image fails here rather
//! than loading and misbehaving later.
//!
//! # Not here
//!
//! The pulse train that carries these bytes on cassette (counted pulses, 4
//! for a clear bit and 9 for a set one) is machine behaviour, like the timing
//! of any tape signal, and stays with the emulator that plays it.

/// Where RAM images start on a ZX80.
pub const RAM_BASE: u16 = 0x4000;
/// Offset of the end-of-program pointer within the image.
pub const END_POINTER_OFFSET: usize = 0x0A;

/// Smallest image the ROM can produce: the system variables alone.
const MIN_LEN: usize = END_POINTER_OFFSET + 2;
/// A ZX80 tops out at 16 KB of RAM.
const MAX_LEN: usize = 16 * 1024;

/// Why an image was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Too short to contain the pointer that describes it.
    TooShort {
        /// Length seen.
        len: usize,
    },
    /// Larger than the machine's address space for RAM.
    TooLong {
        /// Length seen.
        len: usize,
    },
    /// The self-describing length does not match the file.
    LengthMismatch {
        /// Pointer read from offset `$0A`.
        end: u16,
        /// Length that pointer implies.
        expected: usize,
        /// Actual file length.
        len: usize,
    },
}
impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { len } => write!(
                f,
                "image is {len} bytes; a ZX80 image is at least {MIN_LEN}"
            ),
            Self::TooLong { len } => write!(
                f,
                "image is {len} bytes; a ZX80 has at most {MAX_LEN} of RAM"
            ),
            Self::LengthMismatch { end, expected, len } => write!(
                f,
                "image says it ends at ${end:04X}, which is {expected} bytes, but the file is {len}"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// A parsed ZX80 tape image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zx80Image {
    bytes: Vec<u8>,
}

impl Zx80Image {
    /// Parses and validates a `.o` / `.80` image.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the image is too short, too long for a
    /// ZX80's RAM, or does not agree with the end pointer it carries.
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let len = data.len();
        if len < MIN_LEN {
            return Err(ParseError::TooShort { len });
        }
        if len > MAX_LEN {
            return Err(ParseError::TooLong { len });
        }
        let end = u16::from_le_bytes([data[END_POINTER_OFFSET], data[END_POINTER_OFFSET + 1]]);
        let expected = usize::from(end).saturating_sub(usize::from(RAM_BASE));
        if expected != len {
            return Err(ParseError::LengthMismatch { end, expected, len });
        }
        Ok(Self {
            bytes: data.to_vec(),
        })
    }

    /// The image's bytes, as they sit in RAM from `$4000`.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Where the program ends, per the pointer the image carries.
    #[must_use]
    pub fn end_address(&self) -> u16 {
        RAM_BASE.wrapping_add(u16::try_from(self.bytes.len()).unwrap_or(u16::MAX))
    }

    /// Builds the image `SAVE` would write from a RAM dump starting at
    /// `$4000`: the bytes up to the end pointer at offset `$0A`.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the dump is too short to hold the pointer,
    /// or the pointer names a length the dump does not cover or a ZX80
    /// cannot hold.
    pub fn from_ram(ram: &[u8]) -> Result<Self, ParseError> {
        if ram.len() < MIN_LEN {
            return Err(ParseError::TooShort { len: ram.len() });
        }
        let end = u16::from_le_bytes([ram[END_POINTER_OFFSET], ram[END_POINTER_OFFSET + 1]]);
        let expected = usize::from(end).saturating_sub(usize::from(RAM_BASE));
        Self::parse(ram.get(..expected).unwrap_or(ram))
    }

    /// The file bytes: the image exactly as it sits in RAM.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn image(program: &[u8]) -> Vec<u8> {
        let len = MIN_LEN.max(program.len());
        let mut bytes = vec![0u8; len];
        bytes[..program.len()].copy_from_slice(program);
        let end = RAM_BASE + u16::try_from(len).expect("test image fits");
        bytes[END_POINTER_OFFSET..END_POINTER_OFFSET + 2].copy_from_slice(&end.to_le_bytes());
        bytes
    }

    #[test]
    fn accepts_an_image_that_agrees_with_its_own_end_pointer() {
        let raw = image(&[0u8; 64]);
        let parsed = Zx80Image::parse(&raw).expect("valid image");
        assert_eq!(parsed.bytes().len(), 64);
        assert_eq!(parsed.end_address(), RAM_BASE + 64);
    }

    /// The only integrity check the format admits, so it has to be applied.
    /// A truncated image otherwise loads and then misbehaves somewhere else.
    #[test]
    fn rejects_an_image_whose_pointer_disagrees_with_its_length() {
        let mut raw = image(&[0u8; 64]);
        raw.truncate(60);
        let err = Zx80Image::parse(&raw).expect_err("truncated image must be rejected");
        assert_eq!(
            err,
            ParseError::LengthMismatch {
                end: RAM_BASE + 64,
                expected: 64,
                len: 60,
            }
        );
    }

    #[test]
    fn rejects_images_that_cannot_be_zx80_memory() {
        assert_eq!(
            Zx80Image::parse(&[0u8; 4]),
            Err(ParseError::TooShort { len: 4 })
        );
        assert!(matches!(
            Zx80Image::parse(&vec![0u8; MAX_LEN + 1]),
            Err(ParseError::TooLong { .. })
        ));
    }

    #[test]
    fn encode_round_trips() {
        let raw = image(&[7u8; 40]);
        let parsed = Zx80Image::parse(&raw).expect("valid image");
        assert_eq!(parsed.encode(), raw);
    }

    #[test]
    fn from_ram_takes_the_bytes_up_to_the_end_pointer() {
        let mut ram = image(&[1u8; 40]);
        ram.extend_from_slice(&[0xEE; 100]);
        let parsed = Zx80Image::from_ram(&ram).expect("valid dump");
        assert_eq!(parsed.bytes().len(), 40);
        assert_eq!(
            Zx80Image::from_ram(&[0u8; 3]),
            Err(ParseError::TooShort { len: 3 })
        );
        let short = image(&[1u8; 40]);
        assert!(matches!(
            Zx80Image::from_ram(&short[..20]),
            Err(ParseError::LengthMismatch { .. })
        ));
        for e in [
            ParseError::TooShort { len: 1 },
            ParseError::TooLong { len: 1 },
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
