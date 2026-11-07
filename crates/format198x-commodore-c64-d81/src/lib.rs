//! Commodore 1581 `D81` disk images: read, write and master fresh disks with PRG files.
//!
//! `D81` images store decoded 1581 sector data (the CBM logical scheme the
//! drive presents to the C64: 80 tracks of 40 sectors, 256 bytes each), not
//! live IEC or WD177x flux behaviour. This crate is therefore a
//! container/parser layer only — the mirror of `format198x-commodore-c64-d64` for
//! the 3.5" drive.
//!
//! Unlike the D64's speed-zoned geometry, the D81 is a flat run of 256-byte
//! blocks in `(track, sector)` order: block `n = (track - 1) * 40 + sector`
//! sits at byte `n * 256`.

#![doc = include_str!("../README.md")]

const D81_STANDARD_SIZE: usize = 819_200;
/// Standard image plus a one-byte-per-block error-info map (3200 bytes).
const D81_WITH_ERROR_INFO_SIZE: usize = D81_STANDARD_SIZE + 3_200;
const SECTOR_SIZE: usize = 256;
const TRACK_COUNT: u8 = 80;
const SECTORS_PER_TRACK: u8 = 40;
const DIRECTORY_TRACK: u8 = 40;
const DIRECTORY_START_SECTOR: u8 = 3;
const HEADER_TRACK: u8 = 40;
const HEADER_SECTOR: u8 = 0;

/// One parsed directory entry from a `D81` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D81DirectoryEntry {
    /// Entry name decoded to plain text.
    pub name: String,
    /// CBM DOS file type.
    pub file_type: D81FileType,
    /// First data track.
    pub start_track: u8,
    /// First data sector.
    pub start_sector: u8,
    /// Declared block count in 254-byte blocks.
    pub blocks: u16,
    /// Whether the entry is marked closed.
    pub closed: bool,
    /// Whether the entry is marked locked.
    pub locked: bool,
}

/// Parsed `D81` directory information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D81Directory {
    /// Disk name from the header sector.
    pub disk_name: String,
    /// Two-byte disk id.
    pub disk_id: String,
    /// Parsed directory entries.
    pub entries: Vec<D81DirectoryEntry>,
}

/// One loadable PRG extracted from a `D81` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D81Program {
    /// Entry name from the directory.
    pub name: String,
    /// Raw PRG bytes including the two-byte load address prefix.
    pub data: Vec<u8>,
    /// Declared 254-byte block count.
    pub blocks: u16,
}

/// CBM DOS file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D81FileType {
    Del,
    Seq,
    Prg,
    Usr,
    Rel,
    Unknown(u8),
}

/// One `D81` parse failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum D81ParseError {
    /// The image is not one of the currently supported standard sizes.
    UnsupportedSize { actual: usize },

    /// One track number was outside the standard 80-track image.
    InvalidTrack { track: u8 },

    /// One sector number was outside the track's valid range.
    InvalidSector { track: u8, sector: u8 },

    /// One file chain referenced the same sector twice.
    CyclicFileChain { track: u8, sector: u8 },

    /// One directory sector chain referenced the same sector twice.
    CyclicDirectoryChain { track: u8, sector: u8 },

    /// The image does not contain any loadable PRG entries.
    NoLoadableEntries,

    /// One last data sector declared an invalid byte count.
    InvalidLastSectorByteCount { track: u8, sector: u8, count: u8 },
}
impl core::fmt::Display for D81ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedSize { actual } => write!(f, "unsupported D81 size {actual} bytes"),
            Self::InvalidTrack { track } => write!(f, "invalid track {track}"),
            Self::InvalidSector { track, sector } => {
                write!(f, "invalid sector {sector} on track {track}")
            }
            Self::CyclicFileChain { track, sector } => {
                write!(f, "D81 file chain loops at track {track} sector {sector}")
            }
            Self::CyclicDirectoryChain { track, sector } => write!(
                f,
                "D81 directory chain loops at track {track} sector {sector}"
            ),
            Self::NoLoadableEntries => {
                write!(f, "D81 image does not contain any PRG directory entries")
            }
            Self::InvalidLastSectorByteCount {
                track,
                sector,
                count,
            } => write!(
                f,
                "invalid last-sector byte count {count} at track {track} sector {sector}"
            ),
        }
    }
}

