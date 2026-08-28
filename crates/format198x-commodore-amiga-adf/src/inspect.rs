//! Structured, deterministic evidence for how AmigaDOS resolves a path.

use crate::error::Error;
use crate::fs::FileSystem;
use crate::layout::*;
use crate::read::{Disk, collect_ptrs, header_name};
use std::collections::BTreeSet;

/// Volume-level filesystem evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VolumeProvenance {
    /// Boot-block DOS type.
    pub filesystem: FileSystem,
    /// Whether a present boot-block checksum is valid.
    pub boot_checksum_valid: bool,
    /// Root header block.
    pub root_block: u32,
    /// Bitmap-valid flag stored in the root block.
    pub bitmap_valid_flag: u32,
    /// Bitmap blocks named by the root block, in on-disk order.
    pub bitmap_blocks: Vec<u32>,
    /// Optional bitmap-extension block pointer.
    pub bitmap_extension: Option<u32>,
}

/// Evidence for one component of a resolved path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ComponentProvenance {
    /// Component spelling supplied by the caller.
    pub name: String,
    /// Directory whose hash table was searched.
    pub parent_block: u32,
    /// Hash-table bucket selected for the name.
    pub hash_bucket: usize,
    /// Candidate header blocks visited before and including the match.
    pub collision_hops: Vec<u32>,
    /// Matching header block.
    pub header_block: u32,
    /// Header primary type.
    pub primary_type: u32,
    /// Header secondary type.
    pub secondary_type: u32,
    /// Parent pointer stored in the matching header.
    pub stored_parent: u32,
}

/// One OFS linked-data block, kept separate from pointer-table order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OfsDataProvenance {
    /// Data block number.
    pub block: u32,
    /// File-header owner key stored in the block.
    pub header_key: u32,
    /// One-based position stored in the block.
    pub sequence: u32,
    /// Payload bytes stored in the block.
    pub data_size: u32,
    /// Next linked-data block, if any.
    pub next_data: Option<u32>,
    /// Whether the block checksum agrees with its bytes.
    pub checksum_valid: bool,
}

/// Block-level evidence for a resolved file.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileProvenance {
    /// File header block.
    pub header_block: u32,
    /// File length declared by the header.
    pub declared_size: u32,
    /// Extension blocks in chain order.
    pub extension_blocks: Vec<u32>,
    /// Data blocks in header/extension pointer-table order.
    pub pointer_table_data: Vec<u32>,
    /// OFS `first_data`/`next_data` order. Empty for FFS.
    pub ofs_data_chain: Vec<OfsDataProvenance>,
}

/// The complete evidence used to resolve one path.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PathProvenance {
    /// Normalised slash-separated path.
    pub path: String,
    /// One resolution record per component.
    pub components: Vec<ComponentProvenance>,
    /// File-specific block evidence, or `None` for a directory.
    pub file: Option<FileProvenance>,
}

