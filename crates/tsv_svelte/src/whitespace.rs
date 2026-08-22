// Svelte's tokenizer whitespace class

/// Whether `c` is whitespace to **Svelte's parser** — the class that separates tokens,
/// ends a tag-name or attribute-name run, and satisfies a block keyword's required space.
///
/// This is exactly JavaScript's `\s` ([`tsv_lang::is_js_whitespace`], which owns the set and
/// its exhaustive test), because every place Svelte asks the question is spelled in
/// JavaScript: `is_whitespace(cc)` (`1-parse/index.js`, backing `allow_whitespace` /
/// `require_whitespace`) enumerates these code points by hand, and the name-run regexes match
/// the same set through `\s` — `regex_whitespace_or_slash_or_closing_tag = /(\s|\/|>)/` for
/// tag names and `regex_token_ending_character = /[\s=/>"']/` for attribute and directive
/// names (both in `1-parse/state/element.js`), as do the raw-text closes `/<\/script\s*>/`,
/// `/<\/style\s*>/` and the RCDATA close `/<\/textarea(\s[^>]*)?>/iy`.
///
/// It keeps its own name here because the *question* is Svelte-specific — "does this end a
/// token in this parser" — while the shared definition answers only "is this JS `\s`". The
/// two ⚠️ traps (`char::is_whitespace()` differs in both directions; U+FEFF and U+0085 are
/// the witnesses) live with the definition.
///
/// ⚠️ Not [`is_collapsible_ws`](crate::ast::internal::is_collapsible_ws), the narrower render
/// class (`[ \t\n\r]`). That one answers "may a formatter add, drop or respell this without
/// changing what renders"; this one answers "does this end a token". A form feed is
/// whitespace here and rendered content there.
///
/// ⚠️ And **not** `tsv_ts`'s `is_es_whitespace`, which is the `WhiteSpace` production
/// *alone*: a JS lexer matches `LineTerminator` separately because a newline drives ASI,
/// so that predicate deliberately omits `<LF>`/`<CR>`/`<LS>`/`<PS>`. This class is the
/// union of both productions (that is what `\s` means), so it is a strict superset.
pub(crate) use tsv_lang::is_js_whitespace as is_svelte_ws;

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

/// Byte offset of the first character at/after `start` that `is_terminator` accepts, or the
/// end of `source` when none does. The name is `source[start..end]`.
///
/// The one scan behind both of Svelte's `read_tag` name runs — the tag-name class
/// (`parser/element.rs`) and the attribute/directive class (`parser/attribute.rs`) ask the
/// same question of the same cursor and differ **only** in that predicate, so the loop lives
/// here once and each class stays its own named predicate
/// (`is_tag_name_terminator` / `is_attr_name_terminator` — the two sets are genuinely
/// different, and collapsing them into one predicate would be the bug).
///
/// Generic over the predicate rather than taking a `fn` pointer so each caller monomorphizes
/// to its own inlined class test: this runs per character of every name in the document.
#[inline]
pub(crate) fn name_run_end(
    source: &str,
    start: usize,
    is_terminator: impl Fn(char) -> bool,
) -> usize {
    let mut end = start;
    while let Some((c, width)) = char_at(source, end) {
        if is_terminator(c) {
            break;
        }
        end += width;
    }
    end
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

/// Byte offset of the first non-whitespace character at/after `start`, or the end of `source`
/// when none follows.
///
/// Svelte's `allow_whitespace()` (`1-parse/index.js`) over a raw offset rather than the parser
/// cursor — for the scans that own their own cursor and only resync the lexer once at the end.
#[inline]
pub(crate) fn skip_svelte_ws(source: &str, start: usize) -> usize {
    let mut i = start;
    while let Some(width) = svelte_ws_width_at(source, i) {
        i += width;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

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
