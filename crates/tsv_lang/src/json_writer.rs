//! Shared wire-JSON emission substrate.
//!
//! `JsonWriter` is the byte-buffer + scalar-emitter primitive the three
//! language crates' wire-JSON writers (`ast/convert/write/`) build on. It lives
//! here — not in any one language crate — so `tsv_svelte`'s writer can compose
//! `tsv_ts` (embedded `{expr}` / `<script>`) and `tsv_css` (embedded `<style>`)
//! emission into one shared buffer by passing `&mut JsonWriter` across crate
//! boundaries. Each language crate keeps its own node emitters (`node_header`,
//! field helpers, the per-language `Ctx`); only this JSON-scalar substrate is
//! shared.
//!
//! Behind the `json` feature (enabled transitively by each language crate's
//! `convert` feature) so the format-only `@fuzdev/tsv_format_wasm` build — which
//! turns `convert` off — never links `serde_json`.
//!
//! **Escape / format parity contract**: static structure and tokens are written
//! verbatim (debug-asserted escape-free); dynamic strings and non-integral `f64`
//! delegate to `serde_json::to_writer`, so escaping and ryu formatting are
//! exactly `serde_json`'s (the canonical parsers' `JSON.stringify` parity the
//! fixtures pin); integers have a unique decimal form and are hand-formatted
//! (two-digit-pair, the hot path emitting several ints per node).

use crate::swar::{lanes_less_than, splat, zero_lanes};

/// `00`,`01`,…,`99` — the two-digit-pair table behind the integer emitters
/// ([`JsonWriter::u32`] and its wide arm), halving their divisions.
const DEC_PAIRS: [u8; 200] = {
    let mut t = [0u8; 200];
    let mut i = 0;
    while i < 100 {
        t[i * 2] = b'0' + (i / 10) as u8;
        t[i * 2 + 1] = b'0' + (i % 10) as u8;
        i += 1;
    }
    t
};

/// Decimal digits in `u64::MAX` — the width of the wide arm's scratch, and the
/// constant length of the copy that appends it.
const MAX_U64_DIGITS: usize = 20;

/// Digits that fit one `u64` of packed ASCII — the width [`JsonWriter::u32`]
/// generates entirely in a register. Every integer the writers emit (offsets,
/// lines, columns) is far below `99_999_999`; wider values take the cold arm.
const WORD_DIGITS: usize = 8;

/// Decimal digits in `u32::MAX` — the ceiling of [`decimal_width_u32`].
const MAX_U32_DIGITS: usize = 10;

/// Decimal digit count of a `u32` (`0` is one digit) — the [`decimal_width`]
/// sibling for the hot path.
///
/// Same ascending-compare rationale, on the narrower type. The whole point of
/// the `u32` path is that every division in it is 32-bit: a `u64 / 100` lowers
/// to a full 64×64→128 multiply (`mul %rcx` + two shifts), a `u32 / 100` to a
/// single widening `imul`. Taking a `u64` here would sink the argument back
/// into 64-bit arithmetic and undo it.
#[inline]
const fn decimal_width_u32(n: u32) -> usize {
    if n < 10 {
        return 1;
    }
    if n < 100 {
        return 2;
    }
    if n < 1_000 {
        return 3;
    }
    if n < 10_000 {
        return 4;
    }
    if n < 100_000 {
        return 5;
    }
    if n < 1_000_000 {
        return 6;
    }
    /// `POW10[i] == 10^i`, up to the largest power of ten a `u32` holds.
    const POW10: [u32; MAX_U32_DIGITS] = {
        let mut t = [1u32; MAX_U32_DIGITS];
        let mut i = 1;
        while i < MAX_U32_DIGITS {
            t[i] = t[i - 1] * 10;
            i += 1;
        }
        t
    };
    let mut w = 7;
    while w < MAX_U32_DIGITS && n >= POW10[w] {
        w += 1;
    }
    w
}

