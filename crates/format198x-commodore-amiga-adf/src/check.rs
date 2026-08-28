use crate::fs::FileSystem;
use crate::geometry::RESERVED;
use crate::layout::*;
use crate::read::{Disk, collect_ptrs};
use std::collections::{BTreeMap, BTreeSet};

/// One fault found by [`Disk::check`].
///
/// Every variant names the block it concerns, because the block number is what
/// a reader needs to go and look.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Problem {
    /// The boot block carries a bootstrap whose checksum does not match, so the
    /// ROM would refuse to run it.
    BootChecksum,
    /// A block's stored checksum disagrees with its contents.
    Checksum {
        /// The block at fault.
        block: u32,
        /// What kind of block it is — `"root"`, `"header"`, `"bitmap"`,
        /// `"extension"`, `"data"`.
        what: &'static str,
        /// Where it sits in the volume, as far as the walk got.
        path: String,
    },
    /// A pointer names a block outside the disk, or one of the two reserved
    /// boot blocks.
    BlockOutOfRange {
        /// The impossible block number.
        block: u32,
        /// The block holding the pointer.
        from: u32,
        /// Which pointer it was — `"hash chain"`, `"data"`, `"extension"`.
        what: &'static str,
    },
    /// A chain came back to a block it had already visited, which would loop
    /// forever if followed.
    Cycle {
        /// Where the chain returned to.
        block: u32,
        /// Which chain — `"directory"`, `"hash chain"`, `"extension"`.
        what: &'static str,
    },
    /// A file's data blocks hold less than its header says it has.
    ShortFile {
        /// The file.
        path: String,
        /// The size its header declares.
        declared: u32,
        /// What its data blocks actually add up to.
        found: u32,
    },
    /// A file's data blocks describe bytes beyond its declared length.
    LongFile {
        /// The file.
        path: String,
        /// The size its header declares.
        declared: u32,
        /// What its data blocks describe.
        found: u32,
    },
    /// A typed block field does not carry the value its role requires.
    InvalidField {
        /// Block containing the field.
        block: u32,
        /// Volume path associated with the block.
        path: String,
        /// Field name.
        field: &'static str,
        /// Required value.
        expected: u32,
        /// Stored value.
        found: u32,
    },
    /// OFS's pointer table and linked-data chain name different blocks or order.
    DataChainMismatch {
        /// The file.
        path: String,
        /// Header/extension pointer-table order.
        pointer_table: Vec<u32>,
        /// `first_data`/`next_data` order.
        linked_chain: Vec<u32>,
    },
    /// A block is claimed by more than one filesystem owner.
    DuplicateOwnership {
        /// Multiply-owned block.
        block: u32,
        /// Deterministically ordered descriptions of its owners.
        owners: Vec<String>,
    },
    /// The root block says its bitmap is not valid.
    BitmapInvalid {
        /// Stored bitmap-valid flag.
        found: u32,
    },
    /// A bitmap offers a bit beyond the end of the media as free.
    BitmapTailFree {
        /// Bitmap block containing the invalid tail.
        bitmap: u32,
        /// First non-existent block offered as free.
        block: u32,
    },
    /// An entry's secondary type is neither a file nor a directory, so nothing
    /// can say what it is.
    UnknownEntry {
        /// The header block.
        block: u32,
        /// The secondary type found there.
        secondary: u32,
        /// Where it sits in the volume.
        path: String,
    },
    /// Something reaches this block, but the bitmap offers it as free — so the
    /// next write would land on top of live data.
    AllocatedButFree {
        /// The block in question.
        block: u32,
    },
    /// The bitmap holds this block back, but nothing reaches it. Harmless to
    /// read and invisible to a user; it simply costs the space.
    UsedButUnreachable {
        /// The block in question.
        block: u32,
    },
}

