//! WOZ 2, single-sided 5.25-inch bitstream images. No filesystem access.
//!
//! Decoding retains the whole file, including unknown chunks and track padding.
//! Encoding preserves it byte-for-byte unless track bytes are edited; a recorded
//! CRC is recomputed. Flux and 3.5-inch images return explicit unsupported errors.
//! Source: Applesauce WOZ 2.1, cited in README.md.

use std::{error::Error, fmt};

const SIGNATURE: &[u8; 8] = b"WOZ2\xff\x0a\x0d\x0a";

/// Invalid or unsupported input. Errors never contain file contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WozError {
    Signature,
    Checksum,
    Truncated,
    MissingChunk,
    DuplicateChunk,
    ChunkSize,
    InfoVersion,
    DiskType,
    Flux,
    TrackMap,
    TrackBounds,
    TrackOverlap,
    TooLarge,
}
impl fmt::Display for WozError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WOZ image: {self:?}")
    }
}
impl Error for WozError {}

/// Fields required by a 5.25-inch bitstream consumer. Other INFO bytes survive
/// parsing and encoding but are not interpreted by this library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub write_protected: bool,
    pub synchronized: bool,
    pub cleaned: bool,
    pub optimal_bit_timing: u8,
    pub boot_sector_format: u8,
}
impl Default for Info {
    fn default() -> Self {
        Self {
            write_protected: true,
            synchronized: false,
            cleaned: true,
            optimal_bit_timing: 32,
            boot_sector_format: 1,
        }
    }
}

/// Owned logical track. `bit_count` excludes unused tail bits and block padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub bits: Vec<u8>,
    pub bit_count: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct Location {
    start: usize,
    bytes: usize,
    bits: u32,
}

/// Validated image with lossless storage and shared quarter-track mappings.
#[derive(Debug, Clone)]
pub struct Woz {
    bytes: Vec<u8>,
    info: Info,
    map: [u8; 160],
    tracks: [Location; 160],
}

impl Woz {
    /// Decode and validate a WOZ2 bitstream image, including every allocated
    /// track, mapped or not. A zero CRC is accepted as specified.
    pub fn parse(bytes: &[u8]) -> Result<Self, WozError> {
        if bytes.len() < 12 {
            return Err(WozError::Truncated);
        }
        if &bytes[..8] != SIGNATURE {
            return Err(WozError::Signature);
        }
        let recorded = read32(bytes, 8);
        if recorded != 0 && crc32(&bytes[12..]) != recorded {
            return Err(WozError::Checksum);
        }
        let mut cursor = 12usize;
        let mut info_body = None;
        let mut map_body = None;
        let mut track_body = None;
        while cursor < bytes.len() {
            let header = bytes
                .get(cursor..cursor.checked_add(8).ok_or(WozError::TooLarge)?)
                .ok_or(WozError::Truncated)?;
            let size = read32(header, 4) as usize;
            let start = cursor + 8;
            let end = start.checked_add(size).ok_or(WozError::TooLarge)?;
            let body = bytes.get(start..end).ok_or(WozError::Truncated)?;
            if cursor == 12 && &header[..4] != b"INFO" {
                return Err(WozError::MissingChunk);
            }
            let entry = match &header[..4] {
                b"INFO" => Some(&mut info_body),
                b"TMAP" => Some(&mut map_body),
                b"TRKS" => Some(&mut track_body),
                b"FLUX" => return Err(WozError::Flux),
                _ => None,
            };
            if let Some(entry) = entry
                && entry.replace((start, body)).is_some()
            {
                return Err(WozError::DuplicateChunk);
            }
            cursor = end;
        }
        let (_, info) = info_body.ok_or(WozError::MissingChunk)?;
        let (_, map) = map_body.ok_or(WozError::MissingChunk)?;
        let (track_start, table) = track_body.ok_or(WozError::MissingChunk)?;
        if info.len() != 60 || map.len() != 160 || table.len() < 1280 {
            return Err(WozError::ChunkSize);
        }
        if info[0] < 2 {
            return Err(WozError::InfoVersion);
        }
        if info[1] != 1 || info[37] != 1 {
            return Err(WozError::DiskType);
        }
        if info[0] >= 3 && read16(info, 46) != 0 && read16(info, 48) != 0 {
            return Err(WozError::Flux);
        }
        if info[39] == 0 {
            return Err(WozError::TrackBounds);
        }
        let mut tracks = [Location::default(); 160];
        let mut ranges = Vec::new();
        for (index, track) in tracks.iter_mut().enumerate() {
            let d = &table[index * 8..index * 8 + 8];
            let block = usize::from(read16(d, 0));
            let blocks = usize::from(read16(d, 2));
            let bits = read32(d, 4);
            if block == 0 && blocks == 0 && bits == 0 {
                continue;
            }
            let start = block * 512;
            let end = start + blocks * 512;
            if blocks == 0
                || bits == 0
                || u64::from(bits) > (blocks as u64) * 4096
                || start < track_start + 1280
                || end > track_start + table.len()
            {
                return Err(WozError::TrackBounds);
            }
            if ranges.iter().any(|&(a, b)| start < b && a < end) {
                return Err(WozError::TrackOverlap);
            }
            ranges.push((start, end));
            *track = Location {
                start,
                bytes: (bits as usize).div_ceil(8),
                bits,
            };
        }
        for &entry in map {
            if entry != 255
                && (usize::from(entry) >= tracks.len() || tracks[usize::from(entry)].bits == 0)
            {
                return Err(WozError::TrackMap);
            }
        }
        let mut track_map = [255; 160];
        track_map.copy_from_slice(map);
        Ok(Self {
            bytes: bytes.to_vec(),
            info: Info {
                write_protected: info[2] != 0,
                synchronized: info[3] != 0,
                cleaned: info[4] != 0,
                optimal_bit_timing: info[39],
                boot_sector_format: info[38],
            },
            map: track_map,
            tracks,
        })
    }

