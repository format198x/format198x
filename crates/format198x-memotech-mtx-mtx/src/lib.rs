//! Memotech MTX `.mtx` tape images and `.RUN` programs: read and write.
//!
//! Layouts and provenance:
//! `reference/by-system/memotech-mtx/memotech-mtx-tape-and-run-formats.md`.
//!
//! A `.mtx` file is a tape image — the bytes an MTX cassette carries, in order:
//! an `$FF` byte, a 15-byte space-padded name, a little-endian
//! `system_variables_base`, then `$FB4B - base` bytes of system variables and
//! the BASIC program loading from `$4000`. A common variant drops the `$FF`
//! byte and the name, starting straight at the base word.
//!
//! The extension is not the format. A quarter of the files carrying `.mtx` in
//! the TOSEC set are something else, so these parsers validate rather than
//! trust — see the reference document §3.

/// Address the BASIC program loads at.
pub const BASIC_BASE: u16 = 0x4000;
/// The system-variable block ends here, inclusive, so its length falls out of
/// the base rather than being stored.
pub const SYSTEM_VARIABLES_END: u16 = 0xFB4A;
/// Leading byte of a full tape image.
const TAPE_MARKER: u8 = 0xFF;
/// Length of the space-padded tape name.
const NAME_LEN: usize = 15;
/// Longest system-variable block a well-formed image can name.
const MAX_SYSTEM_VARIABLES: usize = 0x1000;

/// Why a `.mtx` image or `.RUN` program could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MtxError {
    /// The full form is too short to hold its marker, name and base word.
    TapeHeaderTooShort,
    /// The image is too short to hold the system-variables base word.
    NoSystemVariablesBase,
    /// The base does not describe a plausible system-variable block.
    ImplausibleBase {
        /// The rejected base.
        base: u16,
    },
    /// The image ends before the system-variable block does.
    TruncatedSystemVariables {
        /// The base word read from the image.
        base: u16,
        /// Bytes the block needs.
        length: usize,
        /// Bytes actually left.
        remaining: usize,
    },
    /// The `.RUN` file is shorter than its 4-byte header.
    RunHeaderTooShort,
    /// The `.RUN` file is shorter than its declared code length.
    TruncatedRunCode {
        /// The declared code length.
        declared: usize,
        /// Bytes actually present after the header.
        actual: usize,
    },
    /// On encode: the name is longer than 15 bytes or not printable ASCII.
    InvalidName,
    /// On encode: the system-variable block's length does not match its base.
    SystemVariablesLengthMismatch {
        /// Bytes the base implies.
        expected: usize,
        /// Bytes supplied.
        actual: usize,
    },
    /// On encode: the `.RUN` code is longer than the 16-bit length field.
    RunCodeTooLong {
        /// The code's byte count.
        actual: usize,
    },
}

impl core::fmt::Display for MtxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TapeHeaderTooShort => write!(f, "tape image too short to hold its header"),
            Self::NoSystemVariablesBase => {
                write!(f, "tape image too short to hold a system-variables base")
            }
            Self::ImplausibleBase { base } => write!(
                f,
                "system-variables base ${base:04X} is not a plausible one; a .mtx image bases \
                 its block near $F8F2, and a quarter of files carrying this extension are a \
                 different format"
            ),
            Self::TruncatedSystemVariables {
                base,
                length,
                remaining,
            } => write!(
                f,
                "system-variables base ${base:04X} wants {length} bytes but only {remaining} remain"
            ),
            Self::RunHeaderTooShort => write!(f, "RUN file too short to hold its header"),
            Self::TruncatedRunCode { declared, actual } => write!(
                f,
                "RUN file declares {declared} bytes of code but carries {actual}"
            ),
            Self::InvalidName => write!(f, "tape name must be at most 15 printable ASCII bytes"),
            Self::SystemVariablesLengthMismatch { expected, actual } => write!(
                f,
                "system-variable block is {actual} bytes but its base implies {expected}"
            ),
            Self::RunCodeTooLong { actual } => write!(
                f,
                "RUN code is {actual} bytes; the length field holds 65535"
            ),
        }
    }
}

impl std::error::Error for MtxError {}

/// Length of the system-variable block a base implies, if plausible.
fn system_variables_len(base: u16) -> Option<usize> {
    let length = usize::from(SYSTEM_VARIABLES_END.wrapping_sub(base)).wrapping_add(1);
    (base <= SYSTEM_VARIABLES_END && length != 0 && length <= MAX_SYSTEM_VARIABLES)
        .then_some(length)
}

/// A parsed `.mtx` tape image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapeImage {
    /// Name a game's own loader matches against; `None` for the headerless
    /// variant, which does not carry one.
    pub name: Option<String>,
    /// Address the system-variable block restores to.
    pub system_variables_base: u16,
    /// The system-variable block.
    pub system_variables: Vec<u8>,
    /// The BASIC program, loading from [`BASIC_BASE`].
    pub basic: Vec<u8>,
}

