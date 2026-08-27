use crate::error::*;
use crate::fs::*;
use crate::layout::*;
use crate::read::*;
use crate::write::*;

/// Follow the on-disk structure to read a top-level file's bytes back —
/// validating the root hash table, file header, data-pointer table, and
/// OFS data-block chain the way a real filesystem would.
fn read_file(img: &[u8], name: &str) -> Vec<u8> {
    let root = &img[DD.root_block() as usize * BSIZE..][..BSIZE];
    let hdr_blk = read_u32(root, 24 + 4 * name_hash(name));
    let hdr = &img[hdr_blk as usize * BSIZE..][..BSIZE];
    let size = read_u32(hdr, BSIZE - 188) as usize;
    let mut blk = read_u32(hdr, 16); // first_data
    let mut out = Vec::new();
    while blk != 0 {
        let d = &img[blk as usize * BSIZE..][..BSIZE];
        let n = read_u32(d, 12) as usize;
        out.extend_from_slice(&d[24..24 + n]);
        blk = read_u32(d, 16);
    }
    assert_eq!(out.len(), size, "declared size vs chained data for {name}");
    out
}

/// Find `name` in directory `dir`, following the hash chain on a collision.
fn lookup(img: &[u8], dir: u32, name: &str) -> u32 {
    let mut e = read_u32(block(img, dir), 24 + 4 * name_hash(name));
    while e != 0 {
        if header_name(img, e) == name {
            return e;
        }
        e = read_u32(block(img, e), BSIZE - 16); // hash_chain
    }
    0
}

/// Resolve a slash-separated path to its header block — a miniature read
/// side, walking directory hash tables and hash chains.
fn resolve(img: &[u8], path: &str) -> u32 {
    let mut blk = DD.root_block();
    for comp in path.split('/').filter(|s| !s.is_empty()) {
        blk = lookup(img, blk, comp);
        assert!(blk != 0, "path component {comp:?} not found");
    }
    blk
}

/// Read a file at `path` by walking the header + extension pointer tables —
/// the way FFS (which has no per-data-block chain) and disk validators
/// navigate. Works for both filesystems and any depth.
fn read_file_via_ptrs(img: &[u8], path: &str, fs: FileSystem) -> Vec<u8> {
    let hdr_blk = resolve(img, path);
    let size = read_u32(block(img, hdr_blk), BSIZE - 188) as usize;
    let mut blocks = Vec::new();
    collect_ptrs(img, hdr_blk, &mut blocks);
    let mut ext = read_u32(block(img, hdr_blk), BSIZE - 8);
    while ext != 0 {
        collect_ptrs(img, ext, &mut blocks);
        ext = read_u32(block(img, ext), BSIZE - 8);
    }
    let mut out = Vec::new();
    for &b in &blocks {
        match fs {
            FileSystem::Ffs => out.extend_from_slice(block(img, b)),
            FileSystem::Ofs => {
                let d = block(img, b);
                let n = read_u32(d, 12) as usize;
                out.extend_from_slice(&d[24..24 + n]);
            }
        }
    }
    out.truncate(size); // FFS's final sector is zero-padded to 512
    out
}

fn assert_checksums(img: &[u8], name: &str) {
    // Every header/data block used carries a valid offset-20 checksum;
    // the bitmap an offset-0 one.
    let check = |blk: u32, off: usize| {
        let b = &img[blk as usize * BSIZE..][..BSIZE];
        assert_eq!(
            read_u32(b, off),
            checksum(b, off),
            "checksum block {blk} of {name}"
        );
    };
    check(DD.root_block(), 20);
    check(DD.bitmap_block(), 0);
    // Walk root entries and their data.
    let root = &img[DD.root_block() as usize * BSIZE..][..BSIZE];
    for slot in 0..HT_SIZE {
        let e = read_u32(root, 24 + 4 * slot);
        if e != 0 {
            check(e, 20);
        }
    }
}

#[test]
fn masters_a_bootable_shape() {
    let exe = b"\x00\x00\x03\xf3 fake hunk exe payload".to_vec();
    let img = master(&exe, "game", "Game").unwrap();
    assert_eq!(img.len(), DD.blocks() as usize * BSIZE);
    assert_eq!(&img[0..4], b"DOS\0");
    assert_eq!(
        read_u32(&img[DD.root_block() as usize * BSIZE..], BSIZE - 4),
        ST_ROOT
    );
    assert_checksums(&img, "game");
    // The exe reads back intact, and the `s` directory is reachable from
    // the root (an empty read: a directory has no file data).
    assert_eq!(read_file(&img, "game"), exe);
    assert!(read_file(&img, "s").is_empty());
}

#[test]
fn round_trips_a_multi_block_file() {
    // 3 data blocks: forces chaining, seq numbers, reverse pointer table.
    let exe: Vec<u8> = (0..OFS_DATA * 2 + 100).map(|i| (i % 251) as u8).collect();
    let img = master(&exe, "big", "Big").unwrap();
    assert_checksums(&img, "big");
    assert_eq!(read_file(&img, "big"), exe);
}

#[test]
fn boot_checksum_reproduces_the_ofs_reference() {
    // Recomputing the OFS boot block must reproduce BOOT_PREFIX byte-for-byte
    // — including the embedded reference checksum at offset 4. This both
    // validates the add-with-carry algorithm and guards OFS byte-identity now
    // that the boot block is built dynamically.
    let mut img = vec![0u8; 1024];
    write_boot_block(&mut img, FileSystem::Ofs, true);
    assert_eq!(&img[..BOOT_PREFIX.len()], &BOOT_PREFIX[..]);
    assert!(img[BOOT_PREFIX.len()..].iter().all(|&b| b == 0));
}

#[test]
fn round_trips_a_multi_block_ffs_file() {
    // FFS: raw 512-byte sectors, no data-block header/chain — navigated
    // entirely by the header/extension pointer tables. Force several blocks
    // and a partial final sector.
    let exe: Vec<u8> = (0..BSIZE * 3 + 137).map(|i| (i % 251) as u8).collect();
    let img = master_fs(&exe, "ffsgame", "FfsGame", FileSystem::Ffs).unwrap();
    assert_eq!(&img[0..4], b"DOS\x01", "FFS boot type");
    // Volume structure (root/bitmap/headers) is identical to OFS and still
    // checksums; the data blocks carry no checksum, by design.
    assert_checksums(&img, "ffsgame");
    assert_eq!(read_file_via_ptrs(&img, "ffsgame", FileSystem::Ffs), exe);
    // The OFS reader (pointer tables) agrees with the chain reader on OFS,
    // confirming the two navigation paths are consistent.
    let ofs = master_fs(&exe, "ffsgame", "FfsGame", FileSystem::Ofs).unwrap();
    assert_eq!(
        read_file_via_ptrs(&ofs, "ffsgame", FileSystem::Ofs),
        read_file(&ofs, "ffsgame")
    );
}

