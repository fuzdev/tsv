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

/// `00`,`01`,…,`99` — the two-digit-pair table behind [`JsonWriter::u64`],
/// halving its divisions.
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

/// Decimal digits in `u64::MAX` — the width of [`JsonWriter::u64`]'s scratch,
/// and the constant length of the copy that appends it.
const MAX_U64_DIGITS: usize = 20;

/// Decimal digit count of `n` (`0` is one digit) — the exact width
/// [`JsonWriter::u64`] front-aligns its digits to.
///
/// An **ascending** compare chain, deliberately: the writer's integers are
/// offsets, lines and columns, so the distribution is overwhelmingly small and
/// the first few compares are perfectly predicted. `ilog10`'s descending
/// halving structure is the better shape for a uniform-over-`u64` load, which
/// this is not.
///
/// ⚠️ The tail is a **bounded loop**, not a call to `ilog10` — and that is a
/// codegen constraint, not a style choice. `ilog10` is out-of-line, so its
/// result is opaque to the caller: LLVM then cannot prove the returned width
/// fits `JsonWriter::u64`'s scratch and re-inserts a `panic_bounds_check` on
/// every one of its four scratch writes. A `while w < MAX_U64_DIGITS` loop
/// makes the range provable, and the bounds checks disappear. Keep any future
/// rewrite provably in `1..=MAX_U64_DIGITS` **at the type/CFG level**.
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
    // sites, and each copy carries the fixed-width scratch blit — which grew
    // the `@fuzdev/tsv_parse_wasm` bundle 6% and blew its publish size bound.
    // Out-of-line the win is unaffected: it comes from removing the libc
    // `memmove` **call** inside the body, not from removing the call *to* the
    // body (which the pre-existing code also paid).
    #[inline(never)]
    pub fn u64(&mut self, n: u64) {
        // Two-digit-pair formatting (itoa's approach): halves the divisions.
        // Writers emit several integers per node, so this is hot.
        //
        // ⚠️ The append is a **fixed-width** copy of the whole scratch, then a
        // `truncate` — deliberately, and it must stay that way. Appending
        // `&tmp[..digits]` instead is a *runtime*-length copy, which lowers to
        // a libc `memmove` **call** whose size-ladder dispatch costs several
        // times the 1–3 bytes it actually moves. At ~6 integers per AST node
        // that dispatch measured 7.5% of the whole parse→JSON run, dwarfing the
        // extra stores a constant-width copy pays. A constant length lets LLVM
        // inline the copy as two stores and never reach libc.
        //
        // Front-aligning the digits is what makes the fixed-width append
        // correct: knowing the exact width up front (`decimal_width`) lets the
        // back-to-front generation land on index 0 rather than on the scratch's
        // tail, so the bytes to keep are the *leading* `digits`.
        // ⚠️ The pair loop is driven by the remaining **width**, not by the
        // remaining value — also a codegen constraint. Driving on `n >= 100`
        // leaves LLVM unable to correlate the trip count with `digits`, so it
        // must assume `i -= 2` can underflow and re-inserts a
        // `panic_bounds_check` on all four scratch writes. `while i >= 2` makes
        // non-underflow syntactic, and with `decimal_width` bounded the whole
        // index range is provable, so every check folds away.
        let digits = decimal_width(n);
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
            // Odd digit count: the leading digit is whatever the pair loop
            // left behind.
            tmp[0] = b'0' + n as u8;
        }
        let len = self.buf.len();
        self.buf.extend_from_slice(&tmp);
        self.buf.truncate(len + digits);
    }

    #[inline]
    pub fn i64(&mut self, n: i64) {
        if n < 0 {
            self.buf.push(b'-');
        }
        self.u64(n.unsigned_abs());
    }

    #[inline]
    pub fn u32(&mut self, n: u32) {
        self.u64(u64::from(n));
    }

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
        // The in-place generation writes through a raw pointer into spare
        // capacity; this pins that it neither clobbers what precedes it nor
        // leaves uninitialized bytes behind across a realloc.
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