impl core::fmt::Display for Problem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BootChecksum => write!(f, "boot block: checksum does not match its bootstrap"),
            Self::Checksum { block, what, path } => {
                write!(
                    f,
                    "block {block} ({what}, {path:?}): checksum does not match"
                )
            }
            Self::BlockOutOfRange { block, from, what } => write!(
                f,
                "block {from}: {what} pointer names block {block}, which cannot exist"
            ),
            Self::Cycle { block, what } => {
                write!(
                    f,
                    "block {block}: {what} returns to a block already visited"
                )
            }
            Self::ShortFile {
                path,
                declared,
                found,
            } => write!(
                f,
                "{path:?}: header declares {declared} bytes, data blocks hold {found}"
            ),
            Self::LongFile {
                path,
                declared,
                found,
            } => write!(
                f,
                "{path:?}: header declares {declared} bytes, data blocks describe {found}"
            ),
            Self::InvalidField {
                block,
                path,
                field,
                expected,
                found,
            } => write!(
                f,
                "block {block} ({path:?}): {field} is {found}, expected {expected}"
            ),
            Self::DataChainMismatch {
                path,
                pointer_table,
                linked_chain,
            } => write!(
                f,
                "{path:?}: pointer-table data {pointer_table:?} disagrees with OFS chain {linked_chain:?}"
            ),
            Self::DuplicateOwnership { block, owners } => {
                write!(f, "block {block}: claimed by {}", owners.join(", "))
            }
            Self::BitmapInvalid { found } => {
                write!(f, "root block: bitmap-valid flag is {found:#010x}")
            }
            Self::BitmapTailFree { bitmap, block } => write!(
                f,
                "bitmap block {bitmap}: non-existent block {block} is marked free"
            ),
            Self::UnknownEntry {
                block,
                secondary,
                path,
            } => write!(
                f,
                "block {block} ({path:?}): secondary type {secondary} is neither a file nor a directory"
            ),
            Self::AllocatedButFree { block } => {
                write!(f, "block {block}: in use, but the bitmap offers it as free")
            }
            Self::UsedButUnreachable { block } => {
                write!(
                    f,
                    "block {block}: held by the bitmap, but nothing reaches it"
                )
            }
        }
    }
}

/// Everything [`Disk::check`] found. An empty report means a sound disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Report {
    /// Every fault found, in the order the walk met them.
    pub problems: Vec<Problem>,
}

impl Report {
    /// Whether the disk is sound — no problems at all.
    pub fn is_sound(&self) -> bool {
        self.problems.is_empty()
    }
}

impl core::fmt::Display for Report {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.problems.is_empty() {
            return write!(f, "no problems found");
        }
        for (i, p) in self.problems.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{p}")?;
        }
        Ok(())
    }
}

/// The walk's running state: what it has found and what it has already been
/// through.
struct Walk<'a, 'b> {
    disk: &'b Disk<'a>,
    problems: Vec<Problem>,
    /// Blocks something reaches, for the bitmap comparison at the end.
    reachable: BTreeSet<u32>,
    /// Every ownership path for duplicate-reference diagnostics.
    owners: BTreeMap<u32, Vec<String>>,
    /// Bitmap pages named by the root block.
    bitmap_blocks: Vec<u32>,
    /// Entry headers already visited, so a cycle is reported once and not
    /// followed.
    seen: BTreeSet<u32>,
}