impl std::error::Error for D81ParseError {}

/// Parses the header and directory sectors from one `D81` image.
///
/// # Errors
///
/// Returns an error if the image size is unsupported or the directory chain is
/// malformed.
pub fn parse_directory(bytes: &[u8]) -> Result<D81Directory, D81ParseError> {
    validate_d81_size(bytes)?;

    let header = sector(bytes, HEADER_TRACK, HEADER_SECTOR)?;
    // Header block: disk name at $04-$13 ($A0-padded), disk id at $16-$17.
    let disk_name = decode_petscii_name(&header[0x04..0x14]);
    let disk_id = decode_petscii_name(&header[0x16..0x18]);

    let mut entries = Vec::new();
    let mut visited = vec![false; total_sector_count()];
    let mut track = DIRECTORY_TRACK;
    let mut sector_num = DIRECTORY_START_SECTOR;

    while track != 0 {
        let linear = linear_sector_index(track, sector_num)?;
        if visited[linear] {
            return Err(D81ParseError::CyclicDirectoryChain {
                track,
                sector: sector_num,
            });
        }
        visited[linear] = true;

        let dir_sector = sector(bytes, track, sector_num)?;
        for index in 0..8 {
            let slot = index * 32;
            let file_type_byte = dir_sector[slot + 2];
            let start_track = dir_sector[slot + 3];
            let start_sector = dir_sector[slot + 4];
            let blocks = u16::from_le_bytes([dir_sector[slot + 30], dir_sector[slot + 31]]);

            if file_type_byte == 0 || start_track == 0 {
                continue;
            }

            entries.push(D81DirectoryEntry {
                name: decode_petscii_name(&dir_sector[slot + 5..slot + 21]),
                file_type: D81FileType::from_byte(file_type_byte & 0x07),
                start_track,
                start_sector,
                blocks,
                closed: file_type_byte & 0x80 != 0,
                locked: file_type_byte & 0x40 != 0,
            });
        }

        track = dir_sector[0];
        sector_num = dir_sector[1];
    }

    Ok(D81Directory {
        disk_name,
        disk_id,
        entries,
    })
}

/// Extracts the first PRG entry from one `D81` image.
///
/// # Errors
///
/// Returns an error if the image is malformed or contains no PRG entries.
pub fn extract_first_prg(bytes: &[u8]) -> Result<D81Program, D81ParseError> {
    let directory = parse_directory(bytes)?;
    let entry = directory
        .entries
        .iter()
        .find(|entry| matches!(entry.file_type, D81FileType::Prg))
        .ok_or(D81ParseError::NoLoadableEntries)?;

    let data = read_sector_chain(bytes, entry.start_track, entry.start_sector)?;
    Ok(D81Program {
        name: entry.name.clone(),
        data,
        blocks: entry.blocks,
    })
}

/// Returns the number of sectors on one `D81` track (a uniform 40).
///
/// # Errors
///
/// Returns an error if the track is outside the supported 80-track range.
pub fn sectors_in_track(track: u8) -> Result<u8, D81ParseError> {
    if track == 0 || track > TRACK_COUNT {
        return Err(D81ParseError::InvalidTrack { track });
    }
    Ok(SECTORS_PER_TRACK)
}

/// Returns one raw decoded 256-byte sector from a `D81` image.
///
/// # Errors
///
/// Returns an error if the image is not a supported standard size or the
/// track/sector pair is outside the image geometry.
pub fn read_sector(bytes: &[u8], track: u8, sector_num: u8) -> Result<&[u8], D81ParseError> {
    validate_d81_size(bytes)?;
    sector(bytes, track, sector_num)
}

/// Writes one 256-byte sector into a `D81` image in place.
///
/// # Errors
///
/// Returns an error if the image is not a supported standard size or the
/// track/sector pair is outside the image geometry.
pub fn write_sector(
    bytes: &mut [u8],
    track: u8,
    sector_num: u8,
    data: &[u8; SECTOR_SIZE],
) -> Result<(), D81ParseError> {
    validate_d81_size(bytes)?;
    let offset = sector_offset(track, sector_num)?;
    bytes[offset..offset + SECTOR_SIZE].copy_from_slice(data);
    Ok(())
}

