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
