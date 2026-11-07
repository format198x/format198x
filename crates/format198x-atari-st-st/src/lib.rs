//! Atari ST `.st` floppy images.
//!
//! A `.st` file is a raw dump of a floppy's sectors, 512 bytes each, in the
//! order track 0 side 0, track 0 side 1, track 1 side 0, and so on (single
//! sided disks have only side 0). It has no header, so the geometry must be
//! inferred:
//!
//! 1. From the boot sector's BIOS parameter block, which TOS writes when it
//!    formats a disk: bytes per sector at `$0B`, total sectors at `$13`,
//!    sectors per track at `$18` and sides at `$1A`, all little-endian. It
//!    is used only when it describes exactly the image's sectors: games and
//!    demos often carry a boot sector that does not match their disk.
//! 2. Failing that, from the image size: 80 to 84 tracks of 9 to 12
//!    sectors first, then any other track count up to 86.
//! 3. Failing that, from the boot sector if its disk fits in the image.
//!
//! Hatari's `floppy.c` (GPL-2.0-or-later, read for facts only) settles the
//! same question the same way: the image size wins over a boot sector that
//! disagrees with it.
//!
//! Nothing is dropped. Bytes past the geometry are kept in
//! [`StImage::trailing`], and [`StImage::to_bytes`] writes the file back
//! unchanged.
//!
//! This crate reads and writes the layout only. Turning it into a disk a
//! floppy controller can read is the emulator's business.

#![forbid(unsafe_code)]

use std::fmt;

/// Bytes per sector on every ST format this crate accepts.
pub const SECTOR_SIZE: usize = 512;

/// A disk's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub tracks: usize,
    pub sides: usize,
    pub sectors_per_track: usize,
}

impl Geometry {
    /// Bytes the geometry describes.
    #[must_use]
    pub fn size(&self) -> usize {
        self.tracks * self.sides * self.sectors_per_track * SECTOR_SIZE
    }
}

/// A parsed `.st` image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StImage {
    pub geometry: Geometry,
    /// The sector data, exactly [`Geometry::size`] bytes.
    pub data: Vec<u8>,
    /// Bytes after the last sector the geometry covers, kept so the file
    /// writes back unchanged.
    pub trailing: Vec<u8>,
}

impl StImage {
    /// The file: the sectors, then any trailing bytes.
    ///
    /// # Errors
    ///
    /// [`StError::DataLength`] if [`StImage::data`] is not the size its
    /// geometry describes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, StError> {
        let expected = self.geometry.size();
        if self.data.len() != expected {
            return Err(StError::DataLength {
                expected,
                actual: self.data.len(),
            });
        }
        let mut out = Vec::with_capacity(self.data.len() + self.trailing.len());
        out.extend_from_slice(&self.data);
        out.extend_from_slice(&self.trailing);
        Ok(out)
    }
}

/// Why an image could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StError {
    /// No whole sector, or not a size any known geometry explains.
    UnknownSize(usize),
    /// Sector data that is not the size its geometry describes.
    DataLength {
        /// Bytes the geometry describes.
        expected: usize,
        /// Bytes the image holds.
        actual: usize,
    },
}

impl fmt::Display for StError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSize(size) => write!(
                f,
                "{size} bytes is not a recognisable .st image (no usable boot sector \
                 geometry and not a standard floppy size)"
            ),
            Self::DataLength { expected, actual } => write!(
                f,
                "an .st image of this geometry holds {expected} bytes of sectors, not {actual}"
            ),
        }
    }
}

impl std::error::Error for StError {}

fn le16(bytes: &[u8], offset: usize) -> usize {
    usize::from(bytes[offset]) | usize::from(bytes[offset + 1]) << 8
}

/// The geometry the boot sector's BIOS parameter block gives, if it is
/// plausible and fits the image.
fn bpb_geometry(data: &[u8]) -> Option<Geometry> {
    if data.len() < SECTOR_SIZE {
        return None;
    }
    let bytes_per_sector = le16(data, 0x0B);
    let total = le16(data, 0x13);
    let sectors_per_track = le16(data, 0x18);
    let sides = le16(data, 0x1A);
    if bytes_per_sector != SECTOR_SIZE
        || !(1..=2).contains(&sides)
        || !(8..=12).contains(&sectors_per_track)
        || total == 0
        || !total.is_multiple_of(sides * sectors_per_track)
    {
        return None;
    }
    let geometry = Geometry {
        tracks: total / (sides * sectors_per_track),
        sides,
        sectors_per_track,
    };
    (1..=86).contains(&geometry.tracks).then_some(geometry)
}

/// A geometry that covers exactly `size` bytes, if any: 80 to 84 tracks of
/// 9 to 12 sectors first, then any other track count up to 86.
fn size_geometry(size: usize) -> Option<Geometry> {
    let standard = 80..=84;
    let others = (1..80).rev().chain(85..=86);
    standard.chain(others).find_map(|tracks| {
        [9, 10, 11, 12].into_iter().find_map(|sectors_per_track| {
            [2, 1].into_iter().find_map(|sides| {
                let geometry = Geometry {
                    tracks,
                    sides,
                    sectors_per_track,
                };
                (geometry.size() == size).then_some(geometry)
            })
        })
    })
}

