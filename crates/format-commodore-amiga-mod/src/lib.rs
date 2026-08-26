//! ProTracker **MOD** modules — parse and write the Amiga tracker format.
//!
//! A `.mod` file holds 31 sample slots of signed 8-bit PCM, a 128-entry order
//! table, and a sequence of 4-channel, 64-row patterns. This crate reads and
//! writes that byte layout **losslessly**: `encode(decode(bytes)) == bytes`
//! for every 4-channel module, verified against 45 real Amiga music-disk
//! modules across two independent corpora (see the task report). **It does
//! not play modules** — no mixer,
//! no tick loop, no effect processing. Playback lives in `play198x-core`:
//! tick scheduling and effect dispatch are playback semantics, not file
//! layout, and keeping them out of this crate is why it stays
//! dependency-free and trivially embeddable behind an FFI boundary. See
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
//! module (`6CHN`, `8CHN`, and Startrekker's `FLT8`) is recognised by
//! [`is_module`] — the sniff is not a promise of decodability — but
//! rejected by [`decode`] with
//! [`DecodeError::UnsupportedChannelCount`] rather than silently
//! misinterpreting its wider pattern rows as 4-channel ones.
//!
//! # Hidden patterns
//!
//! The number of patterns a file stores is **not** reliably implied by
//! anything in the order table — the widely-cited community MOD
//! specification glosses over this entirely. Some real files store
//! additional pattern data referenced only by order-table slots *beyond*
//! the song length ("hidden" patterns that never play but are still
//! physically present); but the unplayed tail of the order table is also
//! where other real files leave non-zero leftover garbage that does *not*
//! correspond to a stored pattern. Nothing in the table itself tells the
//! two apart — one real file's garbage byte implied 233 patterns when only
//! 9 were physically present, which would read straight past the end of
//! the file.
//!
//! [`decode`] instead derives the pattern count from the file's own
//! arithmetic: the format's three regions (the 1084-byte header, the
//! patterns, and all 31 samples' PCM data, concatenated in that order) are
//! contiguous and exhaustive, so the pattern data's exact size is
//! `bytes.len() - 1084 - (the samples' total byte length)` — independent of
//! whatever the order table claims. Verified exact (evenly divisible by
//! 1024) on every file across two independent real-media corpora,
//! including both the hidden-pattern files and the garbage-tail files.
//! [`encode`] writes back exactly `patterns.len()` patterns, so hidden
//! pattern data round-trips like everything else.
//!
//! The size rule assumes nothing follows the last sample, which is not true
//! of every file: a module ripped out of an executable, padded to a block
//! boundary, or stored inside a larger container carries surplus bytes at
//! the end, and reading those as extra patterns shifts every sample's PCM
//! into the junk. The order table caps the count as a cross-check — no file
//! stores a pattern no order-table entry can name — and any surplus beyond
//! that cap is kept verbatim in [`Module::trailing`], so the module still
//! re-encodes byte-identically.
//!
//! # Losslessness: raw fields plus ergonomic accessors
//!
//! An editor (Studio198x's tracker) that opens a module, changes one note,
//! and saves it must not silently corrupt every byte it didn't touch. So
//! [`Module`] and [`Sample`] store the file's raw bytes and words directly —
//! [`Module::title_bytes`], [`Sample::name_bytes`], [`Module::order_table`],
//! [`Module::restart`], [`Module::magic`], [`Sample::finetune_byte`],
//! [`Sample::repeat_start_words`], [`Sample::repeat_length_words`] — rather
//! than a value derived from them. Nothing here is thrown away: a decoded
//! module's *entire* byte content survives, including bytes ProTracker
//! itself never reads (a name's leftover bytes past its NUL, order-table
//! padding past the song length, a finetune byte's unused upper nibble, the
//! specific "no loop" encoding a sample used).
//!
//! Reading a raw byte array is not pleasant API, so every field with a more
//! useful shape also has an accessor: [`Module::title`] and [`Sample::name`]
//! return the trimmed, readable text (decoded as ISO-8859-1, which is what
//! Amiga text is — not UTF-8); [`Module::orders`] returns the
//! slice of the order table actually played; [`Sample::finetune`] returns
//! the signed nibble value; [`Sample::loop_start`], [`Sample::loop_len`],
//! and [`Sample::is_looped`] give the loop points in bytes. Read through the
//! accessors; write through the raw fields (or leave them as `decode` set
//! them) so nothing is lost on the way back out.
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

