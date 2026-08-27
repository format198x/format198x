use crate::error::Error;
use crate::geometry::{DD, Geometry, HD};
use crate::layout::BSIZE;

/// Identify a non-ADF disk-image container by its leading bytes.
///
/// Deliberately small: these are the containers an Amiga disk actually arrives
/// in, and naming one wrongly would be worse than not naming it. Anything
/// unrecognised falls through to the size check, which is the right answer for
/// a truncated or padded ADF. Short inputs simply match nothing — `starts_with`
/// on a slice shorter than the magic is `false`, never a panic.
pub(crate) fn identify_container(img: &[u8]) -> Option<(&'static str, &'static str)> {
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

/// The two questions the raw layer can answer about a file: is it an ADF at
/// all, and if so what shape. Shared by [`Image::open`] and [`ImageMut::open`].
fn geometry_of(bytes: &[u8]) -> Result<Geometry, Error> {
    // Ask what the file is before complaining about how big it is.
    if let Some((format, detail)) = identify_container(bytes) {
        return Err(Error::UnsupportedContainer { format, detail });
    }
    Geometry::for_len(bytes.len()).ok_or(Error::Corrupt {
        what: "image size (neither a DD nor an HD floppy)",
    })
}

/// Name a geometry for an error message.
pub(crate) fn shape_name(geometry: Geometry) -> &'static str {
    if geometry == DD {
        "double-density"
    } else if geometry == HD {
        "high-density"
    } else {
        "unrecognised"
    }
}

/// A raw ADF image: decoded sectors, plus the geometry their number implies.
///
/// This is the layer beneath [`Disk`](crate::Disk). It knows nothing about
/// filesystems — a bootblock-only disk, a copy-protected loader's raw track
/// data, or a wholly blank image are all perfectly good `Image`s. What it
/// offers is addressing: the same bytes reachable either by cylinder/head/
/// sector, the way a drive names them, or by logical block, the way AmigaDOS
/// does.
///
/// Every accessor is bounds-checked and returns a [`Result`]. Nothing here
/// panics on an out-of-range address.
///
/// ```
/// use format198x_commodore_amiga_adf::{DD, Image, Volume, FileSystem};
/// let bytes = Volume::new("Demo", FileSystem::Ofs).build().unwrap();
///
/// let image = Image::open(&bytes).unwrap();
/// assert_eq!(image.geometry(), DD);
/// assert_eq!(image.sector(0, 0, 0).unwrap().len(), 512);
/// assert_eq!(image.track(0, 0).unwrap().len(), 11 * 512);
/// assert!(image.sector(80, 0, 0).is_err()); // past the last cylinder
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    bytes: &'a [u8],
    geometry: Geometry,
}

impl<'a> Image<'a> {
    /// Open a raw ADF image, accepting either DD or HD.
    ///
    /// A file in another disk-image container — IPF, DMS, a zip, a gzipped
    /// `.adz` — is named as what it is ([`Error::UnsupportedContainer`]) rather
    /// than measured, because a size complaint about a format the file never
    /// was sends the reader to check a disk image that is not at fault.
    pub fn open(bytes: &'a [u8]) -> Result<Self, Error> {
        let geometry = geometry_of(bytes)?;
        Ok(Image { bytes, geometry })
    }

    /// The image's geometry.
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// The whole image.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// One 512-byte sector, addressed the way a drive addresses it.
    pub fn sector(&self, cyl: u16, head: u8, sector: u8) -> Result<&'a [u8], Error> {
        self.block(check_chs(self.geometry, cyl, head, sector)?)
    }

    /// A whole track — every sector of one cylinder and head, in order, as a
    /// single contiguous slice.
    ///
    /// An ADF stores its sectors in exactly this order, so a track really is
    /// one run of bytes rather than a gathering of scattered ones. That is what
    /// lets an MFM encoder take this slice straight from the image without
    /// copying it.
    pub fn track(&self, cyl: u16, head: u8) -> Result<&'a [u8], Error> {
        let first = check_chs(self.geometry, cyl, head, 0)?;
        let len = self.geometry.sectors_per_track as usize * BSIZE;
        let off = first as usize * BSIZE;
        Ok(&self.bytes[off..off + len])
    }

    /// One 512-byte block by logical block number — AmigaDOS's own way of
    /// naming a sector, and the addressing the filesystem layer uses.
    pub fn block(&self, lba: u32) -> Result<&'a [u8], Error> {
        let limit = self.geometry.blocks();
        if lba >= limit {
            return Err(Error::OutOfBounds {
                what: "block",
                got: lba,
                limit,
            });
        }
        let off = lba as usize * BSIZE;
        Ok(&self.bytes[off..off + BSIZE])
    }

    /// Re-answer the two questions this layer can ask: is the length one of the
    /// geometries this crate knows, and do the leading bytes still say ADF
    /// rather than IPF, DMS, zip or gzip.
    ///
    /// **A clean result does not mean the sectors are intact.** An ADF is
    /// decoded sectors with no per-sector check data — that absence is exactly
    /// what distinguishes it from a flux-level image. There is nothing at this
    /// layer to checksum, so nothing here can tell you a sector is sound. For
    /// that, interpret the image as a filesystem and use
    /// `Disk::check`.
    ///
    /// Worth running after writing through [`ImageMut`]: a write is free to put
    /// bytes at offset 0 that make the file look like some other container.
    pub fn verify(&self) -> Result<(), Error> {
        geometry_of(self.bytes).map(|_| ())
    }
}

