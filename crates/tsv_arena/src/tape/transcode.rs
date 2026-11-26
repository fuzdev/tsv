//! JSON wire → tape (`docs/tape_format.md` §Producing a tape): the rules `scripts/tape_encode.ts`
//! states over `JSON.parse`'s tree, applied while reading the JSON text once.
//!
//! The reader is a frame stack, not a recursion: an open object or array is a `Frame`, and the
//! loop steps the top one a member at a time, so nesting costs heap, never stack.
//!
//! An object is written against one candidate entry at a time: the first of its `type`'s entries
//! (or the `type`-less entry its position names), each key matched against the entry's next
//! fields — the next key is predicted by the entry's field order, the fields skipped on the way
//! being the ones its mask leaves out — and each value against its field's encoding. A key the
//! entry has no field for, or a value its encoding refuses, rewinds the object (tape, pool,
//! pending refs, read position) and writes it again against the next candidate, and once none is
//! left as a self-describing record, which takes anything. The writers' JSON fits its first
//! candidate but where the schema's variants differ from their base, so a rewind is rare and
//! shallow.

use super::{candidates_of_type, eq_short, first_of_type, next_of_type};
use std::collections::HashMap;
use tsv_lang::tape::shapes::{
    ENTRIES, KEYWORDS, PRE_TYPE_MAX, SHAPES_HASH, TAPE_FORMAT_VERSION, pre_type_encoding,
};
use tsv_lang::tape::{
    Enc, HEADER_ESCAPE, HEADER_NULL, HEADER_WORDS, MAGIC, NO_AUX, NO_BIT, NULL_REF, TAG_ARRAY,
    TAG_FALSE, TAG_NULL, TAG_NUMBER, TAG_OBJECT, TAG_STRING, TAG_TRUE, generic_header, shape_id,
};

/// Why a JSON text could not be transcoded. A wire the writers emit never fails; these are the
/// texts no tape can carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TapeError {
    /// The text is not compact JSON (the writers emit no whitespace) at byte `at`.
    Malformed {
        /// Byte offset into the JSON text.
        at: usize,
    },
    /// A string holds a lone surrogate, which the UTF-8 pool cannot carry, at byte `at`.
    LoneSurrogate {
        /// Byte offset into the JSON text.
        at: usize,
    },
    /// The source is too long for a string ref's signed offset, or the tape for its word count.
    TooLarge,
}

impl std::fmt::Display for TapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { at } => write!(f, "not compact JSON at byte {at}"),
            Self::LoneSurrogate { at } => {
                write!(f, "a string holding a lone surrogate at byte {at}")
            }
            Self::TooLarge => f.write_str("the source or the tape is too large for a tape"),
        }
    }
}

impl std::error::Error for TapeError {}

/// The transcoder's reusable state: its buffers keep their capacity from call to call, so a
/// caller transcoding many documents allocates only as the largest grows them.
#[derive(Default)]
pub struct Transcoder {
    tape: Vec<u32>,
    pool: Vec<u8>,
    pool_units: u32,
    pending: Vec<Pending>,
    frames: Vec<Frame>,
    /// The attempt each object refitted to, by its opening brace and position context: a re-read
    /// of an enclosing object starts it there, so an object is refitted at most once per
    /// candidate however many times its ancestors are rewound. Each rewind still re-reads its
    /// object's whole text, so a chain of objects each refused at its close costs time
    /// quadratic in the chain's depth — crafted JSON only: the parsers' depth limits bound a
    /// writer's wire, where a refusal after an object's children is rare and never nests.
    settled: HashMap<(usize, u16), (u16, bool)>,
    /// Where the furthest refused step began: every object [`Transcoder::settled`] holds
    /// opened before it, so an object opening at or past it is read for the first time and
    /// needs no lookup.
    reread_end: usize,
    /// How many object rewinds the last transcode took.
    refits: usize,
    scratch: Vec<u8>,
    key_scratch: Vec<u8>,
    /// UTF-16 index → byte offset; `INSIDE_PAIR` for a pair's second unit. Empty for ASCII.
    u2b: Vec<u32>,
    /// Byte offset → UTF-16 index, meaningful at character boundaries. Empty for ASCII.
    b2u: Vec<u32>,
    ascii: bool,
    units: u32,
    out: Vec<u8>,
}

impl std::fmt::Debug for Transcoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transcoder").finish_non_exhaustive()
    }
}

const INSIDE_PAIR: u32 = u32::MAX;
const NO_ENTRY: u16 = u16::MAX;

/// A UTF-16 range of the source a string may be sliced from, or none.
type Scope = Option<(u32, u32)>;

/// A string ref waiting for its object to close: the tape slot and the JSON string's body.
#[derive(Clone, Copy)]
struct Pending {
    slot: usize,
    text: Str,
}

/// A JSON string's body (between its quotes).
#[derive(Clone, Copy)]
struct Str {
    begin: usize,
    end: usize,
    escaped: bool,
    ascii: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// An object written against a candidate entry (or, before its `type`, against the keys
    /// some entry carries there).
    Typed,
    /// An object written self-describing.
    Escape,
    /// A list of records.
    List,
    /// A tagged value's array.
    Values,
}

#[derive(Clone, Copy)]
struct Frame {
    kind: Kind,
    /// Nothing read since the opening bracket.
    first: bool,
    /// The scope an object inherits; for an array, the scope its items inherit.
    scope: Scope,
    /// An array's count slot, or an escape record's key-count slot.
    count_slot: usize,
    count: u32,
    // objects only
    open: usize,
    tape_mark: usize,
    pool_mark: usize,
    pool_units_mark: u32,
    pending_mark: usize,
    context: u16,
    attempt: u16,
    candidates: u16,
    entry: u16,
    cursor: u16,
    mask: u32,
    start: Option<u32>,
    end: Option<u32>,
    pre: [(u32, u32); PRE_TYPE_MAX],
    pre_len: u8,
}

impl Frame {
    fn array(kind: Kind, scope: Scope, count_slot: usize) -> Self {
        Self {
            kind,
            first: true,
            scope,
            count_slot,
            count: 0,
            open: 0,
            tape_mark: 0,
            pool_mark: 0,
            pool_units_mark: 0,
            pending_mark: 0,
            context: NO_ENTRY,
            attempt: 0,
            candidates: 0,
            entry: NO_ENTRY,
            cursor: 0,
            mask: 0,
            start: None,
            end: None,
            pre: [(0, 0); PRE_TYPE_MAX],
            pre_len: 0,
        }
    }
}

