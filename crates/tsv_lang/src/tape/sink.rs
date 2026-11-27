//! The direct tape producer: what a language crate's wire writer writes the tape into, from
//! the same walk and the same sites that write the JSON wire (`docs/tape_format.md` §Producing a
//! tape).
//!
//! [`TapeSink`] owns the buffers, which keep their capacity from call to call; [`TapeWriter`] is
//! one document's view of it (the source, the emitted-offset map), the sink a writer's tape copy
//! is written against. A writer states what each of its JSON sites means on the tape — a record
//! opening, a field present, a list's brackets — through the `t_*` methods, and its data
//! through the [`WireSink`] methods it calls on the JSON wire, which here write the words the
//! data carries and nothing for the JSON-only bytes.
//!
//! Records are written pre-order, each header slot reserved when the record opens and filled
//! when it closes (§Records), and each list's count slot patched when it closes. Strings are
//! **true-offset** refs: a string the writer borrowed from the source is the slice it occupies,
//! any other is appended to the pool at once, in write order — the relaxed refs the format
//! accepts (§String refs). Such a tape is graded by printing it back to the JSON wire and by the
//! transcoder as a canonicalizer (`transcode(tape_to_json(direct)) == transcode(json)`), not
//! byte for byte against the transcoder's own.
//!
//! A writer bug the sink can see — a keyword field's value written as a word, a key of an entry
//! other than the open record's, a record, list or self-describing record left open or closed
//! twice (a self-describing record's key count and a list's item count are the frame's own
//! count of what was written, never a number the writer hands in) — is recorded as
//! the first [`SinkError`] and refuses
//! the tape at [`TapeSink::finish_words`] / [`TapeSink::finish_bytes`], in every build: a tape
//! that would decode to another tree is never handed out.

#[cfg(feature = "_tape_literals")]
use super::{Enc, Field, LitOp, TAG_FALSE, TAG_NULL, TAG_NUMBER, TAG_OBJECT, TAG_STRING, TAG_TRUE};
use super::{HEADER_WORDS, MAGIC, NO_BIT, NULL_REF, header_of, shapes};
use crate::{LeadingBom, SpanWords, WirePositions, WireSink, leading_bom_len};

/// Why a tape could not be finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkError {
    /// A source, tape or pool past the format's sizes.
    TooLarge,
    /// A keyword field's value written as a word rather than its index (a writer that states
    /// its keys writes every keyword by its build-time index), or an index of a vocabulary
    /// other than its field's.
    Keyword,
    /// A key written for an entry other than the open record's, or a field the open record's
    /// entry lacks.
    Name,
    /// A record, list or self-describing record closed with none open, left open at the
    /// finish, a field key or a mask bit with no record open on top, an object key with no
    /// self-describing record open, a `strings` field's ref outside its list or a record
    /// inside one, or a JSON-only path taken on the tape.
    Unbalanced,
}

impl std::fmt::Display for SinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "the source or the tape is too large for a tape",
            Self::Keyword => {
                "a writer bug: a keyword field written as its word, or a foreign index"
            }
            Self::Name => "a writer bug: a key or field the open record's entry does not hold",
            Self::Unbalanced => {
                "a writer bug: a record, list or object opened, closed or written out of order"
            }
        })
    }
}

impl std::error::Error for SinkError {}

/// [`Frame::entry`] of a list frame.
const LIST: u16 = u16::MAX;

/// [`Frame::entry`] of a self-describing record's frame (`TapeWriter::t_object_open`). Every
/// entry id is below it, so `entry < OBJECT` is "a typed record".
const OBJECT: u16 = u16::MAX - 1;
const _: () = assert!(shapes::ENTRIES.len() < OBJECT as usize);

/// [`Frame::mask`] of a list frame opened for a `strings` field, whose items are counted refs
/// ([`TapeWriter::t_list_ref`]) and never records.
#[cfg(feature = "_tape_literals")]
const STRINGS_LIST: u32 = 1;

/// An open record or list.
#[derive(Clone, Copy)]
struct Frame {
    /// The header slot (a record) or the count slot (a list).
    slot: u32,
    /// The record's entry, or [`LIST`], or [`OBJECT`].
    entry: u16,
    /// The record's presence mask so far (a list's: `STRINGS_LIST` for a `strings` field's).
    mask: u32,
    /// The list's items so far (a self-describing record's: its keys).
    count: u32,
}

/// The direct producer's reusable buffers. Module doc.
#[derive(Default)]
pub struct TapeSink {
    /// The header and the body, as words.
    tape: Vec<u32>,
    pool: Vec<u8>,
    /// The pool's length in UTF-16 code units.
    pool_units: u32,
    /// The tape text's length in UTF-16 code units (header word 7).
    units: u32,
    frames: Vec<Frame>,
    error: Option<SinkError>,
    /// The finished tape's byte length once `finish_words` has laid the words out in place
    /// (body stored little-endian, the pool packed after it) — so a later finish reads the
    /// laid-out words rather than laying them out again.
    finished: Option<usize>,
}

impl std::fmt::Debug for TapeSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapeSink").finish_non_exhaustive()
    }
}

/// UTF-16 units in UTF-8 `bytes`.
#[inline]
fn utf16_len(bytes: &[u8]) -> u32 {
    if bytes.is_ascii() {
        return bytes.len() as u32;
    }
    let mut n = 0u32;
    for &c in bytes {
        n += u32::from((c & 0xC0) != 0x80) + u32::from(c >= 0xF0);
    }
    n
}