#[test]
fn ffs_is_deterministic_and_denser_than_ofs() {
    let exe = vec![0x5au8; 4000];
    assert_eq!(
        master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap(),
        master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap()
    );
    // 4000 bytes: OFS needs ceil(4000/488)=9 data blocks, FFS ceil(4000/512)
    // =8 — so the FFS image marks fewer blocks used. Compare bitmap free bits.
    let free = |img: &[u8]| -> u32 {
        (0..((DD.blocks() - 2) as usize).div_ceil(32))
            .map(|i| read_u32(block(img, DD.bitmap_block()), 4 + 4 * i).count_ones())
            .sum()
    };
    assert!(
        free(&master_fs(&exe, "d", "D", FileSystem::Ffs).unwrap())
            > free(&master(&exe, "d", "D").unwrap()),
        "FFS should leave more blocks free than OFS for the same file"
    );
}

#[test]
fn round_trips_a_file_needing_extension_blocks() {
    // >72 data blocks: the header's 72 pointer slots overflow into at least
    // one extension block. A general writer must handle files of any size.
    let exe: Vec<u8> = (0..OFS_DATA * 80 + 7).map(|i| (i % 251) as u8).collect();
    let img = master(&exe, "huge", "Huge").unwrap();
    assert_checksums(&img, "huge");
    assert_eq!(read_file(&img, "huge"), exe);

    // The extension chain is well-formed and self-checksummed.
    let hdr = read_u32(block(&img, DD.root_block()), 24 + 4 * name_hash("huge"));
    let mut ext = read_u32(block(&img, hdr), BSIZE - 8);
    let mut ext_seen = 0;
    while ext != 0 {
        let b = block(&img, ext);
        assert_eq!(read_u32(b, 0), T_LIST, "ext block {ext} type");
        assert_eq!(read_u32(b, 20), checksum(b, 20), "ext checksum {ext}");
        ext = read_u32(b, BSIZE - 8);
        ext_seen += 1;
    }
    assert!(ext_seen >= 1, "expected at least one extension block");
}

#[test]
fn dir_insert_chains_on_hash_collision() {
    // Two distinct names that hash to the same slot must both stay
    // reachable: the first in the slot, the second on its hash_chain. This
    // is what makes the writer correct for *any* set of names.
    let mut seen: Vec<(usize, String)> = Vec::new();
    let (mut first, mut second) = (None, None);
    for i in 0..4000u32 {
        let n = format!("f{i}");
        let slot = name_hash(&n);
        if let Some((_, prev)) = seen.iter().find(|(s, _)| *s == slot) {
            first = Some(prev.clone());
            second = Some(n);
            break;
        }
        seen.push((slot, n));
    }
    let (first, second) = (first.unwrap(), second.unwrap());
    assert_eq!(name_hash(&first), name_hash(&second));

    let mut img = vec![0u8; DD.blocks() as usize * BSIZE];
    let parent = DD.root_block();
    dir_insert(&mut img, parent, 100, &first);
    dir_insert(&mut img, parent, 101, &second);
    let slot = 24 + 4 * name_hash(&first);
    assert_eq!(read_u32(block(&img, parent), slot), 100, "slot holds first");
    assert_eq!(
        read_u32(block(&img, 100), BSIZE - 16),
        101,
        "second chains off first"
    );
    assert_eq!(
        read_u32(block(&img, 101), BSIZE - 16),
        0,
        "chain terminates"
    );
}

#[test]
fn deterministic() {
    let exe = vec![0xa5u8; 5000];
    assert_eq!(
        master(&exe, "d", "D").unwrap(),
        master(&exe, "d", "D").unwrap()
    );
}

#[test]
fn volume_writes_a_nested_multi_file_tree() {
    let mut vol = Volume::new("Tree", FileSystem::Ofs);
    vol.add_file("readme", b"top-level file\n").unwrap();
    vol.add_file("c/list", &vec![0x42u8; 2000]).unwrap(); // multi-block, nested
    vol.add_file("c/util/deep", b"deeply nested\n").unwrap(); // two dirs down
    vol.add_file("s/startup-sequence", b"c/list\n").unwrap();
    vol.set_bootable(true);
    let img = vol.build().unwrap();

    assert_eq!(
        read_file_via_ptrs(&img, "readme", FileSystem::Ofs),
        b"top-level file\n"
    );
    assert_eq!(
        read_file_via_ptrs(&img, "c/list", FileSystem::Ofs),
        vec![0x42u8; 2000]
    );
    assert_eq!(
        read_file_via_ptrs(&img, "c/util/deep", FileSystem::Ofs),
        b"deeply nested\n"
    );
    assert_eq!(
        read_file_via_ptrs(&img, "s/startup-sequence", FileSystem::Ofs),
        b"c/list\n"
    );
    // Intermediate paths are directories.
    assert_eq!(
        read_u32(block(&img, resolve(&img, "c")), BSIZE - 4),
        ST_USERDIR
    );
    assert_eq!(
        read_u32(block(&img, resolve(&img, "c/util")), BSIZE - 4),
        ST_USERDIR
    );
    assert!(vol.build().unwrap() == img, "deterministic");
}

#[test]
fn volume_rejects_bad_paths() {
    let mut vol = Volume::new("V", FileSystem::Ofs);
    vol.add_file("a", b"1").unwrap();
    assert!(matches!(
        vol.add_file("a", b"2"),
        Err(Error::BadPath { .. })
    )); // duplicate
    assert!(matches!(
        vol.add_file("a/b", b"3"),
        Err(Error::BadPath { .. })
    )); // through a file
    assert!(matches!(vol.add_file("", b"x"), Err(Error::BadPath { .. }))); // empty
    assert!(
        vol.add_file(&format!("{}/x", "n".repeat(31)), b"y")
            .is_err()
    ); // bad component
}