const TRACK_SECTOR_COUNTS: [u8; 80] = [40; 80];
/// One failure while mastering a `D81` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum D81WriteError {
    /// A disk or file name is too long or holds a character that would not
    /// read back the same (only `A-Z`, `a-z`, `0-9` and space do).
    InvalidName {
        /// The rejected name.
        name: String,
    },
    /// The disk id is not exactly two such characters.
    InvalidId,
    /// The files need more blocks than the disk has free.
    DiskFull,
    /// There are more files than the directory track can list.
    DirectoryFull,
}

impl core::fmt::Display for D81WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName { name } => write!(
                f,
                "D81 name {name:?} must be at most 16 of A-Z, a-z, 0-9 and space"
            ),
            Self::InvalidId => write!(f, "D81 disk id must be two of A-Z, a-z and 0-9"),
            Self::DiskFull => write!(f, "files do not fit the 3160 free blocks of a D81"),
            Self::DirectoryFull => write!(f, "a D81 directory holds at most 296 files"),
        }
    }
}

impl std::error::Error for D81WriteError {}

/// Sectors a 1581 skips between consecutive blocks of a file.
const FILE_INTERLEAVE: u8 = 1;
/// Sectors it skips between consecutive directory blocks.
const DIRECTORY_INTERLEAVE: u8 = 1;

fn encode_name(name: &str, width: usize) -> Result<Vec<u8>, D81WriteError> {
    let raw = name.as_bytes();
    if raw.len() > width || !raw.iter().all(|b| b.is_ascii_alphanumeric() || *b == b' ') {
        return Err(D81WriteError::InvalidName {
            name: name.to_owned(),
        });
    }
    let mut field = raw.to_vec();
    field.resize(width, 0xA0);
    Ok(field)
}

/// Free-sector map for mastering, one flag per sector.
struct Allocator {
    used: Vec<Vec<bool>>,
}

impl Allocator {
    fn new() -> Self {
        Self {
            used: TRACK_SECTOR_COUNTS
                .iter()
                .map(|&n| vec![false; usize::from(n)])
                .collect(),
        }
    }

    fn take(&mut self, track: u8, sector: u8) {
        self.used[usize::from(track - 1)][usize::from(sector)] = true;
    }

    /// The next free sector on `track` at or after `from`, wrapping round
    /// the track. Callers pass `from` as the last sector plus the interleave.
    fn next_on_track(&self, track: u8, from: u8) -> Option<u8> {
        let row = &self.used[usize::from(track - 1)];
        // At most 40 sectors per track, so this always fits a u8.
        #[allow(clippy::cast_possible_truncation)]
        let count = row.len() as u8;
        let mut sector = from % count;
        for _ in 0..count {
            if !row[usize::from(sector)] {
                return Some(sector);
            }
            sector = (sector + 1) % count;
        }
        None
    }

    /// Allocate a data block after (`track`, `sector`), keeping off the
    /// directory track: tracks 39 down to 1, then 41 up to 80.
    fn next_data(&mut self, last: Option<(u8, u8)>) -> Option<(u8, u8)> {
        let order: Vec<u8> = (1..DIRECTORY_TRACK)
            .rev()
            .chain(DIRECTORY_TRACK + 1..=80)
            .collect();
        let (start_index, from) = match last {
            Some((track, sector)) => (
                order.iter().position(|&t| t == track).unwrap_or(0),
                sector + FILE_INTERLEAVE,
            ),
            None => (0, 0),
        };
        for (step, &track) in order.iter().enumerate().skip(start_index) {
            let from = if step == start_index { from } else { 0 };
            if let Some(sector) = self.next_on_track(track, from) {
                self.take(track, sector);
                return Some((track, sector));
            }
        }
        None
    }
}