/// Why a step stopped: the object on top must be written again, or the text is untranscodable.
enum Stop {
    Refit,
    Hard(TapeError),
}

impl From<TapeError> for Stop {
    fn from(e: TapeError) -> Self {
        Self::Hard(e)
    }
}

type Step = Result<usize, Stop>;

/// The byte at `p`, `0` past the end — a byte compact JSON never holds outside a string, so a
/// read past the end fails where the next structural byte is checked.
#[inline]
fn at(json: &[u8], p: usize) -> u8 {
    json.get(p).copied().unwrap_or(0)
}

#[inline]
fn expect(json: &[u8], p: usize, byte: u8) -> Result<usize, TapeError> {
    if at(json, p) == byte {
        Ok(p + 1)
    } else {
        Err(TapeError::Malformed { at: p })
    }
}

/// Scans a string body from `p` (just past its opening quote) to its closing quote, eight bytes a
/// step where eight remain.
#[inline]
fn scan_string(json: &[u8], mut p: usize) -> Result<Str, TapeError> {
    const LO: u64 = 0x0101_0101_0101_0101;
    const HI: u64 = 0x8080_8080_8080_8080;
    let begin = p;
    let mut escaped = false;
    let mut high = false;
    loop {
        if let Some(chunk) = json.get(p..p + 8) {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            let bits = u64::from_le_bytes(word);
            let quotes = bits ^ (LO * u64::from(b'"'));
            let slashes = bits ^ (LO * u64::from(b'\\'));
            let hit =
                ((quotes.wrapping_sub(LO) & !quotes) | (slashes.wrapping_sub(LO) & !slashes)) & HI;
            if hit == 0 {
                high |= bits & HI != 0;
                p += 8;
                continue;
            }
            let at_hit = (hit.trailing_zeros() / 8) as usize;
            let before = if at_hit == 0 {
                0
            } else {
                bits & ((1u64 << (at_hit * 8)) - 1)
            };
            high |= before & HI != 0;
            p += at_hit;
        }
        match json.get(p) {
            Some(b'"') => {
                return Ok(Str {
                    begin,
                    end: p,
                    escaped,
                    ascii: !high,
                });
            }
            Some(b'\\') => {
                escaped = true;
                p += 2;
            }
            Some(&c) => {
                high |= c >= 0x80;
                p += 1;
            }
            None => return Err(TapeError::Malformed { at: p }),
        }
    }
}

/// Decodes an escaped string body into `out`.
fn unescape(json: &[u8], s: Str, out: &mut Vec<u8>) -> Result<(), TapeError> {
    out.clear();
    let hex4 = |q: usize| -> Result<u32, TapeError> {
        let digits = json.get(q..q + 4).ok_or(TapeError::Malformed { at: q })?;
        // `from_str_radix` also takes a leading `+`, which no JSON `\u` escape spells
        if !digits.iter().all(u8::is_ascii_hexdigit) {
            return Err(TapeError::Malformed { at: q });
        }
        let text = std::str::from_utf8(digits).map_err(|_| TapeError::Malformed { at: q })?;
        u32::from_str_radix(text, 16).map_err(|_| TapeError::Malformed { at: q })
    };
    let mut p = s.begin;
    while p < s.end {
        let c = json[p];
        if c != b'\\' {
            out.push(c);
            p += 1;
            continue;
        }
        let escape_at = p;
        let d = at(json, p + 1);
        p += 2;
        let ch = match d {
            b'n' => '\n',
            b't' => '\t',
            b'r' => '\r',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'u' => {
                let mut cp = hex4(p)?;
                p += 4;
                if (0xD800..0xDC00).contains(&cp) && at(json, p) == b'\\' && at(json, p + 1) == b'u'
                {
                    let lo = hex4(p + 2)?;
                    if (0xDC00..0xE000).contains(&lo) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        p += 6;
                    }
                }
                char::from_u32(cp).ok_or(TapeError::LoneSurrogate { at: escape_at })?
            }
            _ => return Err(TapeError::Malformed { at: escape_at }),
        };
        let mut buf = [0u8; 4];
        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
    }
    Ok(())
}

/// UTF-16 units in UTF-8 `bytes`.
#[inline]
fn utf16_len(bytes: &[u8], ascii: bool) -> u32 {
    if ascii {
        return bytes.len() as u32;
    }
    let mut n = 0u32;
    for &c in bytes {
        n += u32::from((c & 0xC0) != 0x80) + u32::from(c >= 0xF0);
    }
    n
}