#[test]
fn data_disk_is_mountable_but_not_bootable() {
    let mut vol = Volume::new("Data", FileSystem::Ofs);
    vol.add_file("notes", b"hello\n").unwrap();
    let img = vol.build().unwrap(); // bootable defaults to false
    assert_eq!(&img[0..4], b"DOS\0");
    // The boot checksum is valid, but there is no bootstrap to run.
    let mut probe = img[..1024].to_vec();
    put_u32(&mut probe, 4, 0);
    assert_eq!(
        boot_checksum(&probe),
        read_u32(&img, 4),
        "data-disk boot checksum"
    );
    assert!(
        img[8..1024].iter().all(|&b| b == 0),
        "no bootstrap on a data disk"
    );
    assert_eq!(
        read_file_via_ptrs(&img, "notes", FileSystem::Ofs),
        b"hello\n"
    );
}

#[test]
fn volume_handles_empty_files() {
    let mut vol = Volume::new("E", FileSystem::Ofs);
    vol.add_file("empty", b"").unwrap();
    let img = vol.build().unwrap();
    let hdr = resolve(&img, "empty");
    assert_eq!(read_u32(block(&img, hdr), BSIZE - 188), 0, "byte_size 0");
    assert_eq!(read_u32(block(&img, hdr), 16), 0, "first_data 0");
    assert_eq!(read_u32(block(&img, hdr), 8), 0, "high_seq 0");
    assert!(read_file_via_ptrs(&img, "empty", FileSystem::Ofs).is_empty());
}

#[test]
fn rejects_disk_full_and_bad_names() {
    // Larger than an 880K disk can hold: a typed disk-full error, not a
    // panic or a corrupt image.
    let too_big = vec![0u8; BSIZE * DD.blocks() as usize];
    assert!(master(&too_big, "x", "X").is_err());
    assert!(master(b"z", "", "V").is_err());
    assert!(master(b"z", &"n".repeat(31), "V").is_err());
}

#[test]
fn disk_reads_back_a_volume() {
    let mut vol = Volume::new("RoundTrip", FileSystem::Ofs);
    vol.add_file("readme", b"top\n").unwrap();
    vol.add_file("c/big", &vec![7u8; 3000]).unwrap(); // multi-block
    vol.add_file("c/util/deep", b"deep\n").unwrap(); // two dirs down
    let img = vol.build().unwrap();

    let disk = Disk::open(&img).unwrap();
    assert_eq!(disk.filesystem(), FileSystem::Ofs);
    assert_eq!(disk.label(), "RoundTrip");
    assert_eq!(disk.read("readme").unwrap(), b"top\n");
    assert_eq!(disk.read("c/big").unwrap(), vec![7u8; 3000]);
    assert_eq!(disk.read("c/util/deep").unwrap(), b"deep\n");
    disk.verify().unwrap();

    // list reports kinds and sizes.
    let mut top = disk.list("").unwrap();
    top.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        top[0],
        Entry {
            name: "c".into(),
            kind: EntryKind::Directory,
            size: 0
        }
    );
    assert_eq!(
        top[1],
        Entry {
            name: "readme".into(),
            kind: EntryKind::File,
            size: 4
        }
    );
    let c = disk.list("c").unwrap();
    assert!(
        c.iter()
            .any(|e| e.name == "util" && e.kind == EntryKind::Directory)
    );
    assert!(c.iter().any(|e| e.name == "big" && e.size == 3000));
}

#[test]
fn disk_round_trips_both_filesystems() {
    let exe: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
    for fs in [FileSystem::Ofs, FileSystem::Ffs] {
        let img = master_fs(&exe, "game", "Game", fs).unwrap();
        let disk = Disk::open(&img).unwrap();
        assert_eq!(disk.filesystem(), fs);
        assert_eq!(disk.read("game").unwrap(), exe);
        assert_eq!(disk.read("s/startup-sequence").unwrap(), b"game\n");
        disk.verify().unwrap();
    }
}

#[test]
fn disk_rejects_garbage_and_bad_paths() {
    assert!(matches!(
        Disk::open(&[0u8; 100]),
        Err(Error::Corrupt { .. })
    )); // wrong size
    let blank = vec![0u8; DD.blocks() as usize * BSIZE];
    assert!(matches!(Disk::open(&blank), Err(Error::Corrupt { .. }))); // no DOS sig

    let img = master(b"hello world payload", "g", "G").unwrap();
    let disk = Disk::open(&img).unwrap();
    assert!(matches!(disk.read("nope"), Err(Error::NotFound { .. })));
    assert!(matches!(disk.read("s"), Err(Error::BadPath { .. }))); // a dir, not a file
    assert!(matches!(disk.list("g"), Err(Error::BadPath { .. }))); // a file, not a dir
    disk.verify().unwrap();
}

#[test]
fn disk_verify_catches_a_flipped_byte() {
    let img = master(b"payload", "g", "G").unwrap();
    // Flip a byte in the root block's name area: open still succeeds (type
    // and secondary-type intact) but the block checksum no longer matches.
    let mut corrupt = img.clone();
    corrupt[DD.root_block() as usize * BSIZE + (BSIZE - 70)] ^= 0xff;
    let disk = Disk::open(&corrupt).unwrap();
    assert!(matches!(disk.verify(), Err(Error::Corrupt { .. })));
}

/// `unwrap_err` would need `Debug` on `Disk`, and a derived one would print the
/// whole disk image.
fn err_of(result: Result<Disk<'_>, Error>) -> Error {
    match result {
        Err(err) => err,
        Ok(_) => panic!("expected a rejection, got a disk"),
    }
}

/// The reported case (emu198x/emu198x#1192): an IPF was rejected as a
/// wrong-sized ADF, which sent the reader to check a disk image that was never
/// at fault. Magic verified against a real SPS dump.
#[test]
fn an_ipf_is_named_rather_than_measured() {
    let mut ipf = b"CAPS".to_vec();
    ipf.extend_from_slice(&[0, 0, 0, 0x0C, 0x1C, 0xD5, 0x73, 0xBA]);
    let err = err_of(Disk::open(&ipf));
    assert!(
        matches!(err, Error::UnsupportedContainer { format: "IPF", .. }),
        "{err:?}"
    );
    let message = err.to_string();
    assert!(message.contains("IPF"), "{message}");
    assert!(
        !message.contains("image size"),
        "the size is not what is wrong: {message}"
    );
}

#[test]
fn other_containers_are_named_too() {
    for (bytes, expected) in [
        (b"UAE-1ADF".to_vec(), "extended ADF"),
        (b"DMS!".to_vec(), "DMS"),
        (b"PK\x03\x04".to_vec(), "zip"),
        (vec![0x1f, 0x8b, 0x08, 0x00], "gzip"),
    ] {
        let err = err_of(Disk::open(&bytes));
        assert!(
            matches!(err, Error::UnsupportedContainer { format, .. } if format == expected),
            "{err:?}"
        );
        assert!(err.to_string().contains(expected), "{err}");
    }
}

