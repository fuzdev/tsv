//! The sink a wire writer writes into: the JSON wire's [`JsonWriter`] or, behind `_tape`, the
//! tape's [`crate::tape::TapeWriter`] — one walk, two products.
//!
//! A language crate's writer body is mounted once per sink (`tsv_css`'s `ast/convert/`: the
//! same file compiled as a module per sink, its `Sink` alias naming the sink), so each copy is
//! concrete and the JSON copy is the code it would be with no tape at all. What the two
//! sinks share is this contract:
//!
//! - **Data methods** mirror the [`JsonWriter`] methods the writers call, one to one. The JSON
//!   impl forwards to exactly that method; the tape writes the words the call's *data* carries
//!   (two offset words for a `start`/`end` pair, a number's `f64`, a string's ref) and nothing
//!   for its JSON-only bytes (literals, separators, quotes). `span_words` answers `None` on the
//!   tape, so a writer's shared-words arms are the JSON copy's alone.
//! - **Tape-structure methods** (`t_*`, behind `_tape`) say what a site's literal means on the
//!   tape: a record opening or closing, a field's mask bit, a list's brackets, a `null` record
//!   or ref, a keyword, a tag. The JSON impl's are empty. A mounted copy's sites reach them
//!   through its `tape!` macro, which the JSON mount expands to nothing, so the JSON copy holds
//!   no call to one; they are on the trait for code shared by both mounts and generic over the
//!   sink.
//!
//! [`crate::tape::TapeWriter`] carries each method inherently too, so a tape copy calls them with
//! no trait in scope, as the JSON copy calls [`JsonWriter`]'s.

use crate::{JsonWriter, SpanWords, WirePositions};

/// What a wire writer writes into. Module doc.
pub trait WireSink {
    /// [`JsonWriter::raw`]: JSON-only bytes.
    fn raw(&mut self, s: &str);
    /// [`JsonWriter::raw_fixed`]: JSON-only bytes.
    fn raw_fixed<const N: usize>(&mut self, s: &[u8; N]);
    /// [`JsonWriter::token`]: JSON-only bytes (the site states the token's tape meaning).
    fn token(&mut self, s: &str);
    /// [`JsonWriter::null`]: JSON-only bytes (the site states which `null` it is).
    fn null(&mut self);
    /// [`JsonWriter::string`]: the tape writes a ref.
    fn string(&mut self, s: &str);
    /// [`JsonWriter::u32`]: a `number` field; the tape writes its `f64`.
    fn u32(&mut self, n: u32);
    /// [`JsonWriter::start_end_tail`]: the tape writes the two (emitted) offsets.
    fn start_end_tail<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    );
    /// [`JsonWriter::start_end_head`]: the tape writes the two (emitted) offsets.
    fn start_end_head<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    );
    /// [`JsonWriter::span_words`]; the tape makes none (module doc).
    fn span_words(start: u32, end: u32) -> Option<SpanWords>;
    /// [`JsonWriter::start_end_head_words`]; unreached on the tape.
    fn start_end_head_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    );
    /// [`JsonWriter::start_end_tail_words`]; unreached on the tape.
    fn start_end_tail_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    );
    /// [`JsonWriter::span_start_end`]: the tape writes the two offsets.
    fn span_start_end(&mut self, positions: WirePositions<'_>, start: u32, end: u32);
    /// [`JsonWriter::span_start_end_field`]: the tape writes the two offsets.
    fn span_start_end_field(&mut self, positions: WirePositions<'_>, start: u32, end: u32);

    /// A typed record of entry `entry` opens (`tape::entry_id`).
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_open(&mut self, entry: u16) {
        let _ = entry;
    }
    /// A typed record opens whose entry is known only at run time, by its name.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_open_named(&mut self, name: &str) {
        let _ = name;
    }
    /// A `type`-less record of entry `entry` opens.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_open_object(&mut self, entry: u16) {
        let _ = entry;
    }
    /// The open record closes: its header is written.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_close(&mut self) {}
    /// A field with presence-mask bit `bit` is present (`tape::NO_BIT`: a required field).
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_bit(&mut self, bit: u8) {
        let _ = bit;
    }
    /// The open record's field `key` is present, its bit looked up at run time.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_bit_key(&mut self, key: &str) {
        let _ = key;
    }
    /// A list's `[`.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_list_open(&mut self) {}
    /// A list's `]`.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_list_close(&mut self) {}
    /// A `null` in a record position.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_null_record(&mut self) {}
    /// An empty self-describing record.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_empty_object(&mut self) {}
    /// A `null` in a nullable-ref position.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_null_ref(&mut self) {}
    /// A boolean field's value.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_bool(&mut self, b: bool) {
        let _ = b;
    }
    /// A number's two words.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_number(&mut self, n: f64) {
        let _ = n;
    }
    /// A tagged value's tag word (`tape::TAG_*`), its payload written after it.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_tag(&mut self, tag: u32) {
        let _ = tag;
    }
    /// A keyword field's value `word`, out of vocabulary `vocab` (`tape::field_aux`).
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_keyword(&mut self, vocab: u16, word: &str) {
        let _ = (vocab, word);
    }
    /// The open record's keyword field `key` has value `word`, its vocabulary looked up at run
    /// time.
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_keyword_key(&mut self, key: &str, word: &str) {
        let _ = (key, word);
    }
    /// A string value: its text (a source slice when borrowed from there, else pooled).
    #[cfg(feature = "_tape")]
    #[inline]
    fn t_ref(&mut self, s: &[u8]) {
        let _ = s;
    }
}

impl WireSink for JsonWriter {
    #[inline]
    fn raw(&mut self, s: &str) {
        JsonWriter::raw(self, s);
    }
    #[inline]
    fn raw_fixed<const N: usize>(&mut self, s: &[u8; N]) {
        JsonWriter::raw_fixed(self, s);
    }
    #[inline]
    fn token(&mut self, s: &str) {
        JsonWriter::token(self, s);
    }
    #[inline]
    fn null(&mut self) {
        JsonWriter::null(self);
    }
    #[inline]
    fn string(&mut self, s: &str) {
        JsonWriter::string(self, s);
    }
    #[inline]
    fn u32(&mut self, n: u32) {
        JsonWriter::u32(self, n);
    }
    #[inline]
    fn start_end_tail<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        JsonWriter::start_end_tail(self, lead, start, end, close);
    }
    #[inline]
    fn start_end_head<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        start: u32,
        end: u32,
        close: &[u8; C],
    ) {
        JsonWriter::start_end_head(self, lead, start, end, close);
    }
    #[inline]
    fn span_words(start: u32, end: u32) -> Option<SpanWords> {
        JsonWriter::span_words(start, end)
    }
    #[inline]
    fn start_end_head_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    ) {
        JsonWriter::start_end_head_words(self, lead, words, close);
    }
    #[inline]
    fn start_end_tail_words<const L: usize, const C: usize>(
        &mut self,
        lead: &[u8; L],
        words: SpanWords,
        close: &[u8; C],
    ) {
        JsonWriter::start_end_tail_words(self, lead, words, close);
    }
    #[inline]
    fn span_start_end(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        JsonWriter::span_start_end(self, positions, start, end);
    }
    #[inline]
    fn span_start_end_field(&mut self, positions: WirePositions<'_>, start: u32, end: u32) {
        JsonWriter::span_start_end_field(self, positions, start, end);
    }
}