/// Decimal digit count of `n` (`0` is one digit) — the exact width the wide arm
/// front-aligns its digits to.
///
/// Only the wide arm reaches this — [`JsonWriter::u32`] carries the hot path on
/// [`decimal_width_u32`] — so the ascending chain here is inherited shape rather
/// than a tuned one. It stays ascending for consistency with its sibling, whose
/// distribution (offsets, lines and columns) really is overwhelmingly small.
///
/// ⚠️ The tail is a **bounded loop**, not a call to `ilog10` — and that is a
/// codegen constraint, not a style choice. `ilog10` is out-of-line, so its
/// result is opaque to the caller: LLVM then cannot prove the returned width
/// fits the wide arm's scratch and re-inserts a `panic_bounds_check` on every
/// one of its four scratch writes. A `while w < MAX_U64_DIGITS` loop makes the
/// range provable, and the bounds checks disappear. Keep any future rewrite
/// provably in `1..=MAX_U64_DIGITS` **at the type/CFG level**.
#[inline]
const fn decimal_width(n: u64) -> usize {
    if n < 10 {
        return 1;
    }
    if n < 100 {
        return 2;
    }
    if n < 1_000 {
        return 3;
    }
    if n < 10_000 {
        return 4;
    }
    if n < 100_000 {
        return 5;
    }
    if n < 1_000_000 {
        return 6;
    }
    /// `POW10[i] == 10^i`, up to the largest power of ten a `u64` holds.
    const POW10: [u64; MAX_U64_DIGITS] = {
        let mut t = [1u64; MAX_U64_DIGITS];
        let mut i = 1;
        while i < MAX_U64_DIGITS {
            t[i] = t[i - 1] * 10;
            i += 1;
        }
        t
    };
    let mut w = 7;
    while w < MAX_U64_DIGITS && n >= POW10[w] {
        w += 1;
    }
    w
}

/// `n`'s decimal digits packed into one `u64` of ASCII, most significant digit
/// at byte 0 — the arithmetic core both integer emitters share
/// ([`JsonWriter::u32`] and [`JsonWriter::stage_u32`]), so there is one
/// implementation and one oracle. `digits` must be `decimal_width_u32(n)` and
/// at most [`WORD_DIGITS`]; the wide arms handle anything larger.
///
/// Two-digit-pair formatting (itoa's approach) halves the divisions, and the
/// writers emit several integers per node, so this is hot.
///
/// ⚠️ The digits are generated **into a register**, never into a stack scratch,
/// and that is a codegen constraint. A constant-length copy out of a scratch
/// array the pair loop just filled with 2-byte stores cannot be
/// store-forwarded past those narrow stores, and the store writes the
/// scratch's full width (20 bytes to keep ~3). Measured, that single store was
/// **71% of the emitter's self time and ~22% of the whole parse→JSON run**.
/// Packing into a `u64` removes both: nothing round-trips through memory, and
/// the store is one word instead of two vectors.
///
/// The accumulation runs least-significant pair first and shifts each earlier
/// pair **up**, so in little-endian byte order the most significant digit
/// lands at index 0 — the caller's write is then a plain prefix.
///
/// ⚠️ The pair loop is driven by the remaining **width**, not by the remaining
/// value. Driving on `n >= 100` leaves LLVM unable to correlate the trip count
/// with `digits`, so it must assume `i -= 2` can underflow; `while i >= 2`
/// makes non-underflow syntactic. The only indexing left is
/// `DEC_PAIRS[(n % 100) * 2]`, provably in bounds.
///
/// ⚠️ `inline`, and that is a **performance** constraint that costs binary
/// size. Out-of-lining it is +5.5% instructions on the parse→JSON path
/// (measured): the staged emitter's whole advantage is that `stage_len` and
/// the scratch base stay in registers across the header, and an opaque call
/// per integer forces them back to the stack. The size it buys — the parse
/// WASM bundle carries ~12 inlined copies — is the deliberate trade.
#[inline]
fn digit_word(n: u32, digits: usize) -> u64 {
    debug_assert!(digits == decimal_width_u32(n) && digits <= WORD_DIGITS);
    let mut word = 0u64;
    let mut i = digits;
    let mut n = n;
    while i >= 2 {
        let pair = (n % 100) as usize * 2;
        n /= 100;
        i -= 2;
        word = (word << 16) | u64::from(DEC_PAIRS[pair]) | (u64::from(DEC_PAIRS[pair + 1]) << 8);
    }
    if i == 1 {
        // Odd digit count: the leading digit is whatever the pair loop left.
        word = (word << 8) | u64::from(b'0' + n as u8);
    }
    word
}

/// Does `bytes` contain a byte JSON must escape?
///
/// The predicate is exactly `serde_json`'s: `0x00..=0x1F`, `"`, and `\`. It
/// answers eight bytes at a time because escaping is the *rare* case — the
/// strings the wire writers push through [`JsonWriter::string`] are identifier
/// names, string-literal bodies and comment text, nearly all of which spend the
/// whole scan confirming misses, the same shape (and the same reason) as the
/// line scans in [`crate::location`].
///
/// The three lane masks are OR-ed and read as a **boolean**, never as a
/// position, so the lowest-lane guarantee documented on [`zero_lanes`] /
/// [`lanes_less_than`] is exactly what is needed: a set mask always implies a
/// genuine hit somewhere. `lanes_less_than(w, 0x20)` already covers `NUL`, so
/// the two `zero_lanes` tests are only the `"` and `\` needles.
///
/// One word is loaded once and tested three ways rather than scanned three
/// times — the same trade `next_ecmascript_terminator` makes for its two
/// needles.
#[inline]
fn needs_escape(bytes: &[u8]) -> bool {
    let mut i = 0;
    while let Some(chunk) = bytes[i..].first_chunk::<8>() {
        let w = u64::from_le_bytes(*chunk);
        let hits =
            lanes_less_than(w, 0x20) | zero_lanes(w ^ splat(b'"')) | zero_lanes(w ^ splat(b'\\'));
        if hits != 0 {
            return true;
        }
        i += 8;
    }
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x20 || b == b'"' || b == b'\\' {
            return true;
        }
        i += 1;
    }
    false
}

