//! Optional real extracted-ROM corpus. No ROM data is committed.
use format198x_snk_neo_geo_romset::{from_files, to_files};
use std::{env, fs};

#[test]
#[ignore = "FIXTURE: set FORMAT198X_NEOGEO_CORPUS_DIR to extracted game directories"]
fn real_sets_keep_all_decoded_regions() {
    let root = env::var("FORMAT198X_NEOGEO_CORPUS_DIR").expect("set corpus directory");
    let mut checked = 0;
    for entry in fs::read_dir(root).expect("read corpus") {
        let dir = entry.expect("game").path();
        if !dir.is_dir() {
            continue;
        }
        let files = fs::read_dir(&dir)
            .expect("read game")
            .map(|entry| {
                let path = entry.expect("file").path();
                (
                    path.file_name()
                        .expect("name")
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).expect("ROM"),
                )
            })
            .collect::<Vec<_>>();
        // A MAME parent set holds only the game's ROMs, so every byte must
        // land in a region: a file the classifier skips is a lost chip.
        let file_bytes: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
        let set = from_files(files).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let region_bytes =
            set.p.len() + set.s.len() + set.m.len() + set.v_a.len() + set.v_b.len() + set.c.len();
        assert_eq!(
            region_bytes,
            file_bytes,
            "{}: some files were not read into a region",
            dir.display()
        );
        let decoded =
            from_files(to_files(&set).expect("write regions")).expect("read written files");
        assert!(decoded == set, "{} regions changed", dir.display());
        println!(
            "{} P={} S={} M={} VA={} VB={} C={}",
            dir.display(),
            set.p.len(),
            set.s.len(),
            set.m.len(),
            set.v_a.len(),
            set.v_b.len(),
            set.c.len()
        );
        checked += 1;
    }
    assert!(checked > 0, "no extracted sets found");
    println!("checked={checked}");
}