    /// Build a canonical image, block-aligned, with a recorded CRC. Input track
    /// byte lengths must be exactly ceil(bit_count / 8). No bits are invented.
    pub fn from_tracks(info: Info, map: [u8; 160], tracks: &[Track]) -> Result<Self, WozError> {
        if tracks.len() > 160 {
            return Err(WozError::TooLarge);
        }
        let mut bytes = vec![0; 1536];
        bytes[..8].copy_from_slice(SIGNATURE);
        bytes[12..16].copy_from_slice(b"INFO");
        bytes[16..20].copy_from_slice(&60u32.to_le_bytes());
        bytes[20] = 2;
        bytes[21] = 1;
        bytes[22] = u8::from(info.write_protected);
        bytes[23] = u8::from(info.synchronized);
        bytes[24] = u8::from(info.cleaned);
        bytes[25..57].fill(b' ');
        bytes[25..35].copy_from_slice(b"Format198x");
        bytes[57] = 1;
        bytes[58] = info.boot_sector_format;
        bytes[59] = info.optimal_bit_timing;
        bytes[80..84].copy_from_slice(b"TMAP");
        bytes[84..88].copy_from_slice(&160u32.to_le_bytes());
        bytes[88..248].copy_from_slice(&map);
        bytes[248..252].copy_from_slice(b"TRKS");
        let mut largest = 0u16;
        for (i, track) in tracks.iter().enumerate() {
            if track.bit_count == 0 || track.bits.len() != (track.bit_count as usize).div_ceil(8) {
                return Err(WozError::TrackBounds);
            }
            let start = u16::try_from(bytes.len() / 512).map_err(|_| WozError::TooLarge)?;
            let blocks =
                u16::try_from(track.bits.len().div_ceil(512)).map_err(|_| WozError::TooLarge)?;
            largest = largest.max(blocks);
            let d = 256 + i * 8;
            bytes[d..d + 2].copy_from_slice(&start.to_le_bytes());
            bytes[d + 2..d + 4].copy_from_slice(&blocks.to_le_bytes());
            bytes[d + 4..d + 8].copy_from_slice(&track.bit_count.to_le_bytes());
            let offset = bytes.len();
            bytes.resize(offset + usize::from(blocks) * 512, 0);
            bytes[offset..offset + track.bits.len()].copy_from_slice(&track.bits);
        }
        bytes[64..66].copy_from_slice(&largest.to_le_bytes());
        let length = u32::try_from(bytes.len() - 256).map_err(|_| WozError::TooLarge)?;
        bytes[252..256].copy_from_slice(&length.to_le_bytes());
        let crc = crc32(&bytes[12..]);
        bytes[8..12].copy_from_slice(&crc.to_le_bytes());
        Self::parse(&bytes)
    }

