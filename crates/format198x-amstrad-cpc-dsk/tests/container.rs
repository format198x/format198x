use format198x_amstrad_cpc_dsk::{parse, parse_container};

fn image(extended: bool, n: u8, payload: usize) -> Vec<u8> {
    let length = (256 + payload).next_multiple_of(256);
    let mut bytes = vec![0; 256 + length];
    let signature: &[u8] = if extended {
        b"EXTENDED CPC DSK File"
    } else {
        b"MV - CPC"
    };
    bytes[..signature.len()].copy_from_slice(signature);
    bytes[0x30] = 1;
    bytes[0x31] = 1;
    if extended {
        bytes[0x34] = (length / 256) as u8;
    } else {
        bytes[0x32..0x34].copy_from_slice(&(length as u16).to_le_bytes());
    }
    let tib = &mut bytes[256..512];
    tib[..10].copy_from_slice(b"Track-Info");
    tib[0x14] = n;
    tib[0x15] = 1;
    tib[0x18..0x1C].copy_from_slice(&[0x12, 0x34, 7, n]);
    if extended {
        tib[0x1E..0x20].copy_from_slice(&(payload as u16).to_le_bytes());
    }
    bytes[512..512 + payload].fill(0xA7);
    bytes
}

#[test]
fn physical_header_and_sector_metadata_are_independent() {
    for extended in [false, true] {
        let mut bytes = image(extended, 2, 512);
        bytes[256 + 0x10..256 + 0x14].copy_from_slice(&[9, 1, 2, 3]);
        bytes[256 + 0x16..256 + 0x18].copy_from_slice(&[0x4E, 0xE5]);
        bytes[256 + 0x1C..256 + 0x1E].copy_from_slice(&[0x20, 0x40]);
        let decoded = parse_container(&bytes).expect("bounded container");
        let header = decoded.tracks[0].header.expect("physical header");
        assert_eq!(
            (
                header.track,
                header.side,
                header.data_rate,
                header.recording_mode
            ),
            (9, 1, 2, 3)
        );
        assert_eq!(
            (header.size_code, header.gap3, header.filler),
            (2, 0x4E, 0xE5)
        );
        let sector = &decoded.tracks[0].sectors[0];
        assert_eq!(
            (
                sector.c, sector.h, sector.id, sector.n, sector.st1, sector.st2
            ),
            (0x12, 0x34, 7, 2, 0x20, 0x40)
        );
        assert_eq!(sector.data, vec![0xA7; 512]);
        assert_eq!(
            parse(&bytes).expect("legacy still reads").tracks[0][0].sectors[0].data,
            sector.data
        );
    }
}

#[test]
fn extended_zero_length_and_arbitrary_recorded_n_remain_metadata() {
    let bytes = image(true, 255, 0);
    let parsed = parse_container(&bytes).expect("zero stored bytes are meaningful");
    assert_eq!(parsed.tracks[0].sectors[0].n, 255);
    assert!(parsed.tracks[0].sectors[0].data.is_empty());
}

#[test]
fn descriptor_table_cannot_consume_sector_payload() {
    let mut bytes = image(true, 2, 512);
    bytes[256 + 0x15] = 30;
    assert!(parse_container(&bytes).is_err());
}

#[test]
fn disk_table_cannot_consume_the_first_track_header() {
    let mut bytes = image(true, 2, 512);
    bytes[0x30] = 205;
    assert!(parse_container(&bytes).is_err());
}

#[test]
fn extents_and_standard_size_codes_are_bounded() {
    let valid = image(false, 2, 512);
    for length in 0..valid.len() {
        assert!(parse_container(&valid[..length]).is_err(), "cut {length}");
    }
    for n in [57, 64, 128, 255] {
        let bytes = image(false, n, 512);
        assert!(parse_container(&bytes).is_err(), "N={n}");
    }
    let mut bytes = image(true, 2, 512);
    bytes[256 + 0x1E..256 + 0x20].copy_from_slice(&1024u16.to_le_bytes());
    assert!(parse_container(&bytes).is_err());
}

#[test]
fn standard_sizes_are_not_clamped_and_reserved_length_is_ignored() {
    let mut bytes = image(false, 7, 16384);
    assert_eq!(
        parse_container(&bytes)
            .expect("large standard sector")
            .tracks[0]
            .sectors[0]
            .data
            .len(),
        16384
    );
    assert_eq!(
        parse(&bytes).expect("legacy compatibility").tracks[0][0].sectors[0]
            .data
            .len(),
        8192
    );
    bytes[256 + 0x1E..256 + 0x20].copy_from_slice(&1u16.to_le_bytes());
    assert_eq!(
        parse_container(&bytes).expect("reserved bytes").tracks[0].sectors[0]
            .data
            .len(),
        16384
    );
    assert_eq!(
        parse(&bytes)
            .expect("legacy reserved-byte policy unchanged")
            .tracks[0][0]
            .sectors[0]
            .data
            .len(),
        1
    );
}

#[test]
fn absent_zeroed_and_present_empty_headers_are_distinct() {
    let mut absent = image(true, 2, 512);
    absent[0x34] = 0;
    assert!(
        parse_container(&absent).expect("absent").tracks[0]
            .header
            .is_none()
    );
    let mut zeroed = image(false, 2, 512);
    zeroed[256..512].fill(0);
    assert!(
        parse_container(&zeroed).expect("zeroed").tracks[0]
            .header
            .is_none()
    );
    assert!(parse(&zeroed).is_err(), "legacy policy stays intact");
    let mut empty = image(true, 2, 512);
    empty[256 + 0x15] = 0;
    let decoded = parse_container(&empty).expect("present empty");
    assert!(decoded.tracks[0].header.is_some());
    assert!(decoded.tracks[0].sectors.is_empty());
}
