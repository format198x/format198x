//! Sector-model encoding, not a byte-preserving rewrite of an original dump.
//!
//! Layout: umbrella `reference/by-system/amstrad-cpc/cpcemu-dsk-container.md`,
//! citing the format authors' Rev.5 specification:
//! <https://cpctech.cpcwiki.de/docs/extdsk.html>.

use crate::{DiskImage, DiskTrack, HEADER_LEN, SECTOR_INFO_LEN, TRACK_HEADER_LEN};

const TABLE_OFFSET: usize = 0x34;
const SECTOR_OFFSET: usize = 0x18;
const MAX_ENTRIES: usize = HEADER_LEN - TABLE_OFFSET;
const MAX_SECTORS: usize = (TRACK_HEADER_LEN - SECTOR_OFFSET) / SECTOR_INFO_LEN;
const MAX_TRACK_BYTES: usize = 255 * 256;

/// Encode the represented sectors as an Extended DSK image.
///
/// Preserves physical track/head placement, sector order (including duplicate
/// IDs), CHRN, ST1/ST2, and every stored byte, including zero-length sectors.
/// Empty tracks become unformatted tracks with no stored block. Output is
/// deterministic; the input is borrowed and unchanged. No file is written.
///
/// This is not a lossless container rewrite: the model has no original creator,
/// gap/filler, data-rate/recording-mode or padding fields. The encoder supplies
/// a creator, gap `0x4e`, filler `0xe5`, unknown rate/mode and zero padding.
/// Weak-sector copies remain uninterpreted stored bytes, not emulation logic.
///
/// ```
/// use format198x_amstrad_cpc_dsk::{DiskImage, DiskTrack, encode_extended, parse};
/// let image = DiskImage {
///     sides: 1,
///     tracks_per_side: 1,
///     tracks: vec![vec![DiskTrack::default()]],
/// };
/// let bytes = encode_extended(&image)?;
/// assert!(parse(&bytes)?.tracks[0][0].sectors.is_empty());
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// Returns an error for inconsistent geometry, more than 204 track/head table
/// entries, more than 29 sectors per track, stored sector lengths exceeding
/// 65535 bytes, or padded track blocks exceeding 65280 bytes. Nothing is
/// truncated, reordered or coerced to a nominal size derived from N.
pub fn encode_extended(image: &DiskImage) -> Result<Vec<u8>, String> {
    if !(1..=2).contains(&image.sides) || image.tracks_per_side == 0 {
        return Err("EDSK requires one or two sides and at least one track".into());
    }
    let entries = usize::from(image.sides) * usize::from(image.tracks_per_side);
    if entries > MAX_ENTRIES {
        return Err("EDSK track size table exceeds the 256-byte disk header".into());
    }
    if image.tracks.len() != usize::from(image.sides)
        || image
            .tracks
            .iter()
            .any(|side| side.len() != usize::from(image.tracks_per_side))
    {
        return Err("EDSK model dimensions differ from declared geometry".into());
    }

    // Validate the whole model before allocating the output. Page counts are
    // already bounded before narrowing; aggregate output is at most ~13 MiB.
    let mut lengths = Vec::with_capacity(entries);
    for cylinder in 0..image.tracks_per_side {
        for side in &image.tracks {
            lengths.push(track_length(&side[usize::from(cylinder)])?);
        }
    }
    let mut bytes = vec![0; HEADER_LEN + lengths.iter().sum::<usize>()];
    bytes[..34].copy_from_slice(b"EXTENDED CPC DSK File\r\nDisk-Info\r\n");
    bytes[0x22..0x2c].copy_from_slice(b"Format198x");
    bytes[0x30] = image.tracks_per_side;
    bytes[0x31] = image.sides;
    let mut cursor = HEADER_LEN;
    for (entry, length) in lengths.into_iter().enumerate() {
        bytes[TABLE_OFFSET + entry] = (length / 256) as u8;
        if length == 0 {
            continue;
        }
        let cylinder = entry / usize::from(image.sides);
        let side = entry % usize::from(image.sides);
        let track = &image.tracks[side][cylinder];
        let block = &mut bytes[cursor..cursor + length];
        block[..12].copy_from_slice(b"Track-Info\r\n");
        block[0x10] = cylinder as u8;
        block[0x11] = side as u8;
        block[0x14] = track.sectors[0].n;
        block[0x15] = track.sectors.len() as u8;
        block[0x16] = 0x4e;
        block[0x17] = 0xe5;
        let mut data_cursor = TRACK_HEADER_LEN;
        for (index, sector) in track.sectors.iter().enumerate() {
            let offset = SECTOR_OFFSET + index * SECTOR_INFO_LEN;
            block[offset..offset + 6].copy_from_slice(&[
                sector.c, sector.h, sector.id, sector.n, sector.st1, sector.st2,
            ]);
            block[offset + 6..offset + 8]
                .copy_from_slice(&(sector.data.len() as u16).to_le_bytes());
            block[data_cursor..data_cursor + sector.data.len()].copy_from_slice(&sector.data);
            data_cursor += sector.data.len();
        }
        cursor += length;
    }
    Ok(bytes)
}

fn track_length(track: &DiskTrack) -> Result<usize, String> {
    if track.sectors.is_empty() {
        return Ok(0);
    }
    if track.sectors.len() > MAX_SECTORS {
        return Err("EDSK sector information exceeds the 256-byte track header".into());
    }
    let mut length = TRACK_HEADER_LEN;
    for sector in &track.sectors {
        if sector.data.len() > usize::from(u16::MAX) {
            return Err("EDSK sector data length exceeds 65535 bytes".into());
        }
        // At most 29 lengths of at most 65535 bytes, so this cannot overflow.
        length += sector.data.len();
    }
    if length > MAX_TRACK_BYTES {
        return Err("EDSK track block exceeds 255 pages of 256 bytes".into());
    }
    Ok(length.div_ceil(256) * 256)
}