impl TapeSink {
    /// A sink with empty buffers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a document. `source` is the text the writer reads (a leading byte-order mark
    /// included), `positions` its span-only wire's emitted-offset map, and `bom` how that wire
    /// counts a leading mark — the language's `WIRE_COORDINATES.bom`: an [`LeadingBom::Elided`]
    /// mark is not part of the tape's text, so no source ref reaches into it.
    pub fn writer<'a>(
        &'a mut self,
        source: &'a str,
        positions: WirePositions<'a>,
        bom: LeadingBom,
    ) -> TapeWriter<'a> {
        self.tape.clear();
        self.tape.resize(HEADER_WORDS, 0);
        self.pool.clear();
        self.pool_units = 0;
        self.frames.clear();
        self.error = None;
        self.finished = None;
        self.units = positions.pos(source.len() as u32);
        let skip = match bom {
            LeadingBom::Elided => leading_bom_len(source),
            LeadingBom::Counted => 0,
        };
        TapeWriter {
            sink: self,
            source,
            positions,
            skip,
            #[cfg(feature = "_tape_literals")]
            pending: None,
        }
    }

    /// The header of the finished tape, or why it cannot be finished.
    fn header(&self) -> Result<[u32; HEADER_WORDS], SinkError> {
        if let Some(e) = self.error {
            return Err(e);
        }
        if !self.frames.is_empty() {
            return Err(SinkError::Unbalanced);
        }
        // past `i32::MAX` UTF-16 units a pool ref's first word would read as a source slice
        if self.pool_units >= i32::MAX as u32 || self.units >= i32::MAX as u32 {
            return Err(SinkError::TooLarge);
        }
        let words =
            u32::try_from(self.tape.len() - HEADER_WORDS).map_err(|_| SinkError::TooLarge)?;
        let pool_bytes = u32::try_from(self.pool.len()).map_err(|_| SinkError::TooLarge)?;
        Ok([
            MAGIC,
            shapes::TAPE_FORMAT_VERSION,
            shapes::SHAPES_HASH,
            words,
            pool_bytes,
            0,
            self.pool_units,
            self.units,
        ])
    }

    /// The finished tape as words, the form a binding hands its host (`tsv_arena`'s
    /// `parse_tape!`): the header and body stored little-endian, the pool packed after them,
    /// the last word zero-padded. Returns the words and the tape's length in bytes; the sink
    /// keeps the words until its next document, and asking again — either finish — reads them
    /// as laid out.
    ///
    /// # Errors
    ///
    /// The first [`SinkError`] the document recorded, [`SinkError::Unbalanced`] for a record or
    /// list left open, [`SinkError::TooLarge`] past the format's sizes.
    pub fn finish_words(&mut self) -> Result<(&[u32], usize), SinkError> {
        if let Some(len) = self.finished {
            return Ok((&self.tape, len));
        }
        let header = self.header()?;
        let len = self.tape.len() * 4 + self.pool.len();
        self.tape[..HEADER_WORDS].copy_from_slice(&header);
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
        self.finished = Some(len);
        Ok((&self.tape, len))
    }

    /// The finished tape as bytes, the form `tsv_debug`'s transcoder returns.
    ///
    /// # Errors
    ///
    /// As [`TapeSink::finish_words`].
    pub fn finish_bytes(&self) -> Result<Vec<u8>, SinkError> {
        if let Some(len) = self.finished {
            // already laid out: each word's bytes in memory are the tape's bytes in order
            let mut out: Vec<u8> = self.tape.iter().flat_map(|w| w.to_ne_bytes()).collect();
            out.truncate(len);
            return Ok(out);
        }
        let header = self.header()?;
        let mut out = Vec::with_capacity(self.tape.len() * 4 + self.pool.len());
        for word in header.iter().chain(&self.tape[HEADER_WORDS..]) {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&self.pool);
        Ok(out)
    }
}

/// One document being written into a [`TapeSink`]. Module doc.
pub struct TapeWriter<'a> {
    sink: &'a mut TapeSink,
    source: &'a str,
    positions: WirePositions<'a>,
    /// The leading bytes no source ref may start in: an elided byte-order mark's.
    skip: usize,
    /// The field a key declared whose value is not written yet — its encoding and `aux` — so
    /// the value's data call writes it as that field lays it down (§Keyed values below).
    /// `None` between fields, and always for a writer that declares no key (`tsv_css`'s,
    /// which states each value's tape meaning at its site). Behind `_tape_literals`, with every
    /// keyed form: in a world without it the sink is the plain one, compiled as before.
    #[cfg(feature = "_tape_literals")]
    pending: Option<(Enc, u16)>,
}

