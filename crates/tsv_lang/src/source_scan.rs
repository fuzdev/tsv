// Source-scanning utilities: locate syntactic delimiters in raw source while
// skipping the trivia (comments and string literals) that can contain a matching
// glyph, so a `,`/`:`/`*`/bracket inside a comment or string is never mistaken
// for the real token.
//
// `skip_trivia` is the single chokepoint. Given a position, if it starts a
// comment or string (per `TriviaProfile`), it returns the position just past
// that span; otherwise `None` — the byte is significant. Every delimiter scan is
// the same loop over `skip_trivia` (find a target, track bracket depth, match a
// keyword), so the escape/comment handling lives in exactly one place. `find_char`
// here is the common single-byte case; the depth-tracking and keyword scanners in
// the language printers inline the loop with their own per-byte logic.
//
// Used by the AST conversion layer (acorn comment duplication) and the printers.

/// Which trivia kinds a scan skips over.
///
/// Languages differ. JS/TS have `//` line comments, `/* */` block comments, and
/// `'`/`"`/`` ` `` string and template literals. CSS has only block comments and
/// strings — a `//` is *not* a comment there (`url(http://…)`), so `line_comments`
/// is off, which keeps a JS-shaped cursor from mis-reading CSS.
///
/// Regex literals are deliberately **not** a profile option here: only the Svelte
/// brace matcher needs `/…/` disambiguation (it requires previous-token lookback),
/// and it carries that logic itself. The inter-node delimiter scans never sit at a
/// regex boundary in practice, matching the historical `skip_string_or_comment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriviaProfile {
    /// `//` to end of line (the newline is consumed as part of the span).
    pub line_comments: bool,
    /// `/* */` block comments.
    pub block_comments: bool,
    /// `'`/`"`/`` ` `` string and template literals, backslash-escape aware.
    /// A template `${…}` is treated as opaque string content (no interpolation
    /// recursion) — matching every existing scanner.
    pub strings: bool,
}

impl TriviaProfile {
    /// Line + block comments, no strings — the classic `find_char_skipping_comments`
    /// behavior. Delimiters between AST nodes never sit inside a string, so the
    /// printer's inter-node gap scans historically skipped only comments.
    pub const COMMENTS: Self = Self {
        line_comments: true,
        block_comments: true,
        strings: false,
    };

    /// JS/TS: line + block comments + strings. Equivalent to the former
    /// `tsv_ts::printer::analysis::skip_string_or_comment`.
    pub const JS: Self = Self {
        line_comments: true,
        block_comments: true,
        strings: true,
    };

    /// CSS: block comments + strings only (no `//`).
    pub const CSS: Self = Self {
        line_comments: false,
        block_comments: true,
        strings: true,
    };
}

/// If `bytes[i]` begins a trivia span (a comment or string per `profile`), return
/// the position just past it; otherwise `None` — the byte is significant.
///
/// An unterminated span (a string or block comment with no close before `end`)
/// returns `end`, so the enclosing scan stops without reading past the bound.
///
/// Callers must ensure `i < end <= bytes.len()`.
#[inline]
pub fn skip_trivia(bytes: &[u8], i: usize, end: usize, profile: TriviaProfile) -> Option<usize> {
    let b = bytes[i];

    // Strings / templates (braces, commas, etc. inside are not significant).
    if profile.strings && (b == b'"' || b == b'\'' || b == b'`') {
        let quote = b;
        let mut j = i + 1;
        while j < end && bytes[j] != quote {
            if bytes[j] == b'\\' {
                j += 1;
            }
            j += 1;
        }
        // `j` is at the closing quote (or past `end` if unterminated); skip past it.
        return Some((j + 1).min(end));
    }

    if b == b'/' && i + 1 < end {
        if profile.line_comments && bytes[i + 1] == b'/' {
            let mut j = i + 2;
            while j < end && bytes[j] != b'\n' {
                j += 1;
            }
            // Consume the terminating newline (whitespace) too, when present.
            return Some((j + 1).min(end));
        }
        if profile.block_comments && bytes[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < end && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            // Skip past the closing `*/`, or to `end` if unterminated.
            return Some(if j + 1 < end { j + 2 } else { end });
        }
    }

    None
}