/// Layout: Commodore 1581 Users Guide, appendix C, pp. 119–120.
/// See the retained primary extract in the umbrella reference library.
///
/// Master a freshly formatted `D81` holding `files` as closed PRG entries,
/// in order.
///
/// The image is the standard 3200-sector size with two BAM sectors in DOS format `3D`.
/// Each file's [`D81Program::data`] is written whole, load address
/// included; its `blocks` field is ignored and recomputed. Data blocks are
/// allocated from track 39 outwards, skipping track 40. Data and directory
/// blocks use interleave 1. Only the root directory is created.
///
/// # Errors
///
/// Returns an error when a name or the id cannot be written so that it reads
/// back the same, the files do not fit, or there are more than 296 of them.
pub fn master(
    disk_name: &str,
    disk_id: &str,
    files: &[D81Program],
) -> Result<Vec<u8>, D81WriteError> {
    let name_field = encode_name(disk_name, 16)?;
    let id = disk_id.as_bytes();
    if id.len() != 2 || !id.iter().all(u8::is_ascii_alphanumeric) {
        return Err(D81WriteError::InvalidId);
    }
    if files.len() > 296 {
        return Err(D81WriteError::DirectoryFull);
    }
    let mut image = vec![0u8; D81_STANDARD_SIZE];
    let mut alloc = Allocator::new();
    alloc.take(HEADER_TRACK, HEADER_SECTOR);
    alloc.take(40, 1);
    alloc.take(40, 2);

    // Directory sectors on track 40, chained with the directory interleave.
    let dir_count = files.len().div_ceil(8).max(1);
    let mut dir_sectors = vec![DIRECTORY_START_SECTOR];
    alloc.take(DIRECTORY_TRACK, DIRECTORY_START_SECTOR);
    while dir_sectors.len() < dir_count {
        let last = dir_sectors[dir_sectors.len() - 1];
        let sector = alloc
            .next_on_track(DIRECTORY_TRACK, last + DIRECTORY_INTERLEAVE)
            .ok_or(D81WriteError::DirectoryFull)?;
        alloc.take(DIRECTORY_TRACK, sector);
        dir_sectors.push(sector);
    }
    let mut dir_blocks = vec![[0u8; SECTOR_SIZE]; dir_count];
    for (i, block) in dir_blocks.iter_mut().enumerate() {
        if let Some(&next) = dir_sectors.get(i + 1) {
            block[0] = DIRECTORY_TRACK;
            block[1] = next;
        } else {
            block[1] = 0xFF;
        }
    }

    for (index, file) in files.iter().enumerate() {
        let entry_name = encode_name(&file.name, 16)?;
        let chunks: Vec<&[u8]> = if file.data.is_empty() {
            vec![&[]]
        } else {
            file.data.chunks(254).collect()
        };
        let mut blocks = Vec::with_capacity(chunks.len());
        let mut last = None;
        for _ in &chunks {
            let at = alloc.next_data(last).ok_or(D81WriteError::DiskFull)?;
            blocks.push(at);
            last = Some(at);
        }
        for (i, chunk) in chunks.iter().enumerate() {
            let mut sector = [0u8; SECTOR_SIZE];
            if let Some(&(track, next)) = blocks.get(i + 1) {
                sector[0] = track;
                sector[1] = next;
            } else {
                // Chunks hold at most 254 bytes, so this fits a u8.
                #[allow(clippy::cast_possible_truncation)]
                let last_byte = chunk.len() as u8 + 1;
                sector[1] = last_byte;
            }
            sector[2..2 + chunk.len()].copy_from_slice(chunk);
            let (track, sector_num) = blocks[i];
            let offset = sector_offset(track, sector_num).map_err(|_| D81WriteError::DiskFull)?;
            image[offset..offset + SECTOR_SIZE].copy_from_slice(&sector);
        }
        let slot = &mut dir_blocks[index / 8][(index % 8) * 32..(index % 8) * 32 + 32];
        slot[2] = 0x82;
        slot[3] = blocks[0].0;
        slot[4] = blocks[0].1;
        slot[5..21].copy_from_slice(&entry_name);
        // At most 3160 blocks, which fits a u16.
        #[allow(clippy::cast_possible_truncation)]
        slot[30..32].copy_from_slice(&(blocks.len() as u16).to_le_bytes());
    }
    for (block, &sector) in dir_blocks.iter().zip(&dir_sectors) {
        let offset =
            sector_offset(DIRECTORY_TRACK, sector).map_err(|_| D81WriteError::DirectoryFull)?;
        image[offset..offset + SECTOR_SIZE].copy_from_slice(block);
    }

    let mut header = [0u8; SECTOR_SIZE];
    header[0] = 40;
    header[1] = 3;
    header[2] = b'D';
    header[4..20].copy_from_slice(&name_field);
    header[20..22].fill(0xA0);
    header[22..24].copy_from_slice(id);
    header[24] = 0xA0;
    header[25..27].copy_from_slice(b"3D");
    header[27..29].fill(0xA0);
    let offset = sector_offset(40, 0).map_err(|_| D81WriteError::DiskFull)?;
    image[offset..offset + SECTOR_SIZE].copy_from_slice(&header);
    for half in 0..2 {
        let mut bam = [0u8; SECTOR_SIZE];
        if half == 0 {
            bam[0] = 40;
            bam[1] = 2;
        } else {
            bam[1] = 0xFF;
        }
        bam[2] = b'D';
        bam[3] = !b'D';
        bam[4..6].copy_from_slice(id);
        bam[6] = 0xC0;
        for (t, row) in alloc.used[half * 40..half * 40 + 40].iter().enumerate() {
            let entry = &mut bam[16 + t * 6..22 + t * 6];
            for (s, &used) in row.iter().enumerate() {
                if !used {
                    entry[0] += 1;
                    entry[1 + s / 8] |= 1 << (s % 8);
                }
            }
        }
        let sector = if half == 0 { 1 } else { 2 };
        let offset = sector_offset(40, sector).map_err(|_| D81WriteError::DiskFull)?;
        image[offset..offset + SECTOR_SIZE].copy_from_slice(&bam);
    }
    Ok(image)
}

