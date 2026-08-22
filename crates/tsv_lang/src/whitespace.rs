// The ECMAScript `\s` CharSet, shared by every reader whose oracle is spelled in JavaScript.

/// Whether `c` is whitespace to a **JavaScript regular expression's `\s`**.
///
/// Per ECMA-262 the `\s` CharSet is the union of the `WhiteSpace` and `LineTerminator`
/// productions: `<TAB>`, `<VT>`, `<FF>`, `<ZWNBSP>` and every code point in general category
/// `Space_Separator` (`Zs`), plus `<LF>`, `<CR>`, `<LS>` and `<PS>` — 25 in total.
///
/// It lives here rather than in one language crate because **two** of them need it and
/// neither can reach the other: `tsv_svelte`'s tokenizer class *is* this set (Svelte spells
/// every whitespace question in JavaScript — see that crate's `is_svelte_ws`), and so is the
/// class `parseCss` skips at its `allow_whitespace()` junctures, which `tsv_css` must match
/// (`tsv_css` is a *dependency* of `tsv_svelte`, so it cannot borrow the predicate from it).
/// One definition, one exhaustive test, no drift.
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

#[cfg(test)]
mod tests {
    use super::*;

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
