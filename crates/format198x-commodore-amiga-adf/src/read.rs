use crate::error::Error;
use crate::fs::FileSystem;
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
    img: &'a [u8],
    fs: FileSystem,
}

/// Identify a non-ADF disk-image container by its leading bytes.
///
/// Deliberately small: these are the containers an Amiga disk actually arrives
/// in, and naming one wrongly would be worse than not naming it. Anything
/// unrecognised falls through to the size check, which is the right answer for
/// a truncated or padded ADF. Short inputs simply match nothing — `starts_with`
/// on a slice shorter than the magic is `false`, never a panic.
fn identify_container(img: &[u8]) -> Option<(&'static str, &'static str)> {
    const CANDIDATES: &[(&[u8], &str, &str)] = &[
        (
            b"CAPS",
            "IPF",
            "a flux-level image from the Software Preservation Society",
        ),
        (
            b"UAE-1ADF",
            "extended ADF",
            "UAE's variable-length-track ADF",
        ),
        (b"DMS!", "DMS", "a Disk Masher System archive"),
        (b"PK\x03\x04", "zip", "a zip archive — extract it first"),
        (b"\x1f\x8b", "gzip", "a gzip stream, most likely an .adz"),
    ];
    CANDIDATES
        .iter()
        .find(|(magic, _, _)| img.starts_with(magic))
        .map(|(_, format, detail)| (*format, *detail))
}

impl<'a> Disk<'a> {
    /// Open and validate an ADF image: it must be an 880 KB DD floppy with a
    /// recognised `DOS` boot signature and a root block. Cheap — the deep
    /// checksum pass is [`verify`](Disk::verify).
    ///
    /// A file in another disk-image container — IPF, DMS, a zip, a gzipped
    /// `.adz` — is named as what it is
    /// ([`Error::UnsupportedContainer`]) rather than measured, because a size
    /// complaint about a format the file never was sends the reader to check a
    /// disk image that is not at fault.
    pub fn open(img: &'a [u8]) -> Result<Self, Error> {
        // Ask what the file is before complaining about how big it is.
        if let Some((format, detail)) = identify_container(img) {
            return Err(Error::UnsupportedContainer { format, detail });
        }
        if img.len() != BLOCKS as usize * BSIZE {
            return Err(Error::Corrupt {
                what: "image size (not an 880K DD floppy)",
            });
        }
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
        let root = &img[ROOT_BLK as usize * BSIZE..][..BSIZE];
        if read_u32(root, 0) != T_HEADER || read_u32(root, BSIZE - 4) != ST_ROOT {
            return Err(Error::Corrupt { what: "root block" });
        }
        Ok(Disk { img, fs })
    }

    /// The volume's filesystem.
    pub fn filesystem(&self) -> FileSystem {
        self.fs
    }

    /// The volume label.
    pub fn label(&self) -> String {
        header_name(self.img, ROOT_BLK)
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
                if guard > BLOCKS {
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
    pub fn verify(&self) -> Result<(), Error> {
        let mut probe = self.img[..1024].to_vec();
        put_u32(&mut probe, 4, 0);
        if boot_checksum(&probe) != read_u32(self.img, 4) {
            return Err(Error::Corrupt {
                what: "boot checksum",
            });
        }
        let root = self.cblock(ROOT_BLK)?;
        if read_u32(root, 20) != checksum(root, 20) {
            return Err(Error::Corrupt {
                what: "root checksum",
            });
        }
        let bm = self.cblock(BITMAP_BLK)?;
        if read_u32(bm, 0) != checksum(bm, 0) {
            return Err(Error::Corrupt {
                what: "bitmap checksum",
            });
        }
        let mut seen = Vec::new();
        self.verify_dir(ROOT_BLK, &mut seen)
    }

    /// The 512-byte slice for a block number, range-checked (2..BLOCKS).
    fn cblock(&self, n: u32) -> Result<&'a [u8], Error> {
        if !(2..BLOCKS).contains(&n) {
            return Err(Error::Corrupt {
                what: "block pointer out of range",
            });
        }
        Ok(&self.img[n as usize * BSIZE..][..BSIZE])
    }

    /// Find `name` in directory `dir`, following the hash chain on a collision.
    fn lookup(&self, dir: u32, name: &str) -> Result<Option<u32>, Error> {
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
            if guard > BLOCKS {
                return Err(Error::Corrupt {
                    what: "hash chain loop",
                });
            }
        }
        Ok(None)
    }

    /// Resolve a slash path to its header block.
    fn resolve(&self, path: &str) -> Result<u32, Error> {
        let mut blk = ROOT_BLK;
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
    fn resolve_dir(&self, path: &str) -> Result<u32, Error> {
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
    fn data_blocks(&self, hdr: u32) -> Result<Vec<u32>, Error> {
        let mut blocks = Vec::new();
        collect_ptrs(self.img, hdr, &mut blocks);
        let mut ext = read_u32(self.cblock(hdr)?, BSIZE - 8);
        let mut guard = 0;
        while ext != 0 {
            let eb = self.cblock(ext)?;
            collect_ptrs(self.img, ext, &mut blocks);
            ext = read_u32(eb, BSIZE - 8);
            guard += 1;
            if guard > BLOCKS {
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
                if guard > BLOCKS {
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
            if guard > BLOCKS {
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
