// Block-comment extent — the single definition of how far a CSS comment reaches.
//
// The comment twin of [`crate::escapes::escape_len`], and it exists for the same reason:
// that answers "how far does this escape reach", this answers it for `/* … */`, and a
// scanner that gets either wrong reads the enclosed text as structure. CSS Syntax 3
// §4.3.2 (*consume comments*) is the rule, and two clauses of it are load-bearing here —
// it **returns nothing** (a comment produces no token, so its interior can never
// contribute a separator or a nesting change), and it runs "up to and including the first
// U+002A ASTERISK (*) followed by a U+002F SOLIDUS (/), **or up to an EOF code point**"
// (so an unterminated comment reaches end-of-input rather than erroring here).
//
// Every scanner that walks a value, prelude, or declaration for *structure* steps over a
// comment through this module rather than re-spelling the rule. It had been re-spelled
// eight times across the crate — as `find("*/")`, as a `skip_block_comment` byte loop, and
// as three separate `in_comment` state machines — which is how the value scanners came to
// disagree with the printer about where a comment ended.

/// Whether a block comment opens at `i`.
///
/// Bounds-checked on both bytes, so a caller may probe the final byte of a slice.
pub(crate) fn is_comment_start(bytes: &[u8], i: usize) -> bool {
    bytes.get(i) == Some(&b'/') && bytes.get(i + 1) == Some(&b'*')
}

/// The end of the block comment opening at `i` — the byte just after its `*/` — or `None`
/// when it is **unterminated**.
///
/// Take this one only when the unterminated case needs its own handling: a caller that
/// preserves a malformed comment verbatim (`ast::convert::strip_css_comments`) or abandons
/// a reconstruction (`value_normalization::extract_property_name`) cannot use
/// [`comment_end`], whose `bytes.len()` answer is the same for "ran off the end" and for a
/// comment whose `*/` legitimately ends at the last byte.
pub(crate) fn comment_end_checked(bytes: &[u8], i: usize) -> Option<usize> {
    debug_assert!(
        is_comment_start(bytes, i),
        "comment_end must be called at a `/*`"
    );
    let mut j = i + 2;
    while j + 1 < bytes.len() {
        if bytes[j] == b'*' && bytes[j + 1] == b'/' {
            return Some(j + 2);
        }
        j += 1;
    }
    None
}

/// The end of the block comment opening at `i` — the byte just after its `*/`, or
/// `bytes.len()` when it is unterminated (§4.3.2's "or up to an EOF code point").
///
/// The right choice for a *structure* scanner, which only needs to not read the interior
/// as structure and has no error to report. [`comment_end_checked`] is for the callers
/// that must tell the two cases apart.
///
/// A byte loop rather than `find("*/")` so no scanner grows a raw substring scan over
/// source (`deno task scan:audit`).
pub(crate) fn comment_end(bytes: &[u8], i: usize) -> usize {
    comment_end_checked(bytes, i).unwrap_or(bytes.len())
}

/// The end of the block comment opening at the **start** of `s`, or `None` when none
/// does — the `&str` face of [`comment_end`], for a scanner that steps over a comment by
/// slicing rather than by byte index.
///
/// The end is a `*/` (or end-of-input) boundary and so always a char boundary, which is
/// what makes it safe to slice with.
pub(crate) fn leading_comment_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    is_comment_start(bytes, 0).then(|| comment_end(bytes, 0))
}

/// The end of the block-comment **run** opening at `i`, or `None` when no comment opens
/// there.
///
/// Consecutive comments are one run: §4.3.2 loops ("Return to the start of this step"), so
/// `/* c *//* d */` is a single stretch of no-token material.
pub(crate) fn comment_run_end(bytes: &[u8], i: usize) -> Option<usize> {
    if !is_comment_start(bytes, i) {
        return None;
    }
    let mut end = i;
    while is_comment_start(bytes, end) {
        end = comment_end(bytes, end);
    }
    Some(end)
}