/// Compact-JSON output buffer.
///
/// All writes are infallible (`Vec<u8>` backing). The escape-sensitive entry
/// points are [`JsonWriter::string`] (full JSON escaping via `serde_json`) and
/// [`JsonWriter::token`] (quoted verbatim — static ASCII tokens only,
/// debug-asserted).
pub struct JsonWriter {
    buf: Vec<u8>,
    /// Scratch for a staged run (see [`JsonWriter::stage_begin`]). A field
    /// rather than a local so it is initialized **once per writer**, not once
    /// per node — a per-call `[0; STAGE_CAP]` is a memset LLVM cannot prove
    /// dead, which is most of the cost the staging is here to remove.
    stage: [u8; STAGE_CAP],
    stage_len: usize,
}

/// Widest a staged run can be.
///
/// Sized so the widest node header cannot overrun it **on the types alone**,
/// not on an argument about reachable values: the longest node type
/// (`TSConstructSignatureDeclaration`, 31 bytes) plus every static fragment
/// plus all six integer fields at the full 20-digit `usize::MAX` width is 305
/// bytes. Real offsets are far smaller — they come from `u32` spans, so 10
/// digits — but pinning the bound to the type removes the need to re-derive
/// that reasoning whenever a field is added, and the buffer is initialized
/// once per writer, so the slack costs nothing per node.
///
/// Overrunning it panics on the slice bound rather than truncating: a staged
/// run is a fixed, auditable shape, so a field that doesn't fit is a bug to
/// size, not a case to handle. `widest_node_header_fits_the_staging_buffer`
/// is the test that holds this.
const STAGE_CAP: usize = 384;

