//! # The workspace's whitespace discipline
//!
//! "Is this whitespace?" has **three** answers in tsv, and which one a site wants follows from
//! the ORACLE it mirrors, never from the language the code is written in:
//!
//! | the site mirrors | class | asked by |
//! | --- | --- | --- |
//! | anything spelled in JavaScript — Svelte's parser, `parseCss`, prettier's `.trim()` | [`is_js_whitespace`] | `tsv_svelte`'s `is_svelte_ws`; `tsv_css`'s wire trims and escape terminators; this crate's directive + comment readers; `tsv_ts`'s comment renderer |
//! | css-syntax-3 tokenization (§4.2 **plus** §3.3, see there) | `tsv_css`'s `is_css_whitespace` (ASCII-only, five) | CSS value separation, value-text collapsing |
//! | "may a formatter respell this without changing what RENDERS" | `tsv_svelte`'s `is_collapsible_ws` (`[ \t\n\r]`) | the Svelte printer's text/fill |
//!
//! A fourth, narrower one is not a class but a transcription: prettier's own cursor scans
//! (`skipSpaces`, and its doc-printer's line-end `trim`) are literally `[' ', '\t']`, so the
//! byte matches that mirror them in `source_scan` and `doc/arena_render.rs` are right to be
//! ASCII-narrow and must not be widened to any of the above.
//!
//! And a **union** — `is_js_whitespace(c) || c.is_whitespace()`, i.e. JS `\s` plus U+0085 — is
//! the right answer at two sites, neither of which the table can place, because neither
//! mirrors an oracle:
//!
//! - `tsv_css`'s `is_boundary_whitespace`: the run `skip_boundary_whitespace` steps. The CSS
//!   lexer reads a `<NEL>` as whitespace where `parseCss` rejects it, so the union is what
//!   makes the printer's backward scan preserve exactly what the parser skipped, tracked gap
//!   included. Mirrors tsv's OWN over-acceptance, deliberately.
//! - `tsv_svelte`'s `narrow_lang_value`: tsv's formattable-`lang` list is an ALLOWLIST the
//!   trim feeds, so widening can only route a name toward formatting, while either class alone
//!   freezes a body prettier formats (JS `\s` keeps a U+0085, `str::trim` keeps a U+FEFF).
//!
//! They stay **two predicates with two arguments**, and folding them into one shared "union
//! whitespace" would be the bug the `is_tag_name_terminator` / `is_attr_name_terminator` split
//! exists to prevent: the expression agreeing is not the question agreeing, and the next
//! change to either site would be made against the wrong one. Where a site mirrors nothing,
//! ask which DIRECTION an error runs in, not which oracle it copies.
//!
//! ⚠️ **Rust's own whitespace is none of them, and reaching for it is the recurring bug.**
//! `str::trim*`, `str::split_whitespace` and `char::is_whitespace` are Unicode `White_Space`,
//! which disagrees with JS `\s` in BOTH directions — it **lacks U+FEFF** and **adds U+0085
//! NEL** — while `u8::is_ascii_whitespace` and a hand-written `b' ' | b'\t' | …` are narrower
//! still, missing every non-ASCII member. Both witnesses are asserted below, and each has been
//! the whole bug on its own; a fix graded against only one of them grades a half-fix as done.
//! The one place `str::trim_ascii*` is right is beneath [`trim_start_js_whitespace`] and its
//! siblings, where the byte it stops on is tested for what it cannot see — `<VT>`, a non-ASCII
//! byte — and the searcher over the full class takes over from there.
//!
//! ⚠️ The `Zs` half of this set is **Unicode-version-dependent** (ECMA-262 mandates the latest
//! Unicode, and U+180E left `Zs` in Unicode 6.3 — spec.html's own §Additions and Changes note
//! says so), while the exhaustive test below grades against *Svelte's* hand-written copy. So a
//! future `Zs` addition would leave tsv and Svelte stale together with the test still green.
//! Checked externally against UCD 15.1: the 17 `Zs` code points are exactly the enumeration
//! here, so the set is right against the SPEC and not merely against the oracle.
//!
//! It has been found in **four** crates, so a site's crate is no evidence either way. The
//! parser-side family lives in `tsv_svelte`'s own `whitespace.rs`; the rest:
//!
//! - the `format-ignore` / `prettier-ignore` recognizers (`comment.rs`) trimmed with
//!   `str::trim`, so `prettier-ignore<ZWNBSP>` was a directive prettier HONORS that tsv
//!   formatted through, and `prettier-ignore<NEL>` one prettier IGNORES that tsv froze — in
//!   all three languages at once, since every printer routes there;
//! - `is_indentable_block_comment` (`printing.rs`) classified on `str::trim_start`, and its
//!   two answers print through entirely different emitters, so either witness moved the whole
//!   comment;
//! - `tsv_ts`'s comment renderer trimmed each interior line with Rust's class, **deleting a
//!   U+0085** prettier keeps, and had no trim at all where prettier applies one (a line
//!   comment's `trimEnd`);
//! - `tsv_css`'s wire trims mirror `read_value`'s `value.trim()`, and its hex-escape
//!   terminator mirrors `read_identifier`'s `(\r\n|\s)?` — a JS regex, which the comment
//!   there had equated with `char::is_whitespace`.

