//! Spectravideo SVI-318/328 `.cas` tape-image codec.
//!
//! CAS is the decoded byte stream presented to the cassette waveform layer.
//! Each block begins with sixteen `$55` bytes followed by `$7f`; another marker
//! ends the current block and begins the next. The layout is cross-checked
//! against MAME's `svi_cas.cpp` implementation.

/// Marker preceding every block in an SVI CAS image.
pub const BLOCK_MARKER: [u8; 17] = [
    0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55,
    0x7F,
];

/// A decoded SVI cassette image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CasImage {
    blocks: Vec<Vec<u8>>,
}

impl CasImage {
    /// Build an image from non-empty block payloads.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError::NoBlocks`] for an empty image or
    /// [`EncodeError::EmptyBlock`] when a block has no payload.
    pub fn new(blocks: Vec<Vec<u8>>) -> Result<Self, EncodeError> {
        if blocks.is_empty() {
            return Err(EncodeError::NoBlocks);
        }
        if let Some(index) = blocks.iter().position(Vec::is_empty) {
            return Err(EncodeError::EmptyBlock { index });
        }
        Ok(Self { blocks })
    }

    /// Marker-delimited payload blocks in tape order.
    #[must_use]
    pub fn blocks(&self) -> &[Vec<u8>] {
        &self.blocks
    }

    /// Consume the image and return its payload blocks.
    #[must_use]
    pub fn into_blocks(self) -> Vec<Vec<u8>> {
        self.blocks
    }
}

/// Why an SVI CAS byte stream could not be decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// The image does not begin with the seventeen-byte block marker.
    MissingInitialMarker,
    /// A marker is followed by another marker or end-of-input.
    EmptyBlock { index: usize },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingInitialMarker => write!(f, "SVI CAS image is missing its initial marker"),
            Self::EmptyBlock { index } => write!(f, "SVI CAS block {index} is empty"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why an SVI CAS image could not be encoded.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// An image must contain at least one block.
    NoBlocks,
    /// Every block must contain at least one payload byte.
    EmptyBlock { index: usize },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoBlocks => write!(f, "SVI CAS image contains no blocks"),
            Self::EmptyBlock { index } => write!(f, "SVI CAS block {index} is empty"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Decode marker-delimited blocks from an SVI CAS image.
///
/// # Errors
///
/// Returns [`DecodeError`] when the initial marker is absent or a block is
/// empty. Marker bytes inside a payload delimit the next block by definition.
pub fn decode(bytes: &[u8]) -> Result<CasImage, DecodeError> {
    if !bytes.starts_with(&BLOCK_MARKER) {
        return Err(DecodeError::MissingInitialMarker);
    }

    let mut blocks = Vec::new();
    let mut start = BLOCK_MARKER.len();
    loop {
        let next = find_marker(bytes, start);
        let end = next.unwrap_or(bytes.len());
        if end == start {
            return Err(DecodeError::EmptyBlock {
                index: blocks.len(),
            });
        }
        blocks.push(bytes[start..end].to_vec());
        let Some(marker_at) = next else {
            break;
        };
        start = marker_at + BLOCK_MARKER.len();
    }
    Ok(CasImage { blocks })
}

/// Encode an SVI CAS image, writing the marker before every payload block.
///
/// # Errors
///
/// Returns [`EncodeError`] if the image contains no blocks or an empty block.
pub fn encode(image: &CasImage) -> Result<Vec<u8>, EncodeError> {
    if image.blocks.is_empty() {
        return Err(EncodeError::NoBlocks);
    }
    let mut bytes = Vec::new();
    for (index, block) in image.blocks.iter().enumerate() {
        if block.is_empty() {
            return Err(EncodeError::EmptyBlock { index });
        }
        bytes.extend_from_slice(&BLOCK_MARKER);
        bytes.extend_from_slice(block);
    }
    Ok(bytes)
}

fn find_marker(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(BLOCK_MARKER.len())
        .position(|window| window == BLOCK_MARKER)
        .map(|relative| start + relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_reencodes_multiple_blocks_losslessly() {
        let mut bytes = BLOCK_MARKER.to_vec();
        bytes.extend_from_slice(&[1, 2, 3]);
        bytes.extend_from_slice(&BLOCK_MARKER);
        bytes.extend_from_slice(&[4, 5]);
        let image = decode(&bytes).expect("valid CAS");
        assert_eq!(image.blocks(), &[vec![1, 2, 3], vec![4, 5]]);
        assert_eq!(encode(&image).expect("encode"), bytes);
    }

    #[test]
    fn rejects_missing_marker() {
        assert_eq!(decode(&[1, 2, 3]), Err(DecodeError::MissingInitialMarker));
    }

    #[test]
    fn rejects_empty_blocks() {
        assert_eq!(
            decode(&BLOCK_MARKER),
            Err(DecodeError::EmptyBlock { index: 0 })
        );
        assert_eq!(
            CasImage::new(vec![Vec::new()]),
            Err(EncodeError::EmptyBlock { index: 0 })
        );
    }
}