impl JsonWriter {
    /// A fresh writer over a buffer pre-sized to `cap` bytes.
    #[inline]
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            stage: [0; STAGE_CAP],
            stage_len: 0,
        }
    }

    /// Begin a **staged run** — a fixed-shape burst of fragments assembled in
    /// the writer's scratch and appended to the output buffer as one write by
    /// [`JsonWriter::stage_flush`].
    ///
    /// This exists because of what a node header costs when written directly.
    /// The header is 16 appends per AST node (10 static fragments and 6
    /// integers), and each one pays `Vec`'s append protocol: reload `len`,
    /// compute `cap - len`, compare against the fragment width, branch to the
    /// grow path, store, update `len`. The integer emitters make it worse than
    /// it looks — [`JsonWriter::u32`] is deliberately `inline(never)` (a size
    /// constraint; see its comment), so every one of the six calls is opaque
    /// to LLVM and forces the surrounding appends to *re-load* the buffer's
    /// pointer, length and capacity afterwards. None of that bookkeeping
    /// survives staging: the scratch's base never moves, its bound is a
    /// compile-time constant, `stage_len` stays in a register across the whole
    /// header, and the integer emission inlines into the one staging site
    /// instead of ~200 writer call sites.
    ///
    /// The cost it trades for is a single runtime-length `extend_from_slice`
    /// per run — a `memmove` call over ~90 bytes, whose loads read bytes the
    /// staging just stored narrowly (the store-forwarding hazard that made a
    /// *20-byte* fixed blit inside `u32` a ~22%-of-run stall). At header scale
    /// that trade pays and was measured to: the copy is a loop over enough
    /// bytes to amortize both the dispatch and the drain, while the
    /// bookkeeping it removes is 16 checks and 6 reloads. **Grade any change
    /// to this shape on `cycles:u`, not instructions** — the hazard is
    /// invisible to an instruction count.
    ///
    /// Runs do not nest and are not reentrant: `stage_begin` resets the
    /// scratch, so every one must reach its `stage_flush` before the next
    /// begins.
    #[inline]
    pub fn stage_begin(&mut self) {
        self.stage_len = 0;
    }

    /// Append a verbatim fragment to the staged run. No escaping — the same
    /// contract as [`JsonWriter::raw`].
    #[inline]
    pub fn stage_raw(&mut self, s: &str) {
        let at = self.stage_len;
        let end = at + s.len();
        self.stage[at..end].copy_from_slice(s.as_bytes());
        self.stage_len = end;
    }

    /// Append a `u32`'s decimal digits to the staged run.
    ///
    /// Shares [`digit_word`] with [`JsonWriter::u32`], so both emitters have
    /// one arithmetic core and one oracle. Staging is strictly simpler than
    /// the direct path: the scratch always has `WORD_DIGITS` bytes of room, so
    /// the full word is stored and `stage_len` advances by the real digit
    /// count — no `truncate`.
    #[inline]
    pub fn stage_u32(&mut self, n: u32) {
        let digits = decimal_width_u32(n);
        if digits > WORD_DIGITS {
            self.stage_u32_wide(n, digits);
            return;
        }
        let at = self.stage_len;
        self.stage[at..at + WORD_DIGITS].copy_from_slice(&digit_word(n, digits).to_le_bytes());
        self.stage_len = at + digits;
    }

    /// The staged wide arm — a value past `99_999_999`, which no offset, line
    /// or column in a real document reaches. `cold` and out-of-line for the
    /// same reason as [`JsonWriter::u64_wide`].
    #[cold]
    #[inline(never)]
    fn stage_u32_wide(&mut self, n: u32, digits: usize) {
        let mut tmp = [0u8; MAX_U32_DIGITS];
        let mut i = digits;
        let mut n = n;
        while i >= 2 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        let at = self.stage_len;
        self.stage[at..at + digits].copy_from_slice(&tmp[..digits]);
        self.stage_len = at + digits;
    }

    /// Append a `usize` to the staged run — the line/column channel, which
    /// narrows to the `u32` worker exactly as [`JsonWriter::usize`] does.
    #[inline]
    pub fn stage_usize(&mut self, n: usize) {
        match u32::try_from(n) {
            Ok(n) => self.stage_u32(n),
            Err(_) => self.stage_usize_wide(n),
        }
    }

    /// A line or column past `u32::MAX` — unreachable for any source a `u32`
    /// span can address, but emitted faithfully rather than silently wrong.
    #[cold]
    #[inline(never)]
    fn stage_usize_wide(&mut self, n: usize) {
        let digits = decimal_width(n as u64);
        let mut tmp = [0u8; MAX_U64_DIGITS];
        let mut i = digits;
        let mut n = n as u64;
        while i >= 2 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        let at = self.stage_len;
        self.stage[at..at + digits].copy_from_slice(&tmp[..digits]);
        self.stage_len = at + digits;
    }

    /// Append the staged run to the output buffer — the single write the whole
    /// shape exists to reach. Leaves the scratch's contents behind; the next
    /// [`JsonWriter::stage_begin`] is what resets it.
    #[inline]
    pub fn stage_flush(&mut self) {
        self.buf.extend_from_slice(&self.stage[..self.stage_len]);
    }

    /// Consume the writer, yielding the emitted bytes.
    #[inline]
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// The bytes written so far (for composing writers / diagnostics).
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Verbatim JSON structure fragment (`{"key":`, `,`, `]`…). No escaping.
    #[inline]
    pub fn raw(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// A quoted static token (node type, operator, kind, keyword). These are
    /// compile-time ASCII strings that never contain `"`, `\`, or control
    /// characters, so they skip the escape scan.
    #[inline]
    pub fn token(&mut self, s: &str) {
        debug_assert!(
            s.bytes().all(|b| b != b'"' && b != b'\\' && b >= 0x20),
            "token must be escape-free: {s:?}"
        );
        self.buf.push(b'"');
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'"');
    }

    /// A dynamic string value, JSON-escaped and quoted.
    ///
    /// The escape set is exactly `serde_json`'s, because anything that needs
    /// escaping still goes through `serde_json::to_writer`. What the prescan
    /// buys is the *common* case: identifier names, string-literal bodies and
    /// comment text are overwhelmingly escape-free, and `serde_json`'s
    /// escaping loop pays a 256-entry table lookup plus `split_at` /
    /// `split_first` bookkeeping **per byte** to establish that. [`needs_escape`]
    /// answers the same question eight bytes at a time, and a clean answer
    /// turns the emission into [`JsonWriter::token`]'s quote-blit-quote.
    ///
    /// ⚠️ The predicate must stay a *superset* of `serde_json`'s `ESCAPE`
    /// table's non-zero set, or a byte that needs escaping would be blitted
    /// raw. That set is `0x00..=0x1F` plus `"` and `\` — nothing else, in
    /// particular not `DEL` and no non-ASCII byte. The equivalence is graded
    /// against `serde_json` itself by `string_matches_serde_json`, exhaustively
    /// over a boundary alphabet and across the 8-byte stride; a corpus cannot
    /// see this (a mis-scan would only surface on the rare input that actually
    /// carries the byte).
    ///
    /// The escaping arm is left where LLVM puts it: `#[cold]`-splitting it into
    /// its own out-of-line function so the hot arm inlines at the ~58 writer
    /// call sites was measured and is a **wash** (fuz_app −0.02%, zzz +0.03%
    /// instructions), so it is not worth the code size. Don't re-mint it.
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn string(&mut self, s: &str) {
        if needs_escape(s.as_bytes()) {
            serde_json::to_writer(&mut self.buf, s).expect("Vec<u8> write is infallible");
            return;
        }
        self.buf.push(b'"');
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'"');
    }

    /// A non-integral `f64` (the rare literal tail) — `serde_json`'s ryu
    /// formatting, matching `serde_json::Number` serialization.
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn f64(&mut self, n: f64) {
        serde_json::to_writer(&mut self.buf, &n).expect("Vec<u8> write is infallible");
    }

    // ⚠️ `inline(never)`, and that is a **size** constraint. The body below is
    // small enough that LLVM will happily inline it at all ~200 writer call
    // sites, and each copy carries the fixed-width blit — which grew the
    // `@fuzdev/tsv_parse_wasm` bundle 6% and blew its publish size bound.
    // Out-of-line the win is unaffected: it comes from removing the libc
    // `memmove` **call** inside the body, not from removing the call *to* the
    // body (which the pre-existing code also paid).
    #[inline(never)]
    pub fn u32(&mut self, n: u32) {
        // The append is a *constant*-length copy of [`digit_word`]'s register,
        // then a `truncate` — a runtime-length copy here would lower to a libc
        // `memmove` **call** whose size-ladder dispatch costs several times the
        // 1–3 bytes it moves. (The staged path pays no such copy at all: its
        // destination is the writer's scratch, which always has room for the
        // full word, so it just advances by `digits`.)
        let digits = decimal_width_u32(n);
        if digits > WORD_DIGITS {
            self.u64_wide(u64::from(n), digits);
            return;
        }
        let len = self.buf.len();
        self.buf
            .extend_from_slice(&digit_word(n, digits).to_le_bytes());
        self.buf.truncate(len + digits);
    }

    /// A `u64` value. **Every integer the writers actually emit — offsets,
    /// lines, columns — fits `u32`**, so this is a dispatcher, not a second
    /// implementation: the `u32` arm carries the real work and keeps its
    /// arithmetic 32-bit. The compare is one perfectly-predicted branch.
    #[inline]
    pub fn u64(&mut self, n: u64) {
        match u32::try_from(n) {
            Ok(n) => self.u32(n),
            Err(_) => self.u64_wide(n, decimal_width(n)),
        }
    }

    /// The wide arm — a value past `99_999_999`, which no offset, line or
    /// column in a real document reaches. Out-of-line and `cold` so the hot arm
    /// keeps its straight-line shape and the wide scratch never enters its
    /// stack frame.
    #[cold]
    #[inline(never)]
    fn u64_wide(&mut self, n: u64, digits: usize) {
        let mut tmp = [0u8; MAX_U64_DIGITS];
        let mut i = digits;
        let mut n = n;
        while i >= 2 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        }
        if i == 1 {
            tmp[0] = b'0' + n as u8;
        }
        self.buf.extend_from_slice(&tmp[..digits]);
    }

    #[inline]
    pub fn i64(&mut self, n: i64) {
        if n < 0 {
            self.buf.push(b'-');
        }
        self.u64(n.unsigned_abs());
    }

    /// A `usize` value — the writers' line and column channel. Narrows to the
    /// `u32` worker rather than widening to `u64`, so lines and columns keep
    /// 32-bit arithmetic too; only a value no source could produce takes the
    /// wide arm.
    #[inline]
    pub fn usize(&mut self, n: usize) {
        self.u64(n as u64);
    }

    #[inline]
    pub fn bool(&mut self, b: bool) {
        self.raw(if b { "true" } else { "false" });
    }

    #[inline]
    pub fn null(&mut self) {
        self.raw("null");
    }
}

