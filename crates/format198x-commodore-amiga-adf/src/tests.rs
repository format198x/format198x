use crate::error::*;
use crate::fs::*;
use crate::layout::*;
use crate::read::*;
use crate::write::*;

/// Follow the on-disk structure to read a top-level file's bytes back —
/// validating the root hash table, file header, data-pointer table, and
/// OFS data-block chain the way a real filesystem would.
fn read_file(img: &[u8], name: &str) -> Vec<u8> {
    let root = &img[ROOT_BLK as usize * BSIZE..][..BSIZE];
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
    let mut blk = ROOT_BLK;
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
    check(ROOT_BLK, 20);
    check(BITMAP_BLK, 0);
    // Walk root entries and their data.
    let root = &img[ROOT_BLK as usize * BSIZE..][..BSIZE];
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
    assert_eq!(img.len(), BLOCKS as usize * BSIZE);
    assert_eq!(&img[0..4], b"DOS\0");
    assert_eq!(
        read_u32(&img[ROOT_BLK as usize * BSIZE..], BSIZE - 4),
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
        (0..((BLOCKS - 2) as usize).div_ceil(32))
            .map(|i| read_u32(block(img, BITMAP_BLK), 4 + 4 * i).count_ones())
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
    let hdr = read_u32(block(&img, ROOT_BLK), 24 + 4 * name_hash("huge"));
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

    let mut img = vec![0u8; BLOCKS as usize * BSIZE];
    let parent = ROOT_BLK;
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
    let too_big = vec![0u8; BSIZE * BLOCKS as usize];
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
    let blank = vec![0u8; BLOCKS as usize * BSIZE];
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
    corrupt[ROOT_BLK as usize * BSIZE + (BSIZE - 70)] ^= 0xff;
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

use crate::geometry::{DD, Geometry, HD};
use crate::image::{Image, ImageMut};

/// The root-block formula reproduces the answer every 1980s AmigaDOS manual
/// gives for a DD floppy. That known answer is what makes it safe to trust the
/// same formula on media this crate has not seen — see [`Geometry::root_block`].
#[test]
fn geometry_reproduces_the_published_dd_numbers() {
    assert_eq!(DD.blocks(), 1760);
    assert_eq!(DD.len(), 901_120);
    assert_eq!(DD.root_block(), 880, "the documented DD root block");
    // The constants this crate has always used, now derived rather than fixed.
    assert_eq!(DD.root_block(), ROOT_BLK);
    assert_eq!(DD.bitmap_block(), BITMAP_BLK);
    assert_eq!(DD.blocks(), BLOCKS);
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

/// A geometry with no known filesystem layout is declined by name rather than
/// guessed at. Its sectors stay reachable through [`Image`].
#[test]
fn the_filesystem_layer_declines_a_shape_it_has_not_verified() {
    let bytes = vec![0u8; HD.len()];
    let image = Image::open(&bytes).unwrap();
    assert_eq!(image.geometry(), HD);
    assert_eq!(image.sector(79, 1, 21).unwrap().len(), BSIZE);

    let err = match Disk::from_image(image) {
        Err(err) => err,
        Ok(_) => panic!("expected a rejection"),
    };
    assert!(
        matches!(
            err,
            Error::UnsupportedGeometry {
                shape: "high-density"
            }
        ),
        "{err:?}"
    );
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
