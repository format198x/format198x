//! MSA archives checked against Hatari's `hmsa` converter in both
//! directions.
//!
//! Point `FORMAT198X_ST_CORPUS_DIR` at a directory of `.st` images and
//! `FORMAT198X_HMSA` at Hatari's `hmsa`. For each image:
//!
//! - Hatari's archive of it, expanded by this crate, must give back the
//!   image's sectors.
//! - This crate's archive of it, expanded by Hatari, must give back the same
//!   sectors.
//!
//! The geometry comes from Hatari's own archive header, so this test needs
//! no `.st` reader of its own. When `hmsa` finds no geometry it writes zero
//! sectors per track; those images are reported, not compared, and the test
//! fails if nothing was compared.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use format198x_atari_st_msa::{SECTOR_SIZE, parse, write};

fn hmsa(tool: &str, file: &Path) {
    let output = Command::new(tool).arg(file).output().expect("run hmsa");
    assert!(output.status.success(), "hmsa failed on {}", file.display());
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_ST_CORPUS_DIR and FORMAT198X_HMSA (Hatari's hmsa)"]
fn archives_agree_with_hatari_both_ways() {
    let (Ok(dir), Ok(tool)) = (
        env::var("FORMAT198X_ST_CORPUS_DIR"),
        env::var("FORMAT198X_HMSA"),
    ) else {
        panic!("set FORMAT198X_ST_CORPUS_DIR and FORMAT198X_HMSA");
    };
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("msa-hatari");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("scratch directory");

    let mut images: Vec<_> = fs::read_dir(&dir)
        .expect("read corpus")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("st"))
        })
        .collect();
    images.sort();
    assert!(!images.is_empty(), "corpus directory held no .st files");

    let mut checked = 0;
    let mut no_geometry = Vec::new();
    for (index, path) in images.iter().enumerate() {
        let st = fs::read(path).expect("read image");

        // Hatari compresses; this crate expands.
        let theirs_st = scratch.join(format!("theirs-{index}.st"));
        fs::write(&theirs_st, &st).expect("copy image");
        hmsa(&tool, &theirs_st);
        let theirs = fs::read(theirs_st.with_extension("msa")).expect("Hatari's archive");
        if theirs.get(2..4) == Some(&[0, 0]) {
            no_geometry.push(path.display().to_string());
            continue;
        }
        let expanded = parse(&theirs).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let covered = expanded.data.len();
        assert!(
            st.len() >= covered && expanded.data == st[..covered],
            "{}: Hatari's archive expands differently",
            path.display()
        );

        // This crate compresses; Hatari expands.
        let ours = write(expanded.sectors_per_track, expanded.sides, &st[..covered])
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let ours_msa = scratch.join(format!("ours-{index}.msa"));
        fs::write(&ours_msa, &ours).expect("write archive");
        hmsa(&tool, &ours_msa);
        let back = fs::read(ours_msa.with_extension("st")).expect("Hatari's expansion");
        assert!(
            back == st[..covered],
            "{}: Hatari expands this crate's archive differently",
            path.display()
        );
        assert_eq!(covered % SECTOR_SIZE, 0);
        checked += 1;
    }
    println!("checked={checked} hatari_found_no_geometry={no_geometry:#?}");
    assert!(checked > 0, "Hatari archived nothing to compare with");
}
