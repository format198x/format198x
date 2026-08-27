use crate::error::Error;
use crate::fs::FileSystem;
use crate::geometry::{Geometry, RESERVED};
use crate::image::Image;
use crate::layout::*;

// ---------------------------------------------------------------------------
// Read side
// ---------------------------------------------------------------------------

/// A directory / file header's AmigaDOS name (length-capped, lossy UTF-8).
pub(crate) fn header_name(img: &[u8], blk: u32) -> String {
    let b = block(img, blk);
    let len = (b[BSIZE - 80] as usize).min(MAX_NAME);
    String::from_utf8_lossy(&b[BSIZE - 79..BSIZE - 79 + len]).into_owned()
}

/// Append a header/extension block's data-block pointers, in file order (the
/// reverse-filled table, capped at `high_seq` and the 72-slot table size).
pub(crate) fn collect_ptrs(img: &[u8], blk: u32, out: &mut Vec<u32>) {
    let b = block(img, blk);
    let n = (read_u32(b, 8) as usize).min(HT_SIZE);
    for i in 0..n {
        out.push(read_u32(b, 24 + 4 * (HT_SIZE - 1 - i)));
    }
}

/// Whether a directory [`Entry`] is a file or a subdirectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A plain file.
    File,
    /// A subdirectory.
    Directory,
}

/// One entry returned by [`Disk::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The entry's name.
    pub name: String,
    /// Whether it is a file or a directory.
    pub kind: EntryKind,
    /// File size in bytes (0 for a directory).
    pub size: u32,
}

/// A read-only view over an ADF image — the counterpart to [`Volume`](crate::Volume). Parses
/// and navigates the volume without panicking on malformed input: every block
/// pointer is range-checked and every chain is loop-bounded, so a corrupt image
/// yields an [`Error::Corrupt`] rather than a panic.
///
/// This is the filesystem layer. Beneath it sits [`Image`], the raw sectors;
/// [`Disk::image`] hands that back for anyone who wants the bytes a drive would
/// see rather than the files AmigaDOS would show.
///
/// ```
/// use format198x_commodore_amiga_adf::{Disk, FileSystem, Volume};
/// let mut vol = Volume::new("Demo", FileSystem::Ofs);
/// vol.add_file("c/hello", b"hi\n").unwrap();
/// let img = vol.build().unwrap();
///
/// let disk = Disk::open(&img).unwrap();
/// assert_eq!(disk.label(), "Demo");
/// assert_eq!(disk.read("c/hello").unwrap(), b"hi\n");
/// disk.verify().unwrap();
/// ```
pub struct Disk<'a> {
    image: Image<'a>,
    img: &'a [u8],
    fs: FileSystem,
}

impl<'a> Disk<'a> {
    /// Open and validate an ADF image: it must be a floppy of a geometry this
    /// crate knows — [`DD`](crate::DD) or [`HD`](crate::HD) — with a recognised
    /// `DOS` boot signature and a root block. Cheap — the deep checksum pass is
    /// [`verify`](Disk::verify).
    ///
    /// A file in another disk-image container — IPF, DMS, a zip, a gzipped
    /// `.adz` — is named as what it is
    /// ([`Error::UnsupportedContainer`]) rather than measured, because a size
    /// complaint about a format the file never was sends the reader to check a
    /// disk image that is not at fault.
    pub fn open(img: &'a [u8]) -> Result<Self, Error> {
        Self::from_image(Image::open(img)?)
    }

    /// Interpret an already-opened raw [`Image`] as an AmigaDOS volume.
    ///
    /// The same validation [`open`](Disk::open) does, minus the container and
    /// size checks the `Image` has already passed.
    pub fn from_image(image: Image<'a>) -> Result<Self, Error> {
        let geometry = image.geometry();
        let img = image.bytes();
        if &img[0..3] != b"DOS" {
            return Err(Error::Corrupt {
                what: "boot-block signature",
            });
        }
        let fs = match img[3] {
            0 => FileSystem::Ofs,
            1 => FileSystem::Ffs,
            _ => {
                return Err(Error::Corrupt {
                    what: "unsupported filesystem type",
                });
            }
        };
        let root = image.block(geometry.root_block())?;
        if read_u32(root, 0) != T_HEADER || read_u32(root, BSIZE - 4) != ST_ROOT {
            return Err(Error::Corrupt { what: "root block" });
        }
        Ok(Disk { image, img, fs })
    }

