// Svelte's tokenizer whitespace class

/// Whether `c` is whitespace to **Svelte's parser** — the class that separates tokens,
/// ends a tag-name or attribute-name run, and satisfies a block keyword's required space.
///
/// This is exactly JavaScript's `\s`, because every place Svelte asks the question is
/// spelled in JavaScript: `is_whitespace(cc)` (`1-parse/index.js`, backing
/// `allow_whitespace` / `require_whitespace`) enumerates these code points by hand, and the
/// name-run regexes match the same set through `\s` —
/// `regex_whitespace_or_slash_or_closing_tag = /(\s|\/|>)/` for tag names and
/// `regex_token_ending_character = /[\s=/>"']/` for attribute and directive names (both in
/// `1-parse/state/element.js`), as do the raw-text closes `/<\/script\s*>/`,
/// `/<\/style\s*>/` and the RCDATA close `/<\/textarea(\s[^>]*)?>/iy`.
///
/// Per ECMA-262, the `\s` CharSet is the union of the `WhiteSpace` and `LineTerminator`
/// productions: `<TAB>`, `<VT>`, `<FF>`, `<ZWNBSP>` and every code point in general category
/// `Space_Separator` (`Zs`), plus `<LF>`, `<CR>`, `<LS>` and `<PS>` — 25 in total.
///
/// ⚠️ **Not Rust's `char::is_whitespace()`**, which is the Unicode `White_Space` property.
/// The two are both 25 code points and differ in *both* directions, so neither is a superset
/// and either substitution is a bug:
///
/// - **U+FEFF** ZERO WIDTH NO-BREAK SPACE is in `\s` but is `Cf`, not `White_Space`. Rust's
///   predicate misses it, so a name run swallows it (`data-attr<U+FEFF>data-attr2` parses as
///   one attribute instead of two).
/// - **U+0085** NEXT LINE is `White_Space` but is not `Zs`, so ECMA-262 excludes it
///   deliberately ("intentionally excludes all code points that have the Unicode
///   'White_Space' property but which are not classified in general category
///   'Space_Separator'"). Rust's predicate over-matches it, which silently *accepts* input
///   Svelte rejects (`{#if<U+0085>cond}`) and splits a name Svelte keeps whole.
///
/// ⚠️ Also **not** [`is_collapsible_ws`](crate::ast::internal::is_collapsible_ws), the
/// narrower render class (`[ \t\n\r]`). That one answers "may a formatter add, drop or
/// respell this without changing what renders"; this one answers "does this end a token".
/// A form feed is whitespace here and rendered content there.
///
/// ⚠️ And **not** `tsv_ts`'s `is_es_whitespace`, which is the `WhiteSpace` production
/// *alone*: a JS lexer matches `LineTerminator` separately because a newline drives ASI,
/// so that predicate deliberately omits `<LF>`/`<CR>`/`<LS>`/`<PS>`. This class is the
/// union of both productions (that is what `\s` means), so it is a strict superset. The
/// three predicates are deliberately separate — each is named for the question it answers,
/// and the exhaustive test below grades this one against Svelte's own enumeration rather
/// than against either sibling.
#[inline]
pub(crate) const fn is_svelte_ws(c: char) -> bool {
    // ASCII is split out rather than folded into one `matches!` so the common path stays a
    // handful of compares instead of a decision tree over the full (mostly non-ASCII) member
    // set — these predicates run per character of every name in the document. The two arms
    // partition the set, and `matches_svelte_is_whitespace_at_every_code_point` grades the
    // whole predicate per code point, so a member landing in the wrong arm cannot hide.
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

/// The character at byte offset `i` in `source` and its UTF-8 width, or `None` at or past
/// the end. `i` must be a character boundary.
///
/// The shared scanning primitive behind the name-run and tag-close scans. It dispatches on
/// the raw byte and **decodes only on the non-ASCII branch** (an ASCII byte *is* its own
/// `char`), the way the lexer's own cursor does — these scans run per character of every
/// name in the document, where an unconditional decode is a measurable regression.
///
/// Returning the width is what keeps a caller on character boundaries: a multi-byte
/// character has to be stepped over whole, because the next call slices `source[i..]`.
/// Handing back the `char` rather than a bool lets each scan ask its own question of it, so
/// the byte-vs-char dispatch exists once instead of once per predicate.
#[inline]
pub(crate) fn char_at(source: &str, i: usize) -> Option<(char, usize)> {
    let b = *source.as_bytes().get(i)?;
    if b.is_ascii() {
        return Some((b as char, 1));
    }
    let c = source[i..].chars().next()?;
    Some((c, c.len_utf8()))
}

/// The UTF-8 width of the Svelte-whitespace character at byte offset `i` in `source`, or
/// `None` when `i` is at/past the end or the character there is not whitespace.
///
/// The scanning form of [`is_svelte_ws`], for the tag-close scans.
#[inline]
pub(crate) fn svelte_ws_width_at(source: &str, i: usize) -> Option<usize> {
    let (c, width) = char_at(source, i)?;
    is_svelte_ws(c).then_some(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Svelte's own `is_whitespace(cc)` (`1-parse/index.js`), transcribed. The predicate
    /// must agree with it at every code point — this is the drop-in contract, and the set is
    /// small enough to check exhaustively rather than by sampling.
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
    fn matches_svelte_is_whitespace_at_every_code_point() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            assert_eq!(
                is_svelte_ws(c),
                svelte_is_whitespace(cp),
                "U+{cp:04X} disagrees with Svelte's is_whitespace"
            );
        }
    }

    /// The two directions Rust's `char::is_whitespace()` gets wrong; each has its own
    /// fixture, and both would be silent if the predicate were swapped for the convenient one.
    #[test]
    fn differs_from_unicode_white_space_in_both_directions() {
        assert!(is_svelte_ws('\u{feff}') && !'\u{feff}'.is_whitespace());
        assert!(!is_svelte_ws('\u{85}') && '\u{85}'.is_whitespace());
    }

    /// `char_at`'s two branches must agree with a plain decode at every code point — the
    /// ASCII branch skips the decoder entirely, so a divergence there would mis-scan silently.
    #[test]
    fn char_at_matches_a_plain_decode_everywhere() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let s = c.to_string();
            assert_eq!(char_at(&s, 0), Some((c, c.len_utf8())), "U+{cp:04X}");
        }
        // a mid-string offset, and the two out-of-range forms
        assert_eq!(char_at("aé", 1), Some(('é', 2)));
        assert_eq!(char_at("a", 1), None);
        assert_eq!(char_at("", 0), None);
    }

    #[test]
    fn width_at_decodes_every_member_and_stops_on_non_whitespace() {
        for cp in 0..=0x10ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            let s = c.to_string();
            assert_eq!(
                svelte_ws_width_at(&s, 0),
                is_svelte_ws(c).then(|| c.len_utf8()),
                "U+{cp:04X}"
            );
        }
        assert_eq!(svelte_ws_width_at("a", 0), None);
        assert_eq!(svelte_ws_width_at("", 0), None);
        assert_eq!(svelte_ws_width_at("a", 9), None);
    }
}
