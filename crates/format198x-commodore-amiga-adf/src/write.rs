use crate::error::Error;
use crate::fs::FileSystem;
use crate::geometry::{DD, Geometry};
use crate::image::ImageMut;
use crate::layout::*;
use crate::mutate::DiskMut;

/// Insert `child` into `parent`'s hash table under `name`, chaining on a slot
/// collision via the sibling chain (`hash_chain` at `BSIZE-16`). This makes the
/// writer correct for *any* set of names, not only ones that happen not to
/// collide.
///
/// Returns the block it altered — the parent when the slot was empty, or the
/// last sibling in the chain when it was not. Does not checksum: the whole-disk
/// builder checksums every header once at the end, and a mutator checksums the
/// one block this names.
pub(crate) fn dir_insert(img: &mut [u8], parent: u32, child: u32, name: &str) -> u32 {
    let slot = 24 + 4 * name_hash(name);
    let head = read_u32(block(img, parent), slot);
    if head == 0 {
        put_u32(block_mut(img, parent), slot, child);
        parent
    } else {
        let mut cur = head;
        loop {
            let next = read_u32(block(img, cur), BSIZE - 16);
            if next == 0 {
                break;
            }
            cur = next;
        }
        put_u32(block_mut(img, cur), BSIZE - 16, child);
        cur
    }
}

/// Extension blocks a file of `data_n` data blocks needs beyond its header's 72
/// pointer slots.
pub(crate) fn ext_count(data_n: usize) -> usize {
    data_n.saturating_sub(1) / HT_SIZE
}

/// Write a file's data blocks — any length. OFS wraps each block in a 24-byte
/// header (type/key/sequence/size/next/checksum) and chains them; FFS writes
/// raw 512-byte sectors, relying on the header/extension pointer tables for
/// order.
pub(crate) fn write_file_data(
    img: &mut [u8],
    fs: FileSystem,
    header_key: u32,
    data_blocks: &[u32],
    payload: &[u8],
) {
    let cap = fs.data_capacity();
    for (i, &blk) in data_blocks.iter().enumerate() {
        let start = i * cap;
        let chunk = &payload[start..(start + cap).min(payload.len())];
        match fs {
            FileSystem::Ffs => {
                block_mut(img, blk)[..chunk.len()].copy_from_slice(chunk);
            }
            FileSystem::Ofs => {
                let next = data_blocks.get(i + 1).copied().unwrap_or(0);
                let b = block_mut(img, blk);
                put_u32(b, 0, T_DATA);
                put_u32(b, 4, header_key);
                put_u32(b, 8, i as u32 + 1); // 1-based sequence
                put_u32(b, 12, chunk.len() as u32);
                put_u32(b, 16, next);
                b[24..24 + chunk.len()].copy_from_slice(chunk);
                let c = checksum(b, 20);
                put_u32(block_mut(img, blk), 20, c);
            }
        }
    }
}

/// Fill a header/ext block's data-pointer table (slots from the top down).
fn put_ptr_table(img: &mut [u8], blk: u32, ptrs: &[u32]) {
    let b = block_mut(img, blk);
    for (i, &d) in ptrs.iter().enumerate() {
        put_u32(b, 24 + 4 * (HT_SIZE - 1 - i), d);
    }
}

/// Write a file header plus any extension blocks holding its data-block
/// pointers (72 per block) — a file of any size, up to the disk. The header is
/// left unchecksummed (a directory insert may set its `hash_chain`); the ext
/// blocks, which inserts never touch, are checksummed here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_file_header(
    img: &mut [u8],
    hdr: u32,
    ext: &[u32],
    name: &str,
    parent: u32,
    data_blocks: &[u32],
    byte_size: u32,
    protect: u32,
) {
    let first = &data_blocks[..data_blocks.len().min(HT_SIZE)];
    {
        let b = block_mut(img, hdr);
        put_u32(b, 0, T_HEADER);
        put_u32(b, 4, hdr); // own block number
        put_u32(b, 8, first.len() as u32); // high_seq: pointers in this block
        put_u32(b, 16, data_blocks.first().copied().unwrap_or(0)); // first_data (0 if empty)
        put_u32(b, BSIZE - 192, protect);
        put_u32(b, BSIZE - 188, byte_size);
        put_name(b, name);
        put_u32(b, BSIZE - 12, parent);
        put_u32(b, BSIZE - 8, ext.first().copied().unwrap_or(0)); // extension
        put_u32(b, BSIZE - 4, ST_FILE);
    }
    put_ptr_table(img, hdr, first);
    for (k, &e) in ext.iter().enumerate() {
        let start = HT_SIZE * (k + 1);
        let these = &data_blocks[start..(start + HT_SIZE).min(data_blocks.len())];
        {
            let b = block_mut(img, e);
            put_u32(b, 0, T_LIST);
            put_u32(b, 4, e); // own block number
            put_u32(b, 8, these.len() as u32);
            put_u32(b, BSIZE - 12, hdr); // parent: the file header
            put_u32(b, BSIZE - 8, ext.get(k + 1).copied().unwrap_or(0)); // next ext
            put_u32(b, BSIZE - 4, ST_FILE);
        }
        put_ptr_table(img, e, these);
        let c = checksum(block(img, e), 20);
        put_u32(block_mut(img, e), 20, c);
    }
}

