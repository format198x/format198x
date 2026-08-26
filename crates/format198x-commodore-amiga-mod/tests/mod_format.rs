//! Identification, and a real-media regression against genuine ProTracker
//! modules extracted from Amiga music disks.
//!
//! The real-media test needs files this repository never commits (the
//! no-media-in-the-repo rule) — see [`decodes_a_directory_of_real_modules`]
//! below for how to run it.

use format198x_commodore_amiga_mod::is_module;

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

/// Decodes every `.mod` file it finds under `MOD198X_CORPUS` and checks
/// three things about it, against genuine Amiga music-disk modules.
///
/// A byte-exact round-trip is the weakest of the three and cannot stand
/// alone: `Module`/`Sample` are lossless, so a real 4-channel module
/// failing to round-trip is a bug — but *both* readings of a disputed
/// 1024-byte block round-trip byte-identically, which is the whole premise
/// of the defect `decode`'s pattern rule exists to avoid. A green
/// round-trip says nothing about whether that rule fired correctly.
///
/// So the test also asserts the one property the two readings do *not*
/// share: every order-table entry the song actually plays must name a
/// pattern that was decoded. Clamp the count too low — the failure mode
/// that shipped and was reverted — and a played order points past the end
/// of `patterns`, which no round-trip can hide.
///
/// (Recomputing the file's byte layout from the decoded module is not worth
/// asserting: `decode` defines `trailing` as whatever is left after the
/// header, `patterns.len()` patterns and the samples, so the sum reaches the
/// file length by construction whichever way the rule went.)
///
/// Then it reports how many modules carried trailing bytes at all, with
/// their names and byte counts, and how many files it scanned — so an empty
/// or mistyped `MOD198X_CORPUS` cannot read as a pass, and so the run says
/// which way the rule went on real media rather than only that it went
/// somewhere.
///
/// Gated behind an environment variable and `#[ignore]` because the modules
/// are real media and this repository never commits media (reference by
/// path only).
///
/// Run it with, for example:
///
/// ```text
/// MOD198X_CORPUS="/path/to/extracted/modules" \
///     cargo test -p format198x-commodore-amiga-mod -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a real MOD corpus on disk; set MOD198X_CORPUS"]
fn decodes_a_directory_of_real_modules() {
    use format198x_commodore_amiga_mod::{MAGIC_OFFSET, decode, encode};

    const HEADER_LEN: usize = MAGIC_OFFSET + 4;
    const PATTERN_LEN: usize = 1024;

    let Ok(corpus) = std::env::var("MOD198X_CORPUS") else {
        eprintln!("skipping: MOD198X_CORPUS not set");
        return;
    };

    let mut scanned = 0usize;
    let mut checked = 0usize;
    let mut identical = 0usize;
    let mut differing = Vec::new();
    let mut with_trailing: Vec<(String, usize, usize, usize)> = Vec::new();
    let mut short_of_orders: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&corpus).expect("read the corpus directory") {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        scanned += 1;
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        if !is_module(&bytes) {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        checked += 1;
        let module = decode(&bytes).unwrap_or_else(|e| panic!("decode {path:?}: {e}"));

        // The decoded pattern count has to cover every pattern the song
        // plays. A count clamped below what the order table plays is the
        // regression this rule was rewritten to remove, and it survives a
        // round-trip untouched, so this is where real media gets to object.
        // `song_length` 0 means the module plays nothing at all — two files
        // in the reference corpus are sample banks shipped in module form,
        // with no pattern data and a leftover order table. There is no
        // played order to check against, so there is nothing to say.
        if let Some(played_max) = module.orders().iter().copied().max()
            && usize::from(played_max) >= module.patterns.len()
        {
            short_of_orders.push(format!(
                "{name}: plays order {played_max} but only {} patterns decoded",
                module.patterns.len()
            ));
        }
        if !module.trailing.is_empty() {
            let sample_bytes: usize = module.samples.iter().map(|s| s.data.len()).sum();
            let size_rule = (bytes.len() - HEADER_LEN - sample_bytes) / PATTERN_LEN;
            with_trailing.push((
                name.clone(),
                module.trailing.len(),
                module.patterns.len(),
                size_rule,
            ));
        }

        let reencoded = encode(&module).unwrap_or_else(|e| panic!("encode {path:?}: {e}"));
        if reencoded == bytes {
            identical += 1;
        } else {
            differing.push(name);
        }
    }

    eprintln!("{scanned} files scanned; {checked} identified as modules and decoded");
    eprintln!(
        "{}/{checked} carried trailing bytes after the last sample",
        with_trailing.len()
    );
    for (name, len, kept, size_rule) in &with_trailing {
        eprintln!(
            "  trailing: {name} ({len} bytes); kept {kept} patterns where the size rule alone wanted {size_rule}"
        );
    }
    eprintln!("{identical}/{checked} round-tripped byte-identical");

    assert!(
        scanned > 0,
        "no files at all under {corpus} — is it the right path?"
    );
    assert!(checked > 0, "no modules found under {corpus}");
    assert!(
        short_of_orders.is_empty(),
        "{}/{checked} modules play an order that names a pattern the decode did not produce: {short_of_orders:?}",
        short_of_orders.len()
    );
    assert!(
        differing.is_empty(),
        "{}/{checked} modules did not round-trip byte-identical: {differing:?}",
        differing.len()
    );
}
