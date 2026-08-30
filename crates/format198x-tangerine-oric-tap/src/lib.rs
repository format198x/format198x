//! Oric-1 and Oric Atmos `.tap` tape-image codec.
//!
//! TAP is the decoded byte stream seen by the ROM loader. Each file is:
//!
//! 1. one or more `$16` leader bytes;
//! 2. a `$24` sync marker;
//! 3. a nine-byte header;
//! 4. a NUL-terminated name of at most seventeen bytes;
//! 5. `end - start + 1` data bytes.
//!
//! The layout is cross-checked against the OSDK `Header` and `Tap2Dsk`
//! utilities and Oricutron's `tape.c`. Primary machine behaviour and the
//! address/name semantics are documented in the 198x reference library's
//! *Oric Atmos Handbook* and *The Oric-1 Companion*.

mod error;

pub use error::{DecodeError, EncodeError};

/// Tape leader byte emitted repeatedly before each file.
pub const LEADER_BYTE: u8 = 0x16;
/// Byte marking the end of the leader and beginning of the header.
pub const SYNC_BYTE: u8 = 0x24;
/// Size of the header following [`SYNC_BYTE`].
pub const HEADER_LEN: usize = 9;
/// Longest name accepted by the ROM cassette commands.
pub const MAX_NAME_LEN: usize = 17;
/// OSDK's default number of leader bytes.
pub const DEFAULT_LEADER_LEN: usize = 3;

/// The ordinary file kinds encoded in header byte 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// Tokenised BASIC program (`$00`).
    Basic,
    /// Machine code or an arbitrary memory block (`$80`).
    MachineCode,
}

impl FileKind {
    const fn byte(self) -> u8 {
        match self {
            Self::Basic => 0x00,
            Self::MachineCode => 0x80,
        }
    }

    const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Basic),
            0x80 => Some(Self::MachineCode),
            _ => None,
        }
    }
}

/// One ordinary file stored in an Oric TAP image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapFile {
    /// Number of `$16` bytes preceding this file. Historical tools produce
    /// anything from one to hundreds; the value is preserved on round-trip.
    pub leader_len: usize,
    /// The nine header bytes exactly as stored.
    pub header: [u8; HEADER_LEN],
    /// File name bytes, excluding the terminating NUL.
    pub name: Vec<u8>,
    /// File contents.
    pub data: Vec<u8>,
}

impl TapFile {
    /// Build an ordinary BASIC or machine-code file with OSDK-compatible
    /// header values and a three-byte leader.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError::InvalidName`] for a name longer than seventeen
    /// bytes or containing NUL, and [`EncodeError::DataTooLong`] when the data
    /// would extend beyond `$FFFF`.
    pub fn new(
        kind: FileKind,
        autorun: bool,
        start: u16,
        name: impl AsRef<[u8]>,
        data: Vec<u8>,
    ) -> Result<Self, EncodeError> {
        let name = name.as_ref();
        validate_name(name)?;
        if data.is_empty() {
            return Err(EncodeError::EmptyData);
        }
        let end = end_address(start, data.len())?;
        let mut header = [0; HEADER_LEN];
        header[2] = kind.byte();
        header[3] = if autorun { 0xC7 } else { 0x00 };
        header[4..6].copy_from_slice(&end.to_be_bytes());
        header[6..8].copy_from_slice(&start.to_be_bytes());
        Ok(Self {
            leader_len: DEFAULT_LEADER_LEN,
            header,
            name: name.to_vec(),
            data,
        })
    }

    /// BASIC or machine-code meaning of header byte 2.
    #[must_use]
    pub fn kind(&self) -> Option<FileKind> {
        FileKind::from_byte(self.header[2])
    }

    /// Whether the ROM should execute the file after loading.
    #[must_use]
    pub fn autorun(&self) -> bool {
        self.header[3] != 0
    }

    /// Inclusive final address from the header.
    #[must_use]
    pub fn end_address(&self) -> u16 {
        u16::from_be_bytes([self.header[4], self.header[5]])
    }

    /// First load address from the header.
    #[must_use]
    pub fn start_address(&self) -> u16 {
        u16::from_be_bytes([self.header[6], self.header[7]])
    }
}