/// A truncated or padded ADF is still an ADF, and the size complaint is the
/// right answer for it. A file shorter than any magic must not panic either.
#[test]
fn an_unrecognised_file_still_gets_the_size_complaint() {
    for bytes in [vec![0u8; 1024], vec![0x1f], Vec::new()] {
        let err = err_of(Disk::open(&bytes));
        assert!(
            matches!(
                err,
                Error::Corrupt {
                    what: "image size (neither a DD nor an HD floppy)"
                }
            ),
            "{err:?}"
        );
        assert!(err.to_string().contains("image size"), "{err}");
    }
}

// ---------------------------------------------------------------------------
// The raw layer: geometry, Image, ImageMut
// ---------------------------------------------------------------------------

use crate::geometry::{DD, Geometry, HD, RESERVED};
use crate::image::{Image, ImageMut};

/// The root-block formula reproduces the answer every 1980s AmigaDOS manual
/// gives for a DD floppy. That known answer is what makes it safe to trust the
/// same formula on media this crate has not seen — see [`Geometry::root_block`].
#[test]
fn geometry_reproduces_the_published_dd_numbers() {
    assert_eq!(DD.blocks(), 1760);
    assert_eq!(DD.len(), 901_120);
    assert_eq!(DD.root_block(), 880, "the documented DD root block");
    // The values this crate used to hardcode, now derived rather than fixed.
    assert_eq!(DD.bitmap_block(), 881);
    assert_eq!(DD.first_free(), 882);
}

/// HD is twice the sectors per track and nothing else: same cylinders, same
/// heads. The root lands halfway across, as on DD.
#[test]
fn geometry_derives_hd_from_the_same_formula() {
    assert_eq!(HD.cylinders, DD.cylinders);
    assert_eq!(HD.heads, DD.heads);
    assert_eq!(HD.sectors_per_track, 2 * DD.sectors_per_track);
    assert_eq!(HD.blocks(), 3520);
    assert_eq!(HD.len(), 1_802_240);
    assert_eq!(HD.root_block(), 1760);
}

/// One bitmap block still covers an HD disk. A bitmap block holds 508 bytes of
/// allocation bits — 4064 blocks' worth — and HD needs 3518. Stated because a
/// reader would otherwise reasonably expect a bitmap-extension chain.
#[test]
fn one_bitmap_block_covers_hd() {
    let bits_per_block = (BSIZE - 4) * 8;
    assert_eq!(bits_per_block, 4064);
    assert!(HD.blocks() as usize - 2 <= bits_per_block);
}

/// The CHS→LBA mapping is the whole of the conversion between the raw layer's
/// two ways of naming the same bytes. Checked against the offsets Emu198x's
/// own reader computes: `((cyl * HEADS + head) * spt + sector) * 512`.
#[test]
fn chs_addresses_agree_with_the_drives_own_arithmetic() {
    for geometry in [DD, HD] {
        let spt = geometry.sectors_per_track as u32;
        for (cyl, head, sector) in [
            (0u16, 0u8, 0u8),
            (0, 1, 0),
            (1, 0, 0),
            (1, 0, 3),
            (79, 1, 1),
        ] {
            let expected = (cyl as u32 * 2 + head as u32) * spt + sector as u32;
            assert_eq!(
                geometry.lba(cyl, head, sector),
                Some(expected),
                "{geometry:?} {cyl}/{head}/{sector}"
            );
        }
        assert_eq!(geometry.lba(80, 0, 0), None, "past the last cylinder");
        assert_eq!(geometry.lba(0, 2, 0), None, "past the last head");
        assert_eq!(
            geometry.lba(0, 0, geometry.sectors_per_track),
            None,
            "past the last sector"
        );
    }
}

/// A track is one contiguous run of bytes, not a gathering of scattered
/// sectors. That is what lets an MFM encoder take the slice straight from the
/// image without copying it — so this asserts identity with the image's own
/// bytes, not merely equal content.
#[test]
fn a_track_is_contiguous_and_matches_its_sectors() {
    let bytes = master(b"payload", "g", "G").unwrap();
    let image = Image::open(&bytes).unwrap();

    let track = image.track(5, 1).unwrap();
    assert_eq!(track.len(), 11 * BSIZE);
    let first = image.sector(5, 1, 0).unwrap();
    assert!(
        std::ptr::eq(track.as_ptr(), first.as_ptr()),
        "the track starts at its first sector, in place"
    );
    for s in 0..11u8 {
        assert_eq!(
            &track[s as usize * BSIZE..][..BSIZE],
            image.sector(5, 1, s).unwrap(),
            "sector {s} within the track"
        );
    }
}

/// Blocks and CHS addresses reach the same bytes — the observation the whole
/// two-layer design rests on.
#[test]
fn blocks_and_sectors_address_the_same_bytes() {
    let bytes = master(b"payload", "g", "G").unwrap();
    let image = Image::open(&bytes).unwrap();
    for (cyl, head, sector) in [(0u16, 0u8, 0u8), (40, 1, 5), (79, 1, 10)] {
        let lba = DD.lba(cyl, head, sector).unwrap();
        assert!(std::ptr::eq(
            image.sector(cyl, head, sector).unwrap().as_ptr(),
            image.block(lba).unwrap().as_ptr()
        ));
    }
}

/// The raw layer accepts both geometries and names the shape it found.
#[test]
fn image_opens_dd_and_hd() {
    assert_eq!(Image::open(&vec![0u8; DD.len()]).unwrap().geometry(), DD);
    assert_eq!(Image::open(&vec![0u8; HD.len()]).unwrap().geometry(), HD);
}

/// Out-of-range addressing returns a typed error naming the coordinate at
/// fault, rather than indexing and panicking. This crate is destined for an FFI
/// boundary where unwinding is undefined behaviour, so this is load-bearing.
#[test]
fn out_of_range_addresses_are_errors_not_panics() {
    let bytes = vec![0u8; DD.len()];
    let image = Image::open(&bytes).unwrap();
    assert!(matches!(
        image.sector(80, 0, 0),
        Err(Error::OutOfBounds {
            what: "cylinder",
            got: 80,
            limit: 80
        })
    ));
    assert!(matches!(
        image.sector(0, 2, 0),
        Err(Error::OutOfBounds { what: "head", .. })
    ));
    assert!(matches!(
        image.sector(0, 0, 11),
        Err(Error::OutOfBounds { what: "sector", .. })
    ));
    assert!(matches!(
        image.track(80, 0),
        Err(Error::OutOfBounds {
            what: "cylinder",
            ..
        })
    ));
    assert!(matches!(
        image.block(1760),
        Err(Error::OutOfBounds {
            what: "block",
            got: 1760,
            limit: 1760
        })
    ));
    // The last valid address of each kind still works.
    assert!(image.sector(79, 1, 10).is_ok());
    assert!(image.block(1759).is_ok());
}

