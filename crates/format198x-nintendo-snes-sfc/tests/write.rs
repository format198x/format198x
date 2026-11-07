//! Writing files back, and setting the checksum pair.

use format198x_nintendo_snes_sfc::{MapMode, SfcError, compute_checksum, parse, set_checksum};

/// A ROM with a plausible header for `map` and no checksum yet.
fn rom(len: usize, map: MapMode, mode: u8) -> Vec<u8> {
    let mut rom: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
    let base = map.header_offset();
    rom[base..base + 21].copy_from_slice(b"FORMAT198X TEST CART ");
    rom[base + 0x15] = mode;
    rom[base + 0x17] = 0x08;
    rom[base + 0x3C..base + 0x3E].copy_from_slice(&0x8000u16.to_le_bytes());
    rom
}

#[test]
fn a_set_checksum_matches_the_rom_it_is_part_of() {
    for (map, mode, len) in [
        (MapMode::LoRom, 0x20, 0x40000),
        (MapMode::HiRom, 0x21, 0x60000), // not a power of two: mirrored
    ] {
        let mut rom = rom(len, map, mode);
        let checksum = set_checksum(&mut rom, map).expect("room for a header");
        let image = parse(&rom).expect("parses");
        assert_eq!(image.header.map, map);
        assert!(image.header.checksum_pair_valid());
        assert_eq!(image.header.checksum, checksum);
        assert_eq!(compute_checksum(&rom), checksum);

        // Setting it again changes nothing.
        let before = rom.clone();
        assert_eq!(set_checksum(&mut rom, map), Ok(checksum));
        assert_eq!(rom, before);
    }
}

#[test]
fn a_rom_too_short_for_the_header_is_refused() {
    let mut short = vec![0; 0x8000];
    assert_eq!(
        set_checksum(&mut short, MapMode::HiRom),
        Err(SfcError::TooShort(0x8000))
    );
}

#[test]
fn files_write_back_with_their_copier_header() {
    let mut rom = rom(0x40000, MapMode::LoRom, 0x20);
    set_checksum(&mut rom, MapMode::LoRom).expect("room for a header");
    let mut file: Vec<u8> = (0..512).map(|i| i as u8).collect();
    file.extend_from_slice(&rom);

    let image = parse(&file).expect("parses");
    assert_eq!(image.copier_header, Some(&file[..512]));
    assert_eq!(image.to_bytes(), file);
    assert_eq!(parse(&rom).expect("parses").to_bytes(), rom);
}