/// Emit a JSON array: `[` + comma-separated items + `]`.
#[inline]
pub fn write_array<T>(
    w: &mut JsonWriter,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut JsonWriter, T),
) {
    w.raw("[");
    let mut first = true;
    for item in items {
        if !first {
            w.raw(",");
        }
        first = false;
        f(w, item);
    }
    w.raw("]");
}

/// Emit a nullable node value: the item through `f`, or `null` — the writer's
/// shape for every `Option` field *without* `skip_serializing_if`.
#[inline]
pub fn write_or_null<T>(w: &mut JsonWriter, item: Option<&T>, f: impl FnOnce(&mut JsonWriter, &T)) {
    match item {
        Some(v) => f(w, v),
        None => w.null(),
    }
}

/// Integer emission is **arithmetic**, and no corpus can grade arithmetic: a
/// wrong digit width writes a wrong offset that still parses as JSON, so a
/// fixture suite, a byte-diff over thousands of files, and every audit gate can
/// all stay green through the bug. The oracle has to live at the declaration —
/// these tests grade `u64` (and the `decimal_width` reservation it trusts)
/// against `std`'s own formatting, exhaustively where exhaustion is possible
/// and over every boundary where it isn't.
#[cfg(test)]
mod tests {
    use super::*;

    fn emit(n: u64) -> String {
        let mut w = JsonWriter::with_capacity(0);
        w.u64(n);
        String::from_utf8(w.into_bytes()).expect("digits are ASCII")
    }