impl<'a> Disk<'a> {
    /// Find everything wrong with this disk, rather than the first thing.
    ///
    /// [`verify`](Disk::verify) stops at the first fault, which answers "is this
    /// disk broken". This answers "what is wrong with this disk": it walks every
    /// directory hash chain and file header chain, follows each file's data
    /// blocks to its declared length, checks the OFS data-block checksums, and
    /// confirms the bitmap agrees with what is actually reachable — a
    /// disagreement being the classic mark of a disk written by something that
    /// got the bitmap wrong. It reports every fault it finds, because a tool
    /// that shows one fault per run is a tool you run many times.
    ///
    /// It never stops early, never panics, and never follows a cycle: a chain
    /// that returns to a block already visited is reported and abandoned.
    ///
    /// ```
    /// use format198x_commodore_amiga_adf::{Disk, FileSystem, Volume};
    /// let mut vol = Volume::new("Sound", FileSystem::Ofs);
    /// vol.add_file("c/hello", b"hi\n").unwrap();
    /// let img = vol.build().unwrap();
    ///
    /// let report = Disk::open(&img).unwrap().check();
    /// assert!(report.is_sound(), "{report}");
    /// ```
    pub fn check(&self) -> Report {
        let mut walk = Walk {
            disk: self,
            problems: Vec::new(),
            reachable: BTreeSet::new(),
            owners: BTreeMap::new(),
            bitmap_blocks: Vec::new(),
            seen: BTreeSet::new(),
        };

        if self.boot_fault().is_some() {
            walk.problems.push(Problem::BootChecksum);
        }

        let geometry = self.geometry();
        let root = geometry.root_block();
        walk.mark(root, "root".to_owned());

        walk.checksum_of(root, 20, "root", "/");
        if let Ok(root_bytes) = self.cblock(root) {
            let bitmap_valid = read_u32(root_bytes, BSIZE - 200);
            if bitmap_valid != u32::MAX {
                walk.problems.push(Problem::BitmapInvalid {
                    found: bitmap_valid,
                });
            }
            for index in 0..25 {
                let bitmap = read_u32(root_bytes, BSIZE - 196 + 4 * index);
                if bitmap == 0 {
                    continue;
                }
                if walk.reach(bitmap, root, "bitmap page", format!("bitmap page {index}")) {
                    walk.bitmap_blocks.push(bitmap);
                    walk.checksum_of(bitmap, 0, "bitmap", "/");
                }
            }
            if walk.bitmap_blocks.len() != 1 {
                walk.problems.push(Problem::InvalidField {
                    block: root,
                    path: "/".to_owned(),
                    field: "bitmap page count",
                    expected: 1,
                    found: walk.bitmap_blocks.len().min(u32::MAX as usize) as u32,
                });
            }
        }
        walk.dir(root, "");
        walk.bitmap_agreement();
        for (&block, owners) in &walk.owners {
            if owners.len() > 1 {
                walk.problems.push(Problem::DuplicateOwnership {
                    block,
                    owners: owners.clone(),
                });
            }
        }

        Report {
            problems: walk.problems,
        }
    }
}

impl Walk<'_, '_> {
    fn mark(&mut self, block: u32, owner: String) {
        self.reachable.insert(block);
        let owners = self.owners.entry(block).or_default();
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }

    /// Check one block's stored checksum against its contents.
    fn checksum_of(&mut self, block: u32, off: usize, what: &'static str, path: &str) {
        let Ok(b) = self.disk.cblock(block) else {
            return;
        };
        if read_u32(b, off) != checksum(b, off) {
            self.problems.push(Problem::Checksum {
                block,
                what,
                path: path.to_owned(),
            });
        }
    }

    /// Whether a pointer is usable, recording it as reachable if so.
    fn reach(&mut self, block: u32, from: u32, what: &'static str, owner: String) -> bool {
        if self.disk.cblock(block).is_err() {
            self.problems
                .push(Problem::BlockOutOfRange { block, from, what });
            return false;
        }
        self.mark(block, owner);
        true
    }