/// The raw layer rejects the same containers the filesystem layer does — the
/// #1192 guard lives beneath both, so there is one copy of it.
#[test]
fn the_raw_layer_names_containers_too() {
    let err = match Image::open(b"CAPS\0\0\0\x0c") {
        Err(err) => err,
        Ok(_) => panic!("expected a rejection"),
    };
    assert!(
        matches!(err, Error::UnsupportedContainer { format: "IPF", .. }),
        "{err:?}"
    );
}

/// A real Amiga writes to floppies. Sectors go in and come back out, in place.
#[test]
fn image_mut_round_trips_a_sector() {
    let mut bytes = ImageMut::blank(DD);
    assert_eq!(bytes.len(), 901_120);
    let mut image = ImageMut::open(&mut bytes).unwrap();
    assert_eq!(image.geometry(), DD);

    let payload: Vec<u8> = (0..BSIZE).map(|i| (i % 251) as u8).collect();
    image.write_sector(40, 1, 5, &payload).unwrap();
    assert_eq!(image.as_image().sector(40, 1, 5).unwrap(), &payload[..]);

    // sector_mut reaches the same bytes.
    image.sector_mut(40, 1, 5).unwrap()[0] = 0xff;
    assert_eq!(image.as_image().sector(40, 1, 5).unwrap()[0], 0xff);

    // Nothing else moved.
    let neighbour = DD.lba(40, 1, 6).unwrap();
    assert!(
        image
            .as_image()
            .block(neighbour)
            .unwrap()
            .iter()
            .all(|&b| b == 0)
    );
}

/// A partial sector is a typed error, not a panic. `copy_from_slice` would
/// panic on a length mismatch, which is exactly the shape of failure this
/// layer exists to remove.
#[test]
fn a_short_sector_write_is_refused() {
    let mut bytes = ImageMut::blank(DD);
    let mut image = ImageMut::open(&mut bytes).unwrap();
    assert!(matches!(
        image.write_sector(0, 0, 0, &[0u8; 511]),
        Err(Error::BadSectorLength { got: 511 })
    ));
    assert!(matches!(
        image.write_sector(0, 0, 0, &[]),
        Err(Error::BadSectorLength { got: 0 })
    ));
    assert!(matches!(
        image.write_sector(80, 0, 0, &[0u8; BSIZE]),
        Err(Error::OutOfBounds { .. })
    ));
}

/// `Image::verify` answers the only two questions this layer can, and a write
/// really can change the answer to one of them.
#[test]
fn raw_verify_catches_a_write_that_disguises_the_container() {
    let mut bytes = master(b"payload", "g", "G").unwrap();
    Image::open(&bytes).unwrap().verify().unwrap();

    let mut image = ImageMut::open(&mut bytes).unwrap();
    let boot = image.sector_mut(0, 0, 0).unwrap();
    boot[..4].copy_from_slice(b"CAPS");
    let err = image.as_image().verify().unwrap_err();
    assert!(
        matches!(err, Error::UnsupportedContainer { format: "IPF", .. }),
        "{err:?}"
    );
}

/// The two layers are views of one thing: a `Disk` hands back the `Image` it
/// was built on, and an `Image` can be interpreted as a `Disk`.
#[test]
fn disk_and_image_are_two_views_of_one_disk() {
    let bytes = master(b"payload", "g", "G").unwrap();

    let disk = Disk::open(&bytes).unwrap();
    assert_eq!(disk.geometry(), DD);
    let image = disk.image();
    assert!(std::ptr::eq(image.bytes().as_ptr(), bytes.as_ptr()));

    // The root block the filesystem uses is the block the raw layer addresses.
    assert_eq!(
        image.block(DD.root_block()).unwrap(),
        &bytes[DD.root_block() as usize * BSIZE..][..BSIZE]
    );

    let again = Disk::from_image(Image::open(&bytes).unwrap()).unwrap();
    assert_eq!(again.label(), disk.label());
    assert_eq!(again.read("g").unwrap(), b"payload");
}

/// A `Geometry` is a plain value: anyone can name one, and the arithmetic
/// holds for shapes this crate has no filesystem support for.
#[test]
fn geometry_is_arithmetic_not_a_lookup_table() {
    let odd = Geometry {
        cylinders: 40,
        heads: 1,
        sectors_per_track: 9,
    };
    assert_eq!(odd.blocks(), 360);
    assert_eq!(odd.len(), 184_320);
    assert_eq!(odd.root_block(), (360 - 1 + 2) >> 1);
    assert_eq!(odd.lba(39, 0, 8), Some(359));
    assert_eq!(odd.lba(0, 1, 0), None);
}

/// A whole HD volume, written and read back by this crate: nested directories,
/// a file large enough to need extension blocks, both filesystems.
#[test]
fn hd_volumes_round_trip_through_both_layers() {
    for fs in [FileSystem::Ofs, FileSystem::Ffs] {
        let big: Vec<u8> = (0..fs.data_capacity() * 80 + 7)
            .map(|i| (i % 251) as u8)
            .collect();
        let mut vol = Volume::new("BigDisk", fs);
        vol.set_geometry(HD);
        vol.add_file("readme", b"top\n").unwrap();
        vol.add_file("c/big", &big).unwrap();
        vol.add_file("c/util/deep", b"deep\n").unwrap();
        let img = vol.build().unwrap();
        assert_eq!(img.len(), 1_802_240, "{fs:?}");

        // The raw layer sees HD geometry; the last sector of the last track is
        // addressable, which it would not be under DD.
        let image = Image::open(&img).unwrap();
        assert_eq!(image.geometry(), HD);
        assert_eq!(image.track(79, 1).unwrap().len(), 22 * BSIZE);
        assert!(image.sector(79, 1, 21).is_ok());

        // The filesystem layer finds its root where the formula says.
        let disk = Disk::open(&img).unwrap();
        assert_eq!(disk.geometry(), HD);
        assert_eq!(disk.geometry().root_block(), 1760);
        assert_eq!(disk.label(), "BigDisk");
        assert_eq!(disk.filesystem(), fs);
        assert_eq!(disk.read("readme").unwrap(), b"top\n");
        assert_eq!(disk.read("c/big").unwrap(), big);
        assert_eq!(disk.read("c/util/deep").unwrap(), b"deep\n");
        disk.verify().unwrap();

        // The root block really is at 1760 and not merely reported as such.
        assert_eq!(
            read_u32(block(&img, 1760), BSIZE - 4),
            ST_ROOT,
            "root block at the HD position"
        );
        assert_eq!(read_u32(block(&img, 1760), 0), T_HEADER);
    }
}