impl D81FileType {
    const fn from_byte(value: u8) -> Self {
        match value {
            0 => Self::Del,
            1 => Self::Seq,
            2 => Self::Prg,
            3 => Self::Usr,
            4 => Self::Rel,
            other => Self::Unknown(other),
        }
    }
}

fn read_sector_chain(
    bytes: &[u8],
    mut track: u8,
    mut sector_num: u8,
) -> Result<Vec<u8>, D81ParseError> {
    let mut data = Vec::with_capacity(SECTOR_SIZE * 4);
    let mut visited = vec![false; total_sector_count()];

    loop {
        let linear = linear_sector_index(track, sector_num)?;
        if visited[linear] {
            return Err(D81ParseError::CyclicFileChain {
                track,
                sector: sector_num,
            });
        }
        visited[linear] = true;

        let block = sector(bytes, track, sector_num)?;
        let next_track = block[0];
        let next_sector = block[1];
        if next_track == 0 {
            if next_sector == 0 {
                return Err(D81ParseError::InvalidLastSectorByteCount {
                    track,
                    sector: sector_num,
                    count: next_sector,
                });
            }
            let used = usize::from(next_sector) - 1;
            if used > 254 {
                return Err(D81ParseError::InvalidLastSectorByteCount {
                    track,
                    sector: sector_num,
                    count: next_sector,
                });
            }
            data.extend_from_slice(&block[2..2 + used]);
            return Ok(data);
        }

        data.extend_from_slice(&block[2..]);
        track = next_track;
        sector_num = next_sector;
    }
}

fn validate_d81_size(bytes: &[u8]) -> Result<(), D81ParseError> {
    match bytes.len() {
        D81_STANDARD_SIZE | D81_WITH_ERROR_INFO_SIZE => Ok(()),
        actual => Err(D81ParseError::UnsupportedSize { actual }),
    }
}

fn sector(bytes: &[u8], track: u8, sector_num: u8) -> Result<&[u8], D81ParseError> {
    let offset = sector_offset(track, sector_num)?;
    Ok(&bytes[offset..offset + SECTOR_SIZE])
}

fn sector_offset(track: u8, sector_num: u8) -> Result<usize, D81ParseError> {
    let linear = linear_sector_index(track, sector_num)?;
    Ok(linear * SECTOR_SIZE)
}