/// The first offset in `[from, to)` that is neither whitespace nor inside a comment — where
/// the region's next real token begins — or `to` when the region is nothing but trivia.
///
/// The composition of this module's rule with the lexer's whitespace set, and the single
/// definition of it: every scanner that walks a *parsed* region looking for a token it has
/// no span for (the declaration scanner's value bounds, the printer's attribute-selector
/// rebuild) asks this rather than re-spelling the pair.
///
/// ⚠️ The whitespace set is the **lexer's** ([`crate::lexer::is_ascii_css_whitespace`], which carries the
/// vertical tab to match `parseCss`), not [`crate::escapes`]'s §4.2 five. That is forced:
/// the region was already tokenized, so a scanner stepping over "the whitespace the parser
/// skipped" must use the set the parser skipped. Narrowing it stops the scan ON a vertical
/// tab and mislocates the token behind it — which, in the attribute-selector rebuild, moved
/// a comment across the matcher (`[attr\u{b}/* c */='value']` → `[attr= /* c */ 'value']`).
///
/// An unterminated comment yields `to`, so the caller stops at the bound rather than reading
/// past it.
pub(crate) fn skip_trivia_forward(bytes: &[u8], from: usize, to: usize) -> usize {
    let mut i = from;
    while i < to {
        if crate::lexer::is_ascii_css_whitespace(bytes[i]) {
            i += 1;
        } else if is_comment_start(bytes, i) {
            match comment_end_checked(bytes, i) {
                Some(end) => i = end,
                None => return to,
            }
        } else {
            return i;
        }
    }
    to
}

/// Where a pseudo selector's NAME begins, given the span that starts at its first `:` —
/// past the colons and past any comment run glued to either of them.
///
/// selectors-4 forbids white space between a `<pseudo-class-selector>`'s / a
/// `<pseudo-element-selector>`'s components, but a comment is no token at all
/// (§4.3.2), so `:/* c */hover`, `::/* c */before` and `:/* c */:before` are all one
/// selector. Every consumer of the name — the printer's case fold, the wire's
/// half-decode, the scoping compiler's `:global` test — takes its start from here
/// rather than assuming the sigil is one or two bytes wide.
///
/// Each step is ANCHORED at a known position rather than searching, which is what keeps
/// it escape-proof: an escape can only begin where the name does, and `comment_run_end`
/// answers `None` on a `\` (`.\/*x*/y` is an ident, not a comment).
pub(crate) fn pseudo_name_start(bytes: &[u8], span_start: u32) -> u32 {
    let mut i = span_start as usize + 1; // past the first `:`
    i = comment_run_end(bytes, i).unwrap_or(i);
    if bytes.get(i) == Some(&b':') {
        i += 1;
        i = comment_run_end(bytes, i).unwrap_or(i);
    }
    i as u32
}

/// Where a class selector's NAME begins — past the `.` and any comment run glued to it.
///
/// The one-juncture sibling of [`pseudo_name_start`], for the same selectors-4 rule
/// ("between **any** of the components of a `<class-selector>`").
///
/// Only the wire writer needs it — the printer reaches the same juncture through
/// [`pseudo_name_start`] — so it is `convert`-gated: `@fuzdev/tsv_format_wasm` builds
/// without that feature, and an ungated item there is dead code in a size-bound artifact.
#[cfg(feature = "convert")]
pub(crate) fn class_name_start(bytes: &[u8], span_start: u32) -> u32 {
    let i = span_start as usize + 1; // past the `.`
    comment_run_end(bytes, i).unwrap_or(i) as u32
}