/// Parse a `.st` image.
///
/// # Errors
///
/// [`StError::UnknownSize`] when neither the boot sector nor the size gives a
/// geometry.
pub fn parse(bytes: &[u8]) -> Result<StImage, StError> {
    let whole = bytes.len() - bytes.len() % SECTOR_SIZE;
    let bpb = bpb_geometry(bytes);
    let geometry = bpb
        .filter(|g| g.size() == whole)
        .or_else(|| size_geometry(whole))
        .or_else(|| bpb.filter(|g| g.size() <= bytes.len()))
        .ok_or(StError::UnknownSize(bytes.len()))?;
    let (data, trailing) = bytes.split_at(geometry.size());
    Ok(StImage {
        geometry,
        data: data.to_vec(),
        trailing: trailing.to_vec(),
    })
}

/// A blank, unformatted image of `geometry`: every byte zero.
#[must_use]
pub fn blank(geometry: Geometry) -> StImage {
    StImage {
        geometry,
        data: vec![0; geometry.size()],
        trailing: Vec::new(),
    }
}

/// The README's examples, run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(test)]
mod tests {
    use super::*;

    fn with_bpb(total: u16, spt: u16, sides: u16, size: usize) -> Vec<u8> {
        let mut data = vec![0u8; size];
        data[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        data[0x13..0x15].copy_from_slice(&total.to_le_bytes());
        data[0x18..0x1A].copy_from_slice(&spt.to_le_bytes());
        data[0x1A..0x1C].copy_from_slice(&sides.to_le_bytes());
        data
    }

    #[test]
    fn the_boot_sector_gives_the_geometry() {
        let data = with_bpb(1600, 10, 2, 819_200);
        let image = parse(&data).expect("parses");
        assert_eq!(
            image.geometry,
            Geometry {
                tracks: 80,
                sides: 2,
                sectors_per_track: 10
            }
        );
        assert_eq!(image.data.len(), 819_200);
    }

    #[test]
    fn a_blank_boot_sector_falls_back_to_the_size() {
        assert_eq!(
            parse(&vec![0; 737_280]).expect("720 KB").geometry,
            Geometry {
                tracks: 80,
                sides: 2,
                sectors_per_track: 9
            }
        );
        assert_eq!(parse(&vec![0; 368_640]).expect("360 KB").geometry.sides, 1);
    }

    #[test]
    fn a_bpb_bigger_than_the_file_is_ignored_and_trailing_bytes_kept() {
        let mut data = with_bpb(1600, 10, 2, 737_280);
        let image = parse(&data).expect("falls back to the size");
        assert_eq!(image.geometry.sectors_per_track, 9);
        data.extend([1, 2, 3]);
        let image = parse(&data).expect("still parses");
        assert_eq!(image.data.len(), 737_280);
        assert_eq!(image.trailing, [1, 2, 3]);
        assert_eq!(image.to_bytes().expect("writes"), data);
    }

    #[test]
    fn the_size_wins_over_a_boot_sector_that_describes_less_of_the_disk() {
        // A single-sided 80-track boot sector on an image that is exactly
        // 82 tracks, two sides, 10 sectors: the size describes the disk.
        let data = with_bpb(800, 10, 1, 82 * 2 * 10 * SECTOR_SIZE);
        let image = parse(&data).expect("parses");
        assert_eq!(
            image.geometry,
            Geometry {
                tracks: 82,
                sides: 2,
                sectors_per_track: 10
            }
        );
        assert!(image.trailing.is_empty());
    }

    #[test]
    fn a_boot_sector_that_fits_is_used_when_the_size_explains_nothing() {
        // Two stray sectors after a 720K disk.
        let data = with_bpb(1440, 9, 2, 737_280 + 2 * SECTOR_SIZE);
        let image = parse(&data).expect("parses");
        assert_eq!(image.geometry.size(), 737_280);
        assert_eq!(image.trailing.len(), 2 * SECTOR_SIZE);
    }

    #[test]
    fn short_track_counts_are_found_by_size() {
        let image = parse(&vec![0; 79 * 9 * SECTOR_SIZE]).expect("79 tracks");
        assert_eq!(
            image.geometry,
            Geometry {
                tracks: 79,
                sides: 1,
                sectors_per_track: 9
            }
        );
    }

    #[test]
    fn data_of_the_wrong_size_is_not_written() {
        let mut image = parse(&vec![0; 368_640]).expect("360 KB");
        image.data.pop();
        assert_eq!(
            image.to_bytes(),
            Err(StError::DataLength {
                expected: 368_640,
                actual: 368_639
            })
        );
        assert!(!image.to_bytes().expect_err("short").to_string().is_empty());
    }

    #[test]
    fn an_odd_size_with_no_bpb_is_rejected() {
        let error = parse(&[0; 1000]).expect_err("rejected");
        assert_eq!(error, StError::UnknownSize(1000));
        assert!(error.to_string().contains("1000"));
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn blank_images_are_zeroed_to_size() {
        let g = Geometry {
            tracks: 80,
            sides: 1,
            sectors_per_track: 9,
        };
        assert_eq!(blank(g).data.len(), 368_640);
    }
}
