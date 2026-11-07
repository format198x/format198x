#![doc = include_str!("../README.md")]

//! Neo Geo cartridge ROM sets, as MAME names and lays them out.
//!
//! A Neo Geo cartridge is not one image but a set of chips, each dumped to
//! its own file: P-ROMs for the 68000, an S-ROM of fix-layer tiles, an
//! M-ROM for the Z80, V-ROMs of ADPCM samples and C-ROMs of sprite tiles.
//! This crate takes those files, by name, and builds the one region per bus
//! a machine reads.
//!
//! A file's role and order come from its extension, or failing that the end
//! of its stem: `201-p1.p1` and `201-p1.bin` are both P-ROM 1. The layout
//! follows MAME's `neogeo.cpp` (BSD-3-Clause):
//!
//! - P-ROMs are stored with each 16-bit word byte-swapped; they are swapped
//!   back to the 68000's order and joined, P1 first. A single 2 MiB P1
//!   with no lower-half recognition code and a complete upper-half header
//!   has its two 1 MiB halves exchanged before mapping.
//! - V-ROMs with one digit (`v1`, `v2`) feed ADPCM-A, which ADPCM-B shares.
//!   Two digits (`v11`, `v21`) name the region first: `v1x` for ADPCM-A,
//!   `v2x` for ADPCM-B, as on the first boards that kept them apart.
//! - C-ROMs pair up, C1 with C2, C3 with C4, and each pair interleaves a
//!   byte at a time, the odd ROM in even bytes.
//!
//! [`to_files`] writes canonical named files from the regions. Original chip
//! names and split boundaries are not retained. ZIP handling belongs to callers.
//!
//! Files that fit no role, a BIOS or a readme in the same archive, are left
//! out. Other bank wiring and encrypted sets are not inferred here.

use std::fmt;

/// The ROMs of one cartridge, in the layout each bus reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RomSet {
    /// P-ROM, big-endian.
    pub p: Vec<u8>,
    /// S-ROM.
    pub s: Vec<u8>,
    /// M-ROM.
    pub m: Vec<u8>,
    /// V-ROM for ADPCM-A (and ADPCM-B when `v_b` is empty).
    pub v_a: Vec<u8>,
    /// V-ROM for ADPCM-B, when the board keeps it apart.
    pub v_b: Vec<u8>,
    /// C-ROM pairs, each interleaved.
    pub c: Vec<u8>,
}

/// Why a ROM set could not be built.
#[derive(Debug, PartialEq, Eq)]
pub enum RomSetError {
    /// No P-ROM: there is nothing for the 68000 to run.
    NoProgramRom,
    /// Two files claim the same role and number.
    Duplicate {
        role: char,
        index: u8,
        first: String,
        second: String,
    },
    /// A C-ROM is missing its partner, or the two differ in size.
    UnpairedSpriteRom { index: u8 },
    /// The interleaved sprite region cannot be split into equal byte lanes.
    OddSpriteRegion,
}

impl fmt::Display for RomSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgramRom => f.write_str("no P-ROM (a file named like p1)"),
            Self::Duplicate {
                role,
                index,
                first,
                second,
            } => {
                write!(f, "two files are {role}{index}: {first} and {second}")
            }
            Self::UnpairedSpriteRom { index } => {
                write!(f, "C-ROM {index} has no partner of the same size")
            }
            Self::OddSpriteRegion => f.write_str("C-ROM region has an odd byte count"),
        }
    }
}

impl std::error::Error for RomSetError {}

/// What part a file plays: its role letter and its number.
fn classify(name: &str) -> Option<(char, u8)> {
    let base = name.rsplit(['/', '\\']).next()?.to_ascii_lowercase();
    let (stem, extension) = base.rsplit_once('.').unwrap_or((&base, ""));
    tag(extension).or_else(|| {
        let tail = stem.rsplit(['-', '_']).next()?;
        tag(tail)
    })
}

/// `p1`, `c12`, `v21` and the like.
fn tag(text: &str) -> Option<(char, u8)> {
    let mut chars = text.chars();
    let role = chars.next()?;
    if !matches!(role, 'p' | 's' | 'm' | 'v' | 'c') {
        return None;
    }
    let digits = chars.as_str();
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let index: u8 = digits.parse().ok()?;
    (index > 0).then_some((role, index))
}

