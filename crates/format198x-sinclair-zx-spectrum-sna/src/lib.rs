//! ZX Spectrum `.sna` snapshots: read and write.
//!
//! The simplest Spectrum snapshot: a fixed layout with no compression.
//!
//! ```text
//! offset  size   field
//! 0       1      I
//! 1       8      HL' DE' BC' AF'   (little-endian words)
//! 9       10     HL DE BC IY IX
//! 19      1      interrupt flags; bit 2 is IFF2
//! 20      1      R
//! 21      4      AF SP
//! 25      1      interrupt mode
//! 26      1      border colour
//! 27      49152  RAM, $4000-$FFFF
//! ```
//!
//! A 48K snapshot stops there. It has no PC field: the program counter is
//! pushed onto the stack, and `SP` points at it, so a loader resumes with a
//! `RET`. [`Sna::pc`] reads it back and [`Sna::new_48k`] pushes it.
//!
//! A 128K snapshot continues with the PC, the `$7FFD` paging byte, a TR-DOS
//! flag and the RAM banks the first 48K did not hold. The first 48K is bank 5,
//! bank 2 and the bank paged at `$C000`. When that paged bank is 2 or 5, it
//! appears twice and six banks follow; otherwise five do.
//!
//! See <https://worldofspectrum.org/faq/reference/formats.htm>.

/// Header length before the RAM.
const HEADER_LEN: usize = 27;
/// One 16K RAM bank.
pub const BANK_LEN: usize = 0x4000;
/// The 48K RAM image, `$4000-$FFFF`.
pub const RAM_LEN: usize = 3 * BANK_LEN;
/// Length of a 48K snapshot.
pub const SNA_48K_LEN: usize = HEADER_LEN + RAM_LEN;
/// Length of the 128K extension's fixed fields.
const EXTENSION_LEN: usize = 4;
/// Where Spectrum RAM starts.
const RAM_BASE: u16 = 0x4000;

/// The register block, as stored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Registers {
    /// Interrupt vector register.
    pub i: u8,
    /// Refresh register.
    pub r: u8,
    /// Interrupt mode, 0 to 2.
    pub im: u8,
    /// IFF2. The format stores no IFF1; loaders copy IFF2 into it.
    pub iff2: bool,
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
    /// SP as stored. In a 48K snapshot it points at the pushed PC.
    pub sp: u16,
}

/// The fields a 128K snapshot adds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extension128 {
    /// Program counter.
    pub pc: u16,
    /// The `$7FFD` paging byte. Its low three bits name the bank at `$C000`.
    pub port_7ffd: u8,
    /// The TR-DOS ROM flag byte, kept as stored (`1` means paged in).
    pub trdos: u8,
    /// The banks the first 48K did not hold, in ascending bank order: every
    /// bank except 5, 2 and the paged one.
    pub other_banks: Vec<Vec<u8>>,
}

/// A `.sna` snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sna {
    /// The register block.
    pub registers: Registers,
    /// Border colour, 0 to 7.
    pub border: u8,
    /// RAM from `$4000` to `$FFFF`; for a 128K snapshot, banks 5, 2 and the
    /// paged bank.
    pub ram: Vec<u8>,
    /// The 128K fields, or `None` for a 48K snapshot.
    pub extension: Option<Extension128>,
}

/// Why a snapshot could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnaError {
    /// Shorter than a 48K snapshot.
    TooShort {
        /// Bytes present.
        len: usize,
    },
    /// Longer than 48K but missing the 128K extension's fields or banks.
    TruncatedExtension {
        /// Bytes present.
        len: usize,
        /// Bytes the paged bank implies.
        expected: usize,
    },
    /// On encode: the RAM is not exactly 48K.
    BadRamLength {
        /// Bytes supplied.
        len: usize,
    },
    /// On encode: a 128K snapshot has the wrong number of other banks, or a
    /// bank is not 16K.
    BadBanks,
    /// On [`Sna::new_48k`]: the stack has no room below `SP` in RAM for the
    /// pushed PC.
    StackOutsideRam {
        /// The stack pointer before the push.
        sp: u16,
    },
}

impl core::fmt::Display for SnaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShort { len } => {
                write!(
                    f,
                    ".SNA too short: {len} bytes (need at least {SNA_48K_LEN})"
                )
            }
            Self::TruncatedExtension { len, expected } => {
                write!(f, ".SNA 128K data too short: {len} bytes (need {expected})")
            }
            Self::BadRamLength { len } => {
                write!(f, ".SNA RAM image is {len} bytes; it must be {RAM_LEN}")
            }
            Self::BadBanks => write!(
                f,
                ".SNA 128K needs five other 16K banks, or six when bank 2 or 5 is paged"
            ),
            Self::StackOutsideRam { sp } => write!(
                f,
                "cannot push PC below SP=${sp:04X}: the stack must lie in RAM above $4001"
            ),
        }
    }
}