fn linear_sector_index(track: u8, sector_num: u8) -> Result<usize, D81ParseError> {
    if track == 0 || track > TRACK_COUNT {
        return Err(D81ParseError::InvalidTrack { track });
    }
    if sector_num >= SECTORS_PER_TRACK {
        return Err(D81ParseError::InvalidSector {
            track,
            sector: sector_num,
        });
    }
    Ok(usize::from(track - 1) * usize::from(SECTORS_PER_TRACK) + usize::from(sector_num))
}

const fn total_sector_count() -> usize {
    TRACK_COUNT as usize * SECTORS_PER_TRACK as usize
}

fn decode_petscii_name(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for &byte in bytes {
        let ch = match byte {
            0x00 | 0xA0 | 0x20 => ' ',
            0x30..=0x39 | 0x41..=0x5A => char::from(byte),
            0x61..=0x7A => char::from(byte),
            0xC1..=0xDA => char::from(byte - 0x80),
            0xE1..=0xFA => char::from(byte - 0x80),
            _ => '?',
        };
        text.push(ch);
    }
    text.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_image() -> Vec<u8> {
        vec![0; D81_STANDARD_SIZE]
    }

    fn put_sector(bytes: &mut [u8], track: u8, sector_num: u8, data: &[u8; SECTOR_SIZE]) {
        write_sector(bytes, track, sector_num, data).expect("synthetic sector should write");
    }

    fn synthetic_image() -> Vec<u8> {
        let mut bytes = blank_image();

        // Header block (40/0): link to first dir sector, disk name + id.
        let mut header = [0u8; SECTOR_SIZE];
        header[0] = DIRECTORY_TRACK;
        header[1] = DIRECTORY_START_SECTOR;
        header[2] = b'D';
        header[0x04..0x0D].copy_from_slice(b"DEMO DISK");
        for byte in &mut header[0x0D..0x14] {
            *byte = 0xA0;
        }
        header[0x16..0x18].copy_from_slice(b"42");
        put_sector(&mut bytes, HEADER_TRACK, HEADER_SECTOR, &header);

        // One directory sector (40/3) with a single PRG entry pointing at 1/0.
        let mut directory = [0u8; SECTOR_SIZE];
        directory[2] = 0x82; // closed PRG
        directory[3] = 1; // start track
        directory[4] = 0; // start sector
        directory[5..10].copy_from_slice(b"HELLO");
        for byte in &mut directory[10..21] {
            *byte = 0xA0;
        }
        directory[30] = 1; // one block
        put_sector(
            &mut bytes,
            DIRECTORY_TRACK,
            DIRECTORY_START_SECTOR,
            &directory,
        );

        // The PRG's single data sector (1/0): last sector, three payload bytes.
        let mut prg = [0u8; SECTOR_SIZE];
        prg[0] = 0; // last sector
        prg[1] = 4; // 3 payload bytes used (count - 1)
        prg[2] = 0x01;
        prg[3] = 0x08; // load address $0801
        prg[4] = 0xAB;
        put_sector(&mut bytes, 1, 0, &prg);

        bytes
    }

    #[test]
    fn standard_size_validates() {
        assert!(read_sector(&blank_image(), 1, 0).is_ok());
    }

    #[test]
    fn rejects_wrong_size() {
        let err = read_sector(&vec![0; 1000], 1, 0).expect_err("wrong size must fail");
        assert_eq!(err, D81ParseError::UnsupportedSize { actual: 1000 });
    }

    #[test]
    fn sector_offset_is_flat_track_major() {
        // Block n = (track-1)*40 + sector, at byte n*256.
        assert_eq!(sector_offset(1, 0).expect("valid"), 0);
        assert_eq!(sector_offset(1, 39).expect("valid"), 39 * 256);
        assert_eq!(sector_offset(2, 0).expect("valid"), 40 * 256);
        assert_eq!(sector_offset(80, 39).expect("valid"), (79 * 40 + 39) * 256);
    }

    #[test]
    fn rejects_out_of_range_track_and_sector() {
        assert_eq!(
            linear_sector_index(0, 0).expect_err("track 0 invalid"),
            D81ParseError::InvalidTrack { track: 0 }
        );
        assert_eq!(
            linear_sector_index(81, 0).expect_err("track 81 invalid"),
            D81ParseError::InvalidTrack { track: 81 }
        );
        assert_eq!(
            linear_sector_index(1, 40).expect_err("sector 40 invalid"),
            D81ParseError::InvalidSector {
                track: 1,
                sector: 40
            }
        );
    }

    #[test]
    fn parses_header_and_directory() {
        let image = synthetic_image();
        let dir = parse_directory(&image).expect("directory parses");
        assert_eq!(dir.disk_name, "DEMO DISK");
        assert_eq!(dir.disk_id, "42");
        assert_eq!(dir.entries.len(), 1);
        assert_eq!(dir.entries[0].name, "HELLO");
        assert_eq!(dir.entries[0].file_type, D81FileType::Prg);
        assert!(dir.entries[0].closed);
    }

    #[test]
    fn extracts_first_prg() {
        let image = synthetic_image();
        let prg = extract_first_prg(&image).expect("PRG extracts");
        assert_eq!(prg.name, "HELLO");
        assert_eq!(prg.data, vec![0x01, 0x08, 0xAB]);
    }

    #[test]
    fn write_then_read_round_trips() {
        let mut image = blank_image();
        let mut payload = [0u8; SECTOR_SIZE];
        payload[0] = 0xCA;
        payload[255] = 0xFE;
        write_sector(&mut image, 40, 17, &payload).expect("write");
        assert_eq!(read_sector(&image, 40, 17).expect("read back"), &payload);
    }

    fn program(name: &str, len: usize) -> D81Program {
        D81Program {
            name: name.to_owned(),
            data: (0..len).map(|i| i.to_le_bytes()[0]).collect(),
            blocks: 999,
        }
    }

    // Independent traversal: occupied sectors must agree with both bitmap bits
    // and popcounts. This does not reuse the writer's allocator.
    fn check_allocation(image: &[u8]) -> Result<usize, String> {
        let mut used = std::collections::BTreeSet::new();
        for s in 0..3 {
            used.insert((40, s));
        }
        let mut at = (DIRECTORY_TRACK, DIRECTORY_START_SECTOR);
        loop {
            if !used.insert(at) {
                return Err("directory overlaps".into());
            }
            let block = read_sector(image, at.0, at.1).map_err(|e| e.to_string())?;
            if block[0] == 0 {
                break;
            }
            at = (block[0], block[1]);
        }
        let directory = parse_directory(image).map_err(|e| e.to_string())?;
        for entry in directory.entries {
            let mut at = (entry.start_track, entry.start_sector);
            let mut blocks = 0;
            loop {
                if at.0 == 40 {
                    return Err("file uses a reserved track".into());
                }
                if !used.insert(at) {
                    return Err("file overlaps".into());
                }
                blocks += 1;
                let block = read_sector(image, at.0, at.1).map_err(|e| e.to_string())?;
                if block[0] == 0 {
                    break;
                }
                at = (block[0], block[1]);
            }
            if blocks != entry.blocks {
                return Err("directory block count".into());
            }
        }
        let mut data_free = 0;
        for t in 1..=80 {
            let n = usize::from(t - 1) % 40;
            let sector = if t <= 40 { 1 } else { 2 };
            let bam = read_sector(image, 40, sector).map_err(|e| e.to_string())?;
            let (declared, bitmap) = (bam[16 + n * 6], &bam[17 + n * 6..22 + n * 6]);
            let count = sectors_in_track(t).map_err(|e| e.to_string())?;
            let mut free = 0;
            for s in 0..bitmap.len() * 8 {
                let bit = bitmap[s / 8] & (1 << (s % 8)) != 0;
                let expected = s < usize::from(count)
                    && !used.contains(&(t, u8::try_from(s).expect("sector fits")));
                if bit != expected {
                    return Err(format!("bitmap mismatch {t}/{s}"));
                }
                if bit {
                    free += 1;
                }
            }
            if free != usize::from(declared) {
                return Err(format!("free count mismatch on {t}"));
            }
            if t != 40 {
                data_free += free;
            }
        }
        Ok(data_free)
    }

    #[test]
    fn mastering_matches_payloads_chains_and_both_free_maps() {
        let files: Vec<_> = [0, 1, 253, 254, 255, 500_000]
            .into_iter()
            .enumerate()
            .map(|(i, n)| program(&format!("FILE{i}"), n))
            .collect();
        let image = master("TEST DISK", "AB", &files).expect("masters");
        assert_eq!(image.len(), D81_STANDARD_SIZE);
        let directory = parse_directory(&image).expect("directory");
        assert_eq!(directory.disk_name, "TEST DISK");
        assert_eq!(directory.disk_id, "AB");
        assert_eq!(directory.entries.len(), files.len());
        for (entry, file) in directory.entries.iter().zip(&files) {
            assert_eq!(entry.name, file.name);
            assert_eq!(entry.file_type, D81FileType::Prg);
            assert!(entry.closed && !entry.locked);
            assert_eq!(
                usize::from(entry.blocks),
                file.data.len().div_ceil(254).max(1)
            );
            assert_eq!(
                read_sector_chain(&image, entry.start_track, entry.start_sector).expect("chain"),
                file.data
            );
        }
        let consumed: usize = files
            .iter()
            .map(|p| p.data.len().div_ceil(254).max(1))
            .sum();
        assert_eq!(check_allocation(&image), Ok(3160 - consumed));
    }

    #[test]
    fn empty_and_exactly_full_images_have_consistent_allocation() {
        let empty = master("", "00", &[]).expect("empty disk");
        assert!(
            parse_directory(&empty)
                .expect("directory")
                .entries
                .is_empty()
        );
        assert_eq!(check_allocation(&empty), Ok(3160));
        let file = program("FULL", 3160 * 254);
        let full = master("FULL", "00", std::slice::from_ref(&file)).expect("exact capacity");
        assert_eq!(check_allocation(&full), Ok(0));
        assert_eq!(extract_first_prg(&full).expect("extract").data, file.data);
        assert_eq!(
            master("OVER", "00", &[program("OVER", 3160 * 254 + 1)]),
            Err(D81WriteError::DiskFull)
        );
    }

    #[test]
    fn directory_capacity_and_multi_sector_links_are_consistent() {
        let files: Vec<_> = (0..296).map(|i| program(&format!("F{i}"), 1)).collect();
        let image = master("DIR", "00", &files).expect("full directory");
        assert_eq!(
            parse_directory(&image).expect("directory").entries.len(),
            296
        );
        assert_eq!(check_allocation(&image), Ok(3160 - 296));
        let mut too_many = files;
        too_many.push(program("OVER", 1));
        assert_eq!(
            master("DIR", "00", &too_many),
            Err(D81WriteError::DirectoryFull)
        );
    }

    #[test]
    fn rejects_names_and_ids_outside_the_d64_contract() {
        for name in ["A NAME OVER 16 CHARS", "BAD!", "é"] {
            assert!(matches!(
                master(name, "AB", &[]),
                Err(D81WriteError::InvalidName { .. })
            ));
            assert!(matches!(
                master("OK", "AB", &[program(name, 1)]),
                Err(D81WriteError::InvalidName { .. })
            ));
        }
        for id in ["", "A", "ABC", "A ", "é"] {
            assert_eq!(master("OK", id, &[]), Err(D81WriteError::InvalidId));
        }
        assert!(master("MiXeD 123", "a1", &[program("ABCDEFGHIJKLMNOP", 1)]).is_ok());
    }

    #[test]
    fn allocation_checks_go_red_for_corrupted_counts_and_bits() {
        let mut image = master("CHECK", "AB", &[program("FILE", 500_000)]).expect("disk");
        assert!(check_allocation(&image).is_ok());
        let offset = sector_offset(40, 2).expect("offset") + 16;
        image[offset] ^= 1;
        assert!(
            check_allocation(&image)
                .expect_err("bad count must fail")
                .contains("count")
        );
        image[offset] ^= 1;
        let offset = sector_offset(40, 2).expect("offset") + 17;
        image[offset] ^= 1;
        assert!(
            check_allocation(&image)
                .expect_err("bad bit must fail")
                .contains("bitmap")
        );
    }
}