/// Width of [`Module::title_bytes`], in bytes.
pub const TITLE_LEN: usize = 20;

/// Width of a [`Sample::name_bytes`] field, in bytes.
pub const SAMPLE_NAME_LEN: usize = 22;

/// Width of [`Module::order_table`], in bytes — the format's fixed
/// 128-entry order table, regardless of how many positions the song
/// actually plays.
pub const ORDER_TABLE_LEN: usize = 128;

/// The recognised ProTracker/Noisetracker/Startrekker magics. `6CHN`,
/// `8CHN` and `FLT8` are recognised here (for [`is_module`]) but rejected
/// by [`decode`] — see the crate documentation's Scope section. `FLT8` is
/// Startrekker's 8-channel magic, the counterpart to its 4-channel `FLT4`.
pub(crate) const MAGICS: [&[u8; 4]; 7] = [
    b"M.K.", b"M!K!", b"FLT4", b"4CHN", b"6CHN", b"8CHN", b"FLT8",
];

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

/// Trim a fixed-width, NUL-padded byte field to the readable text before its
/// first NUL (or its full width, if there is none), decoded as ISO-8859-1.
///
/// Amiga text is Latin-1, not UTF-8, and MOD titles and sample names carry
/// accented letters and box-drawing bytes routinely — sample names are
/// where content authors traditionally hid messages, so they are the least
/// ASCII part of the file. Decoding them as UTF-8 threw the whole string
/// away on a single high byte, which shows up in a metadata panel as an
/// empty title. Latin-1 maps each byte to the code point of the same value,
/// so this is identical to UTF-8 decoding for pure ASCII and never empty
/// for non-empty input.
///
/// The raw bytes (`name_bytes`/`title_bytes`) remain the round-tripping
/// form regardless of what this returns.
fn trimmed_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| b as char).collect()
}

/// One sample slot: signed 8-bit PCM plus the header fields ProTracker plays
/// it with, stored exactly as the file holds them.
///
/// An unused sample slot decodes with `data` empty and every field zero — a
/// module always has exactly 31 of these ([`Module::samples`] is an array,
/// not a `Vec`), most of them unused in a typical song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    /// The sample's 22-byte name field, exactly as stored — including any
    /// bytes after a NUL terminator (real files routinely leave old buffer
    /// content there). Use [`name`](Sample::name) for the readable form.
    pub name_bytes: [u8; SAMPLE_NAME_LEN],
    /// Signed 8-bit PCM sample data.
    pub data: Vec<i8>,
    /// Playback volume, 0..=64 in genuine ProTracker files (the header field
    /// is a full byte, so this crate does not clamp it).
    pub volume: u8,
    /// The raw finetune byte, exactly as stored. Only the low nibble is
    /// meaningful to ProTracker (a signed nibble, -8..=7) — use
    /// [`finetune`](Sample::finetune) for that value — but the upper nibble
    /// is preserved here even though the format never uses it.
    pub finetune_byte: u8,
    /// Loop start, in words from the start of the sample, exactly as
    /// stored.
    pub repeat_start_words: u16,
    /// Loop length, in words, exactly as stored. The format's own
    /// convention for "no loop" is a repeat length of `0` *or* `1`; both
    /// are preserved here rather than collapsed to one value, so encoding
    /// reproduces whichever the file actually used. Use
    /// [`is_looped`](Sample::is_looped)/[`loop_len`](Sample::loop_len) for
    /// the played meaning.
    pub repeat_length_words: u16,
}

impl Sample {
    /// The sample's name, trimmed at its first NUL and decoded as ISO-8859-1
    /// — which is what Amiga text is. Never loses data: the raw
    /// [`name_bytes`](Sample::name_bytes) round-trip regardless.
    #[must_use]
    pub fn name(&self) -> String {
        trimmed_string(&self.name_bytes)
    }

