use crate::fs::FileSystem;
use crate::geometry::RESERVED;
use crate::layout::*;
use crate::read::{Disk, collect_ptrs};
use std::collections::BTreeSet;

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
            seen: BTreeSet::new(),
        };

        if self.boot_fault().is_some() {
            walk.problems.push(Problem::BootChecksum);
        }

        let geometry = self.geometry();
        let root = geometry.root_block();
        let bitmap = geometry.bitmap_block();
        walk.reachable.insert(root);
        walk.reachable.insert(bitmap);

        walk.checksum_of(root, 20, "root", "/");
        walk.checksum_of(bitmap, 0, "bitmap", "/");
        walk.dir(root, "");
        walk.bitmap_agreement();

        Report {
            problems: walk.problems,
        }
    }
}

impl Walk<'_, '_> {
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
    fn reach(&mut self, block: u32, from: u32, what: &'static str) -> bool {
        if self.disk.cblock(block).is_err() {
            self.problems
                .push(Problem::BlockOutOfRange { block, from, what });
            return false;
        }
        self.reachable.insert(block);
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
                if !self.reach(entry, dir, "hash chain") {
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

        // The header's own pointer table, then each extension block's.
        let mut data = Vec::new();
        collect_ptrs(self.disk.bytes(), hdr, &mut data);

        let mut ext = read_u32(hb, BSIZE - 8);
        let mut ext_seen = BTreeSet::new();
        while ext != 0 {
            if !self.reach(ext, hdr, "extension") {
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
            collect_ptrs(self.disk.bytes(), ext, &mut data);
            ext = read_u32(eb, BSIZE - 8);
        }

        // Follow the data to the length the header claims.
        let mut found: u64 = 0;
        for &d in &data {
            if !self.reach(d, hdr, "data") {
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
        if found < declared as u64 {
            self.problems.push(Problem::ShortFile {
                path: path.to_owned(),
                declared,
                found: found.min(u32::MAX as u64) as u32,
            });
        }
    }

    /// Compare the bitmap against what the walk actually reached.
    ///
    /// A block the bitmap offers as free while something still points at it is
    /// the dangerous direction: the next write lands on live data. The other
    /// direction only wastes space.
    fn bitmap_agreement(&mut self) {
        let geometry = self.disk.geometry();
        let bitmap_blk = geometry.bitmap_block();
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
    }
}