impl TapeWriter<'_> {
    /// Records `e` unless an earlier error stands.
    #[cold]
    #[inline(never)]
    fn fail(&mut self, e: SinkError) {
        self.sink.error.get_or_insert(e);
    }

    /// The byte offset of `s` in the source, when the writer borrowed it from there (past an
    /// elided byte-order mark).
    #[inline]
    fn offset_of(&self, s: &[u8]) -> Option<usize> {
        let base = self.source.as_ptr() as usize;
        let p = s.as_ptr() as usize;
        (p >= base + self.skip && p - base + s.len() <= self.source.len()).then(|| p - base)
    }

    #[inline]
    fn push2(&mut self, a: u32, b: u32) {
        self.sink.tape.extend_from_slice(&[a, b]);
    }

    /// Counts an item opening under a list frame (a `strings` field's list holds refs alone,
    /// so a record there refuses the tape).
    #[inline]
    fn count_item(&mut self) {
        if let Some(f) = self.sink.frames.last_mut()
            && f.entry == LIST
        {
            f.count += 1;
            #[cfg(feature = "_tape_literals")]
            if f.mask == STRINGS_LIST {
                self.record_in_strings();
            }
        }
    }

    /// A record opened inside a `strings` field's list.
    #[cfg(feature = "_tape_literals")]
    #[cold]
    #[inline(never)]
    fn record_in_strings(&mut self) {
        self.fail(SinkError::Unbalanced);
    }

    #[inline]
    fn push_frame(&mut self, entry: u16) {
        let frame = Frame {
            slot: self.sink.tape.len() as u32,
            entry,
            mask: 0,
            count: 0,
        };
        self.sink.frames.push(frame);
        self.sink.tape.push(0);
    }

    /// The open record's entry's field `key` (`None` with no record open, or a key it lacks) —
    /// for a writer's debug assertion about a field it writes without declaring the key. Debug
    /// builds alone: it asks the generated lookup by name, which no release build carries.
    #[cfg(all(feature = "_tape_literals", debug_assertions))]
    #[must_use]
    pub fn t_top_field(&self, key: &str) -> Option<Field> {
        match self.sink.frames.last() {
            Some(f) if f.entry < OBJECT => shapes::field_named(f.entry, key.as_bytes())
                .map(|i| shapes::ENTRIES[usize::from(f.entry)].fields[usize::from(i)]),
            _ => None,
        }
    }

    /// The open record's entry, or `None` with no record open on top.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    fn top_entry(&self) -> Option<u16> {
        match self.sink.frames.last() {
            Some(f) if f.entry < OBJECT => Some(f.entry),
            _ => None,
        }
    }

    // ── tape structure: what a site's JSON literal means on the tape ──────────────────────
    //
    // A record or a `null` record, opening under a list, counts as one of its items.

    /// A typed record of entry `entry` opens.
    #[inline]
    pub fn t_open(&mut self, entry: u16) {
        self.enter_record();
        self.count_item();
        self.push_frame(entry);
    }

    /// A `type`-less record of entry `entry` opens.
    #[inline]
    pub fn t_open_object(&mut self, entry: u16) {
        self.t_open(entry);
    }

    /// The open record closes: its header is written.
    #[inline]
    pub fn t_close(&mut self) {
        #[cfg(feature = "_tape_literals")]
        if self.pending.is_some() {
            self.fail(SinkError::Unbalanced);
        }
        match self.sink.frames.pop() {
            Some(f) if f.entry < OBJECT => {
                self.sink.tape[f.slot as usize] = header_of(f.entry, f.mask);
            }
            _ => self.fail(SinkError::Unbalanced),
        }
    }

    /// A field with presence-mask bit `bit` is present ([`NO_BIT`]: a required field, nothing
    /// to record). A bit with no record open on top refuses the tape.
    #[inline]
    pub fn t_bit(&mut self, bit: u8) {
        if bit == NO_BIT {
            return;
        }
        if let Some(f) = self.sink.frames.last_mut()
            && f.entry < OBJECT
        {
            f.mask |= 1 << bit;
            return;
        }
        self.fail(SinkError::Unbalanced);
    }

    /// A list's `[`: its count slot.
    #[inline]
    pub fn t_list_open(&mut self) {
        #[cfg(feature = "_tape_literals")]
        let strings = match self.pending.take() {
            None | Some((Enc::List, _)) => false,
            Some((Enc::Strings, _)) => true,
            Some(_) => {
                self.fail(SinkError::Unbalanced);
                false
            }
        };
        self.push_frame(LIST);
        #[cfg(feature = "_tape_literals")]
        if strings && let Some(f) = self.sink.frames.last_mut() {
            f.mask = STRINGS_LIST;
        }
    }

    /// A list's `]`: its count.
    #[inline]
    pub fn t_list_close(&mut self) {
        match self.sink.frames.pop() {
            Some(f) if f.entry == LIST => self.sink.tape[f.slot as usize] = f.count,
            _ => self.fail(SinkError::Unbalanced),
        }
    }

    /// A `null` in a record position.
    #[inline]
    pub fn t_null_record(&mut self) {
        self.enter_record();
        self.count_item();
        self.sink.tape.push(0);
    }

    /// An empty self-describing record (`{}` where no entry is named).
    #[inline]
    pub fn t_empty_object(&mut self) {
        self.enter_record();
        self.count_item();
        self.push2(super::HEADER_ESCAPE, 0);
    }

    /// A `null` in a nullable-ref position.
    #[inline]
    pub fn t_null_ref(&mut self) {
        #[cfg(feature = "_tape_literals")]
        if let Some((enc, _)) = self.pending.take()
            && enc != Enc::RefNullable
        {
            self.fail(SinkError::Unbalanced);
        }
        self.push2(NULL_REF, 0);
    }

    /// A boolean field's word.
    #[inline]
    pub fn t_bool(&mut self, b: bool) {
        self.sink.tape.push(u32::from(b));
    }

    /// A number's two words.
    #[inline]
    pub fn t_number(&mut self, n: f64) {
        let bits = n.to_bits();
        self.push2(bits as u32, (bits >> 32) as u32);
    }

    /// A tagged value's tag word (`TAG_*`), its payload written after it.
    #[inline]
    pub fn t_tag(&mut self, tag: u32) {
        self.sink.tape.push(tag);
    }

    /// A keyword field's value: `index`, the word's index in vocabulary `vocab`, which the
    /// site resolved at build time (`tape::keyword_id` in a `const`). Under a declared key, the
    /// key's field must be a keyword field of that vocabulary, or the tape is refused.
    #[inline]
    pub fn t_keyword_id(&mut self, vocab: u16, index: u32) {
        #[cfg(feature = "_tape_literals")]
        if self.pending.is_some() {
            self.keyword_keyed(vocab, index);
            return;
        }
        let _ = vocab;
        self.sink.tape.push(index);
    }

    /// A string value: the source slice it occupies when the writer borrowed it from there,
    /// else a pool entry.
    #[inline]
    pub fn t_ref(&mut self, s: &[u8]) {
        if let Some(p) = self.offset_of(s) {
            let (a, b) = (
                self.positions.pos(p as u32),
                self.positions.pos((p + s.len()) as u32),
            );
            self.push2(a, b);
        } else {
            let sink = &mut *self.sink;
            sink.tape
                .extend_from_slice(&[(1 + sink.pool_units).wrapping_neg(), 0]);
            sink.pool.extend_from_slice(s);
            sink.pool_units += utf16_len(s);
            let end = sink.tape.len() - 1;
            sink.tape[end] = sink.pool_units;
        }
    }

    // ── data: the `JsonWriter` calls a writer makes, as the tape reads them ───────────────
    //
    // Inherent, so the tape copy of a writer calls them as the JSON copy calls `JsonWriter`'s,
    // with no trait in scope; the `WireSink` impl below forwards to them.

    // ── keyed values: a value written as the field its key declared lays it down ──────────
    //
    // A writer that states its keys through `t_key` / `t_comments_key` / `t_lit` (`tsv_ts`'s)
    // leaves each value's encoding to the field: its data call writes a ref, a keyword index,
    // nothing (an `own` string, a `null` field) or a tagged value as the field says. With no
    // key declared (`tsv_css`'s writer, which states each value's tape meaning at its site), a
    // data call keeps its plain meaning: a string a ref, a `u32` a number, `null` and `token`
    // nothing. A key declared while another's value is still unwritten, or a value its field
    // cannot hold, refuses the tape.

    //
    // The keyed bodies are out of line: a tape copy's sites are many and its recursion deep, and
    // inlined, each site carried the whole field-encoding match — the tape copy's frames and code
    // grew several-fold over its JSON copy's. Each keyed site is a short run of calls instead;
    // the plain meaning (no key pending) stays inline, as `tsv_css`'s copy relies on.

    /// A key: field `field` of the open record is present, its value next.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn t_field(&mut self, field: Field) {
        if self.pending.is_some() {
            self.fail(SinkError::Unbalanced);
        }
        self.t_bit(field.bit);
        self.pending = Some((field.enc, field.aux));
    }

    /// A key of entry `entry` — field `field`, resolved at build time — its value next. The
    /// open record must be `entry`'s, or the tape is refused ([`SinkError::Name`]): the check a
    /// site's naming of its entry leaves to run time.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn t_key(&mut self, entry: u16, field: Field) {
        if self.top_entry() != Some(entry) {
            self.fail(SinkError::Name);
        }
        self.t_field(field);
    }

    /// A site whose record is one of a few entries chosen at run time found the open record is
    /// none of them: the tape is refused ([`SinkError::Name`]).
    #[cfg(feature = "_tape_literals")]
    #[cold]
    #[inline(never)]
    pub fn t_wrong_entry(&mut self) {
        self.fail(SinkError::Name);
    }

    /// The open record's `leadingComments` (`trailing`: `trailingComments`) key, its value next
    /// — the comment attach's, which closes every kind of node: the field's presence bit is
    /// the open record's entry's, read from a table built at build time (the module's
    /// `COMMENT_BITS`). An entry with no such field refuses the tape.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn t_comments_key(&mut self, trailing: bool) {
        let bit = self
            .top_entry()
            .and_then(|entry| super::COMMENT_BITS.get(usize::from(entry)))
            .map_or(super::NO_COMMENTS, |bits| bits[usize::from(trailing)]);
        if bit == super::NO_COMMENTS {
            self.fail(SinkError::Name);
            return;
        }
        self.t_field(Field {
            key: "",
            enc: Enc::List,
            bit,
            aux: super::NO_AUX,
        });
    }

    /// [`Self::t_keyword_id`] under a pending field.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    fn keyword_keyed(&mut self, vocab: u16, index: u32) {
        match self.pending.take() {
            Some((Enc::Keyword, v)) if v == vocab => self.sink.tape.push(index),
            _ => self.fail(SinkError::Keyword),
        }
    }

    /// An array in a tagged-value field (a Svelte attribute's text-and-tag sequence): the
    /// declared field must be a tagged value, whose array tag and count slot open here. Each
    /// item is a tagged value the writer leads with its own tag word ([`Self::t_tag`]); a
    /// record or `null` record item is counted as it opens, and [`Self::t_list_close`] writes
    /// the count.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn t_value_list_open(&mut self) {
        match self.pending.take() {
            Some((Enc::Value, _)) => {
                self.t_tag(super::TAG_ARRAY);
                self.push_frame(LIST);
            }
            _ => self.fail(SinkError::Unbalanced),
        }
    }

    /// One string of a `strings` field (a Svelte directive's modifiers), inside the list
    /// [`Self::t_list_open`] opened for that field: counted, then its ref. Anywhere else —
    /// another list, a record — it refuses the tape.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn t_list_ref(&mut self, s: &[u8]) {
        if let Some(f) = self.sink.frames.last_mut()
            && f.entry == LIST
            && f.mask == STRINGS_LIST
        {
            f.count += 1;
        } else {
            self.fail(SinkError::Unbalanced);
        }
        self.t_ref(s);
    }

    /// A self-describing record opens (`docs/tape_format.md` §Records — an object no entry
    /// holds, `<svelte:options>`'s `customElement` data): its header and its key-count slot,
    /// under a frame of its own. The writer then writes each key through
    /// [`Self::t_object_key`] and its tagged value after it, and closes it with
    /// [`Self::t_object_close`], which writes the count — so a count the writes do not make is
    /// unrepresentable, and an object left open refuses the tape. Under a declared
    /// tagged-value field its object tag comes first.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn t_object_open(&mut self) {
        self.enter_record();
        self.count_item();
        self.sink.tape.push(super::HEADER_ESCAPE);
        self.push_frame(OBJECT);
    }

    /// One key of the open self-describing record: counted, then its text's ref. The value
    /// the writer writes next is a tagged value. With no such record on top it refuses the
    /// tape.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn t_object_key(&mut self, key: &[u8]) {
        if let Some(f) = self.sink.frames.last_mut()
            && f.entry == OBJECT
        {
            f.count += 1;
        } else {
            self.fail(SinkError::Unbalanced);
        }
        self.t_ref(key);
    }

    /// The open self-describing record closes: its key count is written.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn t_object_close(&mut self) {
        if self.pending.is_some() {
            self.fail(SinkError::Unbalanced);
        }
        match self.sink.frames.pop() {
            Some(f) if f.entry == OBJECT => self.sink.tape[f.slot as usize] = f.count,
            _ => self.fail(SinkError::Unbalanced),
        }
    }

    /// A literal's ops (`tape::literal_ops`), in order. One shared interpreter, out of line: a
    /// site hands it its constant ops, so a tape copy's site is one call (inlined per site, the
    /// loop and its match were not folded away and every site carried them).
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn t_lit(&mut self, ops: &[LitOp]) {
        for &op in ops {
            match op {
                LitOp::Open(entry) | LitOp::OpenObject(entry) => self.t_open(entry),
                LitOp::Close => self.t_close(),
                LitOp::Key { entry, field } => self.t_key(entry, field),
                LitOp::Keyword { vocab, index } => self.t_keyword_id(vocab, index),
                LitOp::True => self.bool(true),
                LitOp::False => self.bool(false),
                LitOp::Null => self.null(),
                LitOp::EmptyObject => self.t_empty_object(),
                LitOp::Str(text) => self.string(text),
                LitOp::Int(n) => self.u32(n),
                LitOp::ListOpen => self.t_list_open(),
                LitOp::ListClose => self.t_list_close(),
            }
        }
    }

    /// A record opens: under a tagged-value field, its tag first.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    fn enter_record(&mut self) {
        if self.pending.is_some() {
            self.enter_record_keyed();
        }
    }

    /// [`Self::enter_record`] under a pending field.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    fn enter_record_keyed(&mut self) {
        match self.pending.take() {
            None | Some((Enc::Record | Enc::List, _)) => {}
            Some((Enc::Value, _)) => self.sink.tape.push(TAG_OBJECT),
            Some(_) => self.fail(SinkError::Unbalanced),
        }
    }

    /// A boolean value.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn bool(&mut self, b: bool) {
        match self.pending.take() {
            Some((Enc::Boolean, _)) => self.t_bool(b),
            Some((Enc::Value, _)) => self.t_tag(if b { TAG_TRUE } else { TAG_FALSE }),
            _ => self.fail(SinkError::Unbalanced),
        }
    }

    /// A `null`: nothing for a writer that declared no key (the site states which `null`).
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn null(&mut self) {
        if self.pending.is_some() {
            self.null_keyed();
        }
    }

    /// [`Self::null`] under a pending field.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    fn null_keyed(&mut self) {
        match self.pending {
            None | Some((Enc::Null, _)) => self.pending = None,
            Some((Enc::Record, _)) => self.t_null_record(),
            Some((Enc::RefNullable, _)) => self.t_null_ref(),
            Some((Enc::Value, _)) => {
                self.pending = None;
                self.t_tag(TAG_NULL);
            }
            Some(_) => self.fail(SinkError::Unbalanced),
        }
    }

    /// A string value: a ref, or as its field lays it down.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn string(&mut self, s: &str) {
        if self.pending.is_some() {
            self.string_keyed(s);
        } else {
            self.t_ref(s.as_bytes());
        }
    }

    /// [`Self::string`] under a pending field.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    fn string_keyed(&mut self, s: &str) {
        match self.pending.take() {
            None | Some((Enc::Ref | Enc::RefNullable, _)) => self.t_ref(s.as_bytes()),
            Some((Enc::Own, _)) => {}
            // a keyword's word: the site writes its build-time index instead (`t_keyword_id`)
            Some((Enc::Keyword, _)) => self.fail(SinkError::Keyword),
            Some((Enc::Value, _)) => {
                self.t_tag(TAG_STRING);
                self.t_ref(s.as_bytes());
            }
            Some(_) => self.fail(SinkError::Unbalanced),
        }
    }

    /// An escape-free token: nothing for a writer that declared no key (the site states its
    /// meaning), else the string its field holds.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn token(&mut self, s: &str) {
        if self.pending.is_some() {
            self.string_keyed(s);
        }
    }

    /// An unsigned integer: a number for a writer that declared no key, else as its field lays
    /// it down.
    #[cfg(feature = "_tape_literals")]
    #[inline]
    pub fn u32(&mut self, n: u32) {
        if self.pending.is_some() {
            self.u32_keyed(n);
        } else {
            self.t_number(f64::from(n));
        }
    }

    /// [`Self::u32`] under a pending field.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    fn u32_keyed(&mut self, n: u32) {
        if let Some((Enc::Offset, _)) = self.pending {
            self.pending = None;
            self.sink.tape.push(n);
        } else {
            self.number(f64::from(n));
        }
    }

    /// A number value.
    #[cfg(feature = "_tape_literals")]
    #[inline(never)]
    pub fn number(&mut self, n: f64) {
        match self.pending.take() {
            None | Some((Enc::Number, _)) => self.t_number(n),
            Some((Enc::Value, _)) => {
                self.t_tag(TAG_NUMBER);
                self.t_number(n);
            }
            Some(_) => self.fail(SinkError::Unbalanced),
        }
    }

    /// A record opens (no field can be pending in a world without the keyed forms).
    #[cfg(not(feature = "_tape_literals"))]
    #[inline]
    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "the keyed world's twin writes through it"
    )]
    fn enter_record(&mut self) {}

    /// A `null`: nothing (the site states which `null`).
    #[cfg(not(feature = "_tape_literals"))]
    #[inline]
    pub fn null(&mut self) {}

    /// A string value: a ref.
    #[cfg(not(feature = "_tape_literals"))]
    #[inline]
    pub fn string(&mut self, s: &str) {
        self.t_ref(s.as_bytes());
    }

    /// An escape-free token: nothing (the site states its meaning).
    #[cfg(not(feature = "_tape_literals"))]
    #[inline]
    pub fn token(&mut self, _: &str) {}

    /// A `number` field.
    #[cfg(not(feature = "_tape_literals"))]
    #[inline]
    pub fn u32(&mut self, n: u32) {
        self.t_number(f64::from(n));
    }

    /// A node's closing burst: its two emitted offsets.
    #[inline]
    pub fn start_end_tail<const L: usize, const C: usize>(
        &mut self,
        _: &[u8; L],
        start: u32,
        end: u32,
        _: &[u8; C],
    ) {
        self.push2(start, end);
    }

    /// A node's opening burst: its two emitted offsets.
    #[inline]
    pub fn start_end_head<const L: usize, const C: usize>(
        &mut self,
        _: &[u8; L],
        start: u32,
        end: u32,
        _: &[u8; C],
    ) {
        self.push2(start, end);
    }

    /// The tape makes no span words, so a writer's shared-words arms are the JSON copy's
    /// alone and its explicit-offset arms the tape's path.
    #[inline]
    pub fn span_words(_: u32, _: u32) -> Option<SpanWords> {
        None
    }

    /// Unreached: the tape makes no span words.
    #[inline]
    pub fn start_end_head_words<const L: usize, const C: usize>(
        &mut self,
        _: &[u8; L],
        _: SpanWords,
        _: &[u8; C],
    ) {
        self.fail(SinkError::Unbalanced);
    }

    /// Unreached: the tape makes no span words.
    #[inline]
    pub fn start_end_tail_words<const L: usize, const C: usize>(
        &mut self,
        _: &[u8; L],
        _: SpanWords,
        _: &[u8; C],
    ) {
        self.fail(SinkError::Unbalanced);
    }

    /// A `start`/`end` pair from byte offsets: its two emitted offsets.
    #[inline]
    pub fn span_start_end(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        self.push2(positions.pos(start), positions.pos(end));
    }

    /// A `start`/`end` pair from byte offsets, behind its key: its two emitted offsets.
    #[inline]
    pub fn span_start_end_field(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        self.push2(positions.pos(start), positions.pos(end));
    }
}