impl std::error::Error for SnaError {}

/// How many banks follow the 128K extension for a given paged bank.
fn other_bank_count(paged: u8) -> usize {
    if paged == 2 || paged == 5 { 6 } else { 5 }
}

fn read_u16(data: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes([data[pos], data[pos + 1]])
}

impl Sna {
    /// Parse a `.sna` file. Anything longer than 48K is read as 128K.
    ///
    /// # Errors
    ///
    /// Returns an error when the file is shorter than 48K, or longer but
    /// missing the 128K fields or banks.
    pub fn parse(data: &[u8]) -> Result<Self, SnaError> {
        if data.len() < SNA_48K_LEN {
            return Err(SnaError::TooShort { len: data.len() });
        }
        let registers = Registers {
            i: data[0],
            hl_alt: read_u16(data, 1),
            de_alt: read_u16(data, 3),
            bc_alt: read_u16(data, 5),
            af_alt: read_u16(data, 7),
            hl: read_u16(data, 9),
            de: read_u16(data, 11),
            bc: read_u16(data, 13),
            iy: read_u16(data, 15),
            ix: read_u16(data, 17),
            iff2: data[19] & 0x04 != 0,
            r: data[20],
            af: read_u16(data, 21),
            sp: read_u16(data, 23),
            im: data[25],
        };
        let border = data[26];
        let ram = data[HEADER_LEN..SNA_48K_LEN].to_vec();
        if data.len() == SNA_48K_LEN {
            return Ok(Self {
                registers,
                border,
                ram,
                extension: None,
            });
        }
        let ext_end = SNA_48K_LEN + EXTENSION_LEN;
        if data.len() < ext_end {
            return Err(SnaError::TruncatedExtension {
                len: data.len(),
                expected: ext_end,
            });
        }
        let port_7ffd = data[SNA_48K_LEN + 2];
        let count = other_bank_count(port_7ffd & 0x07);
        let expected = ext_end + count * BANK_LEN;
        if data.len() < expected {
            return Err(SnaError::TruncatedExtension {
                len: data.len(),
                expected,
            });
        }
        let other_banks = data[ext_end..expected]
            .chunks(BANK_LEN)
            .map(<[u8]>::to_vec)
            .collect();
        Ok(Self {
            registers,
            border,
            ram,
            extension: Some(Extension128 {
                pc: read_u16(data, SNA_48K_LEN),
                port_7ffd,
                trdos: data[SNA_48K_LEN + 3],
                other_banks,
            }),
        })
    }

    /// Build a 48K snapshot the way a `.sna` writer does: push `pc` onto the
    /// stack below `registers.sp`, so the stored `SP` points at it.
    ///
    /// # Errors
    ///
    /// Returns an error when `ram` is not 48K, or the two bytes below `SP`
    /// are not in RAM.
    pub fn new_48k(
        mut registers: Registers,
        pc: u16,
        border: u8,
        mut ram: Vec<u8>,
    ) -> Result<Self, SnaError> {
        if ram.len() != RAM_LEN {
            return Err(SnaError::BadRamLength { len: ram.len() });
        }
        let sp = registers.sp;
        let pushed = sp.wrapping_sub(2);
        if !(RAM_BASE..=0xFFFE).contains(&pushed) || sp < RAM_BASE + 2 {
            return Err(SnaError::StackOutsideRam { sp });
        }
        let at = usize::from(pushed - RAM_BASE);
        ram[at..at + 2].copy_from_slice(&pc.to_le_bytes());
        registers.sp = pushed;
        Ok(Self {
            registers,
            border,
            ram,
            extension: None,
        })
    }