    pub fn info(&self) -> Info {
        self.info
    }
    pub fn track_map(&self) -> &[u8; 160] {
        &self.map
    }
    /// Logical track bytes, MSB first, and exact wrap bit count.
    pub fn track(&self, index: usize) -> Option<(&[u8], u32)> {
        let location = self.tracks.get(index)?;
        if location.bits == 0 {
            return None;
        }
        Some((
            &self.bytes[location.start..location.start + location.bytes],
            location.bits,
        ))
    }
    /// Edit bits in the existing allocation. Track size, mappings, padding and
    /// all unknown chunks are preserved; this does not touch a source file.
    pub fn track_mut(&mut self, index: usize) -> Option<&mut [u8]> {
        let location = self.tracks.get(index)?;
        if location.bits == 0 {
            return None;
        }
        Some(&mut self.bytes[location.start..location.start + location.bytes])
    }
    /// Encode losslessly, recomputing an existing nonzero checksum.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.bytes.clone();
        if read32(&bytes, 8) != 0 {
            let crc = crc32(&bytes[12..]);
            bytes[8..12].copy_from_slice(&crc.to_le_bytes());
        }
        bytes
    }
}

fn read16(b: &[u8], p: usize) -> u16 {
    u16::from_le_bytes([b[p], b[p + 1]])
}
fn read32(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}
/// IEEE CRC32 as used by WOZ, initial and final XOR all ones.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> Woz {
        let mut map = [255; 160];
        map[..3].fill(0);
        Woz::from_tracks(
            Info::default(),
            map,
            &[Track {
                bits: vec![0x81, 0x80],
                bit_count: 9,
            }],
        )
        .expect("valid image")
    }
    #[test]
    fn canonical_crc_and_layout_match_specified_offsets() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        let image = image();
        let bytes = image.to_bytes();
        assert_eq!(&bytes[..8], b"WOZ2\xff\x0a\x0d\x0a");
        assert_eq!(&bytes[256..264], &[3, 0, 1, 0, 9, 0, 0, 0]);
        assert_eq!(&bytes[1536..1538], &[0x81, 0x80]);
        assert_eq!(bytes.len(), 2048);
        assert_eq!(image.track_map()[2], 0);
        assert_eq!(image.track(0), Some((&[0x81, 0x80][..], 9)));
        assert!(image.track(1).is_none());
        assert!(image.track(160).is_none());
        assert_eq!(image.info(), Info::default());
        assert_eq!(Woz::parse(&bytes).expect("round trip").to_bytes(), bytes);
    }
    #[test]
    fn every_truncated_prefix_and_corrupt_checksum_fails() {
        let mut bytes = image().to_bytes();
        bytes[8..12].fill(0);
        for n in 0..bytes.len() {
            assert!(Woz::parse(&bytes[..n]).is_err(), "prefix {n}");
        }
        let mut bytes = image().to_bytes();
        bytes[1536] ^= 1;
        assert_eq!(Woz::parse(&bytes).expect_err("bad crc"), WozError::Checksum);
    }
    #[test]
    fn invalid_maps_allocations_chunks_and_unsupported_surfaces_fail() {
        let cases: &[(usize, &[u8], WozError)] = &[
            (0, b"WOZ1", WozError::Signature),
            (20, &[1], WozError::InfoVersion),
            (21, &[2], WozError::DiskType),
            (57, &[2], WozError::DiskType),
            (59, &[0], WozError::TrackBounds),
            (88, &[160], WozError::TrackMap),
            (88, &[1], WozError::TrackMap),
            (256, &[2, 0], WozError::TrackBounds),
            (258, &[0, 0], WozError::TrackBounds),
            (260, &[0, 0, 0, 0], WozError::TrackBounds),
            (260, &[1, 16, 0, 0], WozError::TrackBounds),
            (16, &[59, 0, 0, 0], WozError::Truncated),
            (12, b"nope", WozError::MissingChunk),
        ];
        for &(offset, value, error) in cases {
            let mut bytes = image().to_bytes();
            bytes[8..12].fill(0);
            bytes[offset..offset + value.len()].copy_from_slice(value);
            assert_eq!(
                Woz::parse(&bytes).expect_err("invalid"),
                error,
                "offset {offset}"
            );
        }
        let mut bytes = image().to_bytes();
        bytes[8..12].fill(0);
        let descriptor = bytes[256..264].to_vec();
        bytes[264..272].copy_from_slice(&descriptor);
        assert_eq!(
            Woz::parse(&bytes).expect_err("overlap"),
            WozError::TrackOverlap
        );
        let mut bytes = image().to_bytes();
        bytes[8..12].fill(0);
        bytes[20] = 3;
        bytes[66] = 4;
        bytes[68] = 1;
        assert_eq!(Woz::parse(&bytes).expect_err("flux"), WozError::Flux);
        let mut bytes = image().to_bytes();
        bytes[8..12].fill(0);
        bytes.extend(b"FLUX\0\0\0\0");
        assert_eq!(Woz::parse(&bytes).expect_err("flux chunk"), WozError::Flux);
        let mut bytes = image().to_bytes();
        bytes[8..12].fill(0);
        bytes.extend(b"INFO\0\0\0\0");
        assert_eq!(
            Woz::parse(&bytes).expect_err("duplicate"),
            WozError::DuplicateChunk
        );
        assert_eq!(
            Woz::parse(&image().to_bytes()[..248]).expect_err("no tracks"),
            WozError::Checksum
        );
    }
    #[test]
    fn edits_preserve_opaque_chunks_padding_and_source_bytes() {
        let mut source = image().to_bytes();
        source[8..12].fill(0);
        source[2047] = 0x6e;
        source.extend(b"TEST\x03\0\0\0abc");
        let original = source.clone();
        let mut image = Woz::parse(&source).expect("opaque chunk");
        assert_eq!(image.to_bytes(), source);
        image.track_mut(0).expect("track")[0] = 0x55;
        assert!(image.track_mut(1).is_none());
        assert!(image.track_mut(160).is_none());
        let encoded = image.to_bytes();
        assert_eq!(encoded[1536], 0x55);
        assert_eq!(encoded[2047], 0x6e);
        assert_eq!(&encoded[2048..], b"TEST\x03\0\0\0abc");
        assert_eq!(source, original);
        assert_eq!(
            Woz::parse(&encoded).expect("edited").track(0),
            Some((&[0x55, 0x80][..], 9))
        );
        let mut checksummed = Woz::parse(&original).expect("image");
        checksummed.bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        checksummed.track_mut(0).expect("track")[0] = 0x33;
        assert!(Woz::parse(&checksummed.to_bytes()).is_ok());
    }
    #[test]
    fn builder_rejects_inconsistent_tracks_and_maps() {
        assert_eq!(
            Woz::from_tracks(Info::default(), [0; 160], &[]).expect_err("missing track"),
            WozError::TrackMap
        );
        for track in [
            Track {
                bits: vec![0],
                bit_count: 0,
            },
            Track {
                bits: vec![0],
                bit_count: 9,
            },
        ] {
            assert_eq!(
                Woz::from_tracks(Info::default(), [0; 160], &[track]).expect_err("bad track"),
                WozError::TrackBounds
            );
        }
        let tracks = vec![
            Track {
                bits: vec![0xff],
                bit_count: 8
            };
            161
        ];
        assert_eq!(
            Woz::from_tracks(Info::default(), [0; 160], &tracks).expect_err("too many tracks"),
            WozError::TooLarge
        );
    }
}
