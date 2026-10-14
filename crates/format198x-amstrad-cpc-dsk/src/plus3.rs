//! Standard 180 KiB +3DOS filesystems in CPCEMU DSK containers.
//!
//! Sources: Amstrad ZX Spectrum +3 manual, pp213–217 (file header, disk
//! specification and XDPB); CPCEMU documentation §7.7.1 (DSK container).
//! CP/M extents use 128-byte records, sixteen one-byte allocation pointers,
//! 1024-byte blocks and a 64-entry directory on this specific format.
//! This writer does not emit boot-sector code, EDSK or other geometries.

use std::collections::BTreeSet;

/// A named file containing its complete +3DOS header and payload.
#[derive(Clone, Copy, Debug)]
pub struct File<'a> {
    /// Uppercase CP/M 8.3 filename, such as `DISK` or `DIAG.BIN`.
    pub name: &'a str,
    /// Complete file bytes; use [`basic`] or [`code`] for +3DOS framing.
    pub bytes: &'a [u8],
}

/// Frame tokenised BASIC without variables, autorunning at the given line.
///
/// # Errors
/// Rejects empty/oversized programs and autorun values outside BASIC line range.
pub fn basic(program: &[u8], autorun: u16) -> Result<Vec<u8>, String> {
    if autorun > 9999 {
        return Err("BASIC autorun line must be in 0..=9999".into());
    }
    let length = u16::try_from(program.len()).map_err(|_| "BASIC program exceeds 65535 bytes")?;
    frame(program, 0, autorun, length)
}

/// Frame CODE with its explicit RAM load address.
///
/// # Errors
/// Rejects empty data, ROM placement and address-space overflow.
pub fn code(payload: &[u8], load: u16) -> Result<Vec<u8>, String> {
    if load < 0x4000 || payload.len() > 0x10000 - usize::from(load) {
        return Err("CODE must fit in RAM without address overflow".into());
    }
    frame(payload, 3, load, 0)
}

fn frame(data: &[u8], kind: u8, param1: u16, param2: u16) -> Result<Vec<u8>, String> {
    let length = u16::try_from(data.len()).map_err(|_| "file payload exceeds 65535 bytes")?;
    if length == 0 {
        return Err("file payload is empty".into());
    }
    let mut bytes = vec![0; 128];
    bytes[..9].copy_from_slice(b"PLUS3DOS\x1a");
    bytes[9] = 1; // issue 1, version 0
    bytes[11..15].copy_from_slice(&(u32::from(length) + 128).to_le_bytes());
    bytes[15] = kind;
    bytes[16..18].copy_from_slice(&length.to_le_bytes());
    bytes[18..20].copy_from_slice(&param1.to_le_bytes());
    bytes[20..22].copy_from_slice(&param2.to_le_bytes());
    bytes[127] = bytes[..127].iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
    bytes.extend_from_slice(data);
    Ok(bytes)
}

fn filename(name: &str) -> Result<[u8; 11], String> {
    let (stem, extension) = name.split_once('.').unwrap_or((name, ""));
    let valid = |s: &str| {
        s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    };
    if stem.is_empty()
        || stem.len() > 8
        || extension.len() > 3
        || !valid(stem)
        || !valid(extension)
        || (name.contains('.') && extension.is_empty())
    {
        return Err(format!("invalid uppercase 8.3 filename: {name}"));
    }
    let mut result = [b' '; 11];
    result[..stem.len()].copy_from_slice(stem.as_bytes());
    result[8..8 + extension.len()].copy_from_slice(extension.as_bytes());
    Ok(result)
}