    fn emit_string(s: &str) -> Vec<u8> {
        let mut w = JsonWriter::with_capacity(0);
        w.string(s);
        w.into_bytes()
    }

    /// [`JsonWriter::string`]'s escape-free fast path must be byte-identical to
    /// the `serde_json` delegation it replaces — the parity contract this
    /// module's whole doc comment rests on.
    ///
    /// Graded exhaustively over an alphabet that carries one member of every
    /// arm of `serde_json`'s `ESCAPE` table plus the bytes adjacent to each
    /// boundary: the named escapes, an unnamed control (`\u00XX`), both
    /// literal escapes, `0x1f`/`0x20` either side of the control cutoff, `DEL`
    /// (which `serde_json` does **not** escape), and a multibyte character
    /// whose UTF-8 bytes are all `>= 0x80`. A corpus cannot grade this: a
    /// mis-scan only surfaces on an input that actually carries the byte, and
    /// several of these never appear in real source at all.
    #[test]
    fn string_matches_serde_json() {
        const ALPHABET: [&str; 12] = [
            "\u{0}", "\u{8}", "\t", "\n", "\u{c}", "\r", "\u{1f}", " ", "\"", "\\", "\u{7f}", "é",
        ];
        // Every string of length 0–3 over the alphabet — enough for two
        // escapes to meet, and to sit at each end of a partial word.
        let mut cases: Vec<String> = vec![String::new()];
        for _ in 0..3 {
            let mut next = Vec::new();
            for base in &cases {
                for piece in ALPHABET {
                    next.push(format!("{base}{piece}"));
                }
            }
            cases.extend(next);
        }
        // ⚠️ The axis a word-at-a-time scan fails on: the same needle at every
        // offset across the 8-byte stride, including the scalar tail. A corpus
        // samples alignment arbitrarily; this pins all of it.
        for piece in ALPHABET {
            for offset in 0..24 {
                let mut s = "a".repeat(offset);
                s.push_str(piece);
                s.push_str(&"b".repeat(24 - offset));
                cases.push(s);
            }
        }
        for case in &cases {
            let ours = emit_string(case);
            let theirs = serde_json::to_vec(case).expect("serde_json serializes a str");
            assert_eq!(
                ours,
                theirs,
                "escape parity broke on {case:?}: ours {:?}, serde_json {:?}",
                String::from_utf8_lossy(&ours),
                String::from_utf8_lossy(&theirs)
            );
        }
    }

    /// The prescan's own answer, against a per-byte oracle — so a
    /// `needs_escape` regression is reported here rather than only as a
    /// slower-but-correct fallback.
    ///
    /// ⚠️ The **length** sweep is as load-bearing as the offset sweep: at a
    /// length that is a multiple of 8 the scalar tail never runs, so a
    /// tail-dropping corruption reads green. Every length 1–24 × every offset
    /// within it puts each needle in both the word body and the tail.
    #[test]
    fn needs_escape_matches_the_per_byte_predicate() {
        for needle in 0..=u8::MAX {
            for len in 1..=24usize {
                for offset in 0..len {
                    let mut bytes = vec![b'a'; len];
                    bytes[offset] = needle;
                    let expected = bytes.iter().any(|&b| b < 0x20 || b == b'"' || b == b'\\');
                    assert_eq!(
                        needs_escape(&bytes),
                        expected,
                        "byte {needle:#04x} at offset {offset} of {len}"
                    );
                }
            }
        }
        assert!(!needs_escape(b""), "the empty slice needs no escaping");
    }