/// Whether `c` is whitespace to a **JavaScript regular expression's `\s`**.
///
/// Per ECMA-262 the `\s` CharSet is the union of the `WhiteSpace` and `LineTerminator`
/// productions: `<TAB>`, `<VT>`, `<FF>`, `<ZWNBSP>` and every code point in general category
/// `Space_Separator` (`Zs`), plus `<LF>`, `<CR>`, `<LS>` and `<PS>` — 25 in total.
///
/// It lives here rather than in one language crate because **five** crates need it and no
/// language crate can serve them all: `tsv_svelte`'s tokenizer class *is* this set (Svelte
/// spells every whitespace question in JavaScript — see that crate's `is_svelte_ws`), and so
/// is the class `parseCss` skips at its `allow_whitespace()` junctures, which `tsv_css` must
/// match — and `tsv_css` is a *dependency* of `tsv_svelte`, so it cannot borrow the predicate
/// from it. Since then this crate's own directive and comment readers, `tsv_ts`'s comment
/// renderer, `tsv_css`'s wire trims, and `tsv_svelte_compile`'s source scans (which re-export
/// it as `text_class::is_js_whitespace`) have joined them, each after writing its own copy
/// with its own restatement of the two traps below. One definition, one exhaustive test, no
/// drift.
///
/// ⚠️ **Not Rust's `char::is_whitespace()`**, which is the Unicode `White_Space` property.
/// The two are both 25 code points and differ in *both* directions, so neither is a superset
/// and either substitution is a bug:
///
/// - **U+FEFF** ZERO WIDTH NO-BREAK SPACE is in `\s` but is `Cf`, not `White_Space`. Rust's
///   predicate misses it.
/// - **U+0085** NEXT LINE is `White_Space` but is not `Zs`, so ECMA-262 excludes it
///   deliberately ("intentionally excludes all code points that have the Unicode
///   'White_Space' property but which are not classified in general category
///   'Space_Separator'"). Rust's predicate over-matches it, which silently *accepts* input
///   the JS-spelled oracle rejects.
///
/// ⚠️ Also **not** the CSS Syntax class (`tsv_css`'s `is_css_whitespace`, ASCII-only) — that
/// one answers css-syntax-3's *tokenization* question and is the right class for value
/// separation and value-text collapsing. This one answers "would `parser.allow_whitespace()`
/// have stepped over it", which is a different question with a different answer at every
/// code point at or above U+00A0.
#[inline]
pub const fn is_js_whitespace(c: char) -> bool {
    // ASCII is split out rather than folded into one `matches!` so the common path stays a
    // handful of compares instead of a decision tree over the full (mostly non-ASCII) member
    // set — these predicates run per character of every name in the document. The two arms
    // partition the set, and `matches_ecmascript_s_at_every_code_point` grades the whole
    // predicate per code point, so a member landing in the wrong arm cannot hide.
    if (c as u32) < 0x80 {
        return matches!(
            c,
            '\u{9}'      // <TAB>
            | '\u{a}'    // <LF>
            | '\u{b}'    // <VT>
            | '\u{c}'    // <FF>
            | '\u{d}'    // <CR>
            | '\u{20}' // SPACE                     (Zs)
        );
    }
    matches!(
        c,
        '\u{a0}'         // NO-BREAK SPACE           (Zs)
        | '\u{1680}'     // OGHAM SPACE MARK         (Zs)
        | '\u{2000}'
            ..='\u{200a}' // EN QUAD..HAIR SPACE (Zs)
        | '\u{2028}'     // <LS>
        | '\u{2029}'     // <PS>
        | '\u{202f}'     // NARROW NO-BREAK SPACE    (Zs)
        | '\u{205f}'     // MEDIUM MATHEMATICAL SPACE (Zs)
        | '\u{3000}'     // IDEOGRAPHIC SPACE        (Zs)
        | '\u{feff}' // <ZWNBSP>
    )
}

/// `s.trim_start_matches(is_js_whitespace)`, the common case answered on bytes.
///
/// A predicate-taking `str` trim builds a searcher that decodes a char from an end before it can
/// reject — tens of instructions to step the one space a comment line or a directive usually
/// carries, at a site asked once per comment line (135K asks a pass on a TypeScript corpus).
/// Five of the class's six ASCII members are `u8::is_ascii_whitespace`, so `str::trim_ascii_start`
/// steps them as bytes with no slice check of its own; the byte it stops on decides the rest —
/// `<VT>` (the one ASCII member that class lacks) or the lead byte of a non-ASCII char may still
/// be a member, and only then does the searcher run, out of line. Every other stop byte is a
/// whole char no member begins, so the byte answer is the searcher's; the `debug_assert` keeps
/// that claim under the fixture suite rather than in prose.
///
/// One body per direction, no runtime flag: a single body over two direction flags came out as
/// three outlined copies of ~300 B each (the `Range` slice's boundary checks, the wide test re-read
/// from both ends), and paid the searcher's price call for call.
#[inline]
pub fn trim_start_js_whitespace(s: &str) -> &str {
    let head = s.trim_ascii_start();
    if stop_byte_may_be_js_whitespace(head.as_bytes().first()) {
        return trim_start_js_whitespace_wide(head);
    }
    debug_assert_eq!(
        head,
        s.trim_start_matches(is_js_whitespace),
        "an ASCII non-member at the trimmed end proves the searcher would stop there too"
    );
    head
}

