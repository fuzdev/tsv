// CSS whitespace classification, shared by the value parser (separation) and the
// printer (value-text normalization) so both agree on what counts as whitespace.

/// CSS whitespace is ASCII-only — tab, line feed, form feed, carriage return, and space.
///
/// ⚠️ **Five, where css-syntax-3 §4.2 defines three.** §4.2 is *newline* (which it fixes at
/// U+000A alone), tab and space; carriage return and form feed are absent from it **because
/// §3.3's input preprocessing has already folded them to U+000A** by the time tokenization
/// runs. tsv does not run that preprocessing — `parse` takes the author's bytes so its offsets
/// stay a drop-in contract with `parseCss` — which puts the two folded code points back on this
/// predicate's plate. Reading §4.2 on its own and narrowing this to `[' ', '\t', '\n']` is
/// therefore a plausible-looking change that stops every value scanner on a `<CR>` or a form
/// feed. The rule is §4.2 **plus** §3.3, and `is_ascii_whitespace` is exactly that union.
///
/// ⚠️ **Not the class the lexer skips at a token boundary**, which is JS `\s`
/// ([`tsv_lang::is_js_whitespace`]) because that is what `parseCss`'s `allow_whitespace()`
/// is. The two answer different questions and differ at every code point at or above
/// U+00A0: this one is about value *separation* and value-text *collapsing*, where a
/// `<NBSP>` is content that must not be rewritten to a space; that one is about whether a
/// token even starts here. Both are right, and swapping either for the other is a bug.
///
/// This is deliberately **not** `char::is_whitespace`,
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

/// Whether `c` is whitespace to `parseCss`'s `allow_whitespace()` **and** identifier content
/// to its `read_identifier` — the ambiguous class, and the exact run a token boundary skips
/// and the printer puts back.
///
/// The two sides are one fact asked from opposite directions:
/// `CssParser::skip_boundary_whitespace` decides what the AST does **not** contain, and
/// `Printer::preserved_boundary_ws` decides what the output must still carry. Spelling it
/// twice is a drift hazard with two silent failure modes — a printer that restores less than
/// the parser skipped deletes source, and one that restores more duplicates it — so both ask
/// this.
///
/// It is JS `\s` minus its ASCII members: an ASCII space really is a separator on both sides
/// of `parseCss`, so it is the printer's to regenerate as indentation, not to preserve
/// verbatim.
#[inline]
pub(crate) fn is_boundary_only_whitespace(c: char) -> bool {
    !c.is_ascii() && tsv_lang::is_js_whitespace(c)
}

/// The whole run `CssParser::skip_boundary_whitespace` steps: the lexer's own whitespace
/// (`is_ascii_css_whitespace` plus the sub-U+00A0 non-ASCII whitespace its dispatch admits)
/// **and** the members hiding inside an identifier token.
///
/// Named once because the printer scans that run **backwards** out of the source, where the
/// parser met it as a token stream, and the two must agree on where it starts — a printer
/// that stops early deletes the members behind its stopping point. Both terms are load-
/// bearing at that seam: `<NBSP><VT>div` needs the ASCII half to reach the `<NBSP>`, and
/// `<NBSP><NEL>div` needs the `White_Space` half.
///
/// ⚠️ Neither term alone is this set, and neither is [`is_css_whitespace`] — that one is the
/// css-syntax-3 *tokenization* class (ASCII, no `<VT>`), the right answer for value
/// separation and the wrong one here. This is JS `\s` ∪ Unicode `White_Space`, which is JS
/// `\s` plus `<NEL>` (U+0085): the lexer reads a `<NEL>` as whitespace where `parseCss`
/// rejects it, so the run the parser really steps includes one, and the printer must scan
/// past it to reach anything behind it. That over-acceptance is the tracked `<NEL>` gap — see
/// [`tests/css_boundary_whitespace.rs`](../../../tests/css_boundary_whitespace.rs) — and
/// mirroring it here is what keeps the printer preserving exactly what the parser skipped,
/// gap included, rather than inventing a second answer.
#[inline]
pub(crate) fn is_boundary_whitespace(c: char) -> bool {
    tsv_lang::is_js_whitespace(c) || c.is_whitespace()
}
