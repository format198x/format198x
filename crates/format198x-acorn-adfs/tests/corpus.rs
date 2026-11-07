//! Real ADFS discs: format, free-space map checksums and root directory.
//!
//! Point `FORMAT198X_ADFS_CORPUS_DIR` at a directory of `.adf` or `.adl`
//! images. Every image must be detected, carry map checksums this crate
//! reproduces, and have a readable root directory. ADFS itself wrote those
//! checksums, so they check [`map_checksum`] against the real filing system.

use std::env;
use std::fs;
use std::path::PathBuf;

use format198x_acorn_adfs::{SECTOR_SIZE, detect, map_checksum, map_total_sectors, root_directory};

#[test]
#[ignore = "FIXTURE: set FORMAT198X_ADFS_CORPUS_DIR to a directory of .adf/.adl images"]
fn real_discs_have_good_maps_and_root_directories() {
    let Ok(dir) = env::var("FORMAT198X_ADFS_CORPUS_DIR") else {
        panic!("set FORMAT198X_ADFS_CORPUS_DIR");
    };
    let mut images: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("read corpus")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("adf") || ext.eq_ignore_ascii_case("adl")
            })
        })
        .collect();
    images.sort();
    assert!(!images.is_empty(), "corpus directory held no ADFS images");

    let mut entries = 0;
    for path in &images {
        let image = fs::read(path).expect("read image");
        let name = path.display();
        let format = detect(&image).unwrap_or_else(|e| panic!("{name}: {e}"));
        for sector in [0, 1] {
            let bytes = &image[sector * SECTOR_SIZE..(sector + 1) * SECTOR_SIZE];
            assert_eq!(
                map_checksum(bytes),
                bytes[SECTOR_SIZE - 1],
                "{name}: map sector {sector} checksum"
            );
        }
        assert_eq!(
            map_total_sectors(&image),
            Some(format.total_sectors()),
            "{name}: the map's size names the detected format"
        );
        entries += root_directory(&image)
            .unwrap_or_else(|e| panic!("{name}: {e}"))
            .entries
            .len();
    }
    println!("checked={} root_entries={entries}", images.len());
}