/// Master files in caller order onto a fresh standard 40-track, single-sided disk.
///
/// No existing disk is edited. Files belong to CP/M user 0; timestamps and host
/// metadata are absent. Unused disk bytes are $E5. The reserved first track has
/// a type-0 disk specification and deliberately fails the executable-boot sum.
///
/// # Errors
/// Rejects invalid/duplicate names, empty files, directory or allocation overflow.
pub fn master(files: &[File<'_>]) -> Result<Vec<u8>, String> {
    if files.is_empty() {
        return Err("at least one file is required".into());
    }
    let mut disk = vec![0xe5; 40 * 9 * 512];
    disk[..16].copy_from_slice(&[0, 0, 40, 9, 2, 1, 3, 2, 42, 82, 0, 0, 0, 0, 0, 0]);
    // Boot-ROM execution is selected by sum == 3. Keep this a data disk even
    // if a later specification/filler edit would accidentally satisfy it.
    if disk[..512].iter().fold(0u8, |sum, b| sum.wrapping_add(*b)) == 3 {
        return Err("data disk unexpectedly has an executable boot checksum".into());
    }
    let mut names = BTreeSet::new();
    let (mut block, mut entry) = (2usize, 0usize);
    for file in files {
        let name = filename(file.name)?;
        if !names.insert(name) {
            return Err(format!("duplicate filename: {}", file.name));
        }
        if file.bytes.is_empty() {
            return Err(format!("empty file: {}", file.name));
        }
        for (extent, bytes) in file.bytes.chunks(16 * 1024).enumerate() {
            let blocks = bytes.len().div_ceil(1024);
            if entry >= 64 || block + blocks > 175 {
                return Err("standard +3 disk capacity exceeded".into());
            }
            let offset = 9 * 512 + entry * 32;
            disk[offset..offset + 32].fill(0);
            disk[offset + 1..offset + 12].copy_from_slice(&name);
            disk[offset + 12] = u8::try_from(extent).map_err(|_| "extent overflow")?;
            disk[offset + 15] =
                u8::try_from(bytes.len().div_ceil(128)).map_err(|_| "record overflow")?;
            for index in 0..blocks {
                disk[offset + 16 + index] =
                    u8::try_from(block + index).map_err(|_| "block overflow")?;
            }
            let start = 9 * 512 + block * 1024;
            disk[start..start + bytes.len()].copy_from_slice(bytes);
            block += blocks;
            entry += 1;
        }
    }
    let track_size = 256 + 9 * 512;
    let mut image = vec![0; 256 + 40 * track_size];
    image[..34].copy_from_slice(b"MV - CPCEMU Disk-File\r\nDisk-Info\r\n");
    image[0x30] = 40;
    image[0x31] = 1;
    image[0x32..0x34].copy_from_slice(&0x1300u16.to_le_bytes());
    for track in 0..40u8 {
        let start = 256 + usize::from(track) * track_size;
        image[start..start + 12].copy_from_slice(b"Track-Info\r\n");
        image[start + 0x10] = track;
        image[start + 0x14..start + 0x18].copy_from_slice(&[2, 9, 82, 0xe5]);
        for sector in 0..9u8 {
            let offset = start + 0x18 + usize::from(sector) * 8;
            image[offset..offset + 4].copy_from_slice(&[track, 0, sector + 1, 2]);
        }
        let raw = usize::from(track) * 9 * 512;
        image[start + 256..start + track_size].copy_from_slice(&disk[raw..raw + 9 * 512]);
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(image: &[u8]) -> Vec<u8> {
        let parsed = crate::parse(image).expect("writer output parses");
        parsed.tracks[0]
            .iter()
            .flat_map(|t| t.sectors.iter().flat_map(|s| s.data.iter().copied()))
            .collect()
    }

    #[test]
    fn multiple_extents_recover_all_payload_and_header() {
        let payload: Vec<u8> = (0..20000).map(|n| (n % 251) as u8).collect();
        let file = code(&payload, 0x8000).expect("CODE");
        let disk = master(&[File {
            name: "DIAG.BIN",
            bytes: &file,
        }])
        .expect("master");
        let bytes = raw(&disk);
        let mut recovered = Vec::new();
        for entry in bytes[4608..6656]
            .as_chunks::<32>()
            .0
            .iter()
            .filter(|e| e[0] == 0)
        {
            for &block in &entry[16..32] {
                if block != 0 {
                    recovered.extend_from_slice(
                        &bytes[4608 + usize::from(block) * 1024
                            ..4608 + (usize::from(block) + 1) * 1024],
                    );
                }
            }
        }
        assert_eq!(&recovered[..file.len()], file);
        assert_eq!(&recovered[128..128 + payload.len()], payload);
        assert_eq!(
            recovered[127],
            recovered[..127].iter().fold(0u8, |a, b| a.wrapping_add(*b))
        );
        assert_ne!(bytes[..512].iter().fold(0u8, |a, b| a.wrapping_add(*b)), 3);
    }

    #[test]
    fn rejects_capacity_names_and_bad_code_placement() {
        for name in ["", "too_long_name", "A.", "A.B.C", "../DISK", "é"] {
            assert!(master(&[File { name, bytes: &[1] }]).is_err());
        }
        let file = File {
            name: "DISK",
            bytes: &[1],
        };
        assert!(master(&[file, file]).is_err());
        assert!(
            master(&[File {
                name: "FULL",
                bytes: &vec![0; 173 * 1024 + 1]
            }])
            .is_err()
        );
        assert!(
            master(&[File {
                name: "FULL",
                bytes: &vec![0; 173 * 1024]
            }])
            .is_ok()
        );
        assert!(code(&[1], 0x3fff).is_err());
        assert!(code(&[1, 2], 0xffff).is_err());
        assert!(code(&[], 0x8000).is_err());
        assert!(basic(&[1], 10000).is_err());
        assert!(master(&[]).is_err());
    }

    #[test]
    fn directory_limit_is_independent_of_data_capacity() {
        let names: Vec<_> = (0..65).map(|n| format!("F{n}")).collect();
        let files: Vec<_> = names
            .iter()
            .map(|name| File { name, bytes: &[1] })
            .collect();
        assert!(master(&files[..64]).is_ok());
        assert!(master(&files).is_err());
    }

    #[test]
    fn basic_header_preserves_autorun_and_program_boundary() {
        let program = [0, 10, 2, 0, 0xe2, 0x0d]; // 10 STOP
        let file = basic(&program, 10).expect("BASIC");
        assert_eq!(
            u32::from_le_bytes(file[11..15].try_into().expect("length")) as usize,
            file.len()
        );
        assert_eq!(&file[15..22], &[0, 6, 0, 10, 0, 6, 0]);
        assert_eq!(&file[128..], program);
        assert_eq!(
            file[127],
            file[..127].iter().fold(0u8, |sum, b| sum.wrapping_add(*b))
        );
    }
}