    /// The finetune value ProTracker plays with: a signed nibble, -8..=7,
    /// taken from the low 4 bits of [`finetune_byte`](Sample::finetune_byte).
    #[must_use]
    pub fn finetune(&self) -> i8 {
        let raw = self.finetune_byte & 0x0F;
        if raw >= 8 {
            (raw as i8) - 16
        } else {
            raw as i8
        }
    }

    /// Loop start, in bytes from the start of [`data`](Sample::data).
    #[must_use]
    pub fn loop_start(&self) -> usize {
        usize::from(self.repeat_start_words) * 2
    }

    /// Whether the sample loops: a repeat length greater than one word.
    #[must_use]
    pub fn is_looped(&self) -> bool {
        self.repeat_length_words > 1
    }

    /// Loop length in bytes, or `0` if [`is_looped`](Sample::is_looped) is
    /// `false`.
    #[must_use]
    pub fn loop_len(&self) -> usize {
        if self.is_looped() {
            usize::from(self.repeat_length_words) * 2
        } else {
            0
        }
    }
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
/// and the pattern data it references — stored exactly as the file holds
/// them, so `encode(decode(bytes)) == bytes` for every 4-channel module.
///
/// `patterns[p]` is a stored pattern (64 rows of 4 channels each);
/// `orders()[i]` is the pattern index played at song position `i`. `decode`
/// stores exactly as many patterns as the file physically contains,
/// derived from the file's total size rather than from the order table
/// (see the crate documentation's "Hidden patterns" section — the order
/// table cannot reliably say how many patterns are stored); `encode` writes
/// exactly `patterns.len()` of them back, whatever that is for a hand-built
/// `Module`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The module's 20-byte title field, exactly as stored — including any
    /// bytes after a NUL terminator. Use [`title`](Module::title) for the
    /// readable form.
    pub title_bytes: [u8; TITLE_LEN],
    /// Every sample slot the format has, used or not — always exactly 31,
    /// which is why this is an array rather than a `Vec`.
    pub samples: [Sample; 31],
    /// How many of [`order_table`](Module::order_table)'s 128 entries the
    /// song actually plays. Legal values are 1..=128 in a genuine file;
    /// carried separately from the table itself so the unplayed remainder
    /// (which real files often leave as non-zero leftover bytes) is never
    /// implied to be unused padding.
    pub song_length: u8,
    /// The full 128-entry order table, exactly as stored. Use
    /// [`orders`](Module::orders) for the played prefix.
    pub order_table: [u8; ORDER_TABLE_LEN],
    /// The restart byte (offset 951 in the file). The community format
    /// documentation describes it as "historically set to 127, but can be
    /// safely ignored" — ProTracker itself does not read it, but it is
    /// preserved here so encoding reproduces it exactly.
    pub restart: u8,
    /// The 4-byte format magic, exactly as stored (`M.K.`, `M!K!`, `FLT4`,
    /// or `4CHN` — [`decode`] rejects `6CHN`/`8CHN`/`FLT8`, see the crate
    /// documentation's Scope section).
    pub magic: [u8; 4],
    /// The stored patterns, each exactly 64 rows of 4 [`Note`]s — a fixed
    /// shape in the format, so a fixed shape in the type.
    pub patterns: Vec<[[Note; 4]; 64]>,
    /// Bytes sitting after the last sample's PCM data, kept verbatim so a
    /// re-encode is byte-identical. Empty for a file that ends exactly where
    /// its sample data does, which is most of them; non-empty for a module
    /// ripped out of an executable, padded to a block boundary, or stored
    /// inside a larger container. [`encode`] appends these unchanged.
    pub trailing: Vec<u8>,
}

impl Module {
    /// The module title, trimmed at its first NUL and decoded as ISO-8859-1
    /// — which is what Amiga text is. Never loses data: the raw
    /// [`title_bytes`](Module::title_bytes) round-trip regardless.
    #[must_use]
    pub fn title(&self) -> String {
        trimmed_string(&self.title_bytes)
    }

    /// The order table's played prefix: `order_table[..song_length]`,
    /// clamped to the table's length so this never panics even on a
    /// hand-built `Module` with an out-of-range `song_length`.
    #[must_use]
    pub fn orders(&self) -> &[u8] {
        let len = (self.song_length as usize).min(self.order_table.len());
        &self.order_table[..len]
    }
}
