//! Writing files: a parsed file back unchanged, and new files from parts.

use format198x_nintendo_nes_ines::{
    HEADER_LEN, HeaderMirroring, InesError, NewImage, Rom, encode, parse,
};

fn new_image<'a>(prg: &'a [u8], chr: &'a [u8]) -> NewImage<'a> {
    NewImage {
        mapper_number: 4,
        mirroring: HeaderMirroring::Vertical,
        has_battery: true,
        nes_2_0: false,
        trainer: None,
        prg_rom: prg,
        chr_rom: chr,
    }
}

#[test]
fn an_ines_file_is_written_as_the_specification_lays_it_out() {
    let prg = vec![0xAA; 2 * 16 * 1024];
    let chr = vec![0xBB; 8 * 1024];
    let file = encode(&new_image(&prg, &chr)).expect("fits iNES");

    // Byte 6: mapper low nibble 4, battery (bit 1), vertical (bit 0).
    assert_eq!(&file[..8], b"NES\x1a\x02\x01\x43\x00");
    assert_eq!(&file[8..HEADER_LEN], &[0; 8]);
    assert_eq!(file.len(), HEADER_LEN + prg.len() + chr.len());
    assert_eq!(&file[HEADER_LEN..HEADER_LEN + prg.len()], &prg[..]);
}

#[test]
fn every_field_survives_a_round_trip() {
    let trainer = [0x11; 512];
    let prg = vec![0x22; 16 * 1024];
    for nes_2_0 in [false, true] {
        for mirroring in [
            HeaderMirroring::Horizontal,
            HeaderMirroring::Vertical,
            HeaderMirroring::FourScreen,
        ] {
            for has_battery in [false, true] {
                let parts = NewImage {
                    mapper_number: if nes_2_0 { 0x2A5 } else { 0xA5 },
                    mirroring,
                    has_battery,
                    nes_2_0,
                    trainer: Some(&trainer),
                    prg_rom: &prg,
                    chr_rom: &[],
                };
                let file = encode(&parts).expect("fits");
                let image = parse(&file).expect("parses");
                assert_eq!(image.header.mapper_number, parts.mapper_number);
                assert_eq!(image.header.mirroring, mirroring);
                assert_eq!(image.header.has_battery, has_battery);
                assert_eq!(image.header.is_nes_2_0, nes_2_0);
                assert_eq!(image.trainer, Some(&trainer[..]));
                assert_eq!(image.prg_rom, &prg[..]);
                assert!(image.chr_rom.is_empty() && image.trailing.is_empty());
                assert_eq!(image.to_bytes().expect("writes"), file);
            }
        }
    }
}

#[test]
fn nes_2_0_counts_banks_up_to_eff_then_uses_the_exponent_form() {
    // 0x100 banks of CHR: past iNES's byte, inside NES 2.0's count.
    let chr = vec![0; 0x100 * 8 * 1024];
    let mut parts = new_image(&[], &chr);
    assert_eq!(
        encode(&parts),
        Err(InesError::UnencodableSize {
            rom: Rom::Chr,
            len: chr.len(),
            nes_2_0: false
        })
    );
    parts.nes_2_0 = true;
    let file = encode(&parts).expect("fits NES 2.0");
    assert_eq!((file[5], file[9]), (0x00, 0x10));
    assert_eq!(parse(&file).expect("parses").chr_rom.len(), chr.len());

    // 24 bytes is no whole bank but is 2^3 × 3, so the exponent form holds it.
    let odd = [0x5A; 24];
    let file = encode(&NewImage {
        prg_rom: &odd,
        ..parts
    })
    .expect("exponent form");
    assert_eq!((file[4], file[9] & 0x0F), ((3 << 2) | 1, 0x0F));
    assert_eq!(parse(&file).expect("parses").prg_rom, &odd[..]);

    // 18 bytes is 2 × 9, and 9 is no 2M + 1 with M ≤ 3.
    assert!(matches!(
        encode(&NewImage {
            prg_rom: &[0; 18],
            ..parts
        }),
        Err(InesError::UnencodableSize { rom: Rom::Prg, .. })
    ));
}

#[test]
fn parts_the_header_cannot_hold_are_refused() {
    let prg = vec![0; 16 * 1024];
    let mut parts = new_image(&prg, &[]);
    parts.mapper_number = 256;
    assert_eq!(
        encode(&parts),
        Err(InesError::MapperOutOfRange {
            mapper: 256,
            nes_2_0: false
        })
    );
    parts.nes_2_0 = true;
    parts.mapper_number = 4096;
    assert!(matches!(
        encode(&parts),
        Err(InesError::MapperOutOfRange { mapper: 4096, .. })
    ));
    parts.mapper_number = 0;
    parts.trainer = Some(&[0; 100]);
    assert_eq!(encode(&parts), Err(InesError::TrainerLength(100)));
    parts.trainer = None;
    parts.nes_2_0 = false;
    parts.prg_rom = &prg[..100];
    assert!(matches!(
        encode(&parts),
        Err(InesError::UnencodableSize {
            rom: Rom::Prg,
            len: 100,
            ..
        })
    ));
    for error in [
        InesError::TrainerLength(1),
        InesError::MapperOutOfRange {
            mapper: 300,
            nes_2_0: false,
        },
        InesError::UnencodableSize {
            rom: Rom::Chr,
            len: 1,
            nes_2_0: true,
        },
        InesError::HeaderMismatch,
    ] {
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn a_header_that_no_longer_matches_its_rom_is_not_written() {
    let prg = vec![0; 16 * 1024];
    let file = encode(&new_image(&prg, &[])).expect("fits");
    let image = parse(&file).expect("parses");

    let mut shortened = image;
    shortened.prg_rom = &prg[..1024];
    assert_eq!(shortened.to_bytes(), Err(InesError::HeaderMismatch));

    let trainer = [0; 512];
    let mut trained = image;
    trained.trainer = Some(&trainer);
    assert_eq!(trained.to_bytes(), Err(InesError::HeaderMismatch));

    let mut unmagic = image;
    unmagic.header_bytes[0] = b'X';
    assert_eq!(unmagic.to_bytes(), Err(InesError::BadMagic));
}

#[test]
fn bytes_the_header_does_not_model_are_kept() {
    let prg = vec![0; 16 * 1024];
    let mut file = encode(&new_image(&prg, &[])).expect("fits");
    // An old dumping tool's signature in the unused header bytes, and data
    // appended after the ROM.
    file[10..15].copy_from_slice(b"Dude!");
    file.extend_from_slice(b"appended title");
    let image = parse(&file).expect("parses");
    assert_eq!(image.trailing, b"appended title");
    assert_eq!(image.to_bytes().expect("writes"), file);
}