impl TapeImage {
    /// Parse a `.mtx` image, accepting both the full and headerless forms.
    ///
    /// # Errors
    ///
    /// Returns an error when the file is too short, or when
    /// `system_variables_base` does not describe a block that fits — which is
    /// how a mislabelled file is caught, since the extension does not prove
    /// the format.
    pub fn parse(bytes: &[u8]) -> Result<Self, MtxError> {
        let (name, body) = if bytes.first() == Some(&TAPE_MARKER) {
            if bytes.len() < 1 + NAME_LEN + 2 {
                return Err(MtxError::TapeHeaderTooShort);
            }
            let raw = &bytes[1..=NAME_LEN];
            let name = String::from_utf8_lossy(raw).trim_end().to_owned();
            (Some(name), &bytes[1 + NAME_LEN..])
        } else {
            (None, bytes)
        };

        if body.len() < 2 {
            return Err(MtxError::NoSystemVariablesBase);
        }
        let base = u16::from_le_bytes([body[0], body[1]]);
        let length = system_variables_len(base).ok_or(MtxError::ImplausibleBase { base })?;
        let rest = &body[2..];
        if rest.len() < length {
            return Err(MtxError::TruncatedSystemVariables {
                base,
                length,
                remaining: rest.len(),
            });
        }
        let (system_variables, basic) = rest.split_at(length);
        Ok(Self {
            name,
            system_variables_base: base,
            system_variables: system_variables.to_vec(),
            basic: basic.to_vec(),
        })
    }

    /// Encode the image: the full form when it has a name, the headerless
    /// form when it does not. The name is padded with spaces to 15 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the name is too long or not printable ASCII,
    /// the base is implausible, or the system-variable block's length does
    /// not match the base.
    pub fn encode(&self) -> Result<Vec<u8>, MtxError> {
        let base = self.system_variables_base;
        let expected = system_variables_len(base).ok_or(MtxError::ImplausibleBase { base })?;
        if self.system_variables.len() != expected {
            return Err(MtxError::SystemVariablesLengthMismatch {
                expected,
                actual: self.system_variables.len(),
            });
        }
        let mut out = Vec::new();
        if let Some(name) = &self.name {
            let raw = name.as_bytes();
            if raw.len() > NAME_LEN || !raw.iter().all(|b| (0x20..0x7F).contains(b)) {
                return Err(MtxError::InvalidName);
            }
            out.push(TAPE_MARKER);
            out.extend_from_slice(raw);
            out.resize(1 + NAME_LEN, b' ');
        }
        out.extend_from_slice(&base.to_le_bytes());
        out.extend_from_slice(&self.system_variables);
        out.extend_from_slice(&self.basic);
        Ok(out)
    }
}

/// A parsed `.RUN` program.
///
/// Documented by MEMU's author but not checked against a real file — the TOSEC
/// set carries none. See the reference document §2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunProgram {
    /// Address the code loads at, and is entered by jumping to.
    pub code_base: u16,
    /// The machine code.
    pub code: Vec<u8>,
}

impl RunProgram {
    /// Parse a `.RUN` program.
    ///
    /// # Errors
    ///
    /// Returns an error when the header is incomplete or the declared length
    /// runs past the end of the file.
    pub fn parse(bytes: &[u8]) -> Result<Self, MtxError> {
        if bytes.len() < 4 {
            return Err(MtxError::RunHeaderTooShort);
        }
        let code_base = u16::from_le_bytes([bytes[0], bytes[1]]);
        let code_length = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
        let code = bytes
            .get(4..4 + code_length)
            .ok_or(MtxError::TruncatedRunCode {
                declared: code_length,
                actual: bytes.len() - 4,
            })?
            .to_vec();
        Ok(Self { code_base, code })
    }