    #[test]
    fn decimal_width_matches_std_formatting() {
        // Exhaustive over every value std can disagree on by one digit: each
        // power of ten and its two neighbours, plus the u64 ceiling.
        let mut pow = 1u64;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    decimal_width(n),
                    n.to_string().len(),
                    "decimal_width({n}) disagrees with std"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, u64::MAX, u64::MAX - 1, u32::MAX as u64, i64::MAX as u64] {
            assert_eq!(decimal_width(n), n.to_string().len(), "decimal_width({n})");
        }
    }

    #[test]
    fn decimal_width_u32_matches_std_formatting() {
        // Exhaustive over every value std can disagree on by one digit, plus
        // the u32 ceiling — the width the hot arm front-aligns to.
        let mut pow = 1u32;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    decimal_width_u32(n),
                    n.to_string().len(),
                    "decimal_width_u32({n}) disagrees with std"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, u32::MAX, u32::MAX - 1, i32::MAX as u32] {
            assert_eq!(decimal_width_u32(n), n.to_string().len(), "u32({n})");
        }
    }

    #[test]
    fn u32_matches_std_across_its_whole_range() {
        // `u32` is the worker, so grade it directly rather than only through
        // the `u64` dispatcher. Every boundary — including 8→9 digits, where
        // the register arm hands off to the wide one — plus an LCG sweep.
        fn emit_u32(n: u32) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.u32(n);
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        let mut pow = 1u32;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    emit_u32(n),
                    n.to_string(),
                    "u32({n}) at a power-of-ten edge"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, 1, u32::MAX, u32::MAX - 1, i32::MAX as u32] {
            assert_eq!(emit_u32(n), n.to_string(), "u32({n})");
        }
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            for n in [state as u32, (state >> 17) as u32, (state >> 33) as u32] {
                assert_eq!(emit_u32(n), n.to_string(), "u32({n})");
            }
        }
    }

    #[test]
    fn u64_matches_std_exhaustively_over_small_values() {
        // Every value through five digits — covers the whole distribution the
        // writer actually emits (offsets, lines, columns) and both arms of the
        // final odd/even-digit branch at every length.
        for n in 0..=100_000u64 {
            assert_eq!(emit(n), n.to_string(), "u64({n})");
        }
    }

    #[test]
    fn u64_matches_std_at_every_boundary_and_across_the_range() {
        let mut pow = 1u64;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(emit(n), n.to_string(), "u64({n}) at a power-of-ten edge");
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [u64::MAX, u64::MAX - 1, u32::MAX as u64, i64::MAX as u64] {
            assert_eq!(emit(n), n.to_string(), "u64({n})");
        }
        // A deterministic LCG sweep, to catch a digit-pair indexing error that
        // the structured cases above could miss.
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            for n in [state, state >> 17, state >> 33, state >> 49] {
                assert_eq!(emit(n), n.to_string(), "u64({n})");
            }
        }
    }

    /// The staged emitter is a **second** arithmetic path to the same digits,
    /// and it is the one every node header now goes through — so it needs its
    /// own oracle, not the direct path's. Same shape as
    /// `u32_matches_std_across_its_whole_range`: every power-of-ten edge
    /// (including 8→9 digits, where the register arm hands off to the cold
    /// one), the range ends, and an LCG sweep for digit-pair indexing errors.
    #[test]
    fn stage_u32_matches_std_across_its_whole_range() {
        fn emit_staged(n: u32) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.stage_begin();
            w.stage_u32(n);
            w.stage_flush();
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        let mut pow = 1u32;
        loop {
            for n in [pow.wrapping_sub(1), pow, pow + 1] {
                assert_eq!(
                    emit_staged(n),
                    n.to_string(),
                    "stage_u32({n}) at a power-of-ten edge"
                );
            }
            match pow.checked_mul(10) {
                Some(next) => pow = next,
                None => break,
            }
        }
        for n in [0, 1, u32::MAX, u32::MAX - 1, i32::MAX as u32] {
            assert_eq!(emit_staged(n), n.to_string(), "stage_u32({n})");
        }
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200_000 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            for n in [state as u32, (state >> 17) as u32, (state >> 33) as u32] {
                assert_eq!(emit_staged(n), n.to_string(), "stage_u32({n})");
            }
        }
    }

    /// The staged `usize` channel (lines and columns), including the cold arm
    /// past `u32::MAX` that no real source reaches but which must still emit
    /// the true value rather than a truncated one.
    #[test]
    fn stage_usize_matches_std_including_past_u32() {
        fn emit_staged(n: usize) -> String {
            let mut w = JsonWriter::with_capacity(0);
            w.stage_begin();
            w.stage_usize(n);
            w.stage_flush();
            String::from_utf8(w.into_bytes()).expect("digits are ASCII")
        }
        for n in 0..=2_000usize {
            assert_eq!(emit_staged(n), n.to_string(), "stage_usize({n})");
        }
        for n in [
            u32::MAX as usize - 1,
            u32::MAX as usize,
            u32::MAX as usize + 1,
            u64::MAX as usize,
            usize::MAX,
        ] {
            assert_eq!(emit_staged(n), n.to_string(), "stage_usize({n})");
        }
    }

    /// A staged run must reproduce exactly what the direct emitters would have
    /// written — the property that makes the staging a pure performance
    /// change. Grades a realistic node-header shape (the widest one: a long
    /// node type plus both `character` fields) against the direct path.
    #[test]
    fn staged_run_matches_the_direct_emitters_byte_for_byte() {
        let ints = [0u32, 7, 42, 999, 1_000, 65_535, 9_999_999, 100_000_000];
        for (i, &start) in ints.iter().enumerate() {
            for &end in &ints[i..] {
                let mut staged = JsonWriter::with_capacity(0);
                staged.stage_begin();
                staged.stage_raw("{\"type\":\"TSConstructSignatureDeclaration\"");
                staged.stage_raw(",\"start\":");
                staged.stage_u32(start);
                staged.stage_raw(",\"end\":");
                staged.stage_u32(end);
                staged.stage_raw(",\"loc\":{\"start\":{\"line\":");
                staged.stage_usize(start as usize);
                staged.stage_raw(",\"character\":");
                staged.stage_u32(end);
                staged.stage_raw("}}");
                staged.stage_flush();

                let mut direct = JsonWriter::with_capacity(0);
                direct.raw("{\"type\":\"TSConstructSignatureDeclaration\"");
                direct.raw(",\"start\":");
                direct.u32(start);
                direct.raw(",\"end\":");
                direct.u32(end);
                direct.raw(",\"loc\":{\"start\":{\"line\":");
                direct.usize(start as usize);
                direct.raw(",\"character\":");
                direct.u32(end);
                direct.raw("}}");

                assert_eq!(
                    staged.into_bytes(),
                    direct.into_bytes(),
                    "staged run diverged from the direct emitters at ({start}, {end})"
                );
            }
        }
    }

    /// Consecutive runs must not leak: `stage_begin` is the only reset, so a
    /// shorter run following a longer one must not carry the tail of the
    /// previous run's scratch into the output.
    #[test]
    fn staged_runs_do_not_leak_between_each_other() {
        let mut w = JsonWriter::with_capacity(0);
        w.stage_begin();
        w.stage_raw(",\"aVeryLongFragmentIndeed\":");
        w.stage_u32(4_294_967_295);
        w.stage_flush();
        w.stage_begin();
        w.stage_raw(",\"x\":");
        w.stage_u32(1);
        w.stage_flush();
        assert_eq!(
            String::from_utf8(w.into_bytes()).expect("ASCII"),
            ",\"aVeryLongFragmentIndeed\":4294967295,\"x\":1"
        );
    }

    /// The staged run's widest realistic shape must fit `STAGE_CAP` — the
    /// bound is a panic, not a truncation, so it has to be proven rather than
    /// assumed. Longest node type + every position field at `u32::MAX` width +
    /// both `character` fields.
    #[test]
    fn widest_node_header_fits_the_staging_buffer() {
        let mut w = JsonWriter::with_capacity(0);
        w.stage_begin();
        w.stage_raw("{\"type\":\"");
        w.stage_raw("TSConstructSignatureDeclaration");
        w.stage_raw("\"");
        w.stage_raw(",\"start\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"end\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"loc\":{\"start\":{\"line\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"column\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"character\":");
        w.stage_usize(usize::MAX);
        w.stage_raw("},\"end\":{\"line\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"column\":");
        w.stage_usize(usize::MAX);
        w.stage_raw(",\"character\":");
        w.stage_usize(usize::MAX);
        w.stage_raw("}}");
        w.stage_flush();
        // Headroom check: the widest run must leave room, not just barely fit.
        assert!(
            w.as_bytes().len() < STAGE_CAP,
            "widest staged header is {} bytes, STAGE_CAP is {STAGE_CAP}",
            w.as_bytes().len()
        );
    }

    #[test]
    fn u64_appends_without_disturbing_prior_bytes() {
        // The append writes a fixed-width word and truncates back; this pins
        // that it neither clobbers what precedes it nor leaves any of the
        // over-written tail behind, across a realloc and at every digit width.
        let mut w = JsonWriter::with_capacity(0);
        let mut expected = String::new();
        for n in (0..5_000u64).map(|i| i.wrapping_mul(2_654_435_761)) {
            w.raw(",");
            expected.push(',');
            w.u64(n);
            expected.push_str(&n.to_string());
        }
        assert_eq!(String::from_utf8(w.into_bytes()).expect("ASCII"), expected);
    }

    #[test]
    fn i64_matches_std() {
        for n in [
            0,
            1,
            -1,
            i64::MAX,
            i64::MIN,
            i64::MIN + 1,
            -99,
            -100,
            i32::MIN as i64,
        ] {
            let mut w = JsonWriter::with_capacity(0);
            w.i64(n);
            assert_eq!(
                String::from_utf8(w.into_bytes()).expect("ASCII"),
                n.to_string(),
                "i64({n})"
            );
        }
    }
}