/// Build a ROM set from `(file name, contents)` pairs.
///
/// # Errors
///
/// Returns an error if there is no P-ROM, two files claim one role, or a
/// C-ROM has no correctly numbered partner of its size. Unrecognised files
/// are ignored; only the first S-ROM and M-ROM in numeric order are retained.
pub fn from_files(
    files: impl IntoIterator<Item = (String, Vec<u8>)>,
) -> Result<RomSet, RomSetError> {
    let mut roles: Vec<(char, u8, String, Vec<u8>)> = Vec::new();
    for (name, bytes) in files {
        let Some((role, index)) = classify(&name) else {
            continue;
        };
        if let Some((_, _, first, _)) = roles.iter().find(|r| r.0 == role && r.1 == index) {
            return Err(RomSetError::Duplicate {
                role,
                index,
                first: first.clone(),
                second: name,
            });
        }
        roles.push((role, index, name, bytes));
    }
    roles.sort_by_key(|r| (r.0, r.1));
    let of = |role: char| roles.iter().filter(move |r| r.0 == role);

    let mut set = RomSet::default();
    let program_parts = of('p').count();
    for (_, index, _, bytes) in of('p') {
        set.p
            .extend(bytes.chunks(2).flat_map(|w| w.iter().rev().copied()));
        if program_parts == 1 && *index == 1 {
            normalise_p1_halves(&mut set.p);
        }
    }
    if set.p.is_empty() {
        return Err(RomSetError::NoProgramRom);
    }
    if let Some((_, _, _, bytes)) = of('s').next() {
        set.s.clone_from(bytes);
    }
    if let Some((_, _, _, bytes)) = of('m').next() {
        set.m.clone_from(bytes);
    }
    for (_, index, _, bytes) in of('v') {
        if *index >= 20 {
            set.v_b.extend_from_slice(bytes);
        } else {
            set.v_a.extend_from_slice(bytes);
        }
    }
    let c: Vec<_> = of('c').collect();
    for pair in c.chunks(2) {
        match pair {
            [(_, odd_index, _, odd), (_, even_index, _, even)]
                if odd_index % 2 == 1
                    && *even_index == odd_index + 1
                    && odd.len() == even.len() =>
            {
                set.c.reserve(odd.len() * 2);
                for (&a, &b) in odd.iter().zip(even) {
                    set.c.push(a);
                    set.c.push(b);
                }
            }
            [(_, index, _, _), ..] => {
                return Err(RomSetError::UnpairedSpriteRom { index: *index });
            }
            [] => {}
        }
    }
    Ok(set)
}

// SNK's Hardware Specification (1989), System Program p.6: recognition
// code at $100, version at $107, nonzero game ID at $108, ROM size at $10A.
// MAME neogeo.cpp::mslug loads a single 2 MiB P1's first MiB at $100000,
// continuing at $000000. Recognise that layout by the header, not a game name.
// Never override a header already in the fixed bank, or infer other layouts.
fn normalise_p1_halves(program: &mut [u8]) {
    const HALF: usize = 0x100000;
    if program.len() != 2 * HALF || &program[0x100..0x107] == b"NEO-GEO" {
        return;
    }
    let header = &program[HALF + 0x100..HALF + 0x10e];
    if &header[..8] == b"NEO-GEO\0"
        && header[8..10] != [0, 0]
        && header[10..14] == (program.len() as u32).to_be_bytes()
    {
        program.rotate_left(HALF);
    }
}

/// Write regions as canonical MAME-named ROM files.
///
/// Produces `p1.p1`, optional `s1.s1`, `m1.m1`, `v11.v11`, `v21.v21`,
/// and `c1.c1`/`c2.c2`. A 2 MiB P region uses two 1 MiB files (`p1.p1`
/// and `p2.p2`) to avoid triggering single-P1 half-layout recognition.
/// Original filenames, chip boundaries and ignored files are not reproduced.
/// Reading the returned files preserves every region byte.
///
/// # Errors
///
/// Returns [`RomSetError::NoProgramRom`] for an empty P region, or
/// [`RomSetError::OddSpriteRegion`] when C cannot split into equal byte lanes.
pub fn to_files(set: &RomSet) -> Result<Vec<(String, Vec<u8>)>, RomSetError> {
    if set.p.is_empty() {
        return Err(RomSetError::NoProgramRom);
    }
    if !set.c.len().is_multiple_of(2) {
        return Err(RomSetError::OddSpriteRegion);
    }
    let mut files = Vec::new();
    let parts = if set.p.len() == 0x200000 {
        0x100000
    } else {
        set.p.len()
    };
    for (index, bytes) in set.p.chunks(parts).enumerate() {
        let number = index + 1;
        files.push((
            format!("p{number}.p{number}"),
            bytes
                .chunks(2)
                .flat_map(|word| word.iter().rev().copied())
                .collect(),
        ));
    }
    for (name, bytes) in [
        ("s1.s1", &set.s),
        ("m1.m1", &set.m),
        ("v11.v11", &set.v_a),
        ("v21.v21", &set.v_b),
    ] {
        if !bytes.is_empty() {
            files.push((name.to_owned(), bytes.clone()));
        }
    }
    if !set.c.is_empty() {
        files.push((
            "c1.c1".to_owned(),
            set.c.iter().step_by(2).copied().collect(),
        ));
        files.push((
            "c2.c2".to_owned(),
            set.c.iter().skip(1).step_by(2).copied().collect(),
        ));
    }
    Ok(files)
}

#[cfg(test)]
mod tests;
