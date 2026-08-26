//! ProTracker **MOD** modules — parse and write the Amiga tracker format.
//!
//! A `.mod` file holds 31 sample slots of signed 8-bit PCM, a 128-entry order
//! table, and a sequence of 4-channel, 64-row patterns. This crate reads and
//! writes that byte layout. **It does not play modules** — no mixer, no tick
//! loop, no effect processing. Playback lives in `play198x-core`: tick
//! scheduling and effect dispatch are playback semantics, not file layout,
//! and keeping them out of this crate is why it stays dependency-free and
//! trivially embeddable behind an FFI boundary. See
//! `198x/reference/by-topic/music-formats/protracker-playback-reference.md`
//! for the playback semantics this crate deliberately does not implement —
//! including a place where the widely-cited community MOD specification is
//! wrong about the vibrato rate. That reference is about what happens *after*
//! parsing; it changed nothing about this crate's byte layout, only confirmed
//! where the parse/play boundary belongs.
//!
//! # Identification
//!
//! Identify a module by the magic at [`MAGIC_OFFSET`] (`is_module`), never by
//! file extension — the classic `.mod` convention is not load-bearing, and
//! trackers on other platforms (Startrekker, etc.) reused the same byte
//! layout under other extensions.
//!
//! # Scope
//!
//! Only 4-channel modules (`M.K.`, `M!K!`, `FLT4`, `4CHN`) decode:
//! [`Note`]'s pattern rows are fixed at 4 channels, so a 6- or 8-channel
//! module (`6CHN`, `8CHN`) is recognised by [`is_module`] — the sniff is not
//! a promise of decodability — but rejected by [`decode`] with
//! [`DecodeError::UnsupportedChannelCount`] rather than silently
//! misinterpreting its wider pattern rows as 4-channel ones.
//!
//! # What a round-trip cannot preserve
//!
//! [`Module`]'s fields are sized to the *meaningful* content of the file
//! (a title trimmed at its terminator, an order table trimmed to the song
//! length, a loop flag rather than a raw repeat length), not to its raw
//! bytes. That is what lets the required tests assert a clean `"SYNTH"`
//! title and a 1-entry `orders` for a 1-position song — but it also means a
//! handful of byte positions are not reconstructable from a decoded
//! `Module`, confirmed empirically against 17 real Amiga music-disk modules
//! (see the task report): every one of them diverged from
//! `encode(decode(bytes))` in only these ways, never in pattern data, sample
//! PCM, or any other header field:
//!
//! - **The restart byte** (offset 951). The community format documentation
//!   describes it as "historically set to 127, but can be safely ignored";
//!   [`encode`] always writes `0`.
//! - **The magic variant** when it isn't `M.K.` — [`encode`] always writes
//!   `M.K.`, even if the original used `M!K!`, `FLT4`, or `4CHN`.
//! - **Bytes trailing a name or the title past its first NUL.** Real
//!   ProTracker files routinely leave non-zero leftover bytes in the
//!   fixed-width name/title fields after the terminator (old buffer
//!   content the tracker never cleared). [`decode`] trims at the first NUL
//!   (required for `Module::title`/[`Sample::name`] to hold a clean
//!   string); [`encode`] zero-pads instead of restoring the leftover bytes.
//! - **Order-table bytes past the song length.** The 128-entry table often
//!   carries non-zero padding after the entries actually played;
//!   [`Module::orders`] holds only the used prefix, so [`encode`] zero-pads
//!   the rest.
//! - **A loop length of exactly one word.** The format's own "no loop"
//!   convention is a repeat length of zero *or* one word; both decode to
//!   [`Sample::loop_len`] `0`, so [`encode`] cannot tell which the original
//!   file used and always writes `0`.
//! - **A finetune byte's unused upper nibble.** Only the low nibble is
//!   meaningful; [`decode`] discards the rest and [`encode`] always writes
//!   it back as zero.
//!
//! Every other byte — every pattern cell, every sample's PCM data, every
//! sample length/volume/loop-start, the song length, and the magic when it
//! is `M.K.` — reproduces exactly.
//!
//! # Example
//!
//! ```
//! use format_commodore_amiga_mod::is_module;
//!
//! let mut bytes = vec![0u8; 1084];
//! bytes[1080..1084].copy_from_slice(b"M.K.");
//! assert!(is_module(&bytes));
//! ```