/// Decode every concatenated ordinary file in an Oric TAP byte stream.
///
/// Header bytes and leader lengths are retained exactly, so
/// `encode(&decode(bytes)?)? == bytes` for accepted input.
///
/// # Errors
///
/// Returns a [`DecodeError`] when a leader, sync marker, header, name, address
/// range, or declared data extent is malformed. Atmos `STORE` arrays are
/// rejected because the ROM does not store a reliable byte length for them.
pub fn decode(bytes: &[u8]) -> Result<Vec<TapFile>, DecodeError> {
    let mut files = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let file_at = pos;
        while bytes.get(pos) == Some(&LEADER_BYTE) {
            pos += 1;
        }
        let leader_len = pos - file_at;
        if leader_len == 0 {
            return Err(DecodeError::MissingLeader { at: pos });
        }
        if bytes.get(pos) != Some(&SYNC_BYTE) {
            return Err(DecodeError::MissingSync {
                at: pos,
                found: bytes.get(pos).copied(),
            });
        }
        pos += 1;

        let available = bytes.len() - pos;
        if available < HEADER_LEN {
            return Err(DecodeError::TruncatedHeader { at: pos, available });
        }
        let mut header = [0; HEADER_LEN];
        header.copy_from_slice(&bytes[pos..pos + HEADER_LEN]);
        pos += HEADER_LEN;

        // Atmos STORE marks arrays in the second flag byte. Their header's
        // apparent size is affected by ROM bugs and is not an address range.
        if header[1] != 0 {
            return Err(DecodeError::UnsupportedArray { at: file_at });
        }

        let name_at = pos;
        let name_end = bytes[pos..bytes.len().min(pos + MAX_NAME_LEN + 1)]
            .iter()
            .position(|byte| *byte == 0)
            .map(|relative| pos + relative)
            .ok_or(DecodeError::UnterminatedName { at: name_at })?;
        let name = bytes[pos..name_end].to_vec();
        pos = name_end + 1;

        let start = u16::from_be_bytes([header[6], header[7]]);
        let end = u16::from_be_bytes([header[4], header[5]]);
        let data_len = end
            .checked_sub(start)
            .map(|length| usize::from(length) + 1)
            .ok_or(DecodeError::ReversedAddressRange { start, end })?;
        let available = bytes.len() - pos;
        if available < data_len {
            return Err(DecodeError::TruncatedData {
                at: pos,
                expected: data_len,
                available,
            });
        }
        let data = bytes[pos..pos + data_len].to_vec();
        pos += data_len;
        files.push(TapFile {
            leader_len,
            header,
            name,
            data,
        });
    }
    Ok(files)
}

/// Encode concatenated Oric TAP files.
///
/// # Errors
///
/// Returns [`EncodeError`] when a file has no leader, has an invalid name, or
/// its data does not agree with its address range.
pub fn encode(files: &[TapFile]) -> Result<Vec<u8>, EncodeError> {
    let capacity = files.iter().fold(0usize, |total, file| {
        total.saturating_add(
            file.leader_len + 1 + HEADER_LEN + file.name.len() + 1 + file.data.len(),
        )
    });
    let mut bytes = Vec::with_capacity(capacity);
    for file in files {
        if file.leader_len == 0 {
            return Err(EncodeError::EmptyLeader);
        }
        validate_name(&file.name)?;
        if file.data.is_empty() {
            return Err(EncodeError::EmptyData);
        }
        let expected_end = end_address(file.start_address(), file.data.len())?;
        if expected_end != file.end_address() {
            return Err(EncodeError::AddressLengthMismatch {
                start: file.start_address(),
                end: file.end_address(),
                length: file.data.len(),
            });
        }
        bytes.resize(bytes.len() + file.leader_len, LEADER_BYTE);
        bytes.push(SYNC_BYTE);
        bytes.extend_from_slice(&file.header);
        bytes.extend_from_slice(&file.name);
        bytes.push(0);
        bytes.extend_from_slice(&file.data);
    }
    Ok(bytes)
}

fn validate_name(name: &[u8]) -> Result<(), EncodeError> {
    if name.len() > MAX_NAME_LEN || name.contains(&0) {
        return Err(EncodeError::InvalidName);
    }
    Ok(())
}

fn end_address(start: u16, length: usize) -> Result<u16, EncodeError> {
    let last_offset = length.saturating_sub(1);
    let last_offset =
        u16::try_from(last_offset).map_err(|_| EncodeError::DataTooLong { start, length })?;
    start
        .checked_add(last_offset)
        .ok_or(EncodeError::DataTooLong { start, length })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine_file() -> TapFile {
        TapFile::new(
            FileKind::MachineCode,
            true,
            0x500,
            "hello",
            vec![0xA9, 0x41, 0x60],
        )
        .expect("fixture is valid")
    }

    #[test]
    fn osdk_shaped_file_round_trips() {
        let file = machine_file();
        let bytes = encode(core::slice::from_ref(&file)).expect("fixture encodes");
        assert_eq!(&bytes[..4], &[0x16, 0x16, 0x16, 0x24]);
        assert_eq!(decode(&bytes), Ok(vec![file]));
    }

    #[test]
    fn concatenated_files_keep_different_leaders() {
        let mut first = machine_file();
        first.leader_len = 1;
        let mut second =
            TapFile::new(FileKind::Basic, false, 0x501, "", vec![1, 2]).expect("fixture is valid");
        second.leader_len = 259;
        let files = vec![first, second];
        let bytes = encode(&files).expect("fixtures encode");
        assert_eq!(decode(&bytes), Ok(files));
    }

    #[test]
    fn truncated_data_reports_declared_and_available_lengths() {
        let mut bytes = encode(&[machine_file()]).expect("fixture encodes");
        bytes.pop();
        assert!(matches!(
            decode(&bytes),
            Err(DecodeError::TruncatedData {
                expected: 3,
                available: 2,
                ..
            })
        ));
    }

    #[test]
    fn unterminated_seventeen_byte_name_is_rejected() {
        let mut bytes = vec![0x16, 0x24];
        let mut header = [0; HEADER_LEN];
        header[4..6].copy_from_slice(&0x500u16.to_be_bytes());
        header[6..8].copy_from_slice(&0x500u16.to_be_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(b"12345678901234567X");
        assert_eq!(
            decode(&bytes),
            Err(DecodeError::UnterminatedName { at: 11 })
        );
    }

    #[test]
    fn empty_data_is_rejected() {
        assert_eq!(
            TapFile::new(FileKind::Basic, false, 0xFFFF, "empty", Vec::new()),
            Err(EncodeError::EmptyData)
        );
    }
}