/// The first occurrence of `needle` (non-empty) in `haystack`.
#[inline]
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    let first = needle[0];
    let last = haystack.len() - needle.len();
    let mut i = 0;
    while i <= last {
        let k = haystack[i..=last].iter().position(|&c| c == first)?;
        i += k;
        if haystack[i..i + needle.len()] == *needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// A plain JSON unsigned integer at `p` (digits, no sign, fraction or exponent) that fits a
/// `u32`: its value and end.
#[inline]
fn plain_u32(json: &[u8], p: usize) -> Option<(u32, usize)> {
    const L: u64 = 0x0101_0101_0101_0101;
    const H: u64 = 0x8080_8080_8080_8080;
    // fewer than eight digits, read eight bytes at a time with no branch per digit: the
    // non-digit bytes found word-wise (exact up to the first one, all this needs), the digits
    // ahead of it shifted to the word's top as leading zeros and combined pairwise
    if let Some(chunk) = json.get(p..p + 8) {
        let w = u64::from_le_bytes([
            chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
        ]);
        let non_digit = ((w.wrapping_sub(0x30 * L) & !w) | w.wrapping_add(0x46 * L) | w) & H;
        let n = (non_digit.trailing_zeros() / 8) as usize;
        if (1..8).contains(&n) {
            if matches!(chunk[n], b'.' | b'e' | b'E') {
                return None;
            }
            let mut v = w.wrapping_sub(0x30 * L) << (8 * (8 - n));
            v = (v.wrapping_mul(10) + (v >> 8)) & 0x00FF_00FF_00FF_00FF;
            v = (v.wrapping_mul(100) + (v >> 16)) & 0x0000_FFFF_0000_FFFF;
            v = (v.wrapping_mul(10000) + (v >> 32)) & 0xFFFF_FFFF;
            return Some((v as u32, p + n));
        }
        if n == 0 {
            return None;
        }
    }
    // up to ten digits accumulate in a `u64` with no overflow, checked once at the end
    let mut q = p;
    let mut v: u64 = 0;
    while q < p + 10 {
        match json.get(q) {
            Some(&c) if c.is_ascii_digit() => {
                v = v * 10 + u64::from(c - b'0');
                q += 1;
            }
            _ => break,
        }
    }
    if q == p + 10 && at(json, q).is_ascii_digit() {
        return plain_u32_long(json, p);
    }
    if q == p || v > u64::from(u32::MAX) || matches!(at(json, q), b'.' | b'e' | b'E') {
        return None;
    }
    Some((v as u32, q))
}

/// [`plain_u32`] past ten digits, which only leading zeros keep in range.
#[cold]
fn plain_u32_long(json: &[u8], p: usize) -> Option<(u32, usize)> {
    let mut q = p;
    let mut v: u64 = 0;
    while let Some(&c) = json.get(q) {
        if !c.is_ascii_digit() {
            break;
        }
        v = v * 10 + u64::from(c - b'0');
        if v > u64::from(u32::MAX) {
            return None;
        }
        q += 1;
    }
    if q == p || matches!(at(json, q), b'.' | b'e' | b'E') {
        return None;
    }
    Some((v as u32, q))
}

/// A JSON number token at `p`: its value and end.
fn number(json: &[u8], p: usize) -> Result<(f64, usize), TapeError> {
    let mut q = p;
    while matches!(at(json, q), b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
        q += 1;
    }
    if q == p {
        return Err(TapeError::Malformed { at: p });
    }
    let text = std::str::from_utf8(&json[p..q]).map_err(|_| TapeError::Malformed { at: p })?;
    let value = text
        .parse::<f64>()
        .map_err(|_| TapeError::Malformed { at: p })?;
    Ok((value, q))
}

/// The end of the JSON literal `word` at `p`.
#[inline]
fn literal(json: &[u8], p: usize, word: &[u8]) -> Result<usize, TapeError> {
    if json.get(p..p + word.len()) == Some(word) {
        Ok(p + word.len())
    } else {
        Err(TapeError::Malformed { at: p })
    }
}

/// A string body's decoded bytes: the JSON bytes themselves when it has no escape, else
/// `scratch` decoded into.
#[inline]
fn decoded<'a>(json: &'a [u8], s: Str, scratch: &'a mut Vec<u8>) -> Result<&'a [u8], TapeError> {
    if s.escaped {
        unescape(json, s, scratch)?;
        Ok(scratch)
    } else {
        Ok(&json[s.begin..s.end])
    }
}

/// The header's two size words, the body's word count and the pool's byte count — refused when
/// either outgrows its word, or the pool's UTF-16 length reaches `i32::MAX`, past which a pool
/// ref's first word (`-1 - start`) would read as a source slice or the null ref.
fn sizes(words: usize, pool_bytes: usize, pool_units: u32) -> Result<(u32, u32), TapeError> {
    if pool_units >= i32::MAX as u32 {
        return Err(TapeError::TooLarge);
    }
    let words = u32::try_from(words).map_err(|_| TapeError::TooLarge)?;
    let pool_bytes = u32::try_from(pool_bytes).map_err(|_| TapeError::TooLarge)?;
    Ok((words, pool_bytes))
}

impl Transcoder {
    /// How many times the last [`Transcoder::transcode`] rewound an object to write it against
    /// another candidate (or self-describing) — the work a refusal costs, at most one per object
    /// per candidate.
    #[must_use]
    pub fn refits(&self) -> usize {
        self.refits
    }

    /// A transcoder with empty buffers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Transcodes a span-only JSON wire `json` — compact, as the writers emit it — against
    /// `source`, the text its offsets index (for a Svelte or CSS document, the source without a
    /// leading byte-order mark, which those wires elide), to a tape.
    ///
    /// # Errors
    ///
    /// [`TapeError`] for a text that is not compact JSON, a string holding a lone surrogate, or a
    /// source or tape past the format's sizes.
    pub fn transcode(&mut self, json: &[u8], source: &str) -> Result<&[u8], TapeError> {
        self.run(json, source)?;
        self.finish()
    }

    /// [`Transcoder::transcode`], the tape as words — the form a binding hands a host, since a
    /// `u32` buffer is 4-aligned by its type and a JS host views the tape in place (a
    /// `Uint32Array`, then the exact bytes over the same memory). Each word's bytes in memory
    /// are the tape's bytes in order, on any host: the header and body words are stored
    /// little-endian and the pool packed into the words after them, its last word
    /// zero-padded. Returns the words and the tape's length in bytes, the pool's last byte
    /// included and the padding not.
    ///
    /// The words are the transcoder's own buffer, the one the transcode wrote the body into —
    /// the tape is laid out where it was written, never copied — and stay valid until the next
    /// transcode. A caller that must own them copies the slice (an exact allocation); a caller
    /// that hands the host a view over them, as the WASM binding does, holds the one buffer a
    /// parked transcoder already keeps.
    ///
    /// # Errors
    ///
    /// As [`Transcoder::transcode`].
    pub fn transcode_words(
        &mut self,
        json: &[u8],
        source: &str,
    ) -> Result<(&[u32], usize), TapeError> {
        self.run(json, source)?;
        let header = self.header()?;
        let len = self.tape.len() * 4 + self.pool.len();
        self.tape[..HEADER_WORDS].copy_from_slice(&header);
        // a no-op on a little-endian host, the loop with it
        for word in &mut self.tape {
            *word = word.to_le();
        }
        let mut chunks = self.pool.chunks_exact(4);
        self.tape.extend(
            chunks
                .by_ref()
                .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]])),
        );
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut last = [0u8; 4];
            last[..rest.len()].copy_from_slice(rest);
            self.tape.push(u32::from_ne_bytes(last));
        }
        Ok((&self.tape, len))
    }

    /// Reads the JSON text into `tape` (after the header's words, which it leaves unwritten) and
    /// `pool`: everything a transcode does but lay out its output.
    fn run(&mut self, json: &[u8], source: &str) -> Result<(), TapeError> {
        self.start(source)?;
        let mut p = self.open_record(json, 0, NO_ENTRY, Some((0, self.units)))?;
        while let Some(top) = self.frames.last() {
            let step = match top.kind {
                Kind::Typed | Kind::Escape => self.step_object(json, p, source),
                Kind::List => self.step_list(json, p),
                Kind::Values => self.step_values(json, p),
            };
            p = match step {
                Ok(p) => p,
                Err(Stop::Refit) => self.refit(p)?,
                Err(Stop::Hard(e)) => return Err(e),
            };
        }
        if p != json.len() {
            return Err(TapeError::Malformed { at: p });
        }
        Ok(())
    }

    fn start(&mut self, source: &str) -> Result<(), TapeError> {
        self.tape.clear();
        self.tape.resize(HEADER_WORDS, 0);
        self.pool.clear();
        self.pool_units = 0;
        self.pending.clear();
        self.frames.clear();
        self.settled.clear();
        self.reread_end = 0;
        self.refits = 0;
        self.ascii = source.is_ascii();
        self.u2b.clear();
        self.b2u.clear();
        if self.ascii {
            self.units = u32::try_from(source.len()).map_err(|_| TapeError::TooLarge)?;
        } else {
            self.b2u.resize(source.len() + 1, 0);
            for (byte, ch) in source.char_indices() {
                self.b2u[byte] = self.u2b.len() as u32;
                self.u2b.push(byte as u32);
                if ch.len_utf16() == 2 {
                    self.u2b.push(INSIDE_PAIR);
                }
            }
            self.units = u32::try_from(self.u2b.len()).map_err(|_| TapeError::TooLarge)?;
            self.b2u[source.len()] = self.units;
            self.u2b.push(source.len() as u32);
        }
        if self.units >= i32::MAX as u32 {
            return Err(TapeError::TooLarge);
        }
        Ok(())
    }

    /// The tape's header words (`docs/tape_format.md` §The tape).
    fn header(&self) -> Result<[u32; HEADER_WORDS], TapeError> {
        let (words, pool_bytes) = sizes(self.body_words(), self.pool.len(), self.pool_units)?;
        Ok([
            MAGIC,
            TAPE_FORMAT_VERSION,
            SHAPES_HASH,
            words,
            pool_bytes,
            0,
            self.pool_units,
            self.units,
        ])
    }

    /// The body's word count: the tape buffer past the header's words.
    #[inline]
    fn body_words(&self) -> usize {
        self.tape.len() - HEADER_WORDS
    }

    fn finish(&mut self) -> Result<&[u8], TapeError> {
        let header = self.header()?;
        self.out.clear();
        self.out.reserve(self.tape.len() * 4 + self.pool.len());
        for word in header.iter().chain(&self.tape[HEADER_WORDS..]) {
            self.out.extend_from_slice(&word.to_le_bytes());
        }
        self.out.extend_from_slice(&self.pool);
        Ok(&self.out)
    }

    /// The UTF-16 range's bytes, `None` when it is out of the source or splits a pair.
    #[inline]
    fn byte_range(&self, start: u32, end: u32) -> Option<(usize, usize)> {
        if start > end || end > self.units {
            return None;
        }
        if self.ascii {
            return Some((start as usize, end as usize));
        }
        let (s, e) = (self.u2b[start as usize], self.u2b[end as usize]);
        if s == INSIDE_PAIR || e == INSIDE_PAIR {
            return None;
        }
        Some((s as usize, e as usize))
    }

    /// An object's own span: its `start` and `end` when both are integers naming a range of the
    /// source that splits no surrogate pair.
    #[inline]
    fn own_span(&self, frame: &Frame) -> Scope {
        let (Some(s), Some(e)) = (frame.start, frame.end) else {
            return None;
        };
        self.byte_range(s, e).map(|_| (s, e))
    }

    /// The scope a value under the object on top inherits: its own span once both its `start`
    /// and `end` were read, else the scope it inherited.
    #[inline]
    fn child_scope(&self) -> Scope {
        let frame = &self.frames[self.frames.len() - 1];
        self.own_span(frame).or(frame.scope)
    }

    /// Reserves a string ref's two words, for the open object to resolve when it closes.
    #[inline]
    fn push_ref(&mut self, json: &[u8], text: Str) -> Result<(), TapeError> {
        if text.escaped {
            // a lone surrogate fails here, as the offline encoder fails on it
            unescape(json, text, &mut self.scratch)?;
        }
        self.pending.push(Pending {
            slot: self.tape.len(),
            text,
        });
        self.tape.extend_from_slice(&[0, 0]);
        Ok(())
    }

    /// Resolves the refs pending since `mark` within `scope` (`docs/tape_format.md` §String
    /// refs): a slice of the source where the text occurs in the scope, else a pool entry.
    fn resolve(&mut self, json: &[u8], source: &str, mark: usize, scope: Scope) {
        let mut scratch = std::mem::take(&mut self.scratch);
        let range = scope.and_then(|(cs, ce)| self.byte_range(cs, ce).map(|b| (cs, b)));
        for k in mark..self.pending.len() {
            let Pending { slot, text } = self.pending[k];
            let Ok(bytes) = decoded(json, text, &mut scratch) else {
                continue; // a lone surrogate was refused when the ref was reserved
            };
            let ascii = text.ascii && (!text.escaped || bytes.is_ascii());
            let units = utf16_len(bytes, ascii);
            if let Some((cs, (bs, be))) = range {
                let found = if bytes.is_empty() {
                    Some(cs)
                } else {
                    find(&source.as_bytes()[bs..be], bytes).map(|i| {
                        if self.ascii {
                            (bs + i) as u32
                        } else {
                            self.b2u[bs + i]
                        }
                    })
                };
                if let Some(u) = found {
                    self.tape[slot] = u;
                    self.tape[slot + 1] = u + units;
                    continue;
                }
            }
            self.tape[slot] = (1 + self.pool_units).wrapping_neg();
            self.pool.extend_from_slice(bytes);
            self.pool_units += units;
            self.tape[slot + 1] = self.pool_units;
        }
        self.pending.truncate(mark);
        self.scratch = scratch;
    }

    /// Opens the record at `p` — `null` or an object — in a position whose context entry is
    /// `context` (or `NO_ENTRY`), inheriting `scope`.
    fn open_record(
        &mut self,
        json: &[u8],
        p: usize,
        context: u16,
        scope: Scope,
    ) -> Result<usize, TapeError> {
        match at(json, p) {
            b'n' => {
                self.tape.push(HEADER_NULL);
                literal(json, p, b"null")
            }
            b'{' => {
                self.open_settled(p, context, scope);
                Ok(p + 1)
            }
            _ => Err(TapeError::Malformed { at: p }),
        }
    }

    /// Pushes the frame of an object met in the read: at its first candidate, or at the attempt an
    /// earlier read of it settled on.
    fn open_settled(&mut self, open: usize, context: u16, scope: Scope) {
        // only a re-read meets an object an earlier read settled: skip the hash otherwise
        let (attempt, escape) = if open >= self.reread_end {
            (0, false)
        } else {
            self.settled
                .get(&(open, context))
                .copied()
                .unwrap_or((0, false))
        };
        self.open_object(open, context, scope, attempt, escape);
    }

    /// Pushes an object's frame: typed, against its `attempt`-th candidate, or self-describing.
    fn open_object(&mut self, open: usize, context: u16, scope: Scope, attempt: u16, escape: bool) {
        let mut frame = Frame::array(Kind::Typed, scope, 0);
        frame.open = open;
        frame.tape_mark = self.tape.len();
        frame.pool_mark = self.pool.len();
        frame.pool_units_mark = self.pool_units;
        frame.pending_mark = self.pending.len();
        frame.context = context;
        frame.attempt = attempt;
        self.tape.push(0);
        if escape {
            frame.kind = Kind::Escape;
            frame.count_slot = self.tape.len();
            self.tape.push(0);
        } else {
            frame.entry = context;
        }
        self.frames.push(frame);
    }

    /// Rewinds the object a refusal belongs to (popping a list frame above it) and opens it again
    /// on its next candidate, or self-describing once none is left; `at` is where the refused
    /// step began (a step reads every member up to a nested frame), past the rewound object's
    /// opening brace.
    fn refit(&mut self, at: usize) -> Result<usize, TapeError> {
        self.reread_end = self.reread_end.max(at);
        while self.frames.last().is_some_and(|f| f.kind == Kind::List) {
            self.frames.pop();
        }
        let frame = match self.frames.pop() {
            Some(frame) if frame.kind == Kind::Typed => frame,
            _ => return Err(TapeError::Malformed { at: 0 }),
        };
        self.tape.truncate(frame.tape_mark);
        self.pool.truncate(frame.pool_mark);
        self.pool_units = frame.pool_units_mark;
        self.pending.truncate(frame.pending_mark);
        self.refits += 1;
        let next = frame.attempt + 1;
        let escape = frame.context != NO_ENTRY || next >= frame.candidates;
        self.settled
            .insert((frame.open, frame.context), (next, escape));
        self.open_object(frame.open, frame.context, frame.scope, next, escape);
        Ok(frame.open + 1)
    }

    /// The members of the object on top, up to its close or the first value that opens a frame
    /// of its own (a nested object or array), which the loop in [`Transcoder::run`] steps next.
    fn step_object(&mut self, json: &[u8], mut p: usize, source: &str) -> Step {
        let top = self.frames.len() - 1;
        loop {
            if at(json, p) == b'}' {
                return self.close_object(json, p, source);
            }
            if !self.frames[top].first {
                p = expect(json, p, b',')?;
            }
            self.frames[top].first = false;
            p = expect(json, p, b'"')?;
            p = match self.predicted_member(json, p, source)? {
                Some(q) => q,
                None => self.member(json, p, source)?,
            };
            if self.frames.len() != top + 1 {
                return Ok(p);
            }
        }
    }

    /// A member of a typed object whose key is spelled, unescaped, as one of the fields its
    /// entry can take next — nearly every member a writer emits: the key is matched against the
    /// field order's prediction in place, never scanned. `None` leaves the member, unread, to
    /// [`Transcoder::member`], which gives any key the same answer the slow way.
    #[inline]
    fn predicted_member(
        &mut self,
        json: &[u8],
        p: usize,
        source: &str,
    ) -> Result<Option<usize>, Stop> {
        let top = self.frames.len() - 1;
        let frame = &self.frames[top];
        if frame.kind != Kind::Typed {
            return Ok(None);
        }
        if frame.entry == NO_ENTRY {
            // the `type` that opens nearly every object
            if json.get(p..p + 5) == Some(&b"type\""[..]) {
                let q = expect(json, p + 5, b':')?;
                return self.resolve_type(json, q).map(Some);
            }
            return Ok(None);
        }
        let fields = ENTRIES[usize::from(frame.entry)].fields;
        let mut cursor = usize::from(frame.cursor);
        let field = loop {
            let Some(field) = fields.get(cursor) else {
                return Ok(None);
            };
            let key = field.key.as_bytes();
            if json
                .get(p..p + key.len())
                .is_some_and(|spelled| eq_short(spelled, key))
                && at(json, p + key.len()) == b'"'
            {
                break *field;
            }
            if field.bit == NO_BIT {
                return Ok(None);
            }
            cursor += 1;
        };
        let q = expect(json, p + field.key.len() + 1, b':')?;
        let frame = &mut self.frames[top];
        frame.cursor = cursor as u16 + 1;
        if field.bit != NO_BIT {
            frame.mask |= 1 << field.bit;
        }
        if field.enc == Enc::Offset {
            // `Enc::Offset` in place, the own span's `start` and `end` read from its one parse
            let (v, end) = plain_u32(json, q).ok_or(Stop::Refit)?;
            match field.key {
                "start" => frame.start = Some(v),
                "end" => frame.end = Some(v),
                _ => {}
            }
            self.tape.push(v);
            return Ok(Some(end));
        }
        // the own span, as `member` reads it
        match field.key {
            "start" => frame.start = plain_u32(json, q).map(|(v, _)| v),
            "end" => frame.end = plain_u32(json, q).map(|(v, _)| v),
            _ => {}
        }
        self.field(json, q, field.enc, field.aux, source).map(Some)
    }

    /// One member of the object on top, its key read whatever its spelling.
    fn member(&mut self, json: &[u8], mut p: usize, source: &str) -> Step {
        let top = self.frames.len() - 1;
        let key = scan_string(json, p)?;
        p = expect(json, key.end + 1, b':')?;
        let key_bytes: &[u8] = if key.escaped {
            unescape(json, key, &mut self.key_scratch)?;
            &self.key_scratch
        } else {
            &json[key.begin..key.end]
        };
        // the own span, from `start` and `end` whatever the object's kind
        if key_bytes == b"start" {
            self.frames[top].start = plain_u32(json, p).map(|(v, _)| v);
        } else if key_bytes == b"end" {
            self.frames[top].end = plain_u32(json, p).map(|(v, _)| v);
        }
        let (kind, entry, cursor, pre_len) = {
            let frame = &self.frames[top];
            (frame.kind, frame.entry, frame.cursor, frame.pre_len)
        };
        if kind == Kind::Escape {
            self.frames[top].count += 1;
            self.push_ref(json, key)?;
            let scope = self.child_scope();
            return Ok(self.value(json, p, scope)?);
        }
        if entry == NO_ENTRY {
            // ahead of the `type`: only a key some entry carries there
            if key_bytes == b"type" {
                return self.resolve_type(json, p);
            }
            let Some(enc) = pre_type_encoding(key_bytes) else {
                return Err(Stop::Refit);
            };
            let n = usize::from(pre_len);
            if n == PRE_TYPE_MAX {
                return Err(Stop::Refit);
            }
            self.frames[top].pre[n] = (key.begin as u32, key.end as u32);
            self.frames[top].pre_len += 1;
            return self.field(json, p, enc, NO_AUX, source);
        }
        let fields = ENTRIES[usize::from(entry)].fields;
        let mut cursor = usize::from(cursor);
        loop {
            let Some(field) = fields.get(cursor) else {
                return Err(Stop::Refit);
            };
            if eq_short(field.key.as_bytes(), key_bytes) {
                break;
            }
            if field.bit == NO_BIT {
                return Err(Stop::Refit);
            }
            cursor += 1;
        }
        let field = fields[cursor];
        let frame = &mut self.frames[top];
        frame.cursor = cursor as u16 + 1;
        if field.bit != NO_BIT {
            frame.mask |= 1 << field.bit;
        }
        self.field(json, p, field.enc, field.aux, source)
    }

    /// The object's `type`: picks its candidate entry and checks the keys read ahead of it.
    fn resolve_type(&mut self, json: &[u8], p: usize) -> Step {
        let top = self.frames.len() - 1;
        if at(json, p) != b'"' {
            return Err(Stop::Refit);
        }
        let s = scan_string(json, p + 1)?;
        let first = first_of_type(decoded(json, s, &mut self.scratch)?);
        let frame = &mut self.frames[top];
        frame.candidates = first.map_or(0, candidates_of_type);
        let mut candidate = first;
        for _ in 0..frame.attempt {
            candidate = candidate.and_then(next_of_type);
        }
        let Some(entry) = candidate else {
            return Err(Stop::Refit);
        };
        let fields = ENTRIES[usize::from(entry)].fields;
        if frame.pre_len == 0
            && let Some(field) = fields.first()
            && field.key == "type"
        {
            // nothing read ahead of a leading `type`: the loop below, unrolled
            frame.entry = entry;
            frame.cursor = 1;
            frame.mask = if field.bit == NO_BIT {
                0
            } else {
                1 << field.bit
            };
            return Ok(s.end + 1);
        }
        let mut cursor = 0;
        let mut mask = 0u32;
        let pre = frame.pre;
        let names = pre[..usize::from(frame.pre_len)]
            .iter()
            .map(|&(b, e)| &json[b as usize..e as usize])
            .chain(std::iter::once(&b"type"[..]));
        for name in names {
            loop {
                let Some(field) = fields.get(cursor) else {
                    return Err(Stop::Refit);
                };
                cursor += 1;
                if eq_short(field.key.as_bytes(), name) {
                    if field.bit != NO_BIT {
                        mask |= 1 << field.bit;
                    }
                    break;
                }
                if field.bit == NO_BIT {
                    return Err(Stop::Refit);
                }
            }
        }
        frame.entry = entry;
        frame.cursor = cursor as u16;
        frame.mask = mask;
        Ok(s.end + 1)
    }

    /// One field's value at `p`, by its encoding; a value the encoding refuses refits the object.
    fn field(&mut self, json: &[u8], p: usize, enc: Enc, aux: u16, source: &str) -> Step {
        let byte = at(json, p);
        match enc {
            // a `type` past its resolution: an entry carries one
            Enc::Type => Err(Stop::Refit),
            Enc::Offset => {
                let (v, q) = plain_u32(json, p).ok_or(Stop::Refit)?;
                self.tape.push(v);
                Ok(q)
            }
            Enc::Boolean => match byte {
                b't' => {
                    self.tape.push(1);
                    Ok(literal(json, p, b"true")?)
                }
                b'f' => {
                    self.tape.push(0);
                    Ok(literal(json, p, b"false")?)
                }
                _ => Err(Stop::Refit),
            },
            Enc::Number => {
                if !matches!(byte, b'0'..=b'9' | b'-') {
                    return Err(Stop::Refit);
                }
                let (v, q) = number(json, p)?;
                self.push_f64(v);
                Ok(q)
            }
            Enc::Null => {
                if byte != b'n' {
                    return Err(Stop::Refit);
                }
                Ok(literal(json, p, b"null")?)
            }
            Enc::Own => {
                if byte != b'"' {
                    return Err(Stop::Refit);
                }
                let s = scan_string(json, p + 1)?;
                let top = self.frames.len() - 1;
                let (start, end) = self.own_span(&self.frames[top]).ok_or(Stop::Refit)?;
                let (bs, be) = self.byte_range(start, end).ok_or(Stop::Refit)?;
                if source.as_bytes()[bs..be] != *decoded(json, s, &mut self.scratch)? {
                    return Err(Stop::Refit);
                }
                Ok(s.end + 1)
            }
            Enc::Keyword => {
                if byte != b'"' {
                    return Err(Stop::Refit);
                }
                let s = scan_string(json, p + 1)?;
                let bytes = decoded(json, s, &mut self.scratch)?;
                let index = KEYWORDS[usize::from(aux)]
                    .iter()
                    .position(|v| eq_short(v.as_bytes(), bytes))
                    .ok_or(Stop::Refit)?;
                self.tape.push(index as u32);
                Ok(s.end + 1)
            }
            Enc::Ref | Enc::RefNullable => match byte {
                b'"' => {
                    let s = scan_string(json, p + 1)?;
                    self.push_ref(json, s)?;
                    Ok(s.end + 1)
                }
                b'n' if enc == Enc::RefNullable => {
                    self.tape.extend_from_slice(&[NULL_REF, 0]);
                    Ok(literal(json, p, b"null")?)
                }
                _ => Err(Stop::Refit),
            },
            Enc::Strings => {
                if byte != b'[' {
                    return Err(Stop::Refit);
                }
                let slot = self.tape.len();
                self.tape.push(0);
                let mut cursor = p + 1;
                if at(json, cursor) == b']' {
                    return Ok(cursor + 1);
                }
                let mut count = 0u32;
                loop {
                    if at(json, cursor) != b'"' {
                        return Err(Stop::Refit);
                    }
                    let s = scan_string(json, cursor + 1)?;
                    self.push_ref(json, s)?;
                    count += 1;
                    cursor = s.end + 1;
                    match at(json, cursor) {
                        b',' => cursor += 1,
                        b']' => break,
                        _ => return Err(Stop::Hard(TapeError::Malformed { at: cursor })),
                    }
                }
                self.tape[slot] = count;
                Ok(cursor + 1)
            }
            Enc::Record => {
                if !matches!(byte, b'{' | b'n') {
                    return Err(Stop::Refit);
                }
                let scope = self.child_scope();
                Ok(self.open_record(json, p, aux, scope)?)
            }
            Enc::List => {
                if byte != b'[' {
                    return Err(Stop::Refit);
                }
                let scope = self.child_scope();
                self.open_array(Kind::List, scope);
                Ok(p + 1)
            }
            Enc::Value => {
                let scope = self.child_scope();
                Ok(self.value(json, p, scope)?)
            }
        }
    }

    #[inline]
    fn push_f64(&mut self, v: f64) {
        let bits = v.to_bits();
        self.tape.push(bits as u32);
        self.tape.push((bits >> 32) as u32);
    }

    fn open_array(&mut self, kind: Kind, scope: Scope) {
        let count_slot = self.tape.len();
        self.tape.push(0);
        self.frames.push(Frame::array(kind, scope, count_slot));
    }

    /// A tagged value at `p` (`docs/tape_format.md` §Tagged values); an array or object opens a
    /// frame.
    fn value(&mut self, json: &[u8], p: usize, scope: Scope) -> Result<usize, TapeError> {
        match at(json, p) {
            b'n' => {
                self.tape.push(TAG_NULL);
                literal(json, p, b"null")
            }
            b't' => {
                self.tape.push(TAG_TRUE);
                literal(json, p, b"true")
            }
            b'f' => {
                self.tape.push(TAG_FALSE);
                literal(json, p, b"false")
            }
            b'"' => {
                self.tape.push(TAG_STRING);
                let s = scan_string(json, p + 1)?;
                self.push_ref(json, s)?;
                Ok(s.end + 1)
            }
            b'[' => {
                self.tape.push(TAG_ARRAY);
                self.open_array(Kind::Values, scope);
                Ok(p + 1)
            }
            b'{' => {
                self.tape.push(TAG_OBJECT);
                self.open_settled(p, NO_ENTRY, scope);
                Ok(p + 1)
            }
            b'0'..=b'9' | b'-' => {
                self.tape.push(TAG_NUMBER);
                let (v, q) = number(json, p)?;
                self.push_f64(v);
                Ok(q)
            }
            _ => Err(TapeError::Malformed { at: p }),
        }
    }

    /// The next item of the array on top (a list of records, or a tagged value's array), or its
    /// close.
    fn step_array(&mut self, json: &[u8], mut p: usize) -> Result<(usize, Scope), Step> {
        let top = self.frames.len() - 1;
        if at(json, p) == b']' {
            let frame = self.frames[top];
            self.frames.pop();
            self.tape[frame.count_slot] = frame.count;
            return Err(Ok(p + 1));
        }
        if !self.frames[top].first {
            p = expect(json, p, b',').map_err(|e| Err(Stop::Hard(e)))?;
        }
        let frame = &mut self.frames[top];
        frame.first = false;
        frame.count += 1;
        Ok((p, frame.scope))
    }

    fn step_list(&mut self, json: &[u8], p: usize) -> Step {
        let (p, scope) = match self.step_array(json, p) {
            Ok(item) => item,
            Err(done) => return done,
        };
        if !matches!(at(json, p), b'{' | b'n') {
            return Err(Stop::Refit);
        }
        Ok(self.open_record(json, p, NO_ENTRY, scope)?)
    }

    fn step_values(&mut self, json: &[u8], p: usize) -> Step {
        let (p, scope) = match self.step_array(json, p) {
            Ok(item) => item,
            Err(done) => return done,
        };
        Ok(self.value(json, p, scope)?)
    }

    /// Closes the object on top: checks its entry is complete, resolves its refs and writes its
    /// header.
    fn close_object(&mut self, json: &[u8], p: usize, source: &str) -> Step {
        let frame = &self.frames[self.frames.len() - 1];
        let (tape_mark, pending_mark) = (frame.tape_mark, frame.pending_mark);
        let header = if frame.kind == Kind::Escape {
            self.tape[frame.count_slot] = frame.count;
            HEADER_ESCAPE
        } else {
            if frame.entry == NO_ENTRY {
                return Err(Stop::Refit);
            }
            let rest = &ENTRIES[usize::from(frame.entry)].fields[usize::from(frame.cursor)..];
            if rest.iter().any(|f| f.bit == NO_BIT) {
                return Err(Stop::Refit);
            }
            shape_id(frame.entry, frame.mask)
                .unwrap_or_else(|| generic_header(frame.entry, frame.mask))
        };
        let scope = self.own_span(frame).or(frame.scope);
        self.resolve(json, source, pending_mark, scope);
        self.tape[tape_mark] = header;
        self.frames.pop();
        if self.body_words() >= u32::MAX as usize {
            return Err(Stop::Hard(TapeError::TooLarge));
        }
        Ok(p + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(json: &str, source: &str) -> Vec<u32> {
        let mut t = Transcoder::new();
        let bytes = t.transcode(json.as_bytes(), source).unwrap();
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    #[test]
    fn the_header_names_the_format() {
        let tape = words(r#"{"a":1}"#, "");
        assert_eq!(&tape[..3], &[MAGIC, TAPE_FORMAT_VERSION, SHAPES_HASH]);
    }

    #[test]
    fn an_unknown_object_is_self_describing() {
        // its one key sliced from the source's whole scope
        let tape = words(r#"{"a":true}"#, "a");
        assert_eq!(&tape[HEADER_WORDS..], &[HEADER_ESCAPE, 1, 0, 1, TAG_TRUE]);
    }

    #[test]
    fn text_no_tape_carries_is_an_error() {
        let mut t = Transcoder::new();
        for json in [r#"{"a":}"#, r#"{"a":1"#, r#"{"a":1} "#, r#"{ "a":1}"#, ""] {
            assert!(t.transcode(json.as_bytes(), "").is_err(), "{json}");
        }
        assert_eq!(
            t.transcode(br#"{"a":"\ud800"}"#, "").unwrap_err(),
            TapeError::LoneSurrogate { at: 6 }
        );
    }

    #[test]
    fn a_pool_past_the_signed_offset_is_refused() {
        assert_eq!(sizes(1, 1, 0), Ok((1, 1)));
        assert_eq!(sizes(1, 1, i32::MAX as u32 - 1), Ok((1, 1)));
        assert_eq!(sizes(1, 1, i32::MAX as u32), Err(TapeError::TooLarge));
    }

    #[test]
    fn a_unicode_escape_takes_hex_digits_only() {
        let mut t = Transcoder::new();
        assert!(t.transcode(br#"{"a":"\u0041"}"#, "").is_ok());
        assert_eq!(
            t.transcode(br#"{"a":"\u+041"}"#, "").unwrap_err(),
            TapeError::Malformed { at: 8 }
        );
    }

    #[test]
    fn the_header_carries_the_source_length() {
        let tape = words(r#"{"a":1}"#, "a😀b");
        assert_eq!(tape[7], 4);
    }

    /// An object that fails its entry only at its close (a required field missing), nested: each
    /// level refits once, never once per refit of an ancestor.
    #[test]
    fn nested_refits_are_linear() {
        // shallow on purpose: an exponential regression must fail the bound, not hang the run
        let depth = 16;
        let mut json = String::from("null");
        for _ in 0..depth {
            json = format!(
                r#"{{"type":"IfStatement","start":0,"end":0,"test":{json},"consequent":null}}"#
            );
        }
        let started = std::time::Instant::now();
        let mut t = Transcoder::new();
        let tape = t.transcode(json.as_bytes(), "").unwrap().to_vec();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{depth} nested refits took {:?}",
            started.elapsed()
        );
        assert_eq!(t.settled.len(), depth, "each level settles once");
        assert!(
            t.refits() <= 2 * depth,
            "{} rewinds for {depth} levels",
            t.refits()
        );
        // every level is written self-describing: header, key count, then its keys
        let body: Vec<u32> = tape[HEADER_WORDS * 4..]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(&body[..2], &[HEADER_ESCAPE, 5]);
    }

    /// The word form is the byte form, word by word in memory, padded to a whole word.
    #[test]
    fn the_word_tape_is_the_byte_tape() {
        let mut t = Transcoder::new();
        for (json, source) in [
            (r#"{"a":1}"#, ""),
            (r#"{"a":"xyz"}"#, ""),
            (r#"{"a":"wxyz"}"#, ""),
            (r#"{"a":"é"}"#, "a😀b"),
            (r#"{"type":"Identifier","start":0,"end":1,"name":"a"}"#, "a"),
        ] {
            let bytes = t.transcode(json.as_bytes(), source).unwrap().to_vec();
            let (words, len) = t.transcode_words(json.as_bytes(), source).unwrap();
            assert_eq!(len, bytes.len(), "{json}");
            assert_eq!(words.len(), bytes.len().div_ceil(4), "{json}");
            let laid: Vec<u8> = words.iter().flat_map(|w| w.to_ne_bytes()).collect();
            assert_eq!(&laid[..len], &bytes[..], "{json}");
            assert!(
                laid[len..].iter().all(|&b| b == 0),
                "{json}: the padding is zero"
            );
        }
        // the byte form after the word form, over the same buffers, is still the tape
        let (_, len) = t.transcode_words(br#"{"a":"xyz"}"#, "").unwrap();
        let bytes = t.transcode(br#"{"a":"xyz"}"#, "").unwrap();
        assert_eq!(bytes.len(), len);
    }

    /// The inline integer reader agrees with the plain digit loop it shortcuts.
    #[test]
    fn plain_u32_reads_as_the_digit_loop_does() {
        let reference = |text: &[u8]| -> Option<(u32, usize)> {
            let digits = text.iter().take_while(|c| c.is_ascii_digit()).count();
            if digits == 0 || matches!(text.get(digits), Some(b'.' | b'e' | b'E')) {
                return None;
            }
            let mut v: u64 = 0;
            for &c in &text[..digits] {
                v = v * 10 + u64::from(c - b'0');
                if v > u64::from(u32::MAX) {
                    return None;
                }
            }
            Some((v as u32, digits))
        };
        let mut cases: Vec<String> = Vec::new();
        for n in [
            0u64,
            7,
            42,
            999,
            1_234_567,
            12_345_678,
            99_999_999,
            123_456_789,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            99_999_999_999,
        ] {
            for tail in ["", ",", "}", "]", ".5", "e3", "E", ":", "\"", "x", "\u{80}"] {
                cases.push(format!("{n}{tail}"));
                cases.push(format!("{n}{tail}{}", ",\"start\":12345678901"));
                cases.push(format!("000{n}{tail}"));
            }
        }
        cases.extend(["", ",", "-1", "/0", ":0", "00000000000000000001,"].map(String::from));
        for case in &cases {
            for pad in ["", "x"] {
                let json = format!("{pad}{case}");
                let at = pad.len();
                assert_eq!(
                    plain_u32(json.as_bytes(), at),
                    reference(&json.as_bytes()[at..]).map(|(v, n)| (v, at + n)),
                    "{json:?}"
                );
            }
        }
    }

    /// The inline compare agrees with the slice compare at every length it branches on.
    #[test]
    fn eq_short_is_equality() {
        let text: Vec<u8> = (0..40u8).map(|i| b'a' + i % 26).collect();
        for n in 0..=text.len() {
            let a = &text[..n];
            assert!(eq_short(a, a), "{n}");
            for i in 0..n {
                let mut b = a.to_vec();
                b[i] ^= 1;
                assert!(!eq_short(a, &b), "{n} at {i}");
            }
            if n > 0 {
                assert!(!eq_short(a, &a[..n - 1]), "{n} against a prefix");
            }
        }
    }

    #[test]
    fn nesting_costs_no_stack() {
        let depth = 100_000;
        let json = format!(r#"{{"a":{}1{}}}"#, "[".repeat(depth), "]".repeat(depth));
        let mut t = Transcoder::new();
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || t.transcode(json.as_bytes(), "").map(<[u8]>::len))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
    }
}