impl WireSink for TapeWriter<'_> {
    #[inline]
    fn raw(&mut self, _: &str) {}
    #[inline]
    fn raw_fixed<const N: usize>(&mut self, _: &[u8; N]) {}
    #[inline]
    fn token(&mut self, s: &str) {
        TapeWriter::token(self, s);
    }
    #[inline]
    fn null(&mut self) {
        TapeWriter::null(self);
    }
    #[inline]
    fn string(&mut self, s: &str) {
        TapeWriter::string(self, s);
    }
    #[inline]
    fn u32(&mut self, n: u32) {
        TapeWriter::u32(self, n);
    }
    #[inline]
    fn start_end_tail<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        TapeWriter::start_end_tail(self, lead, start, end, close);
    }
    #[inline]
    fn start_end_head<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        TapeWriter::start_end_head(self, lead, start, end, close);
    }
    #[inline]
    fn span_words(start: u32, end: u32) -> Option<SpanWords> {
        TapeWriter::span_words(start, end)
    }
    #[inline]
    fn start_end_head_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    ) {
        TapeWriter::start_end_head_words(self, lead, words, close);
    }
    #[inline]
    fn start_end_tail_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    ) {
        TapeWriter::start_end_tail_words(self, lead, words, close);
    }
    #[inline]
    fn span_start_end(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        TapeWriter::span_start_end(self, positions, start, end);
    }
    #[inline]
    fn span_start_end_field(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        TapeWriter::span_start_end_field(self, positions, start, end);
    }

    #[inline]
    fn t_open(&mut self, entry: u16) {
        TapeWriter::t_open(self, entry);
    }
    #[inline]
    fn t_open_object(&mut self, entry: u16) {
        TapeWriter::t_open_object(self, entry);
    }
    #[inline]
    fn t_close(&mut self) {
        TapeWriter::t_close(self);
    }
    #[inline]
    fn t_bit(&mut self, bit: u8) {
        TapeWriter::t_bit(self, bit);
    }
    #[cfg(feature = "_tape_literals")]
    #[inline]
    fn t_comments_key(&mut self, trailing: bool) {
        TapeWriter::t_comments_key(self, trailing);
    }
    #[inline]
    fn t_list_open(&mut self) {
        TapeWriter::t_list_open(self);
    }
    #[inline]
    fn t_list_close(&mut self) {
        TapeWriter::t_list_close(self);
    }
    #[inline]
    fn t_null_record(&mut self) {
        TapeWriter::t_null_record(self);
    }
    #[inline]
    fn t_empty_object(&mut self) {
        TapeWriter::t_empty_object(self);
    }
    #[inline]
    fn t_null_ref(&mut self) {
        TapeWriter::t_null_ref(self);
    }
    #[inline]
    fn t_bool(&mut self, b: bool) {
        TapeWriter::t_bool(self, b);
    }
    #[inline]
    fn t_number(&mut self, n: f64) {
        TapeWriter::t_number(self, n);
    }
    #[inline]
    fn t_tag(&mut self, tag: u32) {
        TapeWriter::t_tag(self, tag);
    }
    #[inline]
    fn t_keyword_id(&mut self, vocab: u16, index: u32) {
        TapeWriter::t_keyword_id(self, vocab, index);
    }
    #[inline]
    fn t_ref(&mut self, s: &[u8]) {
        TapeWriter::t_ref(self, s);
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "_tape_literals")]
    use super::super::field_of;
    use super::super::{
        Header, TAG_NUMBER, entry_id, field_aux, field_bit, keyword_id, read_header,
    };
    use super::*;
    use crate::{LineRule, Wire, WireCoordinates, WireTables};

    const COORDS: WireCoordinates = WireCoordinates::new(LineRule::Lf, LeadingBom::Elided);

    fn words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// One `css:Combinator` record in a one-item list, its name a keyword, under a header the
    /// tables describe; a borrowed string is a source slice in UTF-16 units past an elided BOM,
    /// an owned one a pool entry.
    #[test]
    fn writes_records_lists_refs_and_keywords() {
        let source = "\u{FEFF}é > b";
        let tables = WireTables::new(source, COORDS, Wire::Span);
        let mut sink = TapeSink::new();
        let combinator = entry_id("css:Combinator");
        let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_list_open();
        w.t_open(combinator);
        let vocab = field_aux(combinator, "name");
        w.t_keyword_id(vocab, keyword_id(vocab, ">"));
        w.span_start_end_field(tables.positions(), 6, 7);
        w.t_close();
        w.t_list_close();
        w.t_ref(&source.as_bytes()[3..5]); // `é`, borrowed
        w.t_ref(b"pooled");
        w.t_ref(&source.as_bytes()[..3]); // the elided BOM itself: never a source slice
        w.t_tag(TAG_NUMBER);
        w.u32(7);
        let tape = sink.finish_bytes().expect("finishes");
        let w = words(&tape);
        assert_eq!(w[0], MAGIC);
        assert_eq!(w[3], 14, "body words");
        assert_eq!(w[4], 9, "pool bytes");
        assert_eq!(w[6], 7, "pool units");
        assert_eq!(w[7], 5, "tape text units: the BOM elided");
        let body = &w[HEADER_WORDS..];
        assert_eq!(body[0], 1, "one list item");
        assert_eq!(
            read_header(body[1]),
            Some(Header::Typed {
                entry: combinator,
                mask: 0
            })
        );
        assert_eq!(body[2], 1, "`>` is the vocabulary's second word");
        assert_eq!(&body[3..5], &[2, 3], "`>` is source units [2, 3)");
        assert_eq!(&body[5..7], &[0, 1], "`é` is source units [0, 1)");
        assert_eq!(&body[7..9], &[u32::MAX, 6], "the first pool entry");
        assert_eq!(&body[9..11], &[(1u32 + 6).wrapping_neg(), 7]);
        assert_eq!(body[11], TAG_NUMBER);
        assert_eq!(
            &body[12..14],
            &[0, 7.0f64.to_bits().wrapping_shr(32) as u32]
        );
    }

    /// `finish_words` is `finish_bytes` as words, and asking either again gives the same tape.
    #[test]
    fn finish_words_is_finish_bytes_and_repeatable() {
        let source = "a b";
        let tables = WireTables::new(source, COORDS, Wire::Span);
        let mut sink = TapeSink::new();
        let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_ref(&source.as_bytes()[..1]);
        w.t_ref(b"pooled text");
        let bytes = sink.finish_bytes().expect("finishes");
        let (words_1, len) = sink
            .finish_words()
            .map(|(w, l)| (w.to_vec(), l))
            .expect("finishes");
        assert_eq!(len, bytes.len());
        let as_bytes: Vec<u8> = words_1.iter().flat_map(|w| w.to_ne_bytes()).collect();
        assert_eq!(&as_bytes[..len], &bytes[..]);
        let again = sink.finish_bytes().expect("finishes again");
        assert_eq!(again, bytes, "finish_bytes after finish_words");
        let (words_2, len_2) = sink
            .finish_words()
            .map(|(w, l)| (w.to_vec(), l))
            .expect("finishes again");
        assert_eq!((words_2, len_2), (words_1, len), "finish_words twice");
        // a new document drops the laid-out words
        let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_ref(b"other text");
        let next = sink.finish_bytes().expect("finishes the next document");
        let mut fresh = TapeSink::new();
        let mut w = fresh.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_ref(b"other text");
        assert_eq!(
            next,
            fresh.finish_bytes().expect("finishes"),
            "a document after finish_words"
        );
    }

    /// A present optional field sets its bit in the header's mask.
    #[test]
    fn a_present_bit_moves_the_header() {
        let source = "a";
        let tables = WireTables::new(source, COORDS, Wire::Span);
        let comment = entry_id("css:CSSComment");
        let bit = field_bit(comment, "position");
        assert_ne!(bit, NO_BIT);
        let mut sink = TapeSink::new();
        let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_open(comment);
        w.t_bit(bit);
        w.t_close();
        let tape = sink.finish_bytes().expect("finishes");
        assert_eq!(
            read_header(words(&tape)[HEADER_WORDS]),
            Some(Header::Typed {
                entry: comment,
                mask: 1 << bit
            })
        );
    }

    /// A self-describing record's key count and a `strings` list's item count are the sink's own
    /// counts of what was written.
    #[cfg(feature = "_tape_literals")]
    #[test]
    fn object_keys_and_strings_are_counted() {
        let source = "a";
        let tables = WireTables::new(source, COORDS, Wire::Span);
        let mut sink = TapeSink::new();
        let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
        w.t_object_open();
        w.t_object_key(b"k");
        w.t_tag(TAG_TRUE);
        w.t_object_key(b"l");
        w.t_tag(TAG_OBJECT);
        w.t_object_open();
        w.t_object_close();
        w.t_object_close();
        let on = entry_id("OnDirective");
        w.t_open(on);
        w.t_key(on, field_of(on, "modifiers"));
        w.t_list_open();
        w.t_list_ref(b"m");
        w.t_list_ref(b"n");
        w.t_list_close();
        w.t_close();
        let tape = sink.finish_bytes().expect("finishes");
        let body = &words(&tape)[HEADER_WORDS..];
        // escape header, count, key `k` (two words), its tag, key `l`, its tag, nested record
        assert_eq!(body[0], super::super::HEADER_ESCAPE);
        assert_eq!(body[1], 2, "two keys written");
        assert_eq!(body[8], super::super::HEADER_ESCAPE, "the nested record");
        assert_eq!(body[9], 0, "no keys written");
        // the directive's header, then its modifiers' count
        assert_eq!(body[11], 2, "two modifiers written");
    }

    /// One writer-bug case: its name, the calls that make it, the error it must record.
    type Case = (&'static str, fn(&mut TapeWriter<'_>), SinkError);

    /// Every writer bug the sink sees refuses the tape, in every build.
    #[test]
    fn writer_bugs_refuse_the_tape() {
        let source = "a";
        let tables = WireTables::new(source, COORDS, Wire::Span);
        #[cfg_attr(
            not(feature = "_tape_literals"),
            expect(unused_mut, reason = "the keyed cases extend it")
        )]
        let mut cases: Vec<Case> = vec![
            (
                "a bit with none open",
                |w| w.t_bit(0),
                SinkError::Unbalanced,
            ),
            (
                "a bit on a list",
                |w| {
                    w.t_list_open();
                    w.t_bit(0);
                    w.t_list_close();
                },
                SinkError::Unbalanced,
            ),
            (
                "open at the finish",
                |w| w.t_list_open(),
                SinkError::Unbalanced,
            ),
            (
                "close with none open",
                |w| w.t_close(),
                SinkError::Unbalanced,
            ),
            (
                "a list closed as a record",
                |w| {
                    w.t_list_open();
                    w.t_close();
                },
                SinkError::Unbalanced,
            ),
        ];
        #[cfg(feature = "_tape_literals")]
        cases.extend::<[Case; 5]>([
            (
                "a key of another entry than the open record's",
                |w| {
                    w.t_open(entry_id("css:Combinator"));
                    w.t_key(
                        entry_id("css:Rule"),
                        field_of(entry_id("css:Rule"), "prelude"),
                    );
                    w.t_open(entry_id("css:SelectorList"));
                    w.t_close();
                    w.t_close();
                },
                SinkError::Name,
            ),
            (
                "a key asked of a list",
                |w| {
                    w.t_list_open();
                    w.t_comments_key(false);
                    w.t_list_close();
                },
                SinkError::Name,
            ),
            (
                "a keyword written as its word",
                |w| {
                    let combinator = entry_id("css:Combinator");
                    w.t_open(combinator);
                    w.t_key(combinator, field_of(combinator, "name"));
                    w.string(">");
                    w.t_close();
                },
                SinkError::Keyword,
            ),
            (
                "a keyword index of another vocabulary",
                |w| {
                    let combinator = entry_id("css:Combinator");
                    w.t_open(combinator);
                    w.t_key(combinator, field_of(combinator, "name"));
                    w.t_keyword_id(field_aux(combinator, "name") + 1, 0);
                    w.t_close();
                },
                SinkError::Keyword,
            ),
            (
                "comments on an entry without them",
                |w| {
                    w.t_open(entry_id("css:Combinator"));
                    w.t_comments_key(true);
                    w.t_close();
                },
                SinkError::Name,
            ),
        ]);
        #[cfg(feature = "_tape_literals")]
        cases.extend::<[Case; 6]>([
            (
                "a self-describing record left open",
                |w| w.t_object_open(),
                SinkError::Unbalanced,
            ),
            (
                "an object key with no self-describing record open",
                |w| {
                    w.t_list_open();
                    w.t_object_key(b"k");
                    w.t_list_close();
                },
                SinkError::Unbalanced,
            ),
            (
                "a self-describing record closed as a record",
                |w| {
                    w.t_object_open();
                    w.t_close();
                },
                SinkError::Unbalanced,
            ),
            (
                "a record closed as a self-describing one",
                |w| {
                    w.t_open(entry_id("css:Combinator"));
                    w.t_object_close();
                },
                SinkError::Unbalanced,
            ),
            (
                "a strings ref in a list of records",
                |w| {
                    w.t_list_open();
                    w.t_list_ref(b"m");
                    w.t_list_close();
                },
                SinkError::Unbalanced,
            ),
            (
                "a record in a strings list",
                |w| {
                    let on = entry_id("OnDirective");
                    w.t_open(on);
                    w.t_key(on, field_of(on, "modifiers"));
                    w.t_list_open();
                    w.t_null_record();
                    w.t_list_close();
                    w.t_close();
                },
                SinkError::Unbalanced,
            ),
        ]);
        let mut sink = TapeSink::new();
        for (name, case, expected) in cases {
            let mut w = sink.writer(source, tables.positions(), LeadingBom::Elided);
            case(&mut w);
            assert_eq!(sink.finish_bytes(), Err(expected), "{name}");
            assert_eq!(sink.finish_words().map(|_| ()), Err(expected), "{name}");
        }
        // a fresh document clears the error
        let _ = sink.writer(source, tables.positions(), LeadingBom::Elided);
        assert!(sink.finish_bytes().is_ok());
    }
}