/// A writable raw ADF image — the counterpart to [`Image`]. A real Amiga writes
/// to floppies, so a library that only reads them models half the machine.
///
/// ```
/// use format198x_commodore_amiga_adf::{DD, ImageMut};
/// let mut bytes = ImageMut::blank(DD);
/// let mut image = ImageMut::open(&mut bytes).unwrap();
///
/// image.write_sector(40, 1, 5, &[0xa5; 512]).unwrap();
/// assert_eq!(image.as_image().sector(40, 1, 5).unwrap(), &[0xa5; 512]);
/// ```
#[derive(Debug)]
pub struct ImageMut<'a> {
    bytes: &'a mut [u8],
    geometry: Geometry,
}

impl<'a> ImageMut<'a> {
    /// Open a raw ADF image for writing, accepting either DD or HD. Rejects the
    /// same things [`Image::open`] rejects.
    pub fn open(bytes: &'a mut [u8]) -> Result<Self, Error> {
        let geometry = geometry_of(bytes)?;
        Ok(ImageMut { bytes, geometry })
    }

    /// A zero-filled image of the given geometry — an unformatted disk. It has
    /// no boot block and no filesystem; [`Volume`](crate::Volume) puts those
    /// on.
    pub fn blank(geometry: Geometry) -> Vec<u8> {
        vec![0u8; geometry.len()]
    }

    /// The image's geometry.
    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// A read-only view of the same bytes.
    pub fn as_image(&self) -> Image<'_> {
        Image {
            bytes: self.bytes,
            geometry: self.geometry,
        }
    }

    /// One 512-byte sector, mutable.
    pub fn sector_mut(&mut self, cyl: u16, head: u8, sector: u8) -> Result<&mut [u8], Error> {
        let lba = check_chs(self.geometry, cyl, head, sector)?;
        self.block_mut(lba)
    }

    /// One 512-byte block by logical block number, mutable.
    pub fn block_mut(&mut self, lba: u32) -> Result<&mut [u8], Error> {
        let limit = self.geometry.blocks();
        if lba >= limit {
            return Err(Error::OutOfBounds {
                what: "block",
                got: lba,
                limit,
            });
        }
        let off = lba as usize * BSIZE;
        Ok(&mut self.bytes[off..off + BSIZE])
    }

    /// Replace one sector. `data` must be exactly 512 bytes.
    pub fn write_sector(
        &mut self,
        cyl: u16,
        head: u8,
        sector: u8,
        data: &[u8],
    ) -> Result<(), Error> {
        if data.len() != BSIZE {
            return Err(Error::BadSectorLength { got: data.len() });
        }
        self.sector_mut(cyl, head, sector)?.copy_from_slice(data);
        Ok(())
    }

    /// The whole image.
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }
}

/// Convert a CHS address to a block number, naming the coordinate that is out
/// of range rather than just saying the address is bad.
fn check_chs(geometry: Geometry, cyl: u16, head: u8, sector: u8) -> Result<u32, Error> {
    if cyl >= geometry.cylinders {
        return Err(Error::OutOfBounds {
            what: "cylinder",
            got: cyl as u32,
            limit: geometry.cylinders as u32,
        });
    }
    if head >= geometry.heads {
        return Err(Error::OutOfBounds {
            what: "head",
            got: head as u32,
            limit: geometry.heads as u32,
        });
    }
    if sector >= geometry.sectors_per_track {
        return Err(Error::OutOfBounds {
            what: "sector",
            got: sector as u32,
            limit: geometry.sectors_per_track as u32,
        });
    }
    geometry.lba(cyl, head, sector).ok_or(Error::OutOfBounds {
        what: "block",
        got: 0,
        limit: geometry.blocks(),
    })
}
