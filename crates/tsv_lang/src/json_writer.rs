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

/// Compact-JSON output buffer.
///
/// All writes are infallible (`Vec<u8>` backing). The escape-sensitive entry
/// points are [`JsonWriter::string`] (full JSON escaping via `serde_json`) and
/// [`JsonWriter::token`] (quoted verbatim — static ASCII tokens only,
/// debug-asserted).
pub struct JsonWriter {
    buf: Vec<u8>,
}

impl JsonWriter {
    /// A fresh writer over a buffer pre-sized to `cap` bytes.
    #[inline]
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
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

    /// A dynamic string value, JSON-escaped and quoted. Delegates to
    /// `serde_json` so the escape set is exactly `serde_json`'s.
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn string(&mut self, s: &str) {
        serde_json::to_writer(&mut self.buf, s).expect("Vec<u8> write is infallible");
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
        // Two-digit-pair formatting (itoa's approach): halves the divisions.
        // Writers emit several integers per node, so this is hot.
        //
        // ⚠️ The digits are generated **into a register**, never into a stack
        // scratch, and that is a codegen constraint. The append must be a
        // *constant*-length copy — a runtime-length one lowers to a libc
        // `memmove` **call** whose size-ladder dispatch costs several times the
        // 1–3 bytes it moves — but a constant-length copy out of a scratch
        // array the pair loop just filled with 2-byte stores is worse still:
        // the wide load that feeds the store cannot be store-forwarded past
        // those narrow stores, and the store itself writes the scratch's full
        // width (20 bytes to keep ~3). Measured, that single store instruction
        // was **71% of this function's self time and ~22% of the whole
        // parse→JSON run**. Packing the digits into a `u64` and appending its
        // 8 `to_le_bytes` removes both: nothing round-trips through memory, and
        // the store is one word instead of two vectors.
        //
        // The accumulation runs least-significant pair first and shifts each
        // earlier pair **up**, so in little-endian byte order the most
        // significant digit lands at index 0 — the append is then a plain
        // prefix and `truncate` keeps the leading `digits`.
        // ⚠️ The pair loop is driven by the remaining **width**, not by the
        // remaining value. Driving on `n >= 100` leaves LLVM unable to
        // correlate the trip count with `digits`, so it must assume `i -= 2`
        // can underflow; `while i >= 2` makes non-underflow syntactic. The only
        // indexing left is `DEC_PAIRS[(n % 100) * 2]`, provably in bounds.
        let digits = decimal_width_u32(n);
        if digits > WORD_DIGITS {
            self.u64_wide(u64::from(n), digits);
            return;
        }
        let mut word = 0u64;
        let mut i = digits;
        let mut n = n;
        while i >= 2 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            word =
                (word << 16) | u64::from(DEC_PAIRS[pair]) | (u64::from(DEC_PAIRS[pair + 1]) << 8);
        }
        if i == 1 {
            // Odd digit count: the leading digit is whatever the pair loop
            // left behind.
            word = (word << 8) | u64::from(b'0' + n as u8);
        }
        let len = self.buf.len();
        self.buf.extend_from_slice(&word.to_le_bytes());
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