/// Master `exe` (a KS1.x hunk executable) into a bootable OFS DD `.adf` — the
/// bare A500/KS1.3 shape. Convenience for [`master_fs`] with
/// [`FileSystem::Ofs`].
pub fn master(exe: &[u8], name: &str, volume: &str) -> Result<Vec<u8>, Error> {
    master_fs(exe, name, volume, FileSystem::Ofs)
}

/// Master `exe` (a KS1.x hunk executable) into a bootable DD `.adf` that runs
/// it, on the chosen [`FileSystem`](crate::FileSystem). `name` is the file's on-disk name and the
/// `startup-sequence` command; `volume` is the disk label. Returns the
/// 901,120-byte image. (FFS boots on KS2.0+ only — see [`FileSystem`](crate::FileSystem).)
///
/// A convenience over [`Volume`] for the one-executable bootable disk; use
/// `Volume` directly for arbitrary file/directory trees.
pub fn master_fs(exe: &[u8], name: &str, volume: &str, fs: FileSystem) -> Result<Vec<u8>, Error> {
    validate_name(name, "file name")?;
    let mut vol = Volume::new(volume, fs);
    // `s/startup-sequence` is added before the executable so the block layout
    // matches the historical single-exe master exactly — byte-stable output.
    vol.add_file("s/startup-sequence", format!("{name}\n").as_bytes())?;
    vol.add_file(name, exe)?;
    vol.set_bootable(true);
    vol.build()
}

/// A directory in the volume tree: named children in insertion order.
#[derive(Default)]
struct DirNode {
    entries: Vec<(String, Child)>,
}

enum Child {
    File { bytes: Vec<u8>, protect: u32 },
    Dir(DirNode),
}

impl DirNode {
    /// Get-or-create a child directory named `name`; error if a *file* already
    /// occupies that name.
    fn dir_child(&mut self, name: &str, path: &str) -> Result<&mut DirNode, Error> {
        if let Some(i) = self.entries.iter().position(|(n, _)| name_eq(n, name)) {
            match &mut self.entries[i].1 {
                Child::Dir(d) => Ok(d),
                Child::File { .. } => Err(Error::BadPath {
                    path: path.to_owned(),
                    reason: "a path component is a file, not a directory",
                }),
            }
        } else {
            self.entries
                .push((name.to_owned(), Child::Dir(DirNode::default())));
            let last = self.entries.len() - 1;
            match &mut self.entries[last].1 {
                Child::Dir(d) => Ok(d),
                _ => unreachable!(),
            }
        }
    }

    fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| name_eq(n, name))
    }
}

/// Split a slash-separated path into validated, non-empty components.
pub(crate) fn split_path(path: &str) -> Result<Vec<String>, Error> {
    let parts: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if parts.is_empty() {
        return Err(Error::BadPath {
            path: path.to_owned(),
            reason: "empty path",
        });
    }
    for p in &parts {
        validate_name(p, "path component")?;
    }
    Ok(parts)
}

/// A double-density Amiga floppy volume you fill with files and directories,
/// then [`build`](Volume::build) into a deterministic 880 KB `.adf` image.
///
/// ```
/// use format198x_commodore_amiga_adf::{FileSystem, Volume};
/// let mut vol = Volume::new("MyDisk", FileSystem::Ofs);
/// vol.add_file("c/hello", b"...").unwrap();
/// vol.add_file("s/startup-sequence", b"c/hello\n").unwrap();
/// vol.set_bootable(true);
/// let adf = vol.build().unwrap();
/// assert_eq!(adf.len(), 901_120);
/// ```
pub struct Volume {
    label: String,
    fs: FileSystem,
    geometry: Geometry,
    bootable: bool,
    root: DirNode,
}

