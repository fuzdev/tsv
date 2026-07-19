//! Character classes of the **target** languages' lexical grammars, for the
//! source scans that reason about their text without tokenizing it.
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

/// ECMAScript `WhiteSpace` ∪ `LineTerminator` — equivalently the class JavaScript's
/// `\s` regex matches (ECMA-262 §12.2 table 34 + §12.3 table 35, and §22.2.2.9's
/// `WhiteSpace`/`LineTerminator` production for `\s`).
///
/// `WhiteSpace` is TAB / VT / FF / SP / NBSP / ZWNBSP plus the `Zs` general
/// category (`U+1680`, `U+2000`..=`U+200A`, `U+202F`, `U+205F`, `U+3000`);
/// `LineTerminator` is LF / CR / LS / PS.
pub(crate) fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}' // <TAB>
            | '\u{000A}' // <LF>, a LineTerminator
            | '\u{000B}' // <VT>
            | '\u{000C}' // <FF>
            | '\u{000D}' // <CR>, a LineTerminator
            | '\u{0020}' // <SP>
            | '\u{00A0}' // <NBSP>
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
            | '\u{2028}' // <LS>, a LineTerminator
            | '\u{2029}' // <PS>, a LineTerminator
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}' // <ZWNBSP>
    )
}

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

/// CSS `white-space` (CSS Syntax Level 3 §3.3 — newline, `U+0009`, `U+0020`,
/// where "newline" covers `U+000A` plus the `U+000D` / `U+000C` forms
/// preprocessing folds into it). A **strictly ASCII** class.
///
/// Not interchangeable with [`is_js_whitespace`], and the difference is not
/// cosmetic: every code point at or above `U+0080` is a CSS *ident* code point
/// (§4.2), so `U+00A0` CONTINUES a CSS identifier where JS would end one.
/// Trimming a CSS name with a Unicode-whitespace notion therefore silently
/// renames it — `:global\u{00A0}` reads as `:global` and scopes an element the
/// oracle leaves alone (a MISMATCH, oracle-verified).
pub(crate) fn is_css_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | '\u{0020}'
    )
}