    /// The program counter: the 128K field, or for a 48K snapshot the word
    /// `SP` points at. `None` when a 48K snapshot's `SP` does not point into
    /// RAM.
    #[must_use]
    pub fn pc(&self) -> Option<u16> {
        if let Some(ext) = &self.extension {
            return Some(ext.pc);
        }
        let sp = self.registers.sp;
        if !(RAM_BASE..=0xFFFE).contains(&sp) {
            return None;
        }
        let at = usize::from(sp - RAM_BASE);
        let bytes = self.ram.get(at..at + 2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    /// One 16K RAM bank of a 128K snapshot, 0 to 7. `None` for a 48K
    /// snapshot or a bank number above 7.
    #[must_use]
    pub fn bank(&self, bank: u8) -> Option<&[u8]> {
        let ext = self.extension.as_ref()?;
        let paged = ext.port_7ffd & 0x07;
        let block = |n: usize| self.ram.get(n * BANK_LEN..(n + 1) * BANK_LEN);
        match bank {
            5 => block(0),
            2 => block(1),
            b if b == paged => block(2),
            b if b < 8 => {
                let index = (0..b).filter(|&n| n != 2 && n != 5 && n != paged).count();
                ext.other_banks.get(index).map(Vec::as_slice)
            }
            _ => None,
        }
    }

    /// Encode the snapshot. Bits of the interrupt-flag byte other than IFF2
    /// are written as zero.
    ///
    /// # Errors
    ///
    /// Returns an error when the RAM is not 48K, or a 128K snapshot's banks
    /// do not match its paged bank.
    pub fn encode(&self) -> Result<Vec<u8>, SnaError> {
        if self.ram.len() != RAM_LEN {
            return Err(SnaError::BadRamLength {
                len: self.ram.len(),
            });
        }
        let r = &self.registers;
        let mut out = Vec::with_capacity(SNA_48K_LEN);
        out.push(r.i);
        for word in [
            r.hl_alt, r.de_alt, r.bc_alt, r.af_alt, r.hl, r.de, r.bc, r.iy, r.ix,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.push(if r.iff2 { 0x04 } else { 0x00 });
        out.push(r.r);
        out.extend_from_slice(&r.af.to_le_bytes());
        out.extend_from_slice(&r.sp.to_le_bytes());
        out.push(r.im);
        out.push(self.border);
        out.extend_from_slice(&self.ram);
        if let Some(ext) = &self.extension {
            let count = other_bank_count(ext.port_7ffd & 0x07);
            if ext.other_banks.len() != count || ext.other_banks.iter().any(|b| b.len() != BANK_LEN)
            {
                return Err(SnaError::BadBanks);
            }
            out.extend_from_slice(&ext.pc.to_le_bytes());
            out.push(ext.port_7ffd);
            out.push(ext.trdos);
            for bank in &ext.other_banks {
                out.extend_from_slice(bank);
            }
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

    fn registers() -> Registers {
        Registers {
            i: 0x12,
            r: 0x34,
            im: 2,
            iff2: true,
            af: 0x6677,
            bc: 0xFF11,
            de: 0xDDEE,
            hl: 0xBBCC,
            af_alt: 0x99AA,
            bc_alt: 0x7788,
            de_alt: 0x5566,
            hl_alt: 0x3344,
            ix: 0x4455,
            iy: 0x2233,
            sp: 0x8000,
        }
    }

    #[test]
    fn a_48k_snapshot_round_trips_its_register_file() {
        let sna = Sna::new_48k(registers(), 0x6000, 3, vec![0; RAM_LEN]).expect("builds");
        assert_eq!(sna.registers.sp, 0x7FFE, "PC pushed below SP");
        assert_eq!(sna.pc(), Some(0x6000));
        let bytes = sna.encode().expect("encodes");
        assert_eq!(bytes.len(), SNA_48K_LEN);
        assert_eq!(bytes[0], 0x12);
        assert_eq!(&bytes[7..9], &[0xAA, 0x99], "AF' is little-endian");
        assert_eq!(bytes[19], 0x04);
        assert_eq!(&bytes[23..25], &[0xFE, 0x7F]);
        assert_eq!(Sna::parse(&bytes), Ok(sna));
    }

    /// The bytes `pasmo --sna` writes for a program at `$8000` entered at
    /// `$8000`: IFF2, IM 1, white border, SP `$FFFC`, attributes `$38`.
    #[test]
    fn new_48k_produces_the_pasmo_layout() {
        let mut ram = vec![0u8; RAM_LEN];
        ram[0x1800..0x1B00].fill(0x38);
        ram[0x4000] = 0xC9;
        let regs = Registers {
            iff2: true,
            im: 1,
            sp: 0xFFFE,
            ..Registers::default()
        };
        let bytes = Sna::new_48k(regs, 0x8000, 7, ram)
            .expect("builds")
            .encode()
            .expect("encodes");
        let mut header = [0u8; HEADER_LEN];
        header[19] = 0x04;
        header[23] = 0xFC;
        header[24] = 0xFF;
        header[25] = 0x01;
        header[26] = 0x07;
        assert_eq!(&bytes[..HEADER_LEN], &header);
        assert_eq!(&bytes[HEADER_LEN + 0xBFFC..], &[0x00, 0x80, 0x00, 0x00]);
    }

    fn snapshot_128k(paged: u8) -> Sna {
        let count = other_bank_count(paged);
        Sna {
            registers: registers(),
            border: 1,
            ram: (0..RAM_LEN).map(|i| (i / BANK_LEN) as u8 + 0x10).collect(),
            extension: Some(Extension128 {
                pc: 0x1234,
                port_7ffd: 0x10 | paged,
                trdos: 0,
                other_banks: (0..count).map(|n| vec![n as u8 + 0x20; BANK_LEN]).collect(),
            }),
        }
    }

    #[test]
    fn a_128k_snapshot_round_trips_with_five_or_six_banks() {
        for paged in 0..8u8 {
            let sna = snapshot_128k(paged);
            let bytes = sna.encode().expect("encodes");
            assert_eq!(
                bytes.len(),
                SNA_48K_LEN + 4 + other_bank_count(paged) * BANK_LEN
            );
            assert_eq!(Sna::parse(&bytes), Ok(sna));
        }
    }

    #[test]
    fn banks_come_from_the_right_places() {
        let sna = snapshot_128k(0);
        assert_eq!(sna.bank(5).expect("bank 5")[0], 0x10);
        assert_eq!(sna.bank(2).expect("bank 2")[0], 0x11);
        assert_eq!(sna.bank(0).expect("paged bank")[0], 0x12);
        assert_eq!(sna.bank(1).expect("bank 1")[0], 0x20);
        assert_eq!(sna.bank(3).expect("bank 3")[0], 0x21);
        assert_eq!(sna.bank(7).expect("bank 7")[0], 0x24);
        assert_eq!(sna.bank(8), None);
        assert_eq!(sna.pc(), Some(0x1234));
        let six = snapshot_128k(5);
        assert_eq!(six.bank(0).expect("bank 0")[0], 0x20);
        assert_eq!(six.bank(7).expect("bank 7")[0], 0x25);
        let small = Sna::new_48k(registers(), 0, 0, vec![0; RAM_LEN]).expect("builds");
        assert_eq!(small.bank(0), None);
    }

    #[test]
    fn short_and_truncated_files_are_rejected() {
        assert_eq!(Sna::parse(&[0; 100]), Err(SnaError::TooShort { len: 100 }));
        let mut bytes = snapshot_128k(0).encode().expect("encodes");
        bytes.truncate(SNA_48K_LEN + 2);
        assert!(matches!(
            Sna::parse(&bytes),
            Err(SnaError::TruncatedExtension { .. })
        ));
        let mut bytes = snapshot_128k(2).encode().expect("encodes");
        bytes.truncate(bytes.len() - 1);
        assert_eq!(
            Sna::parse(&bytes),
            Err(SnaError::TruncatedExtension {
                len: bytes.len(),
                expected: bytes.len() + 1
            })
        );
    }

    #[test]
    fn a_48k_pc_outside_ram_is_none_not_a_panic() {
        let mut sna = Sna::new_48k(registers(), 0, 0, vec![0; RAM_LEN]).expect("builds");
        sna.registers.sp = 0x3FFF;
        assert_eq!(sna.pc(), None);
        sna.registers.sp = 0xFFFF;
        assert_eq!(sna.pc(), None);
    }

    #[test]
    fn construction_and_encode_reject_bad_shapes() {
        let mut regs = registers();
        assert_eq!(
            Sna::new_48k(regs, 0, 0, vec![0; 10]),
            Err(SnaError::BadRamLength { len: 10 })
        );
        regs.sp = 0x4001;
        assert_eq!(
            Sna::new_48k(regs, 0, 0, vec![0; RAM_LEN]),
            Err(SnaError::StackOutsideRam { sp: 0x4001 })
        );
        regs.sp = 0x0000;
        assert!(Sna::new_48k(regs, 0, 0, vec![0; RAM_LEN]).is_err());
        let mut sna = snapshot_128k(0);
        if let Some(ext) = sna.extension.as_mut() {
            ext.other_banks.pop();
        }
        assert_eq!(sna.encode(), Err(SnaError::BadBanks));
        sna.ram.pop();
        assert_eq!(
            sna.encode(),
            Err(SnaError::BadRamLength { len: RAM_LEN - 1 })
        );
        for e in [
            SnaError::TooShort { len: 1 },
            SnaError::TruncatedExtension {
                len: 1,
                expected: 2,
            },
            SnaError::BadRamLength { len: 1 },
            SnaError::BadBanks,
            SnaError::StackOutsideRam { sp: 1 },
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
