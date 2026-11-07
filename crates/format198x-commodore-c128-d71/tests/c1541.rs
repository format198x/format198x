//! Explicit independent oracle: `cargo test -p format198x-commodore-c128-d71 --test c1541 -- --ignored`.
//! Requires VICE c1541 on PATH (or C1541 set to its executable).
use format198x_commodore_c128_d71::{
    D71Program, extract_first_prg, master, parse_directory, read_sector,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove oracle scratch");
    }
}
fn c1541(args: &[&str]) -> String {
    let output = Command::new(std::env::var_os("C1541").unwrap_or_else(|| "c1541".into()))
        .args(args)
        .output()
        .expect("run independent c1541");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("text output");
    assert!(
        !stdout.contains("ERR") && !stdout.contains("error"),
        "{stdout}"
    );
    stdout
}
fn text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 scratch path")
}
fn metadata(image: &[u8]) -> Vec<u8> {
    [(18, 0), (53, 0)]
        .into_iter()
        .flat_map(|(t, s)| read_sector(image, t, s).expect("metadata").to_vec())
        .collect()
}

#[test]
#[ignore = "requires installed VICE c1541; run explicitly for independent qualification"]
fn independent_images_extract_and_validate_without_allocation_repairs() {
    let root = std::env::temp_dir().join(format!("format198x-D71-c1541-{}", std::process::id()));
    fs::create_dir(&root).expect("unique scratch");
    let scratch = Scratch(root);
    let root = &scratch.0;
    let payload: Vec<u8> = (0usize..200_000).map(|i| i.to_le_bytes()[0]).collect();
    let source = root.join("payload.prg");
    fs::write(&source, &payload).expect("source");
    let ours = root.join("ours.d71");
    let image = master(
        "ORACLE",
        "AB",
        &[D71Program {
            name: "PAYLOAD".into(),
            data: payload.clone(),
            blocks: 0,
        }],
    )
    .expect("master");
    fs::write(&ours, &image).expect("our image");
    let listing = c1541(&["-attach", text(&ours), "-dir"]);
    assert!(
        listing.contains("payload") || listing.contains("PAYLOAD"),
        "{listing}"
    );
    assert!(listing.contains("540 blocks free."), "{listing}");
    let extracted = root.join("extracted.prg");
    c1541(&["-attach", text(&ours), "-read", "*", text(&extracted)]);
    assert_eq!(
        fs::read(&extracted).expect("independent extraction"),
        payload
    );
    let clean = root.join("clean.d71");
    fs::copy(&ours, &clean).expect("copy");
    c1541(&["-attach", text(&clean), "-validate"]);
    assert_eq!(
        metadata(&fs::read(&clean).expect("validated copy")),
        metadata(&image),
        "c1541 repaired clean allocation metadata"
    );
    let mut corrupt = image.clone();
    corrupt[(683 * 256) + (357 * 256)] ^= 1;
    let damaged = root.join("damaged.d71");
    fs::write(&damaged, &corrupt).expect("damaged");
    c1541(&["-attach", text(&damaged), "-validate"]);
    let repaired = fs::read(&damaged).expect("repaired");
    assert_ne!(
        metadata(&repaired),
        metadata(&corrupt),
        "corruption control must be observable"
    );
    assert_eq!(
        metadata(&repaired),
        metadata(&image),
        "corruption must repair to the known clean map"
    );
    let independent = root.join("independent.d71");
    c1541(&[
        "-format",
        "ORACLE,AB",
        "d71",
        text(&independent),
        "-write",
        text(&source),
        "payload",
    ]);
    let external = fs::read(&independent).expect("independent image");
    let directory = parse_directory(&external).expect("external directory");
    assert_eq!(directory.entries.len(), 1);
    assert_eq!(
        extract_first_prg(&external)
            .expect("read external file")
            .data,
        payload
    );
    println!(
        "D71: 200_000 payload bytes extracted both directions; clean BAM unchanged, damaged bitmap repaired; 540 data blocks free"
    );
}