impl Disk<'_> {
    /// Return deterministic volume-level filesystem evidence.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Corrupt`] when the root block cannot be read.
    pub fn volume_provenance(&self) -> Result<VolumeProvenance, Error> {
        let root_block = self.root_block();
        let root = self.cblock(root_block)?;
        let bitmap_blocks = (0..25)
            .map(|index| read_u32(root, BSIZE - 196 + index * 4))
            .filter(|&block| block != 0)
            .collect();
        let extension = read_u32(root, BSIZE - 96);
        Ok(VolumeProvenance {
            filesystem: self.filesystem(),
            boot_checksum_valid: self.boot_fault().is_none(),
            root_block,
            bitmap_valid_flag: read_u32(root, BSIZE - 200),
            bitmap_blocks,
            bitmap_extension: (extension != 0).then_some(extension),
        })
    }

    /// Explain how `path` resolves and which blocks represent its contents.
    ///
    /// OFS pointer-table order and linked-data order are traversed independently
    /// so callers can compare them rather than receiving one derived view.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] when a component is absent and
    /// [`Error::Corrupt`] for an out-of-range pointer or structural loop.
    pub fn inspect(&self, path: &str) -> Result<PathProvenance, Error> {
        let names: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        let mut parent = self.root_block();
        let mut components = Vec::with_capacity(names.len());

        for (index, name) in names.iter().enumerate() {
            let parent_bytes = self.cblock(parent)?;
            let hash_bucket = name_hash(name);
            let mut candidate = read_u32(parent_bytes, 24 + 4 * hash_bucket);
            let mut collision_hops = Vec::new();
            let mut seen = BTreeSet::new();
            let header_block = loop {
                if candidate == 0 {
                    return Err(Error::NotFound {
                        path: path.to_owned(),
                    });
                }
                if !seen.insert(candidate) {
                    return Err(Error::Corrupt {
                        what: "hash chain loop",
                    });
                }
                collision_hops.push(candidate);
                let header = self.cblock(candidate)?;
                if name_eq(&header_name(self.bytes(), candidate), name) {
                    break candidate;
                }
                candidate = read_u32(header, BSIZE - 16);
            };

            let header = self.cblock(header_block)?;
            let secondary_type = read_u32(header, BSIZE - 4);
            components.push(ComponentProvenance {
                name: (*name).to_owned(),
                parent_block: parent,
                hash_bucket,
                collision_hops,
                header_block,
                primary_type: read_u32(header, 0),
                secondary_type,
                stored_parent: read_u32(header, BSIZE - 12),
            });
            if index + 1 < names.len() && secondary_type != ST_USERDIR {
                return Err(Error::BadPath {
                    path: path.to_owned(),
                    reason: "a path component is a file, not a directory",
                });
            }
            parent = header_block;
        }

        let file = match components.last() {
            Some(component) if component.secondary_type == ST_FILE => {
                Some(self.inspect_file(component.header_block)?)
            }
            _ => None,
        };
        Ok(PathProvenance {
            path: names.join("/"),
            components,
            file,
        })
    }

    fn inspect_file(&self, header_block: u32) -> Result<FileProvenance, Error> {
        let header = self.cblock(header_block)?;
        let declared_size = read_u32(header, BSIZE - 188);
        let mut pointer_table_data = Vec::new();
        collect_ptrs(self.bytes(), header_block, &mut pointer_table_data);

        let mut extension_blocks = Vec::new();
        let mut seen_extensions = BTreeSet::new();
        let mut extension = read_u32(header, BSIZE - 8);
        while extension != 0 {
            if !seen_extensions.insert(extension) {
                return Err(Error::Corrupt {
                    what: "extension chain loop",
                });
            }
            extension_blocks.push(extension);
            let bytes = self.cblock(extension)?;
            collect_ptrs(self.bytes(), extension, &mut pointer_table_data);
            extension = read_u32(bytes, BSIZE - 8);
        }

        let mut ofs_data_chain = Vec::new();
        if self.filesystem() == FileSystem::Ofs {
            let mut seen_data = BTreeSet::new();
            let mut data = read_u32(header, 16);
            while data != 0 {
                if !seen_data.insert(data) {
                    return Err(Error::Corrupt {
                        what: "OFS data chain loop",
                    });
                }
                let bytes = self.cblock(data)?;
                let next = read_u32(bytes, 16);
                ofs_data_chain.push(OfsDataProvenance {
                    block: data,
                    header_key: read_u32(bytes, 4),
                    sequence: read_u32(bytes, 8),
                    data_size: read_u32(bytes, 12),
                    next_data: (next != 0).then_some(next),
                    checksum_valid: read_u32(bytes, 20) == checksum(bytes, 20),
                });
                data = next;
            }
        }

        Ok(FileProvenance {
            header_block,
            declared_size,
            extension_blocks,
            pointer_table_data,
            ofs_data_chain,
        })
    }
}