    /// Walk a directory's hash table and every sibling chain hanging off it.
    fn dir(&mut self, dir: u32, path: &str) {
        let Ok(b) = self.disk.cblock(dir) else {
            return;
        };
        for slot in 0..HT_SIZE {
            let mut entry = read_u32(b, 24 + 4 * slot);
            let mut guard = 0u32;
            while entry != 0 {
                if !self.reach(
                    entry,
                    dir,
                    "hash chain",
                    format!("entry in {path:?} bucket {slot}"),
                ) {
                    break;
                }
                if !self.seen.insert(entry) {
                    self.problems.push(Problem::Cycle {
                        block: entry,
                        what: "directory",
                    });
                    break;
                }
                let Ok(eb) = self.disk.cblock(entry) else {
                    break;
                };
                self.field(entry, path, "primary type", T_HEADER, read_u32(eb, 0));
                self.field(entry, path, "header key", entry, read_u32(eb, 4));
                self.field(entry, path, "parent", dir, read_u32(eb, BSIZE - 12));
                let name = self
                    .disk
                    .entry_name(entry)
                    .unwrap_or_else(|_| String::from("?"));
                let child = if path.is_empty() {
                    name
                } else {
                    format!("{path}/{name}")
                };
                self.checksum_of(entry, 20, "header", &child);

                match read_u32(eb, BSIZE - 4) {
                    ST_USERDIR => self.dir(entry, &child),
                    ST_FILE => self.file(entry, &child),
                    secondary => self.problems.push(Problem::UnknownEntry {
                        block: entry,
                        secondary,
                        path: child,
                    }),
                }

                entry = read_u32(eb, BSIZE - 16);
                guard += 1;
                if guard > self.disk.geometry().blocks() {
                    self.problems.push(Problem::Cycle {
                        block: entry,
                        what: "hash chain",
                    });
                    break;
                }
            }
        }
    }

    /// Walk a file's extension chain and data blocks, checking what each
    /// filesystem actually records.
    fn file(&mut self, hdr: u32, path: &str) {
        let Ok(hb) = self.disk.cblock(hdr) else {
            return;
        };
        let declared = read_u32(hb, BSIZE - 188);
        self.field(hdr, path, "primary type", T_HEADER, read_u32(hb, 0));
        self.field(hdr, path, "header key", hdr, read_u32(hb, 4));

        // The header's own pointer table, then each extension block's.
        let mut data = Vec::new();
        collect_ptrs(self.disk.bytes(), hdr, &mut data);

        let mut ext = read_u32(hb, BSIZE - 8);
        let mut ext_seen = BTreeSet::new();
        while ext != 0 {
            if !self.reach(ext, hdr, "extension", format!("{path:?} extension")) {
                break;
            }
            if !ext_seen.insert(ext) {
                self.problems.push(Problem::Cycle {
                    block: ext,
                    what: "extension",
                });
                break;
            }
            self.checksum_of(ext, 20, "extension", path);
            let Ok(eb) = self.disk.cblock(ext) else {
                break;
            };
            self.field(ext, path, "primary type", T_LIST, read_u32(eb, 0));
            self.field(ext, path, "header key", ext, read_u32(eb, 4));
            self.field(ext, path, "parent", hdr, read_u32(eb, BSIZE - 12));
            self.field(
                ext,
                path,
                "secondary type",
                ST_FILE,
                read_u32(eb, BSIZE - 4),
            );
            let high_seq = read_u32(eb, 8);
            if high_seq > HT_SIZE as u32 {
                self.field(ext, path, "high sequence", HT_SIZE as u32, high_seq);
            }
            collect_ptrs(self.disk.bytes(), ext, &mut data);
            ext = read_u32(eb, BSIZE - 8);
        }

        // Follow the data to the length the header claims.
        let mut found: u64 = 0;
        for (index, &d) in data.iter().enumerate() {
            if !self.reach(d, hdr, "data", format!("{path:?} data #{}", index + 1)) {
                continue;
            }
            let Ok(db) = self.disk.cblock(d) else {
                continue;
            };
            match self.disk.filesystem() {
                FileSystem::Ffs => found += BSIZE as u64,
                FileSystem::Ofs => {
                    self.checksum_of(d, 20, "data", path);
                    let n = read_u32(db, 12) as usize;
                    found += n.min(OFS_DATA) as u64;
                }
            }
        }
        if self.disk.filesystem() == FileSystem::Ffs && found < declared as u64 {
            self.problems.push(Problem::ShortFile {
                path: path.to_owned(),
                declared,
                found: found.min(u32::MAX as u64) as u32,
            });
        }

        if self.disk.filesystem() == FileSystem::Ofs {
            self.ofs_chain(hdr, path, hb, &data);
        }
    }