/// HD holds what DD cannot — the only reason to want it.
#[test]
fn hd_holds_what_dd_cannot() {
    let payload = vec![0x42u8; 1_200_000];

    let mut dd = Volume::new("Small", FileSystem::Ffs);
    dd.add_file("payload", &payload).unwrap();
    assert!(
        matches!(dd.build(), Err(Error::DiskFull { .. })),
        "1.2 MB does not fit an 880 KB floppy"
    );

    let mut hd = Volume::new("Big", FileSystem::Ffs);
    hd.set_geometry(HD);
    hd.add_file("payload", &payload).unwrap();
    let img = hd.build().unwrap();
    assert_eq!(Disk::open(&img).unwrap().read("payload").unwrap(), payload);
}

/// A volume fills its disk, not half of it.
///
/// Allocation used to walk upward from the root block and never revisit what
/// lay below, so a DD floppy topped out near 432 KB — under half its media —
/// and the whole lower half sat unreachable. Now that allocation goes through
/// the bitmap, both halves are in play.
#[test]
fn a_volume_fills_its_disk() {
    for (geometry, media) in [(DD, 901_120usize), (HD, 1_802_240)] {
        // Comfortably more than half the disk, which is what used to fail.
        let payload = vec![0x5au8; media * 3 / 4];
        let mut vol = Volume::new("Full", FileSystem::Ffs);
        vol.set_geometry(geometry);
        vol.add_file("payload", &payload).unwrap();
        let img = vol.build().unwrap();

        let disk = Disk::open(&img).unwrap();
        assert_eq!(disk.read("payload").unwrap(), payload);
        disk.verify().unwrap();

        // Blocks below the root really are in use now.
        let below = (RESERVED..geometry.root_block())
            .filter(|&n| block(&img, n).iter().any(|&b| b != 0))
            .count();
        assert!(below > 0, "{geometry:?} still leaves its lower half empty");
    }
}

/// HD writes stay deterministic — the contract that makes committed `.adf`
/// deliverables byte-reproducible does not lapse on bigger media.
#[test]
fn hd_output_is_deterministic() {
    let build = || {
        let mut vol = Volume::new("D", FileSystem::Ofs);
        vol.set_geometry(HD);
        vol.add_file("a", &vec![0xa5u8; 5000]).unwrap();
        vol.build().unwrap()
    };
    assert_eq!(build(), build());
}

/// The acceptance test the spec asks for: read an HD ADF this crate did not
/// write.
///
/// Every HD fact in the spec is derived — from Commodore's `rootblock.c`
/// formula, and from arithmetic about how many blocks a bitmap block covers —
/// and derived facts about on-disk layout have been wrong in this family
/// before. So HD support is not claimed on the strength of the formula alone.
///
/// Ignored by default and reads its image from `ADF_HD_IMAGE`, because no media
/// lives in this repository. Run it as:
///
/// ```text
/// ADF_HD_IMAGE=/path/to/disk.adf cargo test -- --ignored hd_image_from_the_wild
/// ```
///
/// It was run before HD support was claimed, against images written by
/// amitools' `xdftool` — an independent implementation, and the same one this
/// crate's DD layout facts were originally taken from. Both filesystems were
/// checked. It found the root block at 1760, one bitmap block and no bitmap
/// extension, exactly as the formula predicts.
#[test]
#[ignore = "needs an HD ADF; set ADF_HD_IMAGE"]
fn hd_image_from_the_wild() {
    let Ok(path) = std::env::var("ADF_HD_IMAGE") else {
        panic!("set ADF_HD_IMAGE to the path of an HD .adf");
    };
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"));

    let image = Image::open(&bytes).unwrap();
    assert_eq!(image.geometry(), HD, "{path} is not an HD image");
    assert_eq!(image.geometry().blocks(), 3520);
    assert_eq!(image.track(79, 1).unwrap().len(), 22 * BSIZE);

    let disk = Disk::from_image(image).unwrap();
    assert_eq!(disk.geometry().root_block(), 1760);

    // The root block is where the formula says, in the image's own bytes.
    let root = block(&bytes, 1760);
    assert_eq!(read_u32(root, 0), T_HEADER, "root primary type");
    assert_eq!(read_u32(root, BSIZE - 4), ST_ROOT, "root secondary type");

    // One bitmap block covers the whole disk: bm_pages[0] is set, the rest are
    // empty, and there is no bitmap-extension chain.
    assert_eq!(
        read_u32(root, BSIZE - 200),
        0xffff_ffff,
        "bitmap valid flag"
    );
    assert_eq!(read_u32(root, BSIZE - 196), 1761, "bm_pages[0]");
    for i in 1..25 {
        assert_eq!(read_u32(root, BSIZE - 196 + 4 * i), 0, "bm_pages[{i}]");
    }
    assert_eq!(read_u32(root, BSIZE - 96), 0, "no bitmap extension");

    // Everything the volume holds reads back, and every checksum agrees.
    fn walk(disk: &Disk, path: &str) {
        for e in disk.list(path).unwrap() {
            let child = if path.is_empty() {
                e.name.clone()
            } else {
                format!("{path}/{}", e.name)
            };
            match e.kind {
                EntryKind::Directory => walk(disk, &child),
                EntryKind::File => {
                    let bytes = disk.read(&child).unwrap();
                    assert_eq!(bytes.len(), e.size as usize, "{child} size");
                }
            }
        }
    }
    walk(&disk, "");
    disk.verify().unwrap();
}

