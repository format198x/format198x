use crate::error::Error;
use crate::fs::FileSystem;
use crate::geometry::{DD, Geometry};
use crate::layout::*;

/// Insert `child` into `parent`'s hash table under `name`, chaining on a slot
/// collision via the sibling chain (`hash_chain` at `BSIZE-16`). This makes the
/// writer correct for *any* set of names, not only ones that happen not to
/// collide. Does not checksum — the caller checksums headers after all inserts
/// (an insert may set a header's `hash_chain`).
pub(crate) fn dir_insert(img: &mut [u8], parent: u32, child: u32, name: &str) {
    let slot = 24 + 4 * name_hash(name);
    let head = read_u32(block(img, parent), slot);
    if head == 0 {
        put_u32(block_mut(img, parent), slot, child);
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
    }
}

/// Extension blocks a file of `data_n` data blocks needs beyond its header's 72
/// pointer slots.
fn ext_count(data_n: usize) -> usize {
    data_n.saturating_sub(1) / HT_SIZE
}

/// Write a file's data blocks — any length. OFS wraps each block in a 24-byte
/// header (type/key/sequence/size/next/checksum) and chains them; FFS writes
/// raw 512-byte sectors, relying on the header/extension pointer tables for
/// order.
fn write_file_data(
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
fn write_file_header(
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
        if let Some(i) = self.entries.iter().position(|(n, _)| n == name) {
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
        self.entries.iter().any(|(n, _)| n == name)
    }
}

/// Split a slash-separated path into validated, non-empty components.
fn split_path(path: &str) -> Result<Vec<String>, Error> {
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
    pub fn build(&self) -> Result<Vec<u8>, Error> {
        validate_name(&self.label, "volume name")?;

        let geometry = self.geometry;
        let root_blk = geometry.root_block();
        let bitmap_blk = geometry.bitmap_block();
        let first_free = geometry.first_free();
        let blocks = geometry.blocks();

        // Plan: assign blocks to every directory header, file header, file
        // extension block, and data block, in a deterministic pre-order walk.
        let mut planned: Vec<Planned> = Vec::new();
        let mut next = first_free;
        plan_dir(&self.root, root_blk, self.fs, &mut next, &mut planned);
        let used_end = next; // first_free..used_end are the file-tree blocks

        if used_end > blocks {
            return Err(Error::DiskFull {
                needed: used_end - first_free,
                available: blocks - first_free,
            });
        }

        let mut img = vec![0u8; geometry.len()];
        write_boot_block(&mut img, self.fs, self.bootable);

        // Data blocks + headers (headers unchecksummed; an insert may set a
        // header's hash_chain).
        for p in &planned {
            match p {
                Planned::File {
                    hdr,
                    ext,
                    data,
                    parent,
                    name,
                    bytes,
                    protect,
                } => {
                    write_file_data(&mut img, self.fs, *hdr, data, bytes);
                    write_file_header(
                        &mut img,
                        *hdr,
                        ext,
                        name,
                        *parent,
                        data,
                        bytes.len() as u32,
                        *protect,
                    );
                }
                Planned::Dir { hdr, parent, name } => {
                    let b = block_mut(&mut img, *hdr);
                    put_u32(b, 0, T_HEADER);
                    put_u32(b, 4, *hdr); // own block
                    put_name(b, name);
                    put_u32(b, BSIZE - 12, *parent);
                    put_u32(b, BSIZE - 4, ST_USERDIR);
                }
            }
        }

        // Root block (structure only; entries inserted below).
        {
            let b = block_mut(&mut img, root_blk);
            put_u32(b, 0, T_HEADER);
            put_u32(b, 12, HT_SIZE as u32); // hash-table size (root only)
            put_u32(b, BSIZE - 200, 0xffff_ffff); // bitmap flag: valid
            put_u32(b, BSIZE - 196, bitmap_blk); // bm_pages[0]
            put_name(b, &self.label);
            put_u32(b, BSIZE - 4, ST_ROOT);
        }

        // Insert every entry into its parent (in pre-order, so sibling chain
        // order on a hash collision is deterministic), then checksum all
        // headers — an insert can set a header's hash_chain.
        for p in &planned {
            let (parent, hdr, name) = p.link();
            dir_insert(&mut img, parent, hdr, name);
        }
        let c = checksum(block(&img, root_blk), 20);
        put_u32(block_mut(&mut img, root_blk), 20, c);
        for p in &planned {
            let (_, hdr, _) = p.link();
            let c = checksum(block(&img, hdr), 20);
            put_u32(block_mut(&mut img, hdr), 20, c);
        }

        // Bitmap block: 1 = free. Mark the used blocks used.
        {
            // One bitmap block covers both geometries: 508 bytes of bits is
            // 4064 blocks' worth, and HD needs 3518. No extension chain.
            let words = ((blocks - 2) as usize).div_ceil(32); // blocks 2..blocks
            let mut map = vec![0xffff_ffffu32; words];
            let mut mark_used = |n: u32| {
                let i = (n - 2) as usize;
                map[i / 32] &= !(1u32 << (i % 32));
            };
            mark_used(root_blk);
            mark_used(bitmap_blk);
            for n in first_free..used_end {
                mark_used(n);
            }
            // Bits past the last real block (blocks-1) don't exist: mark used.
            for n in blocks..(2 + words as u32 * 32) {
                mark_used(n);
            }
            let b = block_mut(&mut img, bitmap_blk);
            for (i, w) in map.iter().enumerate() {
                put_u32(b, 4 + 4 * i, *w);
            }
            let c = checksum(b, 0);
            put_u32(block_mut(&mut img, bitmap_blk), 0, c);
        }

        Ok(img)
    }
}

/// A tree node with its assigned blocks, ready to write.
enum Planned<'a> {
    File {
        hdr: u32,
        ext: Vec<u32>,
        data: Vec<u32>,
        parent: u32,
        name: &'a str,
        bytes: &'a [u8],
        protect: u32,
    },
    Dir {
        hdr: u32,
        parent: u32,
        name: &'a str,
    },
}

