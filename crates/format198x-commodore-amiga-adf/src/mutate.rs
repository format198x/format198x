use crate::error::Error;
use crate::fs::FileSystem;
use crate::geometry::{Geometry, RESERVED};
use crate::image::{Image, ImageMut};
use crate::layout::*;
use crate::read::Disk;
use crate::write::{dir_insert, ext_count, split_path, write_file_data, write_file_header};

/// A writable AmigaDOS volume — the layer that changes a disk that already
/// exists, rather than building one from nothing.
///
/// [`Volume`](crate::Volume) is a builder: it produces a whole image and cannot
/// touch one afterwards. An emulator writing a save file, an authoring tool
/// replacing a single asset, or a learner dropping a binary onto a working disk
/// all need this instead.
///
/// Every operation allocates or frees blocks in the volume bitmap, splices the
/// directory hash chain, and recomputes each checksum it disturbs — the root,
/// the headers, the bitmap, and for OFS the per-block data checksums that FFS
/// does not carry.
///
/// ```
/// use format198x_commodore_amiga_adf::{DiskMut, FileSystem, Volume};
/// let mut img = Volume::new("Save", FileSystem::Ofs).build().unwrap();
///
/// let mut disk = DiskMut::open(&mut img).unwrap();
/// disk.create_dir("saves").unwrap();
/// disk.write_file("saves/game.sav", b"level 3").unwrap();
/// assert_eq!(disk.as_disk().read("saves/game.sav").unwrap(), b"level 3");
///
/// disk.write_file("saves/game.sav", b"level 4").unwrap(); // replaces in place
/// disk.delete("saves/game.sav").unwrap();
/// assert!(disk.as_disk().read("saves/game.sav").is_err());
/// disk.as_disk().verify().unwrap();
/// ```
pub struct DiskMut<'a> {
    image: ImageMut<'a>,
    fs: FileSystem,
    /// Where the next allocation scan starts. A hint only — correctness never
    /// depends on it, so freeing a block simply resets it.
    hint: u32,
}

impl<'a> DiskMut<'a> {
    /// Open an existing ADF image for modification. Validates it exactly as
    /// [`Disk::open`] does.
    pub fn open(bytes: &'a mut [u8]) -> Result<Self, Error> {
        let fs = {
            let image = Image::open(bytes)?;
            Disk::from_image(image)?.filesystem()
        };
        let image = ImageMut::open(bytes)?;
        let hint = image.geometry().first_free();
        Ok(DiskMut { image, fs, hint })
    }

    /// Format `bytes` as an empty volume and open it — boot block, root block
    /// and bitmap, and nothing else.
    ///
    /// `bytes` must already be the right length for a geometry this crate
    /// knows; [`ImageMut::blank`] produces one.
    ///
    /// ```
    /// use format198x_commodore_amiga_adf::{DiskMut, FileSystem, HD, ImageMut};
    /// let mut bytes = ImageMut::blank(HD);
    /// let mut disk = DiskMut::format(&mut bytes, "Blank", FileSystem::Ffs, false).unwrap();
    /// disk.write_file("readme", b"hello\n").unwrap();
    /// assert_eq!(disk.as_disk().label(), "Blank");
    /// ```
    pub fn format(
        bytes: &'a mut [u8],
        label: &str,
        fs: FileSystem,
        bootable: bool,
    ) -> Result<Self, Error> {
        validate_name(label, "volume name")?;
        let mut image = ImageMut::open(bytes)?;
        let geometry = image.geometry();
        let root_blk = geometry.root_block();
        let bitmap_blk = geometry.bitmap_block();

        let img = image.bytes_mut();
        img.fill(0);
        write_boot_block(img, fs, bootable);

        {
            let b = block_mut(img, root_blk);
            put_u32(b, 0, T_HEADER);
            put_u32(b, 12, HT_SIZE as u32); // hash-table size (root only)
            put_u32(b, BSIZE - 200, 0xffff_ffff); // bitmap flag: valid
            put_u32(b, BSIZE - 196, bitmap_blk); // bm_pages[0]
            put_name(b, label);
            put_u32(b, BSIZE - 4, ST_ROOT);
        }

        // Every block free but the root, the bitmap, and the bits past the end
        // of the disk, which name no block and so must never be handed out.
        let words = ((geometry.blocks() - RESERVED) as usize).div_ceil(32);
        {
            let b = block_mut(img, bitmap_blk);
            for i in 0..words {
                put_u32(b, 4 + 4 * i, 0xffff_ffff);
            }
        }
        let hint = geometry.first_free();
        let mut disk = DiskMut { image, fs, hint };
        disk.set_free(root_blk, false);
        disk.set_free(bitmap_blk, false);
        for n in geometry.blocks()..(RESERVED + words as u32 * 32) {
            disk.set_free(n, false);
        }
        disk.reseal();
        Ok(disk)
    }

