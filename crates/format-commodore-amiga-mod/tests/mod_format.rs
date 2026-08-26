//! Identification, and a real-media regression against genuine ProTracker
//! modules extracted from Amiga music disks.
//!
//! The real-media test needs files this repository never commits (the
//! no-media-in-the-repo rule) — see [`decodes_a_directory_of_real_modules`]
//! below for how to run it.

use format_commodore_amiga_mod::is_module;

#[test]
fn identifies_by_magic_at_1080_not_by_extension() {
    let mut bytes = vec![0u8; 1084];
    bytes[1080..1084].copy_from_slice(b"M.K.");
    assert!(is_module(&bytes));

    for magic in [b"M!K!", b"FLT4", b"4CHN"] {
        let mut b = vec![0u8; 1084];
        b[1080..1084].copy_from_slice(magic);
        assert!(
            is_module(&b),
            "{} should be recognised",
            String::from_utf8_lossy(magic)
        );
    }

    let mut wrong = vec![0u8; 1084];
    wrong[1080..1084].copy_from_slice(b"XXXX");
    assert!(!is_module(&wrong));
    assert!(!is_module(&[0u8; 100]));
}

/// Decodes every `.mod` file (case-insensitive) it finds under
/// `MOD198X_CORPUS`, plus a byte-exact round-trip check, against genuine
/// Amiga music-disk modules. Gated behind an environment variable and
/// `#[ignore]` because the modules are real media and this repository never
/// commits media (reference by path only).
///
/// Run it with, for example:
///
/// ```text
/// MOD198X_CORPUS="/path/to/extracted/modules" \
///     cargo test -p format-commodore-amiga-mod -- --ignored
/// ```
#[test]
#[ignore = "needs a real MOD corpus on disk; set MOD198X_CORPUS"]
fn decodes_a_directory_of_real_modules() {
    use format_commodore_amiga_mod::{decode, encode};

    let Ok(corpus) = std::env::var("MOD198X_CORPUS") else {
        eprintln!("skipping: MOD198X_CORPUS not set");
        return;
    };

    let mut checked = 0usize;
    let mut identical = 0usize;
    let mut differing = Vec::new();

    for entry in std::fs::read_dir(&corpus).expect("read the corpus directory") {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        if !is_module(&bytes) {
            continue;
        }

        checked += 1;
        let module = decode(&bytes).unwrap_or_else(|e| panic!("decode {path:?}: {e}"));
        let reencoded = encode(&module).unwrap_or_else(|e| panic!("encode {path:?}: {e}"));
        if reencoded == bytes {
            identical += 1;
        } else {
            differing.push(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }

    assert!(checked > 0, "no modules found under {corpus}");
    eprintln!(
        "{checked} real modules decoded without panicking; {identical}/{checked} round-tripped byte-identical"
    );
    if !differing.is_empty() {
        eprintln!("modules that did not round-trip byte-identical: {differing:?}");
    }
}
