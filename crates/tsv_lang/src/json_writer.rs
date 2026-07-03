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

    #[inline]
    pub fn u64(&mut self, n: u64) {
        // Two-digit-pair formatting (itoa's approach): halves the divisions.
        // Writers emit several integers per node, so this is hot.
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
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        let mut n = n;
        while n >= 100 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        }
        if n >= 10 {
            let pair = n as usize * 2;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        } else {
            i -= 1;
            tmp[i] = b'0' + n as u8;
        }
        self.buf.extend_from_slice(&tmp[i..]);
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
