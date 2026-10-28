//! Opt-in bounded container view retaining physical track-header metadata.
use crate::{DiskSector, parse_track_impl};

/// Physical metadata recorded in a Track Information Block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackHeader {
    pub track: u8,
    pub side: u8,
    pub data_rate: u8,
    pub recording_mode: u8,
    pub size_code: u8,
    pub gap3: u8,
    pub filler: u8,
}

/// One slot in container order, with sector order unchanged.
#[derive(Clone, Debug)]
pub struct ContainerTrack {
    /// None for an absent EDSK slot or an entirely zeroed track header.
    /// A present header with no sectors remains Some, rather than disappearing.
    pub header: Option<TrackHeader>,
    pub sectors: Vec<DiskSector>,
}

/// A checked container, independent of controller geometry or state.
#[derive(Clone, Debug)]
pub struct Container {
    pub tracks_per_side: u8,
    pub sides: u8,
    /// Interleaved slots: track 0 side 0, track 0 side 1, track 1 side 0, ...
    /// Physical TIB C/H are retained separately; they need not match the slot.
    pub tracks: Vec<ContainerTrack>,
}

/// Decode a bounded DSK/EDSK container without discarding physical TIB metadata.
///
/// Descriptor tables must fit the 256-byte TIB; all payloads must fit their
/// declared track extents. Standard DSK uses the track size code without
/// clamping and ignores the EDSK-only stored-length bytes. Extended sectors
/// retain their exact stored length, including zero, independently of N.
///
/// Empty/zeroed slots are preserved. Duplicate or unusual address marks and
/// status flags remain data: the controller adapter decides representability.
/// This is not a lossless container editor: creator bytes and padding are not
/// retained. Existing [`crate::parse`] keeps its historical compatibility policy.
///
/// Layout authority: umbrella reference/by-system/amstrad-cpc/cpcemu-dsk-container.md.
///
/// # Errors
/// Rejects invalid signatures/dimensions and out-of-bounds tables or extents.
pub fn parse_container(data: &[u8]) -> Result<Container, String> {
    let header = data.get(..256).ok_or("DSK file shorter than disk header")?;
    let extended = if header.starts_with(b"EXTENDED CPC DSK File") {
        true
    } else if header.starts_with(b"MV - CPC") {
        false
    } else {
        return Err("not a CPCEMU DSK container".into());
    };
    let tracks_per_side = header[0x30];
    let sides = header[0x31];
    if tracks_per_side == 0 || !(1..=2).contains(&sides) {
        return Err("DSK requires nonzero tracks and one or two sides".into());
    }
    let count = usize::from(tracks_per_side) * usize::from(sides);
    let table = if extended {
        Some(
            header
                .get(0x34..0x34 + count)
                .ok_or("EDSK table exceeds disk header")?,
        )
    } else {
        None
    };
    let standard_length = usize::from(u16::from_le_bytes([header[0x32], header[0x33]]));
    let mut tracks = Vec::with_capacity(count);
    let mut cursor = 256usize;
    for slot in 0..count {
        let length = table.map_or(standard_length, |table| usize::from(table[slot]) * 256);
        if extended && length == 0 {
            tracks.push(ContainerTrack {
                header: None,
                sectors: Vec::new(),
            });
            continue;
        }
        if length < 256 {
            return Err(format!("track slot {slot} is shorter than its header"));
        }
        let end = cursor
            .checked_add(length)
            .ok_or("DSK track extent overflow")?;
        let block = data
            .get(cursor..end)
            .ok_or_else(|| format!("track slot {slot} exceeds file"))?;
        cursor = end;
        let tib = &block[..256];
        if tib.iter().all(|&byte| byte == 0) {
            tracks.push(ContainerTrack {
                header: None,
                sectors: Vec::new(),
            });
            continue;
        }
        let decoded = parse_track_impl(block, extended, true)
            .map_err(|error| format!("track slot {slot}: {error}"))?;
        tracks.push(ContainerTrack {
            header: Some(TrackHeader {
                track: tib[0x10],
                side: tib[0x11],
                data_rate: tib[0x12],
                recording_mode: tib[0x13],
                size_code: tib[0x14],
                gap3: tib[0x16],
                filler: tib[0x17],
            }),
            sectors: decoded.sectors,
        });
    }
    Ok(Container {
        tracks_per_side,
        sides,
        tracks,
    })
}
