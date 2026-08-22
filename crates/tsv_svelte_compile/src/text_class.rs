//! Character classes and token predicates of the **target** languages' lexical
//! grammars, for the source scans and object-key shaping that reason about their
//! text without tokenizing it.
//!
//! Rust's own `char::is_whitespace` is the Unicode `White_Space` property, which
//! is NOT the same set, in **both** directions:
//!
//! - `U+FEFF` (`<ZWNBSP>`) is ECMAScript `WhiteSpace` (ECMA-262 §12.2, table 34)
//!   but carries no `White_Space` property, so `char::is_whitespace` says NO.
//!   Using it to skip JS trivia therefore **under-reports** — `static\u{FEFF}{…}`
//!   is a legal static block a `char::is_whitespace` scan does not see.
//! - `U+0085` (`<NEL>`) has the `White_Space` property but is neither
//!   ECMAScript `WhiteSpace` nor a `LineTerminator`, so `char::is_whitespace`
//!   says YES where JS says no. That direction only ever **over**-reports, which
//!   for every consumer here costs at most an extra refusal.
//!
//! A source scan whose whitespace notion is the HOST language's rather than the
//! target language's is a recurring defect in this crate, so the class lives
//! here once rather than being re-derived per scan.

/// ECMAScript `WhiteSpace` ∪ `LineTerminator` — the class JavaScript's `\s` matches.
///
/// Re-exported from [`tsv_lang`] rather than enumerated here: three crates in this workspace
/// need the same 25 code points ([`tsv_svelte`]'s tokenizer class, the class `parseCss` skips
/// at its `allow_whitespace()` junctures, and this crate's source scans), and each had
/// written its own copy with its own restatement of the two traps above. One definition means
/// one exhaustive per-code-point test — which this copy never had — and no way for the three
/// to drift.
pub(crate) use tsv_lang::is_js_whitespace;

/// `String.prototype.trim` — strips a leading and trailing [`is_js_whitespace`]
/// run, the `TrimString` production's `WhiteSpace`/`LineTerminator` class
/// (ECMA-262 §22.1.3.32).
///
/// For mirroring an oracle expression that is literally a JS `.trim()`. Rust's
/// `str::trim` is *not* it: it strips `U+0085` (`<NEL>`) which JS keeps, and
/// keeps `U+FEFF` which JS strips.
pub(crate) fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// The character starting at byte `pos`: its [`is_js_whitespace`] verdict and
/// its UTF-8 byte length.
///
/// For the byte-cursor trivia scans, which cannot use `u8::is_ascii_whitespace`:
/// that misses `<VT>` (`U+000B`, ASCII but not in Rust's ASCII-whitespace set)
/// and every non-ASCII JS whitespace, whose UTF-8 continuation bytes then read
/// as token text. Both make such a scan stop early — under-reporting, the
/// direction those scans exist to avoid.
///
/// The verdict and the step are returned **together, always**, because a caller
/// that advances by anything but a whole character walks onto a continuation
/// byte — which both mis-reads the text and (as a byte index into a `&str`)
/// panics. Splitting the two invited exactly that: a predicate-shaped
/// `is_whitespace_at(source, pos) -> Option<len>` leaves the non-whitespace
/// branch to invent its own step, and `pos += 1` after a `café` is a crash on
/// ordinary, legal source. There is no precondition to violate here — `pos` off
/// a boundary or past the end yields `None` rather than panicking.
pub(crate) struct JsChar {
    pub(crate) is_whitespace: bool,
    /// The character's UTF-8 length: the only sound step for a byte cursor,
    /// whichever branch the caller takes.
    pub(crate) len: usize,
}

pub(crate) fn js_char_at(source: &str, pos: usize) -> Option<JsChar> {
    let c = source.get(pos..)?.chars().next()?;
    Some(JsChar {
        is_whitespace: is_js_whitespace(c),
        len: c.len_utf8(),
    })
}

/// Whether `name` matches the ECMAScript identifier grammar
/// (`/^[a-zA-Z_$][a-zA-Z_$0-9]*$/`) — the oracle's `regex_is_valid_identifier`
/// gate (`b.key`), which decides whether an object key (a `style:`/`class:`
/// directive property, a component prop) prints as a bare identifier or a quoted
/// string literal. `format_canonical` applies the same test when dropping quotes
/// off a string-literal key, so a non-shorthand key can always be a string
/// literal; the identifier form matters only for the object-shorthand `{ color }`
/// a `style:color` shorthand builds. The single home of the check — it was
/// duplicated character-for-character across the attribute and component
/// emitters, two positions no one fixture exercises both of.
pub(crate) fn is_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// CSS `white-space` (CSS Syntax Level 3 §3.3 — newline, `U+0009`, `U+0020`,
/// where "newline" covers `U+000A` plus the `U+000D` / `U+000C` forms
/// preprocessing folds into it). A **strictly ASCII** class.
///
/// Not interchangeable with [`is_js_whitespace`], and the difference is not
/// cosmetic: `U+00A0` CONTINUES a CSS identifier where JS would end one, so
/// trimming a CSS name with a Unicode-whitespace notion silently renames it —
/// `:global\u{00A0}` reads as `:global` and scopes an element the oracle leaves
/// alone (a MISMATCH, oracle-verified).
///
/// That behavior is the **historical** ident rule — "every code point at or above
/// `U+0080` is an ident code point" — which tsv and Svelte both implement, and
/// which the oracle-matching contract makes the one that counts here. Current
/// css-syntax-3 is **narrower**: its *non-ASCII ident code point* is an explicit
/// enumeration (`U+00B7`, `U+00C0`–`U+00D6`, … , ≥`U+10000`) that, by its own note,
/// "excludes a number of characters that appear as whitespace". `U+00A0` falls in
/// the gap before the first range and is therefore **not** a spec-current ident
/// code point — so neither tsv nor the oracle is spec-current here, and the
/// example above is a statement about the two implementations, not about the spec.
pub(crate) fn is_css_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
    )
}
