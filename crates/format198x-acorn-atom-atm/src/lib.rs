//! Reader and writer for the Acorn Atom `.atm` program image (the Wouter Ras format used by
//! Atomulator and others).
//!
//! An `.atm` file is a direct memory image: a 22-byte header followed by the
//! program body, loaded into RAM at the header's load address.
//!
//! ```text
//! offset  size  field
//! 0       16    filename (ASCII, zero/space padded)
//! 16      2     load address  (little-endian)
//! 18      2     execution address (little-endian)
//! 20      2     length of the body (little-endian)
//! 22      N     body (N = length bytes)
//! ```

/// Fixed `.atm` header size: 16-byte name + three little-endian `u16`s.
const HEADER_LEN: usize = 22;
/// Filename field width.
const NAME_LEN: usize = 16;

/// A parsed `.atm` program image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtmImage {
    /// The stored filename (trailing NUL/space padding trimmed).
    pub filename: String,
    /// Address the body loads to.
    pub load_address: u16,
    /// Address to begin execution at (auto-run / `LINK` target).
    pub exec_address: u16,
    /// The program body.
    pub payload: Vec<u8>,
}

impl AtmImage {
    /// One past the last byte the body occupies in memory.
    #[must_use]
    pub fn end_address(&self) -> usize {
        self.load_address as usize + self.payload.len()
    }
}

/// Errors from [`encode`].
#[derive(Debug, PartialEq, Eq)]
pub enum AtmEncodeError {
    /// The filename does not fit the 16-byte field, or is not printable ASCII.
    InvalidName,
    /// The body is longer than the 16-bit length field can describe.
    BodyTooLong {
        /// The body's byte count.
        actual: usize,
    },
}

impl core::fmt::Display for AtmEncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName => write!(
                f,
                "`.atm` filename must be at most {NAME_LEN} printable ASCII bytes"
            ),
            Self::BodyTooLong { actual } => write!(
                f,
                "`.atm` body is {actual} bytes; the length field holds at most 65535"
            ),
        }
    }
}

impl std::error::Error for AtmEncodeError {}

/// Errors from [`parse`].
#[derive(Debug, PartialEq, Eq)]
pub enum AtmError {
    /// The image is smaller than the 22-byte header.
    TooShort {
        /// The actual byte count.
        actual: usize,
    },
    /// The declared body length does not match the bytes present.
    LengthMismatch {
        /// The length field's value.
        declared: usize,
        /// Bytes actually present after the header.
        actual: usize,
    },
}
impl core::fmt::Display for AtmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { actual } => write!(
                f,
                "`.atm` image is {actual} bytes; need at least {HEADER_LEN} for the header"
            ),
            Self::LengthMismatch { declared, actual } => write!(
                f,
                "`.atm` declares a {declared}-byte body but {actual} bytes follow the header"
            ),
        }
    }
}

impl std::error::Error for AtmError {}

/// Parse an `.atm` program image.
///
/// # Errors
///
/// Returns [`AtmError::TooShort`] if the input is smaller than the header, or
/// [`AtmError::LengthMismatch`] if the declared body length exceeds the bytes
/// available.
pub fn parse(bytes: &[u8]) -> Result<AtmImage, AtmError> {
    if bytes.len() < HEADER_LEN {
        return Err(AtmError::TooShort {
            actual: bytes.len(),
        });
    }
    let filename = bytes[0..NAME_LEN]
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect::<String>()
        .trim_end()
        .to_owned();
    let load_address = u16::from_le_bytes([bytes[16], bytes[17]]);
    let exec_address = u16::from_le_bytes([bytes[18], bytes[19]]);
    let declared = usize::from(u16::from_le_bytes([bytes[20], bytes[21]]));
    let body = &bytes[HEADER_LEN..];
    if declared > body.len() {
        return Err(AtmError::LengthMismatch {
            declared,
            actual: body.len(),
        });
    }
    Ok(AtmImage {
        filename,
        load_address,
        exec_address,
        payload: body[..declared].to_vec(),
    })
}

