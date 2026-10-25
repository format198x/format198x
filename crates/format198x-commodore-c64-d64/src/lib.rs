//! Commodore 1541 `D64` disk images: read, write and master.
//!
//! `D64` images store decoded 1541 sector data, not live IEC or GCR drive
//! behaviour. This crate is therefore a container layer only: it reads the
//! directory and files, reads and writes single sectors, and masters a fresh
//! disk holding PRG files ([`master`]).

const D64_STANDARD_SIZE: usize = 174_848;
const D64_STANDARD_WITH_ERROR_INFO_SIZE: usize = 175_531;
const SECTOR_SIZE: usize = 256;
const DIRECTORY_TRACK: u8 = 18;
const DIRECTORY_START_SECTOR: u8 = 1;
const BAM_TRACK: u8 = 18;
const BAM_SECTOR: u8 = 0;
const TRACK_SECTOR_COUNTS: [u8; 35] = [
    21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21, 19, 19, 19, 19, 19, 19, 19,
    18, 18, 18, 18, 18, 18, 17, 17, 17, 17, 17,
];

/// One parsed directory entry from a `D64` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D64DirectoryEntry {
    /// Entry name decoded to plain text.
    pub name: String,
    /// CBM DOS file type.
    pub file_type: D64FileType,
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

/// Parsed `D64` directory information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D64Directory {
    /// Disk name from the BAM sector.
    pub disk_name: String,
    /// Two-byte disk id, when present.
    pub disk_id: String,
    /// Parsed directory entries.
    pub entries: Vec<D64DirectoryEntry>,
}

/// One loadable PRG extracted from a `D64` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct D64Program {
    /// Entry name from the directory.
    pub name: String,
    /// Raw PRG bytes including the two-byte load address prefix.
    pub data: Vec<u8>,
    /// Declared 254-byte block count.
    pub blocks: u16,
}

/// CBM DOS file type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D64FileType {
    Del,
    Seq,
    Prg,
    Usr,
    Rel,
    Unknown(u8),
}

