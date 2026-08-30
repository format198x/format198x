//! Typed failures for Oric TAP decoding and encoding.

/// Why a byte stream could not be decoded as an ordinary Oric TAP image.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// A file did not begin with at least one `$16` leader byte.
    MissingLeader { at: usize },
    /// The byte following the leader was not the `$24` sync marker.
    MissingSync { at: usize, found: Option<u8> },
    /// Fewer than nine header bytes remained after the sync marker.
    TruncatedHeader { at: usize, available: usize },
    /// No NUL terminator appeared within the ROM's 17-byte name limit.
    UnterminatedName { at: usize },
    /// The end address precedes the start address.
    ReversedAddressRange { start: u16, end: u16 },
    /// The image ended before the header-declared data length was present.
    TruncatedData {
        at: usize,
        expected: usize,
        available: usize,
    },
    /// An Atmos `STORE` array uses a different, unreliable length convention.
    UnsupportedArray { at: usize },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MissingLeader { at } => write!(f, "missing Oric TAP leader at byte {at}"),
            Self::MissingSync { at, found } => match found {
                Some(byte) => write!(
                    f,
                    "expected Oric TAP sync byte $24 at byte {at}, found ${byte:02X}"
                ),
                None => write!(
                    f,
                    "expected Oric TAP sync byte $24 at byte {at}, found end of input"
                ),
            },
            Self::TruncatedHeader { at, available } => write!(
                f,
                "truncated Oric TAP header at byte {at}: need 9 bytes, found {available}"
            ),
            Self::UnterminatedName { at } => write!(
                f,
                "Oric TAP name at byte {at} is not NUL-terminated within 17 bytes"
            ),
            Self::ReversedAddressRange { start, end } => write!(
                f,
                "Oric TAP end address ${end:04X} precedes start address ${start:04X}"
            ),
            Self::TruncatedData {
                at,
                expected,
                available,
            } => write!(
                f,
                "truncated Oric TAP data at byte {at}: expected {expected} bytes, found {available}"
            ),
            Self::UnsupportedArray { at } => write!(
                f,
                "Atmos STORE array at byte {at} has no reliable address-sized TAP length"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why files could not be encoded as an Oric TAP image.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// A file name exceeds the ROM's seventeen-byte limit or contains NUL.
    InvalidName,
    /// The data cannot fit between the chosen start address and `$FFFF`.
    DataTooLong { start: u16, length: usize },
    /// At least one leader byte is required.
    EmptyLeader,
    /// Ordinary Oric TAP records always describe at least one memory byte.
    EmptyData,
    /// An edited header no longer describes the attached data length.
    AddressLengthMismatch { start: u16, end: u16, length: usize },
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidName => write!(f, "Oric TAP names must contain 0 to 17 non-NUL bytes"),
            Self::DataTooLong { start, length } => write!(
                f,
                "{length} bytes do not fit in Oric memory from ${start:04X}"
            ),
            Self::EmptyLeader => write!(f, "an Oric TAP file needs at least one $16 leader byte"),
            Self::EmptyData => write!(
                f,
                "an ordinary Oric TAP file cannot contain zero data bytes"
            ),
            Self::AddressLengthMismatch { start, end, length } => write!(
                f,
                "Oric TAP address range ${start:04X}-${end:04X} does not describe {length} data bytes"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}