/// Encode an `.atm` program image.
///
/// The filename is written as ASCII and padded with NUL bytes, so
/// `parse(&encode(image)?)` returns the same image when the name has no
/// trailing spaces.
///
/// # Errors
///
/// Returns [`AtmEncodeError::InvalidName`] when the name is longer than 16
/// bytes or holds a byte outside printable ASCII, and
/// [`AtmEncodeError::BodyTooLong`] when the body exceeds 65535 bytes.
pub fn encode(image: &AtmImage) -> Result<Vec<u8>, AtmEncodeError> {
    let name = image.filename.as_bytes();
    if name.len() > NAME_LEN || !name.iter().all(|b| (0x20..0x7F).contains(b)) {
        return Err(AtmEncodeError::InvalidName);
    }
    let length = u16::try_from(image.payload.len()).map_err(|_| AtmEncodeError::BodyTooLong {
        actual: image.payload.len(),
    })?;
    let mut out = Vec::with_capacity(HEADER_LEN + image.payload.len());
    out.extend_from_slice(name);
    out.resize(NAME_LEN, 0);
    out.extend_from_slice(&image.load_address.to_le_bytes());
    out.extend_from_slice(&image.exec_address.to_le_bytes());
    out.extend_from_slice(&length.to_le_bytes());
    out.extend_from_slice(&image.payload);
    Ok(out)
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn image(name: &[u8], load: u16, exec: u16, body: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; NAME_LEN];
        v[..name.len()].copy_from_slice(name);
        v.extend_from_slice(&load.to_le_bytes());
        v.extend_from_slice(&exec.to_le_bytes());
        v.extend_from_slice(&(body.len() as u16).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_a_valid_image() {
        let raw = image(b"DEFENDER", 0x2900, 0x2900, &[0xA9, 0x42, 0x60]);
        let atm = parse(&raw).expect("valid .atm");
        assert_eq!(atm.filename, "DEFENDER");
        assert_eq!(atm.load_address, 0x2900);
        assert_eq!(atm.exec_address, 0x2900);
        assert_eq!(atm.payload, vec![0xA9, 0x42, 0x60]);
        assert_eq!(atm.end_address(), 0x2903);
    }

    #[test]
    fn rejects_a_short_image() {
        assert_eq!(parse(&[0u8; 10]), Err(AtmError::TooShort { actual: 10 }));
    }

    #[test]
    fn rejects_a_truncated_body() {
        let mut raw = image(b"X", 0x0200, 0x0200, &[1, 2, 3]);
        raw.truncate(HEADER_LEN + 1); // declares 3 body bytes, only 1 present
        assert_eq!(
            parse(&raw),
            Err(AtmError::LengthMismatch {
                declared: 3,
                actual: 1,
            })
        );
    }

    #[test]
    fn trims_name_padding_but_ignores_trailing_garbage_body() {
        // A body longer than `length` is allowed; only `length` bytes are taken.
        let mut raw = image(b"HI", 0x70, 0x70, &[0xEE]);
        raw.push(0xFF); // extra byte beyond the declared length
        let atm = parse(&raw).expect("valid");
        assert_eq!(atm.filename, "HI");
        assert_eq!(atm.payload, vec![0xEE]);
    }

    #[test]
    fn encode_round_trips() {
        let atm = AtmImage {
            filename: "DEFENDER".to_owned(),
            load_address: 0x2900,
            exec_address: 0x2A00,
            payload: vec![0xA9, 0x42, 0x60],
        };
        let bytes = encode(&atm).expect("encodes");
        assert_eq!(
            bytes,
            image(b"DEFENDER", 0x2900, 0x2A00, &[0xA9, 0x42, 0x60])
        );
        assert_eq!(parse(&bytes), Ok(atm));
    }

    #[test]
    fn encode_rejects_bad_names_and_long_bodies() {
        let mut atm = AtmImage {
            filename: "A NAME THAT IS TOO LONG".to_owned(),
            load_address: 0,
            exec_address: 0,
            payload: Vec::new(),
        };
        assert_eq!(encode(&atm), Err(AtmEncodeError::InvalidName));
        atm.filename = "caf\u{e9}".to_owned();
        assert_eq!(encode(&atm), Err(AtmEncodeError::InvalidName));
        atm.filename = "OK".to_owned();
        atm.payload = vec![0; 0x1_0000];
        assert_eq!(
            encode(&atm),
            Err(AtmEncodeError::BodyTooLong { actual: 0x1_0000 })
        );
        assert!(!AtmEncodeError::InvalidName.to_string().is_empty());
        assert!(AtmError::TooShort { actual: 1 }.to_string().contains('1'));
    }
}