/// One `D64` parse failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum D64ParseError {
    /// The image is not one of the currently supported standard sizes.
    UnsupportedSize { actual: usize },

    /// One track number was outside the standard 35-track image.
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
impl core::fmt::Display for D64ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedSize { actual } => write!(f, "unsupported D64 size {actual} bytes"),
            Self::InvalidTrack { track } => write!(f, "invalid track {track}"),
            Self::InvalidSector { track, sector } => {
                write!(f, "invalid sector {sector} on track {track}")
            }
            Self::CyclicFileChain { track, sector } => {
                write!(f, "D64 file chain loops at track {track} sector {sector}")
            }
            Self::CyclicDirectoryChain { track, sector } => write!(
                f,
                "D64 directory chain loops at track {track} sector {sector}"
            ),
            Self::NoLoadableEntries => {
                write!(f, "D64 image does not contain any PRG directory entries")
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

impl std::error::Error for D64ParseError {}

/// Parses the BAM and directory sectors from one `D64` image.
///
/// # Errors
///
/// Returns an error if the image size is unsupported or the directory chain is
/// malformed.
pub fn parse_directory(bytes: &[u8]) -> Result<D64Directory, D64ParseError> {
    validate_d64_size(bytes)?;

    let bam = sector(bytes, BAM_TRACK, BAM_SECTOR)?;
    let disk_name = decode_petscii_name(&bam[0x90..0xA0]);
    let disk_id = decode_petscii_name(&bam[0xA2..0xA4]);

    let mut entries = Vec::new();
    let mut visited = vec![false; total_sector_count()];
    let mut track = DIRECTORY_TRACK;
    let mut sector_num = DIRECTORY_START_SECTOR;

    while track != 0 {
        let linear = linear_sector_index(track, sector_num)?;
        if visited[linear] {
            return Err(D64ParseError::CyclicDirectoryChain {
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

            entries.push(D64DirectoryEntry {
                name: decode_petscii_name(&dir_sector[slot + 5..slot + 21]),
                file_type: D64FileType::from_byte(file_type_byte & 0x07),
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

    Ok(D64Directory {
        disk_name,
        disk_id,
        entries,
    })
}

/// Extracts the first PRG entry from one `D64` image.
///
/// # Errors
///
/// Returns an error if the image is malformed or contains no PRG entries.
pub fn extract_first_prg(bytes: &[u8]) -> Result<D64Program, D64ParseError> {
    let directory = parse_directory(bytes)?;
    let entry = directory
        .entries
        .iter()
        .find(|entry| matches!(entry.file_type, D64FileType::Prg))
        .ok_or(D64ParseError::NoLoadableEntries)?;

    let data = read_sector_chain(bytes, entry.start_track, entry.start_sector)?;
    Ok(D64Program {
        name: entry.name.clone(),
        data,
        blocks: entry.blocks,
    })
}

/// Returns the number of sectors on one standard 35-track `D64` track.
///
/// # Errors
///
/// Returns an error if the track is outside the supported 35-track range.
pub fn sectors_in_track(track: u8) -> Result<u8, D64ParseError> {
    if track == 0 || usize::from(track) > TRACK_SECTOR_COUNTS.len() {
        return Err(D64ParseError::InvalidTrack { track });
    }

    Ok(TRACK_SECTOR_COUNTS[usize::from(track - 1)])
}

/// Returns one raw decoded 256-byte sector from a `D64` image.
///
/// # Errors
///
/// Returns an error if the image is not a supported standard size or the
/// track/sector pair is outside the image geometry.
pub fn read_sector(bytes: &[u8], track: u8, sector_num: u8) -> Result<&[u8], D64ParseError> {
    validate_d64_size(bytes)?;
    sector(bytes, track, sector_num)
}

/// Writes one 256-byte sector into a `D64` image in place.
///
/// The sector-level counterpart to [`read_sector`]: a drive emulator that
/// decodes modified tracks back to 256-byte sectors can land them here.
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
) -> Result<(), D64ParseError> {
    validate_d64_size(bytes)?;
    let offset = sector_offset(track, sector_num)?;
    bytes[offset..offset + SECTOR_SIZE].copy_from_slice(data);
    Ok(())
}

/// One failure while mastering a `D64` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum D64WriteError {
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

impl core::fmt::Display for D64WriteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName { name } => write!(
                f,
                "D64 name {name:?} must be at most 16 of A-Z, a-z, 0-9 and space"
            ),
            Self::InvalidId => write!(f, "D64 disk id must be two of A-Z, a-z and 0-9"),
            Self::DiskFull => write!(f, "files do not fit the 664 free blocks of a D64"),
            Self::DirectoryFull => write!(f, "a D64 directory holds at most 144 files"),
        }
    }
}

impl std::error::Error for D64WriteError {}

/// Sectors a 1541 skips between consecutive blocks of a file.
const FILE_INTERLEAVE: u8 = 10;
/// Sectors it skips between consecutive directory blocks.
const DIRECTORY_INTERLEAVE: u8 = 3;

fn encode_name(name: &str, width: usize) -> Result<Vec<u8>, D64WriteError> {
    let raw = name.as_bytes();
    if raw.len() > width || !raw.iter().all(|b| b.is_ascii_alphanumeric() || *b == b' ') {
        return Err(D64WriteError::InvalidName {
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
        // At most 21 sectors per track, so this always fits a u8.
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
    /// directory track: tracks 17 down to 1, then 19 up to 35.
    fn next_data(&mut self, last: Option<(u8, u8)>) -> Option<(u8, u8)> {
        let order: Vec<u8> = (1..DIRECTORY_TRACK)
            .rev()
            .chain(DIRECTORY_TRACK + 1..=35)
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

/// Master a freshly formatted `D64` holding `files` as closed PRG entries,
/// in order.
///
/// The image is the standard 683-sector size with a BAM in DOS format `A`.
/// Each file's [`D64Program::data`] is written whole, load address
/// included; its `blocks` field is ignored and recomputed. Data blocks are
/// allocated from track 17 outwards with the 1541's interleave of 10, and
/// directory blocks on track 18 with an interleave of 3.
///
/// # Errors
///
/// Returns an error when a name or the id cannot be written so that it reads
/// back the same, the files do not fit, or there are more than 144 of them.
pub fn master(
    disk_name: &str,
    disk_id: &str,
    files: &[D64Program],
) -> Result<Vec<u8>, D64WriteError> {
    let name_field = encode_name(disk_name, 16)?;
    let id = disk_id.as_bytes();
    if id.len() != 2 || !id.iter().all(u8::is_ascii_alphanumeric) {
        return Err(D64WriteError::InvalidId);
    }
    if files.len() > 144 {
        return Err(D64WriteError::DirectoryFull);
    }
    let mut image = vec![0u8; D64_STANDARD_SIZE];
    let mut alloc = Allocator::new();
    alloc.take(BAM_TRACK, BAM_SECTOR);

    // Directory sectors on track 18, chained with the directory interleave.
    let dir_count = files.len().div_ceil(8).max(1);
    let mut dir_sectors = vec![DIRECTORY_START_SECTOR];
    alloc.take(DIRECTORY_TRACK, DIRECTORY_START_SECTOR);
    while dir_sectors.len() < dir_count {
        let last = dir_sectors[dir_sectors.len() - 1];
        let sector = alloc
            .next_on_track(DIRECTORY_TRACK, last + DIRECTORY_INTERLEAVE)
            .ok_or(D64WriteError::DirectoryFull)?;
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
            let at = alloc.next_data(last).ok_or(D64WriteError::DiskFull)?;
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
            let offset = sector_offset(track, sector_num).map_err(|_| D64WriteError::DiskFull)?;
            image[offset..offset + SECTOR_SIZE].copy_from_slice(&sector);
        }
        let slot = &mut dir_blocks[index / 8][(index % 8) * 32..(index % 8) * 32 + 32];
        slot[2] = 0x82;
        slot[3] = blocks[0].0;
        slot[4] = blocks[0].1;
        slot[5..21].copy_from_slice(&entry_name);
        // At most 664 blocks, which fits a u16.
        #[allow(clippy::cast_possible_truncation)]
        slot[30..32].copy_from_slice(&(blocks.len() as u16).to_le_bytes());
    }
    for (block, &sector) in dir_blocks.iter().zip(&dir_sectors) {
        let offset =
            sector_offset(DIRECTORY_TRACK, sector).map_err(|_| D64WriteError::DirectoryFull)?;
        image[offset..offset + SECTOR_SIZE].copy_from_slice(block);
    }

    let mut bam = [0u8; SECTOR_SIZE];
    bam[0] = DIRECTORY_TRACK;
    bam[1] = DIRECTORY_START_SECTOR;
    bam[2] = b'A';
    for (t, row) in alloc.used.iter().enumerate() {
        let entry = &mut bam[4 + t * 4..8 + t * 4];
        let mut free = 0u8;
        for (s, &used) in row.iter().enumerate() {
            if !used {
                free += 1;
                entry[1 + s / 8] |= 1 << (s % 8);
            }
        }
        entry[0] = free;
    }
    bam[0x90..0xA0].copy_from_slice(&name_field);
    bam[0xA0] = 0xA0;
    bam[0xA1] = 0xA0;
    bam[0xA2..0xA4].copy_from_slice(id);
    bam[0xA4] = 0xA0;
    bam[0xA5] = b'2';
    bam[0xA6] = b'A';
    bam[0xA7..0xAB].fill(0xA0);
    let offset = sector_offset(BAM_TRACK, BAM_SECTOR).map_err(|_| D64WriteError::DiskFull)?;
    image[offset..offset + SECTOR_SIZE].copy_from_slice(&bam);
    Ok(image)
}

impl D64FileType {
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
) -> Result<Vec<u8>, D64ParseError> {
    let mut data = Vec::with_capacity(SECTOR_SIZE * 4);
    let mut visited = vec![false; total_sector_count()];

    loop {
        let linear = linear_sector_index(track, sector_num)?;
        if visited[linear] {
            return Err(D64ParseError::CyclicFileChain {
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
                return Err(D64ParseError::InvalidLastSectorByteCount {
                    track,
                    sector: sector_num,
                    count: next_sector,
                });
            }
            let used = usize::from(next_sector) - 1;
            if used > 254 {
                return Err(D64ParseError::InvalidLastSectorByteCount {
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

fn validate_d64_size(bytes: &[u8]) -> Result<(), D64ParseError> {
    match bytes.len() {
        D64_STANDARD_SIZE | D64_STANDARD_WITH_ERROR_INFO_SIZE => Ok(()),
        actual => Err(D64ParseError::UnsupportedSize { actual }),
    }
}

fn sector(bytes: &[u8], track: u8, sector_num: u8) -> Result<&[u8], D64ParseError> {
    let offset = sector_offset(track, sector_num)?;
    Ok(&bytes[offset..offset + SECTOR_SIZE])
}

fn sector_offset(track: u8, sector_num: u8) -> Result<usize, D64ParseError> {
    let linear = linear_sector_index(track, sector_num)?;
    Ok(linear * SECTOR_SIZE)
}

fn linear_sector_index(track: u8, sector_num: u8) -> Result<usize, D64ParseError> {
    if track == 0 || usize::from(track) > TRACK_SECTOR_COUNTS.len() {
        return Err(D64ParseError::InvalidTrack { track });
    }

    let sectors_in_track = TRACK_SECTOR_COUNTS[usize::from(track - 1)];
    if sector_num >= sectors_in_track {
        return Err(D64ParseError::InvalidSector {
            track,
            sector: sector_num,
        });
    }

    let prior_sectors: usize = TRACK_SECTOR_COUNTS[..usize::from(track - 1)]
        .iter()
        .map(|&count| usize::from(count))
        .sum();
    Ok(prior_sectors + usize::from(sector_num))
}

const fn total_sector_count() -> usize {
    let mut total = 0usize;
    let mut index = 0usize;
    while index < TRACK_SECTOR_COUNTS.len() {
        total += TRACK_SECTOR_COUNTS[index] as usize;
        index += 1;
    }
    total
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

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_image() -> Vec<u8> {
        vec![0; D64_STANDARD_SIZE]
    }

    fn put_sector(bytes: &mut [u8], track: u8, sector_num: u8, sector: &[u8; SECTOR_SIZE]) {
        write_sector(bytes, track, sector_num, sector).expect("synthetic sector should write");
    }

    fn synthetic_image() -> Vec<u8> {
        let mut bytes = blank_image();

        let mut bam = [0u8; SECTOR_SIZE];
        bam[0] = 18;
        bam[1] = 1;
        bam[0x90..0x98].copy_from_slice(b"DEMO DIS");
        bam[0x98] = b'K';
        bam[0xA2..0xA4].copy_from_slice(b"42");
        put_sector(&mut bytes, BAM_TRACK, BAM_SECTOR, &bam);

        let mut directory = [0u8; SECTOR_SIZE];
        directory[2] = 0x82;
        directory[3] = 1;
        directory[4] = 0;
        directory[5..10].copy_from_slice(b"HELLO");
        directory[30..32].copy_from_slice(&(1u16).to_le_bytes());
        put_sector(
            &mut bytes,
            DIRECTORY_TRACK,
            DIRECTORY_START_SECTOR,
            &directory,
        );

        let mut file_sector = [0u8; SECTOR_SIZE];
        file_sector[0] = 0;
        file_sector[1] = 6;
        file_sector[2..7].copy_from_slice(&[0x01, 0x08, 0x11, 0x22, 0x33]);
        put_sector(&mut bytes, 1, 0, &file_sector);

        bytes
    }

    #[test]
    fn parses_directory_from_synthetic_image() {
        let image = synthetic_image();
        let directory = parse_directory(&image).expect("synthetic D64 should parse");

        assert_eq!(directory.disk_name, "DEMO DISK");
        assert_eq!(directory.disk_id, "42");
        assert_eq!(directory.entries.len(), 1);
        assert_eq!(directory.entries[0].name, "HELLO");
        assert_eq!(directory.entries[0].file_type, D64FileType::Prg);
        assert_eq!(directory.entries[0].start_track, 1);
        assert_eq!(directory.entries[0].start_sector, 0);
        assert_eq!(directory.entries[0].blocks, 1);
        assert!(directory.entries[0].closed);
        assert!(!directory.entries[0].locked);
    }

    #[test]
    fn extracts_first_prg_from_synthetic_image() {
        let image = synthetic_image();
        let program = extract_first_prg(&image).expect("synthetic D64 should expose a PRG");

        assert_eq!(program.name, "HELLO");
        assert_eq!(program.blocks, 1);
        assert_eq!(program.data, vec![0x01, 0x08, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn exposes_track_geometry() {
        assert_eq!(sectors_in_track(1), Ok(21));
        assert_eq!(sectors_in_track(18), Ok(19));
        assert_eq!(sectors_in_track(25), Ok(18));
        assert_eq!(sectors_in_track(31), Ok(17));
        assert_eq!(
            sectors_in_track(36),
            Err(D64ParseError::InvalidTrack { track: 36 })
        );
    }

    #[test]
    fn reads_raw_decoded_sector_bytes() {
        let image = synthetic_image();
        let sector = read_sector(&image, 1, 0).expect("synthetic D64 sector should read");

        assert_eq!(&sector[..7], &[0x00, 0x06, 0x01, 0x08, 0x11, 0x22, 0x33]);
    }

    #[test]
    fn write_sector_round_trips_through_read_sector() {
        let mut image = blank_image();
        let mut data = [0u8; SECTOR_SIZE];
        data[0] = 0xDE;
        data[1] = 0xAD;
        data[SECTOR_SIZE - 1] = 0xBE;

        write_sector(&mut image, 17, 5, &data).expect("write should land on a valid sector");
        let read = read_sector(&image, 17, 5).expect("written sector should read back");

        assert_eq!(read, &data[..]);
    }

    #[test]
    fn write_sector_rejects_out_of_range_sector() {
        let mut image = blank_image();
        let data = [0u8; SECTOR_SIZE];
        // Track 31 has only 17 sectors (0..=16); sector 20 is out of range.
        let err = write_sector(&mut image, 31, 20, &data).expect_err("must reject bad sector");
        assert_eq!(
            err,
            D64ParseError::InvalidSector {
                track: 31,
                sector: 20
            }
        );
    }

    #[test]
    fn write_sector_rejects_unsupported_size() {
        let mut tiny = vec![0u8; 100];
        let data = [0u8; SECTOR_SIZE];
        let err = write_sector(&mut tiny, 1, 0, &data).expect_err("must reject bad image size");
        assert_eq!(err, D64ParseError::UnsupportedSize { actual: 100 });
    }

    #[test]
    fn rejects_unsupported_sizes() {
        let err = parse_directory(&[0; 123]).expect_err("bad D64 size must fail");
        assert_eq!(err, D64ParseError::UnsupportedSize { actual: 123 });
    }

    #[test]
    fn rejects_cyclic_file_chain() {
        let mut image = synthetic_image();
        let mut sector = [0u8; SECTOR_SIZE];
        sector[0] = 1;
        sector[1] = 0;
        put_sector(&mut image, 1, 0, &sector);

        let err = extract_first_prg(&image).expect_err("cyclic D64 chain must fail");
        assert_eq!(
            err,
            D64ParseError::CyclicFileChain {
                track: 1,
                sector: 0,
            }
        );
    }

    fn program(name: &str, len: usize) -> D64Program {
        D64Program {
            name: name.to_owned(),
            // Truncation is fine: this is only a fill pattern.
            #[allow(clippy::cast_possible_truncation)]
            data: (0..len).map(|i| (i * 7 + 1) as u8).collect(),
            blocks: 0,
        }
    }

    fn free_blocks(image: &[u8]) -> u32 {
        let bam = read_sector(image, BAM_TRACK, BAM_SECTOR).expect("bam");
        (1..=35u8)
            .filter(|&t| t != DIRECTORY_TRACK)
            .map(|t| u32::from(bam[usize::from(t) * 4]))
            .sum()
    }

    #[test]
    fn a_mastered_disk_reads_back_its_files() {
        let files = vec![
            program("FIRST", 2),
            program("Second 2", 254),
            program("THIRD", 255),
            program("BIG", 40_000),
        ];
        let image = master("TEST DISK", "AB", &files).expect("masters");
        assert_eq!(image.len(), D64_STANDARD_SIZE);
        let dir = parse_directory(&image).expect("directory");
        assert_eq!(dir.disk_name, "TEST DISK");
        assert_eq!(dir.disk_id, "AB");
        assert_eq!(dir.entries.len(), files.len());
        let expected_blocks = [1u16, 1, 2, 158];
        for ((entry, file), blocks) in dir.entries.iter().zip(&files).zip(expected_blocks) {
            assert_eq!(entry.name, file.name);
            assert_eq!(entry.file_type, D64FileType::Prg);
            assert!(entry.closed);
            assert_eq!(entry.blocks, blocks);
            let data =
                read_sector_chain(&image, entry.start_track, entry.start_sector).expect("chain");
            assert_eq!(data, file.data);
        }
        assert_eq!(free_blocks(&image), 664 - 162);
        assert_eq!(
            extract_first_prg(&image).expect("first").data,
            files[0].data
        );
    }

    #[test]
    fn a_long_directory_chains_on_track_eighteen() {
        let files: Vec<D64Program> = (0..20).map(|i| program(&format!("F{i}"), 10)).collect();
        let image = master("MANY", "01", &files).expect("masters");
        let dir = parse_directory(&image).expect("directory");
        assert_eq!(dir.entries.len(), 20);
        assert_eq!(dir.entries[19].name, "F19");
        let bam = read_sector(&image, BAM_TRACK, BAM_SECTOR).expect("bam");
        assert_eq!(
            bam[18 * 4],
            19 - 1 - 3,
            "BAM and three directory sectors used"
        );
    }

    #[test]
    fn an_empty_disk_has_every_block_free() {
        let image = master("", "00", &[]).expect("masters");
        assert!(
            parse_directory(&image)
                .expect("directory")
                .entries
                .is_empty()
        );
        assert_eq!(free_blocks(&image), 664);
    }

    #[test]
    fn mastering_rejects_what_cannot_be_written() {
        assert!(matches!(
            master("BAD!", "AB", &[]),
            Err(D64WriteError::InvalidName { .. })
        ));
        assert_eq!(master("OK", "A", &[]), Err(D64WriteError::InvalidId));
        assert_eq!(
            master("OK", "AB", &[program("HUGE", 664 * 254 + 1)]),
            Err(D64WriteError::DiskFull)
        );
        let many: Vec<D64Program> = (0..145).map(|_| program("X", 1)).collect();
        assert_eq!(master("OK", "AB", &many), Err(D64WriteError::DirectoryFull));
        for e in [
            D64WriteError::InvalidId,
            D64WriteError::DiskFull,
            D64WriteError::DirectoryFull,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
