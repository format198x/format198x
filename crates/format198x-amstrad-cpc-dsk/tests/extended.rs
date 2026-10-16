use format198x_amstrad_cpc_dsk::{DiskImage, DiskSector, DiskTrack, encode_extended, parse, plus3};

fn sector(data: Vec<u8>) -> DiskSector {
    DiskSector {
        c: 7,
        h: 1,
        id: 0x41,
        n: 7,
        st1: 0x20,
        st2: 0x40,
        data,
    }
}

fn image(sectors: Vec<DiskSector>) -> DiskImage {
    DiskImage {
        sides: 1,
        tracks_per_side: 1,
        tracks: vec![vec![DiskTrack { sectors }]],
    }
}

fn assert_model_equal(left: &DiskImage, right: &DiskImage) {
    assert_eq!(
        (left.sides, left.tracks_per_side),
        (right.sides, right.tracks_per_side)
    );
    assert_eq!(left.tracks.len(), right.tracks.len());
    for (left, right) in left.tracks.iter().zip(&right.tracks) {
        assert_eq!(left.len(), right.len());
        for (left, right) in left.iter().zip(right) {
            assert_eq!(left.sectors.len(), right.sectors.len());
            for (left, right) in left.sectors.iter().zip(&right.sectors) {
                assert_eq!(
                    (
                        left.c, left.h, left.id, left.n, left.st1, left.st2, &left.data
                    ),
                    (
                        right.c,
                        right.h,
                        right.id,
                        right.n,
                        right.st1,
                        right.st2,
                        &right.data
                    )
                );
            }
        }
    }
}

#[test]
fn bytes_match_authored_rev5_layout_with_duplicate_ids_and_zero_length_sector() -> Result<(), String>
{
    let mut empty = sector(vec![]);
    empty.c = 9;
    empty.h = 0;
    empty.n = 0xff;
    empty.st1 = 1;
    empty.st2 = 2;
    let model = image(vec![sector(vec![0xaa, 0xbb, 0xcc]), empty]);
    let output = encode_extended(&model)?;
    // Independently specified fixed offsets and bytes, not this crate's parser.
    let mut expected = vec![0; 768];
    expected[..34].copy_from_slice(b"EXTENDED CPC DSK File\r\nDisk-Info\r\n");
    expected[34..44].copy_from_slice(b"Format198x");
    expected[48..54].copy_from_slice(&[1, 1, 0, 0, 2, 0]);
    expected[256..268].copy_from_slice(b"Track-Info\r\n");
    expected[276..280].copy_from_slice(&[7, 2, 0x4e, 0xe5]);
    expected[280..296].copy_from_slice(&[
        7, 1, 0x41, 7, 0x20, 0x40, 3, 0, 9, 0, 0x41, 0xff, 1, 2, 0, 0,
    ]);
    expected[512..515].copy_from_slice(&[0xaa, 0xbb, 0xcc]);
    assert_eq!(output, expected);
    assert_model_equal(&parse(&output)?, &model);
    assert_eq!(encode_extended(&model)?, output);
    assert_eq!(model.tracks[0][0].sectors[0].data, [0xaa, 0xbb, 0xcc]);
    Ok(())
}

#[test]
fn double_sided_tracks_interleave_and_unformatted_tracks_take_no_space() -> Result<(), String> {
    let model = DiskImage {
        sides: 2,
        tracks_per_side: 2,
        tracks: vec![
            vec![
                DiskTrack {
                    sectors: vec![sector(vec![0x11])],
                },
                DiskTrack::default(),
            ],
            vec![
                DiskTrack::default(),
                DiskTrack {
                    sectors: vec![sector(vec![0x22])],
                },
            ],
        ],
    };
    let output = encode_extended(&model)?;
    assert_eq!(output.len(), 1280);
    assert_eq!(&output[0x34..0x38], &[2, 0, 0, 2]);
    assert_eq!(&output[0x110..0x112], &[0, 0]);
    assert_eq!(&output[0x310..0x312], &[1, 1]);
    assert_eq!((output[0x200], output[0x400]), (0x11, 0x22));
    assert_model_equal(&parse(&output)?, &model);
    Ok(())
}

#[test]
fn full_disk_table_fits_but_one_extra_entry_is_rejected() -> Result<(), String> {
    let mut model = DiskImage {
        sides: 2,
        tracks_per_side: 102,
        tracks: vec![vec![DiskTrack::default(); 102]; 2],
    };
    assert_eq!(encode_extended(&model)?.len(), 256);
    assert_model_equal(&parse(&encode_extended(&model)?)?, &model);
    model.tracks_per_side = 103;
    for side in &mut model.tracks {
        side.push(DiskTrack::default());
    }
    assert!(encode_extended(&model).is_err());
    Ok(())
}

#[test]
fn full_sector_table_fits_but_overflow_is_not_silently_truncated() -> Result<(), String> {
    let mut model = image(vec![sector(vec![]); 29]);
    let output = encode_extended(&model)?;
    assert_eq!((output.len(), output[0x34], output[0x115]), (512, 1, 29));
    assert_model_equal(&parse(&output)?, &model);
    model.tracks[0][0].sectors.push(sector(vec![]));
    assert!(encode_extended(&model).is_err());
    Ok(())
}

#[test]
fn maximum_track_size_fits_but_one_extra_byte_and_large_sector_reject() -> Result<(), String> {
    let mut model = image(vec![sector(vec![0x5a; 65024])]);
    let output = encode_extended(&model)?;
    assert_eq!((output.len(), output[0x34]), (65536, 255));
    assert_eq!(&output[0x11e..0x120], &[0, 0xfe]);
    assert_model_equal(&parse(&output)?, &model);
    model.tracks[0][0].sectors[0].data.push(0);
    assert!(encode_extended(&model).is_err());
    model.tracks[0][0].sectors[0].data.resize(65536, 0);
    assert!(encode_extended(&model).is_err());
    Ok(())
}

#[test]
fn combined_sector_lengths_cannot_overflow_a_track() {
    assert!(encode_extended(&image(vec![sector(vec![1; 32768]), sector(vec![2; 32768])])).is_err());
}

#[test]
fn malformed_geometry_and_extra_or_missing_tracks_reject() {
    let valid = image(vec![sector(vec![1])]);
    for sides in [0, 3] {
        let mut model = valid.clone();
        model.sides = sides;
        assert!(encode_extended(&model).is_err());
    }
    let mut model = valid.clone();
    model.tracks_per_side = 0;
    assert!(encode_extended(&model).is_err());
    for tracks in [
        vec![],
        vec![vec![]],
        vec![vec![DiskTrack::default(); 2]],
        vec![vec![DiskTrack::default()]; 2],
    ] {
        let mut model = valid.clone();
        model.tracks = tracks;
        assert!(encode_extended(&model).is_err());
    }
}

#[test]
fn standard_plus3_mastering_converts_without_changing_sector_model() -> Result<(), String> {
    let program = plus3::code(&[0xc9], 0x8000)?;
    let standard = plus3::master(&[plus3::File {
        name: "RETURN.BIN",
        bytes: &program,
    }])?;
    let model = parse(&standard)?;
    let extended = encode_extended(&model)?;
    assert_ne!(&extended[..34], &standard[..34]);
    assert_eq!(extended.len(), standard.len());
    assert_model_equal(&parse(&extended)?, &model);
    Ok(())
}