impl Volume {
    /// A new, empty volume with the given label and filesystem. Not bootable
    /// until [`set_bootable(true)`](Volume::set_bootable).
    pub fn new(label: &str, fs: FileSystem) -> Self {
        Volume {
            label: label.to_owned(),
            fs,
            geometry: DD,
            bootable: false,
            root: DirNode::default(),
        }
    }

    /// Set whether the disk carries the boot bootstrap. A bootable disk runs
    /// `s/startup-sequence`; a data disk is mountable but does not boot.
    pub fn set_bootable(&mut self, bootable: bool) -> &mut Self {
        self.bootable = bootable;
        self
    }

    /// Choose the media this volume is written onto. Defaults to
    /// [`DD`](crate::DD), the 880 KB floppy every Amiga can read; pass
    /// [`HD`](crate::HD) for the 1.76 MB one an A3000/A4000 HD drive writes.
    ///
    /// Only the block count and the root block's position change — the volume
    /// structure, the boot block and both filesystems are identical either way.
    pub fn set_geometry(&mut self, geometry: Geometry) -> &mut Self {
        self.geometry = geometry;
        self
    }

    /// Add a file at `path` (slash-separated, e.g. `"s/startup-sequence"`),
    /// creating any intermediate directories. Protection defaults to a normal
    /// readable/executable file; use [`add_file_with_protection`] to override.
    ///
    /// [`add_file_with_protection`]: Volume::add_file_with_protection
    pub fn add_file(&mut self, path: &str, bytes: &[u8]) -> Result<&mut Self, Error> {
        self.add_file_with_protection(path, bytes, EXE_PROTECT)
    }

    /// Add a file with explicit AmigaDOS protection bits (active-low RWED; see
    /// the crate docs). Otherwise like [`add_file`](Volume::add_file).
    pub fn add_file_with_protection(
        &mut self,
        path: &str,
        bytes: &[u8],
        protect: u32,
    ) -> Result<&mut Self, Error> {
        let parts = split_path(path)?;
        let (dirs, leaf) = parts.split_at(parts.len() - 1);
        let leaf = &leaf[0];
        let mut cur = &mut self.root;
        for d in dirs {
            cur = cur.dir_child(d, path)?;
        }
        if cur.has(leaf) {
            return Err(Error::BadPath {
                path: path.to_owned(),
                reason: "already exists",
            });
        }
        cur.entries.push((
            leaf.clone(),
            Child::File {
                bytes: bytes.to_vec(),
                protect,
            },
        ));
        Ok(self)
    }

    /// Add an explicit (possibly empty) directory at `path`, creating any
    /// intermediate directories. Idempotent for an existing directory; errors
    /// if a file already occupies the path.
    pub fn add_dir(&mut self, path: &str) -> Result<&mut Self, Error> {
        let parts = split_path(path)?;
        let mut cur = &mut self.root;
        for p in &parts {
            cur = cur.dir_child(p, path)?;
        }
        Ok(self)
    }

    /// Build the deterministic `.adf` image — 901,120 bytes for the default
    /// [`DD`](crate::DD) geometry, 1,802,240 for [`HD`](crate::HD). Errors only
    /// if the tree does not fit on the disk or the volume label is invalid.
    ///
    /// Expressed over [`DiskMut`](crate::DiskMut): format a blank image, then
    /// place each entry. There is one implementation of putting a file on an
    /// Amiga disk, and both this builder and the mutator go through it, so
    /// neither can drift from the other.
    pub fn build(&self) -> Result<Vec<u8>, Error> {
        validate_name(&self.label, "volume name")?;
        let mut img = ImageMut::blank(self.geometry);
        {
            let mut disk = DiskMut::format(&mut img, &self.label, self.fs, self.bootable)?;
            let root = self.geometry.root_block();
            place_tree(&mut disk, &self.root, root)?;
            disk.reseal();
        }
        Ok(img)
    }
}

/// Place `dir`'s children under `parent`, depth first in insertion order.
///
/// The order is the contract: block numbers fall out of the order they are
/// asked for, so walking the tree this way is what keeps a given input
/// producing a given image, byte for byte.
fn place_tree(disk: &mut DiskMut, dir: &DirNode, parent: u32) -> Result<(), Error> {
    for (name, child) in &dir.entries {
        match child {
            Child::File { bytes, protect } => {
                disk.place_file(parent, name, bytes, *protect)?;
            }
            Child::Dir(sub) => {
                let hdr = disk.place_dir(parent, name)?;
                place_tree(disk, sub, hdr)?;
            }
        }
    }
    Ok(())
}
