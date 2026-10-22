use super::block;
use crate::geometry::RESERVED;
use crate::layout::{BSIZE, block_mut, checksum, put_u32, read_u32};
use crate::{DD, Disk, FileSystem, Geometry, HD, Problem, Volume};

fn set_free(image: &mut [u8], bitmap: u32, number: u32, free: bool) {
    let index = (number - RESERVED) as usize;
    let offset = 4 + 4 * (index / 32);
    let word = read_u32(block(image, bitmap), offset);
    let mask = 1 << (index % 32);
    put_u32(
        block_mut(image, bitmap),
        offset,
        if free { word | mask } else { word & !mask },
    );
}

fn rechecksum(image: &mut [u8], bitmap: u32) {
    let sum = checksum(block(image, bitmap), 0);
    put_u32(block_mut(image, bitmap), 0, sum);
}

fn fixture(fs: FileSystem, geometry: Geometry) -> (Vec<u8>, u32) {
    let mut volume = Volume::new("Slack", fs);
    volume.set_geometry(geometry);
    volume.add_file("file", b"payload").unwrap();
    let mut image = volume.build().unwrap();
    let root = geometry.root_block();
    let old_bitmap = geometry.bitmap_block();
    let bitmap = root + 100;
    let original = block(&image, old_bitmap).to_vec();
    block_mut(&mut image, bitmap).copy_from_slice(&original);
    put_u32(block_mut(&mut image, root), BSIZE - 196, bitmap);
    set_free(&mut image, bitmap, old_bitmap, true);
    set_free(&mut image, bitmap, bitmap, false);
    let root_sum = checksum(block(&image, root), 20);
    put_u32(block_mut(&mut image, root), 20, root_sum);

    let allocation_words = (geometry.blocks() - RESERVED).div_ceil(32) as usize;
    let slack_start = 4 + allocation_words * 4;
    assert!(block(&image, bitmap)[slack_start..].iter().all(|&b| b == 0));
    // Synthetic slack, not bytes copied from copyrighted native media.
    block_mut(&mut image, bitmap)[slack_start..].fill(0xa5);
    rechecksum(&mut image, bitmap);
    (image, bitmap)
}

#[test]
fn accepts_unused_bitmap_words_without_changing_file_contents() {
    for geometry in [DD, HD] {
        for fs in [FileSystem::Ofs, FileSystem::Ffs] {
            let (image, bitmap) = fixture(fs, geometry);
            let disk = Disk::open(&image).unwrap();
            assert_eq!(disk.volume_provenance().unwrap().bitmap_blocks, [bitmap]);
            assert!(disk.check().is_sound(), "{}", disk.check());
            disk.verify().unwrap();
            assert_eq!(disk.read("file").unwrap(), b"payload");
        }
    }
}

#[test]
fn unused_words_remain_part_of_the_bitmap_checksum() {
    let (mut image, bitmap) = fixture(FileSystem::Ofs, DD);
    block_mut(&mut image, bitmap)[BSIZE - 1] ^= 1;
    let disk = Disk::open(&image).unwrap();
    assert!(matches!(
        disk.check().problems.as_slice(),
        [Problem::Checksum { block, what: "bitmap", .. }] if *block == bitmap
    ));
    assert!(disk.verify().is_err());
}

#[test]
fn slack_does_not_hide_allocation_disagreements_or_final_word_padding() {
    for geometry in [DD, HD] {
        for fs in [FileSystem::Ofs, FileSystem::Ffs] {
            let (image, bitmap) = fixture(fs, geometry);
            let root = geometry.root_block();
            let last = geometry.blocks() - 1;
            for (number, free, expected) in [
                (root, true, Problem::AllocatedButFree { block: root }),
                (last, false, Problem::UsedButUnreachable { block: last }),
            ] {
                let mut corrupt = image.clone();
                set_free(&mut corrupt, bitmap, number, free);
                rechecksum(&mut corrupt, bitmap);
                let disk = Disk::open(&corrupt).unwrap();
                assert_eq!(disk.check().problems, [expected]);
                assert!(disk.verify().is_err());
            }
            let padded_end = RESERVED + (geometry.blocks() - RESERVED).div_ceil(32) * 32;
            for number in geometry.blocks()..padded_end {
                let mut corrupt = image.clone();
                set_free(&mut corrupt, bitmap, number, true);
                rechecksum(&mut corrupt, bitmap);
                let disk = Disk::open(&corrupt).unwrap();
                assert_eq!(
                    disk.check().problems,
                    [Problem::BitmapTailFree {
                        bitmap,
                        block: number
                    }]
                );
                assert!(disk.verify().is_err());
            }
        }
    }
}