    fn field(&mut self, block: u32, path: &str, field: &'static str, expected: u32, found: u32) {
        if expected != found {
            self.problems.push(Problem::InvalidField {
                block,
                path: path.to_owned(),
                field,
                expected,
                found,
            });
        }
    }

    /// Walk OFS's independently linked representation and compare it with the
    /// header/extension pointer tables.
    fn ofs_chain(&mut self, hdr: u32, path: &str, header: &[u8], pointers: &[u32]) {
        let mut linked = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = read_u32(header, 16);
        let mut sequence = 1u32;
        let mut bytes = 0u64;
        while current != 0 {
            if !seen.insert(current) {
                self.problems.push(Problem::Cycle {
                    block: current,
                    what: "OFS data chain",
                });
                break;
            }
            if !self.reach(
                current,
                hdr,
                "OFS next_data",
                format!("{path:?} data #{sequence}"),
            ) {
                break;
            }
            linked.push(current);
            let Ok(block) = self.disk.cblock(current) else {
                break;
            };
            self.field(current, path, "primary type", T_DATA, read_u32(block, 0));
            self.field(current, path, "header key", hdr, read_u32(block, 4));
            self.field(current, path, "sequence", sequence, read_u32(block, 8));
            let size = read_u32(block, 12);
            if size > OFS_DATA as u32 {
                self.field(current, path, "data size maximum", OFS_DATA as u32, size);
            }
            bytes += u64::from(size.min(OFS_DATA as u32));
            self.checksum_of(current, 20, "data", path);
            current = read_u32(block, 16);
            sequence += 1;
        }
        if linked != pointers {
            self.problems.push(Problem::DataChainMismatch {
                path: path.to_owned(),
                pointer_table: pointers.to_vec(),
                linked_chain: linked,
            });
        }
        let declared = u64::from(read_u32(header, BSIZE - 188));
        if bytes != declared {
            let problem = if bytes < declared {
                Problem::ShortFile {
                    path: path.to_owned(),
                    declared: declared as u32,
                    found: bytes as u32,
                }
            } else {
                Problem::LongFile {
                    path: path.to_owned(),
                    declared: declared as u32,
                    found: bytes.min(u64::from(u32::MAX)) as u32,
                }
            };
            self.problems.push(problem);
        }
    }

    /// Compare the bitmap against what the walk actually reached.
    ///
    /// A block the bitmap offers as free while something still points at it is
    /// the dangerous direction: the next write lands on live data. The other
    /// direction only wastes space.
    fn bitmap_agreement(&mut self) {
        let geometry = self.disk.geometry();
        let Some(&bitmap_blk) = self.bitmap_blocks.first() else {
            return;
        };
        let Ok(bitmap) = self.disk.cblock(bitmap_blk) else {
            return;
        };
        for n in RESERVED..geometry.blocks() {
            let i = (n - RESERVED) as usize;
            let free = read_u32(bitmap, 4 + 4 * (i / 32)) & (1 << (i % 32)) != 0;
            match (self.reachable.contains(&n), free) {
                (true, true) => self.problems.push(Problem::AllocatedButFree { block: n }),
                (false, false) => self.problems.push(Problem::UsedButUnreachable { block: n }),
                _ => {}
            }
        }

        let represented = (geometry.blocks() - RESERVED) as usize;
        let capacity = (BSIZE - 4) * 8;
        for index in represented..capacity {
            let free = read_u32(bitmap, 4 + 4 * (index / 32)) & (1 << (index % 32)) != 0;
            if free {
                self.problems.push(Problem::BitmapTailFree {
                    bitmap: bitmap_blk,
                    block: RESERVED + index as u32,
                });
                break;
            }
        }
    }
}