    /// A read-only view of the volume — `list`, `read`, `verify`, and the raw
    /// [`Image`] beneath.
    pub fn as_disk(&self) -> Disk<'_> {
        Disk::from_image(self.image.as_image())
            .unwrap_or_else(|_| unreachable!("a DiskMut is a valid volume by construction"))
    }

    /// The media's geometry.
    pub fn geometry(&self) -> Geometry {
        self.image.geometry()
    }

    /// The volume's filesystem.
    pub fn filesystem(&self) -> FileSystem {
        self.fs
    }

    /// Write `bytes` to `path`, replacing whatever is there and creating any
    /// missing directories along the way.
    ///
    /// Replacing frees the old file's blocks before allocating the new one's,
    /// so rewriting a file with one the same size or smaller always fits.
    pub fn write_file(&mut self, path: &str, bytes: &[u8]) -> Result<(), Error> {
        self.write_file_with_protection(path, bytes, EXE_PROTECT)
    }

    /// Like [`write_file`](DiskMut::write_file), with explicit AmigaDOS
    /// protection bits (active-low RWED; see the crate docs).
    pub fn write_file_with_protection(
        &mut self,
        path: &str,
        bytes: &[u8],
        protect: u32,
    ) -> Result<(), Error> {
        let (parent, leaf) = self.parent_of(path, true)?;
        if let Some(existing) = self.as_disk().lookup(parent, &leaf)? {
            if read_u32(self.as_disk().cblock(existing)?, BSIZE - 4) != ST_FILE {
                return Err(Error::BadPath {
                    path: path.to_owned(),
                    reason: "is a directory, not a file",
                });
            }
            self.unlink(parent, &leaf)?;
            self.release(existing)?;
        }
        self.place_file(parent, &leaf, bytes, protect)?;
        self.reseal();
        Ok(())
    }

    /// Create a directory at `path`, and any missing directories above it.
    /// Succeeds quietly if it is already there; errors if a file holds the name.
    pub fn create_dir(&mut self, path: &str) -> Result<(), Error> {
        let parts = split_path(path)?;
        let mut parent = self.geometry().root_block();
        for name in &parts {
            parent = self.dir_child(parent, name, path)?;
        }
        self.reseal();
        Ok(())
    }

    /// Remove the file or directory at `path`, freeing every block it held.
    ///
    /// A directory must be empty; removing one with children would orphan them,
    /// and silently leaking blocks is worse than refusing.
    pub fn delete(&mut self, path: &str) -> Result<(), Error> {
        let (parent, leaf) = self.parent_of(path, false)?;
        let Some(hdr) = self.as_disk().lookup(parent, &leaf)? else {
            return Err(Error::NotFound {
                path: path.to_owned(),
            });
        };
        if read_u32(self.as_disk().cblock(hdr)?, BSIZE - 4) == ST_USERDIR
            && !self.as_disk().list(path)?.is_empty()
        {
            return Err(Error::BadPath {
                path: path.to_owned(),
                reason: "directory is not empty",
            });
        }
        self.unlink(parent, &leaf)?;
        self.release(hdr)?;
        self.reseal();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Placing entries — the one implementation of putting a node on a disk.
    // `Volume::build` walks its tree and calls these, so there is no second
    // copy of the block layout to drift from this one.
    // -----------------------------------------------------------------------

    /// Write a file header, its extension blocks and its data blocks under
    /// `parent`, and link it in. Returns the header block.
    pub(crate) fn place_file(
        &mut self,
        parent: u32,
        name: &str,
        bytes: &[u8],
        protect: u32,
    ) -> Result<u32, Error> {
        validate_name(name, "file name")?;
        let data_n = if bytes.is_empty() {
            0
        } else {
            bytes.len().div_ceil(self.fs.data_capacity())
        };

        // Check the whole cost before spending any of it. Allocating until the
        // disk runs out would leave a half-written file linked to nothing.
        let needed = 1 + ext_count(data_n) + data_n;
        let available = self.free_blocks();
        if needed as u32 > available {
            return Err(Error::DiskFull {
                needed: needed as u32,
                available,
            });
        }

        // Allocated in the order the whole-disk builder has always used —
        // header, then extension blocks, then data — so that re-expressing
        // `Volume::build` over this leaves its output byte-identical.
        let hdr = self.alloc()?;
        let mut ext = Vec::with_capacity(ext_count(data_n));
        for _ in 0..ext_count(data_n) {
            ext.push(self.alloc()?);
        }
        let mut data = Vec::with_capacity(data_n);
        for _ in 0..data_n {
            data.push(self.alloc()?);
        }

        let fs = self.fs;
        let img = self.image.bytes_mut();
        write_file_data(img, fs, hdr, &data, bytes);
        write_file_header(
            img,
            hdr,
            &ext,
            name,
            parent,
            &data,
            bytes.len() as u32,
            protect,
        );
        let touched = dir_insert(img, parent, hdr, name);
        self.seal(hdr);
        self.seal(touched);
        Ok(hdr)
    }

    /// Write a directory header under `parent` and link it in. Returns the
    /// header block.
    pub(crate) fn place_dir(&mut self, parent: u32, name: &str) -> Result<u32, Error> {
        validate_name(name, "path component")?;
        let hdr = self.alloc()?;
        let img = self.image.bytes_mut();
        {
            let b = block_mut(img, hdr);
            b.fill(0);
            put_u32(b, 0, T_HEADER);
            put_u32(b, 4, hdr); // own block
            put_name(b, name);
            put_u32(b, BSIZE - 12, parent);
            put_u32(b, BSIZE - 4, ST_USERDIR);
        }
        let touched = dir_insert(img, parent, hdr, name);
        self.seal(hdr);
        self.seal(touched);
        Ok(hdr)
    }

    /// Recompute the bitmap and root checksums. Called once per public
    /// operation rather than per block, since both are disturbed by nearly
    /// everything and neither is expensive.
    pub(crate) fn reseal(&mut self) {
        self.seal_at(self.geometry().bitmap_block(), 0);
        self.seal(self.geometry().root_block());
    }

    // -----------------------------------------------------------------------
    // Paths
    // -----------------------------------------------------------------------

    /// Split `path` into the block of its parent directory and its last
    /// component, optionally creating the directories above it.
    fn parent_of(&mut self, path: &str, create: bool) -> Result<(u32, String), Error> {
        let parts = split_path(path)?;
        let (dirs, leaf) = parts.split_at(parts.len() - 1);
        let mut parent = self.geometry().root_block();
        for name in dirs {
            parent = if create {
                self.dir_child(parent, name, path)?
            } else {
                match self.as_disk().lookup(parent, name)? {
                    Some(blk) => blk,
                    None => {
                        return Err(Error::NotFound {
                            path: path.to_owned(),
                        });
                    }
                }
            };
        }
        Ok((parent, leaf[0].clone()))
    }

    /// The block of `name` under `parent`, creating the directory if it is not
    /// there. Errors if a file holds the name.
    fn dir_child(&mut self, parent: u32, name: &str, path: &str) -> Result<u32, Error> {
        if let Some(blk) = self.as_disk().lookup(parent, name)? {
            return if read_u32(self.as_disk().cblock(blk)?, BSIZE - 4) == ST_USERDIR {
                Ok(blk)
            } else {
                Err(Error::BadPath {
                    path: path.to_owned(),
                    reason: "a path component is a file, not a directory",
                })
            };
        }
        self.place_dir(parent, name)
    }

    /// Take `child` out of `parent`'s hash table, mending the sibling chain
    /// around it, and clear the removed header's own chain pointer.
    fn unlink(&mut self, parent: u32, name: &str) -> Result<u32, Error> {
        let slot = 24 + 4 * name_hash(name);
        let disk = self.as_disk();
        let head = read_u32(disk.cblock(parent)?, slot);

        let mut prev = None;
        let mut cur = head;
        let mut guard = 0;
        let hdr = loop {
            if cur == 0 {
                return Err(Error::NotFound {
                    path: name.to_owned(),
                });
            }
            if disk.entry_name(cur)? == name {
                break cur;
            }
            prev = Some(cur);
            cur = read_u32(disk.cblock(cur)?, BSIZE - 16);
            guard += 1;
            if guard > self.geometry().blocks() {
                return Err(Error::Corrupt {
                    what: "hash chain loop",
                });
            }
        };
        let next = read_u32(self.as_disk().cblock(hdr)?, BSIZE - 16);

        let img = self.image.bytes_mut();
        match prev {
            None => put_u32(block_mut(img, parent), slot, next),
            Some(p) => put_u32(block_mut(img, p), BSIZE - 16, next),
        }
        put_u32(block_mut(img, hdr), BSIZE - 16, 0);
        self.seal(prev.unwrap_or(parent));
        Ok(hdr)
    }

    /// Free every block an unlinked entry held — its data, its extension
    /// chain, and the header itself.
    fn release(&mut self, hdr: u32) -> Result<(), Error> {
        let secondary = read_u32(self.as_disk().cblock(hdr)?, BSIZE - 4);
        if secondary == ST_FILE {
            let disk = self.as_disk();
            let data = disk.data_blocks(hdr)?;
            let mut ext = Vec::new();
            let mut e = read_u32(disk.cblock(hdr)?, BSIZE - 8);
            let mut guard = 0;
            while e != 0 {
                ext.push(e);
                e = read_u32(disk.cblock(e)?, BSIZE - 8);
                guard += 1;
                if guard > self.geometry().blocks() {
                    return Err(Error::Corrupt {
                        what: "extension chain loop",
                    });
                }
            }
            for blk in data.into_iter().chain(ext) {
                self.set_free(blk, true);
                self.wipe(blk);
            }
        }
        self.set_free(hdr, true);
        self.wipe(hdr);
        self.hint = self.geometry().first_free();
        Ok(())
    }

    /// Zero a freed block. Not required by the format — nothing reads a free
    /// block — but it keeps output deterministic, so a disk written, emptied
    /// and rewritten matches one written straight.
    fn wipe(&mut self, n: u32) {
        block_mut(self.image.bytes_mut(), n).fill(0);
    }

    // -----------------------------------------------------------------------
    // The bitmap
    // -----------------------------------------------------------------------

    /// Take the lowest free block, preferring the region above the root.
    ///
    /// The upper region is searched first because that is the only region the
    /// whole-disk builder ever used, so a volume that fitted there before is
    /// laid out identically now. The region below the root is what makes the
    /// rest of the disk reachable at all — the builder allocated upward from
    /// the root and never looked beneath it, which capped a volume at half its
    /// media.
    fn alloc(&mut self) -> Result<u32, Error> {
        let g = self.geometry();
        let regions = [(self.hint, g.blocks()), (RESERVED, g.root_block())];
        for (start, end) in regions {
            for n in start..end {
                if self.is_free(n) {
                    self.set_free(n, false);
                    self.hint = n + 1;
                    return Ok(n);
                }
            }
        }
        Err(Error::DiskFull {
            needed: 1,
            available: self.free_blocks(),
        })
    }

    /// How many 512-byte blocks the volume has free.
    ///
    /// Read from the bitmap, so it reflects everything written and deleted so
    /// far. A file costs its data blocks plus a header, plus one extension
    /// block per 72 data blocks beyond the first 72.
    pub fn free_blocks(&self) -> u32 {
        let g = self.geometry();
        (RESERVED..g.blocks()).filter(|&n| self.is_free(n)).count() as u32
    }

    /// Whether the bitmap says block `n` is free. Blocks outside the bitmap's
    /// range — the two boot blocks — are never free.
    fn is_free(&self, n: u32) -> bool {
        let g = self.geometry();
        if !(RESERVED..g.blocks()).contains(&n) {
            return false;
        }
        let i = (n - RESERVED) as usize;
        let bitmap = &self.image.as_image().bytes()[g.bitmap_block() as usize * BSIZE..][..BSIZE];
        read_u32(bitmap, 4 + 4 * (i / 32)) & (1 << (i % 32)) != 0
    }

    /// Mark block `n` free or used. A set bit means free, which is AmigaDOS's
    /// convention and the opposite of most.
    fn set_free(&mut self, n: u32, free: bool) {
        let g = self.geometry();
        let i = (n - RESERVED) as usize;
        let bitmap_blk = g.bitmap_block();
        let b = block_mut(self.image.bytes_mut(), bitmap_blk);
        let off = 4 + 4 * (i / 32);
        let mut w = read_u32(b, off);
        if free {
            w |= 1 << (i % 32);
        } else {
            w &= !(1 << (i % 32));
        }
        put_u32(b, off, w);
    }

    // -----------------------------------------------------------------------
    // Checksums
    // -----------------------------------------------------------------------

    /// Recompute a header-style checksum (offset 20).
    fn seal(&mut self, n: u32) {
        self.seal_at(n, 20);
    }

    fn seal_at(&mut self, n: u32, off: usize) {
        let img = self.image.bytes_mut();
        let c = checksum(block(img, n), off);
        put_u32(block_mut(img, n), off, c);
    }
}