/// Advance past a declaration's post-value TAIL — whitespace, block comments, `!important` —
/// to its `;`/`}` terminator, returning that index (or `bytes.len()`).
///
/// Mirrors Svelte's `read_declaration`: `read_value` returns with the scan index AT the
/// terminator and the declaration's `end` is taken there — so trailing whitespace and
/// comments after the value (and after `!important`) sit inside the declaration extent.
/// Only whitespace, comments, and the `!important` tail can occur between the parsed
/// value's end and the terminator, so a flat byte walk is safe (no string/url content).
///
/// ⚠️ **That safety argument is the whole contract — do not point this at a region
/// that can hold content.** It is blind to strings and `url()`, unlike Svelte's own
/// `read_value`, so over an at-rule PRELUDE it stops at the first `;`/`}` inside one
/// (`@import url("a;b.css") /* c */;`) and silently truncates. The prelude's ends come from
/// the parser instead (`write.rs`'s `collect_prelude_comments`).
///
/// Its two callers are the same question from the two sides of the crate — the wire writer's
/// declaration extent and the printer's `end_after_semicolon`, which needs the gap AFTER a
/// declaration to start where the declaration's text really stops. That is why it lives here
/// rather than in `ast::convert`: the printer cannot see that module (it is behind the
/// `convert` feature, off in the format-only WASM build), and the alternative was a second
/// spelling of a scan whose whole value is that there is one. ⚠️ The printer's INTERNAL
/// declaration span can end before `!important` where the wire's does not, so a caller that
/// assumes the span already reaches the `;` is right for every shape but that one — which is
/// how a positional blank-line rule came to read `!important;` as the start of its gap and
/// drop the author's blank line.
pub(crate) fn scan_to_terminator(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b';' | b'}' => break,
            b'/' if is_comment_start(bytes, i) => {
                i = comment_end(bytes, i);
            }
            _ => i += 1,
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_trivia_forward_uses_the_lexers_whitespace_set() {
        // The vertical tab is whitespace to the lexer (parseCss parity) but not to
        // `escapes`'s §4.2 set — a scan over a tokenized region must use the former, or it
        // stops on the VT and reports it as the next token's start.
        let src = b"attr/* c */='value'";
        assert_eq!(skip_trivia_forward(src, 4, src.len()), 12);
        assert_eq!(&src[12..13], b"=");
        // The rest of the set, a glued run, and a trivia-only region.
        assert_eq!(skip_trivia_forward(b"\t\n\r\x0c x", 0, 6), 5);
        assert_eq!(skip_trivia_forward(b"/* a *//* b */x", 0, 15), 14);
        assert_eq!(skip_trivia_forward(b"  /* a */ ", 0, 10), 10);
        // An unterminated comment stops at the bound rather than running past it.
        assert_eq!(skip_trivia_forward(b"/* a", 0, 4), 4);
    }

    #[test]
    fn comment_end_spans_the_whole_comment() {
        assert_eq!(comment_end(b"/* c */x", 0), 7);
        assert_eq!(comment_end(b"a/* c */", 1), 8);
        // The shortest comment, and the shortest that is NOT one.
        assert_eq!(comment_end(b"/**/", 0), 4);
        assert_eq!(comment_end(b"/*/", 0), 3);
        assert_eq!(comment_end_checked(b"/*/", 0), None);
        // A `*/` inside the body does not end it early — the FIRST one does.
        assert_eq!(comment_end(b"/* a */ b */", 0), 7);
    }

    #[test]
    fn unterminated_reaches_end_of_input() {
        // §4.3.2's "or up to an EOF code point": the structure scanner's answer is the end
        // of input, and only `comment_end_checked` distinguishes it from a real close.
        assert_eq!(comment_end(b"/* c", 0), 4);
        assert_eq!(comment_end_checked(b"/* c", 0), None);
        // A comment whose `*/` lands exactly on the last byte is NOT unterminated, and is
        // the case that makes the two functions differ in kind rather than in value.
        assert_eq!(comment_end(b"/* c */", 0), 7);
        assert_eq!(comment_end_checked(b"/* c */", 0), Some(7));
    }

    #[test]
    fn leading_comment_end_is_the_str_face_of_comment_end() {
        assert_eq!(leading_comment_end("/* c */x"), Some(7));
        assert_eq!(
            leading_comment_end("x/* c */"),
            None,
            "must open AT the start"
        );
        assert_eq!(leading_comment_end(""), None);
        // §4.3.2's EOF clause, so a slicing caller consumes the rest rather than looping.
        assert_eq!(leading_comment_end("/* c"), Some(4));
        // Multibyte interiors: the returned end is a `*/` boundary, always sliceable.
        let s = "/* ünïcödé */2n";
        assert_eq!(&s[..leading_comment_end(s).unwrap()], "/* ünïcödé */");
    }

    #[test]
    fn run_end_folds_consecutive_comments() {
        assert_eq!(comment_run_end(b"/* c *//* d */x", 0), Some(14));
        assert_eq!(
            comment_run_end(b"/* c */ /* d */", 0),
            Some(7),
            "a gap ends the run"
        );
        assert_eq!(comment_run_end(b"x/* c */", 0), None);
        assert_eq!(comment_run_end(b"", 0), None);
        // An unterminated comment ends the run at end-of-input rather than looping.
        assert_eq!(comment_run_end(b"/* c *//* d", 0), Some(11));
    }

    #[test]
    fn is_comment_start_is_bounds_checked_on_both_bytes() {
        assert!(is_comment_start(b"/*", 0));
        assert!(!is_comment_start(b"/", 0));
        assert!(!is_comment_start(b"", 0));
        assert!(!is_comment_start(b"/*", 5));
        assert!(!is_comment_start(b"*/", 0));
    }
}