/// A disk formatted but never made bootable carries no bootstrap and a zero
/// boot-checksum field, and `verify` must accept it.
///
/// The ROM validates the boot block only when it is about to run the
/// bootstrap, so a disk with nothing to run has nothing to check. AmigaDOS
/// `Format` leaves the field zero until `Install` writes the bootstrap.
/// amitools does the same — `BootBlock.write` computes a checksum only when
/// boot code is present and otherwise stores zero — so *every* data disk
/// xdftool produces looked corrupt to this crate until this was fixed. Verified
/// against xdftool-formatted DD and HD images, OFS and FFS, none of which this
/// repository stores.
#[test]
fn a_formatted_but_uninstalled_disk_is_not_corrupt() {
    let mut vol = Volume::new("Data", FileSystem::Ofs);
    vol.add_file("notes", b"hello\n").unwrap();
    let mut img = vol.build().unwrap(); // bootable defaults to false

    // Exactly what Format-without-Install leaves behind: no bootstrap, and a
    // zero checksum field rather than one computed over the empty block.
    put_u32(&mut img, 4, 0);
    assert!(
        img[12..1024].iter().all(|&b| b == 0),
        "a data disk carries no bootstrap"
    );

    let disk = Disk::open(&img).unwrap();
    disk.verify().unwrap();
    assert_eq!(disk.read("notes").unwrap(), b"hello\n");
}

/// The leniency is narrow. A disk that *does* carry a bootstrap, or that stores
/// a checksum at all, still has to have the right one — otherwise zeroing the
/// field would be a way to hide a corrupt boot block.
#[test]
fn a_wrong_boot_checksum_is_still_corrupt() {
    // A bootable disk with its checksum zeroed: there is a bootstrap, so the
    // checksum is not optional.
    let mut img = master(b"payload", "g", "G").unwrap();
    assert!(img[12..1024].iter().any(|&b| b != 0), "bootstrap present");
    put_u32(&mut img, 4, 0);
    let disk = Disk::open(&img).unwrap();
    assert!(matches!(
        disk.verify(),
        Err(Error::Corrupt {
            what: "boot checksum"
        })
    ));

    // A data disk with a stored-but-wrong checksum: still wrong.
    let mut vol = Volume::new("Data", FileSystem::Ofs);
    vol.add_file("notes", b"hello\n").unwrap();
    let mut img = vol.build().unwrap();
    put_u32(&mut img, 4, 0xdead_beef);
    let disk = Disk::open(&img).unwrap();
    assert!(matches!(
        disk.verify(),
        Err(Error::Corrupt {
            what: "boot checksum"
        })
    ));
}

// ---------------------------------------------------------------------------
// DiskMut: changing a disk that already exists
// ---------------------------------------------------------------------------

use crate::mutate::DiskMut;

/// The gap this fills: `Volume` can only build a disk from nothing. Opening one
/// that exists and changing it is what an emulator writing a save file, or a
/// tool replacing one asset, actually needs.
#[test]
fn disk_mut_adds_to_a_disk_that_already_exists() {
    let mut vol = Volume::new("Work", FileSystem::Ofs);
    vol.add_file("readme", b"original\n").unwrap();
    let mut img = vol.build().unwrap();

    {
        let mut disk = DiskMut::open(&mut img).unwrap();
        disk.create_dir("saves/slot1").unwrap();
        disk.write_file("saves/slot1/game.sav", b"level 3").unwrap();
        disk.write_file("notes", b"added later\n").unwrap();
    }

    let disk = Disk::open(&img).unwrap();
    assert_eq!(disk.read("readme").unwrap(), b"original\n", "untouched");
    assert_eq!(disk.read("saves/slot1/game.sav").unwrap(), b"level 3");
    assert_eq!(disk.read("notes").unwrap(), b"added later\n");
    disk.verify().unwrap();

    // The new directories are real directories, reachable by listing.
    let saves = disk.list("saves").unwrap();
    assert_eq!(saves.len(), 1);
    assert_eq!(saves[0].kind, EntryKind::Directory);
}

/// Replacing a file frees what it held. Rewriting the same file many times must
/// not leak blocks, or a save-game slot would eventually fill the disk.
#[test]
fn rewriting_a_file_does_not_leak_blocks() {
    for fs in [FileSystem::Ofs, FileSystem::Ffs] {
        let mut img = Volume::new("Save", fs).build().unwrap();
        let mut disk = DiskMut::open(&mut img).unwrap();

        disk.write_file("game.sav", &vec![0u8; 20_000]).unwrap();
        let after_first = disk.free_blocks();

        for i in 0..50u8 {
            disk.write_file("game.sav", &vec![i; 20_000]).unwrap();
        }
        assert_eq!(
            disk.free_blocks(),
            after_first,
            "{fs:?}: 50 rewrites of the same size must cost nothing"
        );
        assert_eq!(disk.as_disk().read("game.sav").unwrap(), vec![49u8; 20_000]);
        disk.as_disk().verify().unwrap();
    }
}

/// Deleting returns every block the entry held — data, extension chain and
/// header — so a disk emptied and refilled behaves like a fresh one.
#[test]
fn deleting_returns_every_block() {
    let mut img = Volume::new("Churn", FileSystem::Ofs).build().unwrap();
    let mut disk = DiskMut::open(&mut img).unwrap();
    let empty = disk.free_blocks();

    // Large enough to need extension blocks, so the whole chain is exercised.
    let big: Vec<u8> = (0..488 * 100).map(|i| (i % 251) as u8).collect();
    disk.write_file("a/b/big", &big).unwrap();
    assert!(disk.free_blocks() < empty);
    assert_eq!(disk.as_disk().read("a/b/big").unwrap(), big);

    disk.delete("a/b/big").unwrap();
    disk.delete("a/b").unwrap();
    disk.delete("a").unwrap();
    assert_eq!(disk.free_blocks(), empty, "everything came back");
    assert!(disk.as_disk().list("").unwrap().is_empty());
    disk.as_disk().verify().unwrap();
}

/// A disk written, emptied and rewritten matches one written straight — the
/// determinism contract surviving mutation, not just construction.
#[test]
fn a_reused_disk_matches_a_fresh_one() {
    let mut vol = Volume::new("Same", FileSystem::Ofs);
    vol.add_file("keep", b"keep\n").unwrap();
    let fresh = {
        let mut v = Volume::new("Same", FileSystem::Ofs);
        v.add_file("keep", b"keep\n").unwrap();
        v.add_file("later", &vec![7u8; 3000]).unwrap();
        v.build().unwrap()
    };

    let mut img = vol.build().unwrap();
    {
        let mut disk = DiskMut::open(&mut img).unwrap();
        disk.write_file("scratch", &vec![1u8; 9000]).unwrap();
        disk.delete("scratch").unwrap();
        disk.write_file("later", &vec![7u8; 3000]).unwrap();
    }
    assert_eq!(img, fresh, "a reused disk is byte-identical to a fresh one");
}