mod error;
mod parse;
mod write;

pub use error::{DecodeError, EncodeError};
pub use parse::decode;
pub use write::encode;

/// Byte offset of the 4-byte format magic that identifies a ProTracker
/// module. Identification always reads this offset, never the file
/// extension.
pub const MAGIC_OFFSET: usize = 1080;

/// The recognised ProTracker/Noisetracker/Startrekker magics. `6CHN` and
/// `8CHN` are recognised here (for [`is_module`]) but rejected by
/// [`decode`] — see the crate documentation's Scope section.
pub(crate) const MAGICS: [&[u8; 4]; 6] = [b"M.K.", b"M!K!", b"FLT4", b"4CHN", b"6CHN", b"8CHN"];

/// Whether `bytes` looks like a ProTracker MOD module: long enough to hold
/// the fixed header, with a recognised magic at [`MAGIC_OFFSET`].
///
/// This is a cheap sniff, not a validation — [`decode`] does the real
/// checking and can still fail (or reject the channel count) on bytes this
/// function accepts. Identification is by magic, never by file extension.
#[must_use]
pub fn is_module(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC_OFFSET + 4
        && MAGICS
            .iter()
            .any(|m| &bytes[MAGIC_OFFSET..MAGIC_OFFSET + 4] == *m)
}

/// One sample slot: signed 8-bit PCM plus the header fields ProTracker plays
/// it with.
///
/// An unused sample slot decodes with `data` empty and every numeric field
/// zero — a module always has exactly 31 of these, most of them unused in a
/// typical song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// The sample's name (up to 22 bytes in the file), trimmed at the first
    /// NUL.
    pub name: String,
    /// Signed 8-bit PCM sample data.
    pub data: Vec<i8>,
    /// Playback volume, 0..=64 in genuine ProTracker files (the header field
    /// is a full byte, so this crate does not clamp it).
    pub volume: u8,
    /// Finetune, -8..=7 (a signed nibble; a hostile header's upper nibble is
    /// discarded rather than preserved — see [`encode`]'s documentation).
    pub finetune: i8,
    /// Loop start, in bytes from the start of `data`.
    pub loop_start: usize,
    /// Loop length, in bytes. `0` means the sample does not loop — the
    /// header's own convention for "no loop" (a repeat length of one word or
    /// less) collapses onto this same value, so the original raw repeat
    /// length is not recoverable once decoded.
    pub loop_len: usize,
}

/// One 4-byte pattern cell: which sample retriggers (if any), the period to
/// play it at, and a raw effect number and parameter.
///
/// This is exactly what the file bytes say — decoding stops at the effect
/// number and parameter. What effect `4` (vibrato) or `9` (sample offset)
/// *means* during playback is `play198x-core`'s job, not this crate's; see
/// the crate documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Note {
    /// Sample number to trigger, 0 meaning "no change". Full byte range:
    /// the pattern cell's two nibbles combine to 8 bits, even though
    /// genuine ProTracker files only use 0..=31.
    pub sample: u8,
    /// Amiga hardware period, 0 meaning "no note". 12 significant bits in
    /// the file (0..=4095); this field carries the full decoded value.
    pub period: u16,
    /// Raw effect number, 0..=15 (one nibble).
    pub effect: u8,
    /// Effect parameter byte, meaning dependent on `effect`.
    pub param: u8,
}

/// A parsed ProTracker MOD module: title, 31 sample slots, the order table,
/// and the pattern data it references.
///
/// `patterns[p]` is a stored pattern (64 rows of 4 channels each);
/// `orders[i]` is the pattern index played at song position `i`. `decode`
/// stores exactly `max(orders) + 1` patterns, matching how many the file
/// physically contains; `encode` writes exactly `patterns.len()` of them
/// back, whatever that is for a hand-built `Module`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The module title (up to 20 bytes in the file), trimmed at the first
    /// NUL.
    pub title: String,
    /// Always 31 entries — every sample slot the format has, used or not.
    pub samples: Vec<Sample>,
    /// The song's order table: which pattern index plays at each position.
    pub orders: Vec<u8>,
    /// The stored patterns, each 64 rows of 4 [`Note`]s.
    pub patterns: Vec<Vec<[Note; 4]>>,
}