/// `s.trim_end_matches(is_js_whitespace)` — see [`trim_start_js_whitespace`].
#[inline]
pub fn trim_end_js_whitespace(s: &str) -> &str {
    let head = s.trim_ascii_end();
    if stop_byte_may_be_js_whitespace(head.as_bytes().last()) {
        return trim_end_js_whitespace_wide(head);
    }
    debug_assert_eq!(head, s.trim_end_matches(is_js_whitespace));
    head
}

/// `s.trim_matches(is_js_whitespace)` — see [`trim_start_js_whitespace`].
#[inline]
pub fn trim_js_whitespace(s: &str) -> &str {
    let inner = s.trim_ascii();
    let bytes = inner.as_bytes();
    if stop_byte_may_be_js_whitespace(bytes.first()) || stop_byte_may_be_js_whitespace(bytes.last())
    {
        return trim_js_whitespace_wide(inner);
    }
    debug_assert_eq!(inner, s.trim_matches(is_js_whitespace));
    inner
}

/// Whether the byte a `trim_ascii*` stopped on may still belong to [`is_js_whitespace`]: `<VT>`,
/// or any byte of a non-ASCII char (a lead byte at a head, a continuation byte at a tail).
#[inline]
const fn stop_byte_may_be_js_whitespace(b: Option<&u8>) -> bool {
    matches!(b, Some(&b) if b == 0x0b || b >= 0x80)
}

#[cold]
#[inline(never)]
fn trim_start_js_whitespace_wide(s: &str) -> &str {
    s.trim_start_matches(is_js_whitespace)
}

#[cold]
#[inline(never)]
fn trim_end_js_whitespace_wide(s: &str) -> &str {
    s.trim_end_matches(is_js_whitespace)
}

#[cold]
#[inline(never)]
fn trim_js_whitespace_wide(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte-gated trims agree with the searchers at every arrangement of ASCII members
    /// (`<VT>` among them — the member `trim_ascii` does not step), the non-ASCII witnesses
    /// (`<NBSP>`, `<ZWNBSP>` — members; `<NEL>`, `é` — not) and content, zero to four pieces
    /// long — the axis a corpus never samples, since real source carries none of the non-ASCII
    /// members at a comment's edge.
    #[test]
    fn byte_trims_match_the_searchers() {
        const PIECES: [&str; 10] = [
            " ", "\t", "\n", "\u{b}", "\u{a0}", "\u{feff}", "\u{85}", "\u{2028}", "x", "é",
        ];
        let mut s = String::new();
        for len in 0..=4u32 {
            for code in 0..PIECES.len().pow(len) {
                s.clear();
                let mut c = code;
                for _ in 0..len {
                    s.push_str(PIECES[c % PIECES.len()]);
                    c /= PIECES.len();
                }
                assert_eq!(
                    trim_js_whitespace(&s),
                    s.trim_matches(is_js_whitespace),
                    "{s:?}"
                );
                assert_eq!(
                    trim_start_js_whitespace(&s),
                    s.trim_start_matches(is_js_whitespace),
                    "{s:?}"
                );
                assert_eq!(
                    trim_end_js_whitespace(&s),
                    s.trim_end_matches(is_js_whitespace),
                    "{s:?}"
                );
            }
        }
    }

    /// Svelte's own `is_whitespace(cc)` (`1-parse/index.js`), transcribed — the hand-written
    /// enumeration of JS `\s` that both of tsv's JS-spelled oracles are read through. The
    /// predicate must agree with it at every code point: this is the drop-in contract, and
    /// the set is small enough to check exhaustively rather than by sampling.
    const fn svelte_is_whitespace(cc: u32) -> bool {
        if cc == 32 || (cc <= 13 && cc >= 9) {
            return true;
        }
        if cc < 160 {
            return false;
        }
        cc == 160
            || cc == 5760
            || (cc >= 8192 && cc <= 8202)
            || cc == 8232
            || cc == 8233
            || cc == 8239
            || cc == 8287
            || cc == 12288
            || cc == 65279
    }

    #[test]
    fn matches_ecmascript_s_at_every_code_point() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            assert_eq!(
                is_js_whitespace(c),
                svelte_is_whitespace(cp),
                "U+{cp:04X} disagrees with the JS `\\s` enumeration"
            );
        }
    }

    /// The two directions Rust's `char::is_whitespace()` gets wrong; each has its own
    /// assertion, and both would be silent if the predicate were swapped for the convenient
    /// one.
    #[test]
    fn differs_from_unicode_white_space_in_both_directions() {
        assert!(is_js_whitespace('\u{feff}') && !'\u{feff}'.is_whitespace());
        assert!(!is_js_whitespace('\u{85}') && '\u{85}'.is_whitespace());
    }
}
