//! Real `.st` images: byte-exact round trips, and geometry checked against
//! Hatari.
//!
//! Point `FORMAT198X_ST_CORPUS_DIR` at a directory of `.st` images. Every
//! image must parse and write back unchanged.
//!
//! With `FORMAT198X_HMSA` also set to Hatari's `hmsa` converter, each image
//! is converted to MSA by Hatari, and the sectors per track and sides in
//! Hatari's MSA header must match the geometry this crate chose. When `hmsa`
//! finds no geometry it writes zero sectors per track; those images are
//! reported, not compared, and the test fails if nothing was compared.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use format198x_atari_st_st::parse;

fn st_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read corpus")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("st"))
        })
        .collect();
    found.sort();
    found
}

fn corpus() -> Vec<PathBuf> {
    let Ok(dir) = env::var("FORMAT198X_ST_CORPUS_DIR") else {
        panic!("set FORMAT198X_ST_CORPUS_DIR");
    };
    let files = st_files(Path::new(&dir));
    assert!(!files.is_empty(), "corpus directory held no .st files");
    files
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_ST_CORPUS_DIR to a directory of .st images"]
fn real_images_write_back_unchanged() {
    let mut trailing = 0;
    for path in corpus() {
        let bytes = fs::read(&path).expect("read image");
        let image = parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let written = image.to_bytes().expect("writes");
        assert!(written == bytes, "{} changed on write", path.display());
        trailing += usize::from(!image.trailing.is_empty());
    }
    println!("with trailing bytes: {trailing}");
}

#[test]
#[ignore = "FIXTURE: set FORMAT198X_ST_CORPUS_DIR and FORMAT198X_HMSA (Hatari's hmsa)"]
fn geometry_matches_hatari() {
    let Ok(hmsa) = env::var("FORMAT198X_HMSA") else {
        panic!("set FORMAT198X_HMSA to Hatari's hmsa converter");
    };
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("st-hatari");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("scratch directory");

    let mut checked = 0;
    let mut no_geometry = Vec::new();
    let mut disagreements = Vec::new();
    for (index, path) in corpus().into_iter().enumerate() {
        let bytes = fs::read(&path).expect("read image");
        let ours = parse(&bytes).expect("parses").geometry;

        let copy = scratch.join(format!("{index}.st"));
        fs::write(&copy, &bytes).expect("copy image");
        let status = Command::new(&hmsa)
            .arg(&copy)
            .output()
            .expect("run hmsa")
            .status;
        assert!(status.success(), "hmsa failed on {}", path.display());
        let msa = fs::read(copy.with_extension("msa")).expect("hmsa wrote an archive");
        let word = |at: usize| usize::from(u16::from_be_bytes([msa[at], msa[at + 1]]));
        let (spt, sides, tracks) = (word(2), word(4) + 1, word(8) + 1);

        if spt == 0 {
            no_geometry.push(path.display().to_string());
            continue;
        }
        checked += 1;
        if (spt, sides) != (ours.sectors_per_track, ours.sides) {
            disagreements.push(format!(
                "{}: ours {}x{}x{}, Hatari {tracks}x{sides}x{spt}",
                path.display(),
                ours.tracks,
                ours.sides,
                ours.sectors_per_track
            ));
        }
    }
    println!("checked={checked} hatari_found_no_geometry={no_geometry:#?}");
    assert!(checked > 0, "Hatari gave no geometry to compare with");
    assert!(disagreements.is_empty(), "{disagreements:#?}");
}