    /// The raw sectors beneath the filesystem.
    pub fn image(&self) -> Image<'a> {
        self.image
    }

    /// The media's geometry.
    pub fn geometry(&self) -> Geometry {
        self.image.geometry()
    }

    /// The volume's filesystem.
    pub fn filesystem(&self) -> FileSystem {
        self.fs
    }

    /// The volume label.
    pub fn label(&self) -> String {
        header_name(self.img, self.root_block())
    }

    /// List the entries of a directory (`""` or `"/"` is the root).
    pub fn list(&self, path: &str) -> Result<Vec<Entry>, Error> {
        let dir = self.resolve_dir(path)?;
        let b = self.cblock(dir)?;
        let mut out = Vec::new();
        for slot in 0..HT_SIZE {
            let mut e = read_u32(b, 24 + 4 * slot);
            let mut guard = 0;
            while e != 0 {
                let eb = self.cblock(e)?;
                let kind = if read_u32(eb, BSIZE - 4) == ST_USERDIR {
                    EntryKind::Directory
                } else {
                    EntryKind::File
                };
                let size = match kind {
                    EntryKind::File => read_u32(eb, BSIZE - 188),
                    EntryKind::Directory => 0,
                };
                out.push(Entry {
                    name: header_name(self.img, e),
                    kind,
                    size,
                });
                e = read_u32(eb, BSIZE - 16); // hash_chain
                guard += 1;
                if guard > self.blocks() {
                    return Err(Error::Corrupt {
                        what: "hash chain loop",
                    });
                }
            }
        }
        Ok(out)
    }

    /// Read a file's bytes by path (any depth, OFS or FFS).
    pub fn read(&self, path: &str) -> Result<Vec<u8>, Error> {
        let hdr = self.resolve(path)?;
        let hb = self.cblock(hdr)?;
        if read_u32(hb, BSIZE - 4) != ST_FILE {
            return Err(Error::BadPath {
                path: path.to_owned(),
                reason: "is a directory, not a file",
            });
        }
        let size = read_u32(hb, BSIZE - 188) as usize;
        let blocks = self.data_blocks(hdr)?;
        let mut out = Vec::with_capacity(size);
        for d in blocks {
            let db = self.cblock(d)?;
            match self.fs {
                FileSystem::Ffs => out.extend_from_slice(db),
                FileSystem::Ofs => {
                    let n = read_u32(db, 12) as usize;
                    if n > OFS_DATA {
                        return Err(Error::Corrupt {
                            what: "OFS data-block size",
                        });
                    }
                    out.extend_from_slice(&db[24..24 + n]);
                }
            }
        }
        if out.len() < size {
            return Err(Error::Corrupt {
                what: "file shorter than its declared size",
            });
        }
        out.truncate(size);
        Ok(out)
    }

    /// Verify every checksum in the volume — the boot block, the root, the
    /// bitmap, and every reachable header, extension, and (OFS) data block —
    /// plus structural sanity (block pointers in range, no directory cycles).
    ///
    /// Fast, and stops at the first fault: it answers "is this disk broken".
    pub fn verify(&self) -> Result<(), Error> {
        if let Some(what) = self.boot_checksum_fault() {
            return Err(Error::Corrupt { what });
        }
        let root = self.cblock(self.root_block())?;
        if read_u32(root, 20) != checksum(root, 20) {
            return Err(Error::Corrupt {
                what: "root checksum",
            });
        }
        let bm = self.cblock(self.geometry().bitmap_block())?;
        if read_u32(bm, 0) != checksum(bm, 0) {
            return Err(Error::Corrupt {
                what: "bitmap checksum",
            });
        }
        let mut seen = Vec::new();
        self.verify_dir(self.root_block(), &mut seen)
    }

    /// Why the boot checksum is wrong, or `None` if it is sound — or absent
    /// for a good reason.
    ///
    /// A disk formatted but never made bootable carries no bootstrap and a zero
    /// checksum field, and that is the format working as intended rather than a
    /// fault: the ROM validates the boot block only when it is about to run the
    /// bootstrap, so a disk with nothing to run has nothing to check. AmigaDOS
    /// `Format` leaves it this way until `Install` writes the bootstrap, and
    /// amitools does the same. Treating it as corruption would condemn most
    /// data disks ever written.
    ///
    /// Anything else is checked: a stored checksum that is present, or a
    /// bootstrap that is present, must agree.
    fn boot_checksum_fault(&self) -> Option<&'static str> {
        let stored = read_u32(self.img, 4);
        if stored == 0 && !has_boot_code(self.img) {
            return None; // formatted, never installed
        }
        let mut probe = self.img[..1024].to_vec();
        put_u32(&mut probe, 4, 0);
        (boot_checksum(&probe) != stored).then_some("boot checksum")
    }

    /// The name in an entry's header block.
    pub(crate) fn entry_name(&self, blk: u32) -> Result<String, Error> {
        self.cblock(blk)?;
        Ok(header_name(self.img, blk))
    }

    /// Blocks on this volume's media.
    fn blocks(&self) -> u32 {
        self.geometry().blocks()
    }

    /// Where the root block sits on this volume's media.
    fn root_block(&self) -> u32 {
        self.geometry().root_block()
    }

    /// The 512-byte slice for a filesystem block pointer.
    ///
    /// Stricter than [`Image::block`]: a filesystem pointer may not name the
    /// two reserved boot blocks, so the accepted range is `2..blocks`.
    pub(crate) fn cblock(&self, n: u32) -> Result<&'a [u8], Error> {
        if n < RESERVED {
            return Err(Error::Corrupt {
                what: "block pointer out of range",
            });
        }
        self.image.block(n).map_err(|_| Error::Corrupt {
            what: "block pointer out of range",
        })
    }

    /// Find `name` in directory `dir`, following the hash chain on a collision.
    pub(crate) fn lookup(&self, dir: u32, name: &str) -> Result<Option<u32>, Error> {
        let b = self.cblock(dir)?;
        let mut e = read_u32(b, 24 + 4 * name_hash(name));
        let mut guard = 0;
        while e != 0 {
            let eb = self.cblock(e)?;
            if header_name(self.img, e) == name {
                return Ok(Some(e));
            }
            e = read_u32(eb, BSIZE - 16);
            guard += 1;
            if guard > self.blocks() {
                return Err(Error::Corrupt {
                    what: "hash chain loop",
                });
            }
        }
        Ok(None)
    }

    /// Resolve a slash path to its header block.
    pub(crate) fn resolve(&self, path: &str) -> Result<u32, Error> {
        let mut blk = self.root_block();
        for comp in path.split('/').filter(|s| !s.is_empty()) {
            match self.lookup(blk, comp)? {
                Some(next) => blk = next,
                None => {
                    return Err(Error::NotFound {
                        path: path.to_owned(),
                    });
                }
            }
        }
        Ok(blk)
    }

    /// Resolve a path that must be a directory (root or user dir).
    pub(crate) fn resolve_dir(&self, path: &str) -> Result<u32, Error> {
        let blk = self.resolve(path)?;
        let sec = read_u32(self.cblock(blk)?, BSIZE - 4);
        if sec == ST_ROOT || sec == ST_USERDIR {
            Ok(blk)
        } else {
            Err(Error::BadPath {
                path: path.to_owned(),
                reason: "is a file, not a directory",
            })
        }
    }

    /// Gather a file's data blocks in order, from its header and extension
    /// chain (each block pointer range-checked, the chain loop-bounded).
    pub(crate) fn data_blocks(&self, hdr: u32) -> Result<Vec<u32>, Error> {
        let mut blocks = Vec::new();
        collect_ptrs(self.img, hdr, &mut blocks);
        let mut ext = read_u32(self.cblock(hdr)?, BSIZE - 8);
        let mut guard = 0;
        while ext != 0 {
            let eb = self.cblock(ext)?;
            collect_ptrs(self.img, ext, &mut blocks);
            ext = read_u32(eb, BSIZE - 8);
            guard += 1;
            if guard > self.blocks() {
                return Err(Error::Corrupt {
                    what: "extension chain loop",
                });
            }
        }
        for &d in &blocks {
            self.cblock(d)?; // range-check every pointer up front
        }
        Ok(blocks)
    }

    /// Recursively verify a directory's entries: header checksums, file data,
    /// and no cycles (`seen` guards against a directory reachable twice).
    fn verify_dir(&self, dir: u32, seen: &mut Vec<u32>) -> Result<(), Error> {
        let b = self.cblock(dir)?;
        for slot in 0..HT_SIZE {
            let mut e = read_u32(b, 24 + 4 * slot);
            let mut guard = 0;
            while e != 0 {
                if seen.contains(&e) {
                    return Err(Error::Corrupt {
                        what: "directory cycle",
                    });
                }
                seen.push(e);
                let eb = self.cblock(e)?;
                if read_u32(eb, 20) != checksum(eb, 20) {
                    return Err(Error::Corrupt {
                        what: "header checksum",
                    });
                }
                let sec = read_u32(eb, BSIZE - 4);
                if sec == ST_USERDIR {
                    self.verify_dir(e, seen)?;
                } else if sec == ST_FILE {
                    self.verify_file(e)?;
                } else {
                    return Err(Error::Corrupt {
                        what: "unknown secondary type",
                    });
                }
                e = read_u32(eb, BSIZE - 16);
                guard += 1;
                if guard > self.blocks() {
                    return Err(Error::Corrupt {
                        what: "hash chain loop",
                    });
                }
            }
        }
        Ok(())
    }

    /// Verify a file's extension-block checksums and (OFS) data-block checksums.
    fn verify_file(&self, hdr: u32) -> Result<(), Error> {
        let mut ext = read_u32(self.cblock(hdr)?, BSIZE - 8);
        let mut guard = 0;
        while ext != 0 {
            let eb = self.cblock(ext)?;
            if read_u32(eb, 20) != checksum(eb, 20) {
                return Err(Error::Corrupt {
                    what: "extension-block checksum",
                });
            }
            ext = read_u32(eb, BSIZE - 8);
            guard += 1;
            if guard > self.blocks() {
                return Err(Error::Corrupt {
                    what: "extension chain loop",
                });
            }
        }
        if self.fs == FileSystem::Ofs {
            for d in self.data_blocks(hdr)? {
                let db = self.cblock(d)?;
                if read_u32(db, 20) != checksum(db, 20) {
                    return Err(Error::Corrupt {
                        what: "OFS data-block checksum",
                    });
                }
            }
        }
        Ok(())
    }
}