impl Planned<'_> {
    /// The (parent, own-header, name) triple every node inserts into its parent.
    fn link(&self) -> (u32, u32, &str) {
        match self {
            Planned::File {
                parent, hdr, name, ..
            } => (*parent, *hdr, name),
            Planned::Dir { parent, hdr, name } => (*parent, *hdr, name),
        }
    }
}

/// Take `n` consecutive blocks from the allocation cursor, returning the first.
fn take_blocks(next: &mut u32, n: u32) -> u32 {
    let base = *next;
    *next += n;
    base
}

/// Assign blocks to `dir`'s subtree, pre-order, appending to `out`.
fn plan_dir<'a>(
    dir: &'a DirNode,
    parent: u32,
    fs: FileSystem,
    next: &mut u32,
    out: &mut Vec<Planned<'a>>,
) {
    for (name, child) in &dir.entries {
        match child {
            Child::File { bytes, protect } => {
                let hdr = take_blocks(next, 1);
                let data_n = if bytes.is_empty() {
                    0
                } else {
                    bytes.len().div_ceil(fs.data_capacity())
                };
                let ext: Vec<u32> = (0..ext_count(data_n))
                    .map(|_| take_blocks(next, 1))
                    .collect();
                let data: Vec<u32> = (0..data_n).map(|_| take_blocks(next, 1)).collect();
                out.push(Planned::File {
                    hdr,
                    ext,
                    data,
                    parent,
                    name,
                    bytes,
                    protect: *protect,
                });
            }
            Child::Dir(sub) => {
                let hdr = take_blocks(next, 1);
                out.push(Planned::Dir { hdr, parent, name });
                plan_dir(sub, hdr, fs, next, out);
            }
        }
    }
}
