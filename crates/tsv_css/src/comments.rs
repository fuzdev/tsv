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

#[cfg(test)]
mod tests {
    use super::*;

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
