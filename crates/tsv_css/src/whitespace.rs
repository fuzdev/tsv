// CSS whitespace classification, shared by the value parser (separation) and the
// printer (value-text normalization) so both agree on what counts as whitespace.

/// CSS whitespace is ASCII-only — tab, line feed, form feed, carriage return, and
/// space (css-syntax-3 §4.2). This is deliberately **not** `char::is_whitespace`,
/// which follows the Unicode `White_Space` property and would treat NBSP
/// (U+00A0), NEL, the ideographic space, etc. as separators (it also includes
/// U+000B VT, which CSS does not). Those code points are ordinary value content —
/// both prettier and Svelte's `parseCss` keep them inside their token — so all of
/// CSS value *separation* (`ValueCursor` here, and the byte-scanning
/// `classify_separators` via the equivalent `u8::is_ascii_whitespace`) and
/// value-text whitespace *collapsing* (the printer's `normalize_css_whitespace`)
/// act only on ASCII whitespace; otherwise a non-ASCII-whitespace code point would
/// be silently rewritten to a space.
#[inline]
pub(crate) fn is_css_whitespace(c: char) -> bool {
    c.is_ascii_whitespace()
}

/// The whitespace a CSS value-**boundary** trim strips: the ASCII whitespace set
/// **plus** U+000B (vertical tab) — i.e. exactly what `str::trim` removes *within
/// ASCII*. It differs from [`is_css_whitespace`] only on VT (which `str::trim`
/// eats), so replacing a `str::trim*` with this predicate is byte-identical for
/// ASCII input — the point is what it *excludes*: a non-ASCII "whitespace" char
/// (NBSP U+00A0, em space U+2003, NEL, …) is CSS value **content**, not a
/// separator, so it is never trimmed. Trimming one would silently drop content
/// and, for a quoted-string element, desync the leaf's text from its span (the
/// string printer then extracts a span that no longer begins with a quote and
/// emits nothing — deleting the whole element). Every value-boundary trim — in the
/// value parser (`ValueParser`) and the printer's `normalize_css_whitespace` — uses
/// this in place of `str::trim*`.
#[inline]
pub(crate) fn is_ascii_trim_ws(c: char) -> bool {
    c.is_ascii() && c.is_whitespace()
}