    /// Encode the program: base, length, then the code.
    ///
    /// # Errors
    ///
    /// Returns [`MtxError::RunCodeTooLong`] when the code exceeds 65535 bytes.
    pub fn encode(&self) -> Result<Vec<u8>, MtxError> {
        let len = u16::try_from(self.code.len()).map_err(|_| MtxError::RunCodeTooLong {
            actual: self.code.len(),
        })?;
        let mut out = Vec::with_capacity(4 + self.code.len());
        out.extend_from_slice(&self.code_base.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&self.code);
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

    const USUAL_BASE: u16 = 0xF8F2;

    fn tape(name: Option<&str>, base: u16, basic: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(name) = name {
            out.push(TAPE_MARKER);
            let mut padded = format!("{name:<15}").into_bytes();
            padded.truncate(NAME_LEN);
            out.extend_from_slice(&padded);
        }
        out.extend_from_slice(&base.to_le_bytes());
        let length = usize::from(SYSTEM_VARIABLES_END - base) + 1;
        out.extend(std::iter::repeat_n(0xAAu8, length));
        out.extend_from_slice(basic);
        out
    }

    #[test]
    fn a_full_tape_image_splits_into_its_three_parts() {
        let image = tape(Some("TACHYON"), USUAL_BASE, &[0x01, 0x02, 0x03]);
        let parsed = TapeImage::parse(&image).expect("well-formed");
        assert_eq!(parsed.name.as_deref(), Some("TACHYON"));
        assert_eq!(parsed.system_variables_base, USUAL_BASE);
        assert_eq!(parsed.system_variables.len(), 0x0259, "601 bytes at $F8F2");
        assert_eq!(parsed.basic, [0x01, 0x02, 0x03]);
    }

    #[test]
    fn the_headerless_variant_parses_without_a_name() {
        let image = tape(None, USUAL_BASE, &[0x09]);
        let parsed = TapeImage::parse(&image).expect("headerless variant");
        assert_eq!(parsed.name, None);
        assert_eq!(parsed.system_variables_base, USUAL_BASE);
        assert_eq!(parsed.basic, [0x09]);
    }

    #[test]
    fn a_mislabelled_file_is_rejected_rather_than_half_loaded() {
        // $3E8A is one of the bases seen on files that carry the extension but
        // are a different format; it wants a 49 KB block.
        let mut image = tape(Some("BOGUS"), USUAL_BASE, &[0x00]);
        image[16..18].copy_from_slice(&0x3E8Au16.to_le_bytes());
        let error = TapeImage::parse(&image)
            .expect_err("implausible base")
            .to_string();
        assert!(error.contains("$3E8A"), "{error}");
    }

    #[test]
    fn a_truncated_system_variable_block_is_rejected() {
        let mut image = tape(Some("SHORT"), USUAL_BASE, &[]);
        image.truncate(image.len() - 10);
        let error = TapeImage::parse(&image).expect_err("truncated").to_string();
        assert!(error.contains("only"), "{error}");
    }

    #[test]
    fn a_run_program_carries_its_own_load_address() {
        let mut file = 0x8000u16.to_le_bytes().to_vec();
        file.extend_from_slice(&3u16.to_le_bytes());
        file.extend_from_slice(&[0xC3, 0x00, 0x80]);
        let parsed = RunProgram::parse(&file).expect("well-formed");
        assert_eq!(parsed.code_base, 0x8000);
        assert_eq!(parsed.code, [0xC3, 0x00, 0x80]);
    }

    #[test]
    fn a_run_program_shorter_than_it_claims_is_rejected() {
        let mut file = 0x8000u16.to_le_bytes().to_vec();
        file.extend_from_slice(&99u16.to_le_bytes());
        file.extend_from_slice(&[0x00, 0x01]);
        let error = RunProgram::parse(&file).expect_err("short").to_string();
        assert!(error.contains("declares 99 bytes"), "{error}");
    }

    #[test]
    fn both_tape_forms_round_trip() {
        for name in [Some("TACHYON"), None] {
            let bytes = tape(name, USUAL_BASE, &[0x01, 0x02, 0x03]);
            let image = TapeImage::parse(&bytes).expect("parses");
            assert_eq!(image.encode().expect("encodes"), bytes);
        }
    }

    #[test]
    fn tape_encode_rejects_inconsistent_images() {
        let mut image = TapeImage::parse(&tape(Some("OK"), USUAL_BASE, &[])).expect("parses");
        image.name = Some("A NAME TOO LONG!".to_owned());
        assert_eq!(image.encode(), Err(MtxError::InvalidName));
        image.name = None;
        image.system_variables.pop();
        assert!(matches!(
            image.encode(),
            Err(MtxError::SystemVariablesLengthMismatch { .. })
        ));
        image.system_variables_base = 0x3E8A;
        assert_eq!(
            image.encode(),
            Err(MtxError::ImplausibleBase { base: 0x3E8A })
        );
    }

    #[test]
    fn run_programs_round_trip_and_reject_short_headers() {
        let run = RunProgram {
            code_base: 0x8000,
            code: vec![0xC3, 0x00, 0x80],
        };
        assert_eq!(RunProgram::parse(&run.encode().expect("encodes")), Ok(run));
        assert_eq!(RunProgram::parse(&[0, 0]), Err(MtxError::RunHeaderTooShort));
        let long = RunProgram {
            code_base: 0,
            code: vec![0; 0x1_0000],
        };
        assert_eq!(
            long.encode(),
            Err(MtxError::RunCodeTooLong { actual: 0x1_0000 })
        );
        for e in [
            MtxError::TapeHeaderTooShort,
            MtxError::NoSystemVariablesBase,
            MtxError::RunHeaderTooShort,
            MtxError::InvalidName,
            MtxError::RunCodeTooLong { actual: 1 },
            MtxError::SystemVariablesLengthMismatch {
                expected: 1,
                actual: 2,
            },
        ] {
            assert!(!e.to_string().is_empty());
        }
        assert_eq!(TapeImage::parse(&[0xFF]), Err(MtxError::TapeHeaderTooShort));
        assert_eq!(
            TapeImage::parse(&[0x00]),
            Err(MtxError::NoSystemVariablesBase)
        );
    }
}