/// Refusing to delete a non-empty directory. Removing it would orphan its
/// children, and silently leaking their blocks is worse than saying no.
#[test]
fn a_non_empty_directory_is_not_deleted() {
    let mut img = Volume::new("Tree", FileSystem::Ofs).build().unwrap();
    let mut disk = DiskMut::open(&mut img).unwrap();
    disk.write_file("d/file", b"x").unwrap();

    assert!(matches!(
        disk.delete("d"),
        Err(Error::BadPath {
            reason: "directory is not empty",
            ..
        })
    ));
    assert_eq!(disk.as_disk().read("d/file").unwrap(), b"x");

    disk.delete("d/file").unwrap();
    disk.delete("d").unwrap();
    assert!(disk.as_disk().list("").unwrap().is_empty());
}

/// Names that land in the same hash slot must survive being added and removed
/// in any order — the sibling chain has to be mended, not just truncated.
#[test]
fn removing_from_the_middle_of_a_hash_chain_mends_it() {
    // Three names sharing one of the 72 slots.
    let mut by_slot: std::collections::HashMap<usize, Vec<String>> =
        std::collections::HashMap::new();
    let mut trio = None;
    for i in 0..20_000u32 {
        let n = format!("f{i}");
        let e = by_slot.entry(name_hash(&n)).or_default();
        e.push(n);
        if e.len() == 3 {
            trio = Some(e.clone());
            break;
        }
    }
    let trio = trio.expect("three colliding names");
    assert_eq!(name_hash(&trio[0]), name_hash(&trio[2]));

    let mut img = Volume::new("Hash", FileSystem::Ofs).build().unwrap();
    let mut disk = DiskMut::open(&mut img).unwrap();
    for n in &trio {
        disk.write_file(n, n.as_bytes()).unwrap();
    }
    // Remove the middle one: the chain must close over it.
    disk.delete(&trio[1]).unwrap();
    assert_eq!(disk.as_disk().read(&trio[0]).unwrap(), trio[0].as_bytes());
    assert_eq!(disk.as_disk().read(&trio[2]).unwrap(), trio[2].as_bytes());
    assert!(disk.as_disk().read(&trio[1]).is_err());

    // Then the head, then the tail.
    disk.delete(&trio[0]).unwrap();
    assert_eq!(disk.as_disk().read(&trio[2]).unwrap(), trio[2].as_bytes());
    disk.delete(&trio[2]).unwrap();
    assert!(disk.as_disk().list("").unwrap().is_empty());
    disk.as_disk().verify().unwrap();
}

/// A full disk refuses cleanly. A partly written file linked to nothing would
/// be worse than an error, so the cost is checked before any of it is spent.
#[test]
fn a_full_disk_refuses_without_half_writing() {
    let mut img = Volume::new("Full", FileSystem::Ffs).build().unwrap();
    let mut disk = DiskMut::open(&mut img).unwrap();
    disk.write_file("hog", &vec![0u8; 800_000]).unwrap();

    let before = disk.free_blocks();
    let err = disk.write_file("toobig", &vec![1u8; 400_000]).unwrap_err();
    assert!(matches!(err, Error::DiskFull { .. }), "{err:?}");
    assert_eq!(disk.free_blocks(), before, "a refusal costs nothing");
    assert!(disk.as_disk().read("toobig").is_err());
    disk.as_disk().verify().unwrap();
}

/// Formatting a blank image produces a mountable volume — the entry point that
/// makes an HD disk reachable without going through `Volume`.
#[test]
fn format_produces_a_mountable_volume() {
    for geometry in [DD, HD] {
        for fs in [FileSystem::Ofs, FileSystem::Ffs] {
            let mut bytes = ImageMut::blank(geometry);
            {
                let mut disk = DiskMut::format(&mut bytes, "Fresh", fs, false).unwrap();
                assert_eq!(disk.geometry(), geometry);
                disk.write_file("hello", b"world\n").unwrap();
            }
            let disk = Disk::open(&bytes).unwrap();
            assert_eq!(disk.label(), "Fresh");
            assert_eq!(disk.filesystem(), fs);
            assert_eq!(disk.geometry(), geometry);
            assert_eq!(disk.read("hello").unwrap(), b"world\n");
            disk.verify().unwrap();
        }
    }
}

/// The mutator writes through to the raw layer, because they are one disk.
#[test]
fn mutation_is_visible_at_the_raw_layer() {
    let mut img = Volume::new("Both", FileSystem::Ofs).build().unwrap();
    let root = DD.root_block();
    {
        let mut disk = DiskMut::open(&mut img).unwrap();
        disk.write_file("f", b"hello").unwrap();
        // The header landed in the first free block above the root.
        let image = disk.as_disk().image();
        assert_eq!(read_u32(image.block(root + 2).unwrap(), 0), T_HEADER);
    }
    assert_eq!(read_u32(block(&img, root + 2), 0), T_HEADER);
}

/// Bad paths are refused rather than half-applied.
#[test]
fn disk_mut_rejects_bad_paths() {
    let mut img = Volume::new("V", FileSystem::Ofs).build().unwrap();
    let mut disk = DiskMut::open(&mut img).unwrap();
    disk.write_file("a", b"1").unwrap();

    assert!(matches!(
        disk.write_file("", b"x"),
        Err(Error::BadPath { .. })
    ));
    assert!(matches!(
        disk.write_file("a/b", b"x"),
        Err(Error::BadPath { .. })
    )); // through a file
    assert!(matches!(disk.create_dir("a"), Err(Error::BadPath { .. }))); // a file holds the name
    assert!(matches!(disk.delete("nope"), Err(Error::NotFound { .. })));
    assert!(
        disk.write_file(&format!("{}/x", "n".repeat(31)), b"y")
            .is_err()
    );
    disk.as_disk().verify().unwrap();
}

/// An HD disk takes mutation as readily as a DD one — the acceptance bar is
/// read, write and verify at both layers on both geometries.
#[test]
fn hd_disks_are_mutable_too() {
    let mut bytes = ImageMut::blank(HD);
    {
        let mut disk = DiskMut::format(&mut bytes, "BigWork", FileSystem::Ffs, false).unwrap();
        let payload = vec![0x33u8; 1_000_000];
        disk.write_file("data/payload", &payload).unwrap();
        assert_eq!(disk.as_disk().read("data/payload").unwrap(), payload);
        disk.delete("data/payload").unwrap();
        disk.write_file("data/small", b"tiny").unwrap();
    }
    let disk = Disk::open(&bytes).unwrap();
    assert_eq!(disk.geometry(), HD);
    assert_eq!(disk.read("data/small").unwrap(), b"tiny");
    disk.verify().unwrap();
}