/// Find the first occurrence of `target` in `bytes[start..end]`, skipping trivia
/// per `profile`. Returns the byte's position, or `None` if not found.
///
/// `target` must not itself be a trivia-introducing byte (`/`, `'`, `"`, `` ` ``)
/// — those are consumed as trivia and would never match.
#[inline]
pub fn find_char(
    bytes: &[u8],
    start: usize,
    end: usize,
    target: u8,
    profile: TriviaProfile,
) -> Option<usize> {
    let mut i = start;
    while i < end {
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        if bytes[i] == target {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Skip over a comment (line or block) starting at position `i`.
///
/// Returns `Some(new_i)` where `new_i` is the position AFTER the comment (ready
/// for the next iteration), or `None` if not at a comment. Unlike `skip_trivia`,
/// a line comment stops AT the terminating newline (not past it) — this exact
/// convention is relied on by the AST comment-attachment position math, so it is
/// kept distinct.
pub fn skip_comment(bytes: &[u8], i: usize, end: usize) -> Option<usize> {
    if i + 1 >= end || bytes[i] != b'/' {
        return None;
    }
    if bytes[i + 1] == b'/' {
        // Line comment - skip to end of line
        let mut j = i + 2;
        while j < end && bytes[j] != b'\n' {
            j += 1;
        }
        Some(j)
    } else if bytes[i + 1] == b'*' {
        // Block comment - skip to */
        let mut j = i + 2;
        while j + 1 < end && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
            j += 1;
        }
        Some(j + 2) // Past the */
    } else {
        None
    }
}

/// Find the first occurrence of a byte in source between `start` and `end`, skipping comments.
///
/// Returns the position of the byte, or `None` if not found. Thin wrapper over
/// `find_char` with the comments-only profile.
#[inline]
pub fn find_char_skipping_comments(
    bytes: &[u8],
    start: usize,
    end: usize,
    target: u8,
) -> Option<usize> {
    find_char(bytes, start, end, target, TriviaProfile::COMMENTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(src: &str, target: u8, profile: TriviaProfile) -> Option<usize> {
        find_char(src.as_bytes(), 0, src.len(), target, profile)
    }

    #[test]
    fn find_char_plain() {
        assert_eq!(find("a, b", b',', TriviaProfile::JS), Some(1));
        assert_eq!(find("abc", b',', TriviaProfile::JS), None);
    }

    #[test]
    fn skips_comma_inside_block_comment() {
        // The `,` at index 5 is inside `/* , */`; the real delimiter is at 10.
        assert_eq!(find("a /* , */ , b", b',', TriviaProfile::JS), Some(10));
    }

    #[test]
    fn skips_comma_inside_line_comment() {
        // `// , ` runs to the newline; the real comma follows it.
        assert_eq!(find("a // , \n , b", b',', TriviaProfile::JS), Some(9));
    }

    #[test]
    fn skips_comma_inside_string() {
        // `','` is a string literal under the JS profile; real comma at 6.
        assert_eq!(find("a ',' , b", b',', TriviaProfile::JS), Some(6));
    }

    #[test]
    fn string_escape_does_not_end_string_early() {
        // `'\,'` — the backslash consumes the comma at index 2, so it is NOT the
        // delimiter; the real comma is at index 5.
        let src = r"'\,' , x";
        assert_eq!(find(src, b',', TriviaProfile::JS), Some(5));
    }

    #[test]
    fn comments_profile_does_not_skip_strings() {
        // Under COMMENTS, a quote is just a significant byte, so a comma inside
        // what JS would treat as a string IS found (index 1)...
        assert_eq!(find("',',x", b',', TriviaProfile::COMMENTS), Some(1));
        // ...whereas JS skips the string and finds the comma after it (index 3).
        assert_eq!(find("',',x", b',', TriviaProfile::JS), Some(3));
    }

    #[test]
    fn css_profile_does_not_treat_double_slash_as_comment() {
        // CSS has no `//` line comments (`url(http://…)`). Under CSS the `;` after
        // `//c` is reached at index 6...
        assert_eq!(find("a:b//c;d", b';', TriviaProfile::CSS), Some(6));
        // ...but under JS the `//c;d` is a line comment, swallowing the `;`.
        assert_eq!(find("a:b//c;d", b';', TriviaProfile::JS), None);
    }

    #[test]
    fn css_profile_skips_block_comment_and_string() {
        // The CSS property-colon case: `:` inside `/*;*/` is not the delimiter.
        assert_eq!(find("a/*;*/:b", b':', TriviaProfile::CSS), Some(6));
        // A `:` inside a string is likewise skipped.
        assert_eq!(find("a':':b", b':', TriviaProfile::CSS), Some(4));
    }

    #[test]
    fn assertion_close_angle_skips_comment() {
        // `<T /* > */>x` — the `>` inside the comment is skipped; real `>` at 10.
        assert_eq!(find("<T /* > */>x", b'>', TriviaProfile::JS), Some(10));
    }

    #[test]
    fn unterminated_trivia_does_not_panic_and_finds_nothing() {
        assert_eq!(find("a /* b", b',', TriviaProfile::JS), None); // open block comment
        assert_eq!(find("a 'bc", b',', TriviaProfile::JS), None); // open string
        assert_eq!(find("a /* , ", b',', TriviaProfile::JS), None); // comma trapped in open comment
    }

    #[test]
    fn skip_trivia_returns_position_past_span() {
        // Block comment `/* x */` at 0..7 → past the `*/` is index 7.
        assert_eq!(skip_trivia(b"/* x */ y", 0, 9, TriviaProfile::JS), Some(7));
        // String `'ab'` at 0..4 → past the closing quote is index 4.
        assert_eq!(skip_trivia(b"'ab' c", 0, 6, TriviaProfile::JS), Some(4));
        // Line comment consumes the newline too.
        assert_eq!(skip_trivia(b"// x\ny", 0, 6, TriviaProfile::JS), Some(5));
        // A non-trivia byte (and a `/` that is division, not a comment) → None.
        assert_eq!(skip_trivia(b"a, b", 0, 4, TriviaProfile::JS), None);
        assert_eq!(skip_trivia(b"a/b", 1, 3, TriviaProfile::JS), None);
    }

    #[test]
    fn skip_comment_keeps_its_distinct_conventions() {
        // Block comment: position PAST the closing `*/` (index 7).
        assert_eq!(skip_comment(b"/* x */ y", 0, 9), Some(7));
        // Line comment: stops AT the newline (index 4), not past it — relied on
        // by the AST comment-attachment position math.
        assert_eq!(skip_comment(b"// x\ny", 0, 6), Some(4));
        // Not a comment.
        assert_eq!(skip_comment(b"a/b", 0, 3), None);
        assert_eq!(skip_comment(b"/x", 0, 2), None);
    }

    #[test]
    fn find_char_skipping_comments_skips_comments_not_strings() {
        // Comment-borne comma skipped...
        assert_eq!(
            find_char_skipping_comments(b"a /* , */ , b", 0, 13, b','),
            Some(10)
        );
        // ...but a string-borne comma is found (strings are not trivia here).
        assert_eq!(find_char_skipping_comments(b"',',x", 0, 5, b','), Some(1));
    }
}
