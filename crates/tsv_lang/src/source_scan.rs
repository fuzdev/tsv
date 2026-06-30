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
/// Regex literals are deliberately **not** a profile option here: a `/…/` needs
/// previous-token lookback to tell it from division, which a stateless forward
/// `skip_trivia` can't carry as a flag. The disambiguation lives in the separate
/// [`is_regex_start`] / [`skip_regex_literal`] helpers below, which the
/// depth-tracking scanners that *do* sit at a regex boundary (the Svelte brace
/// matcher, the TS arrow-vs-paren lookahead) call alongside `skip_trivia`. A
/// plain inter-node delimiter scan never sits at a regex boundary in practice,
/// matching the historical `skip_string_or_comment`.
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
    // Hot path: almost every byte is significant, so reject anything that can't
    // open trivia with a cheap compare and keep this small enough to inline into
    // the per-byte finder loops. Only the four openers (`"` `'` `` ` `` `/`) can
    // begin a string/comment; their scans live in the `#[cold]`
    // `skip_trivia_scan` below, kept out of line so the rare branch can't bloat
    // the callers — the scan loops made the old single function too big to
    // inline, leaving its call/return overhead the bulk of its `perf` self-time.
    let b = bytes[i];
    if b != b'"' && b != b'\'' && b != b'`' && b != b'/' {
        return None;
    }
    skip_trivia_scan(bytes, i, end, profile, b)
}

/// Cold tail of [`skip_trivia`]: `bytes[i]` (passed as `b`) is one of the four
/// trivia openers. Scan past the string/comment it begins, or return `None` if
/// the active `profile` doesn't treat it as trivia (a `/` that isn't `//`/`/*`,
/// or a quote with `strings` disabled).
#[cold]
#[inline(never)]
fn skip_trivia_scan(
    bytes: &[u8],
    i: usize,
    end: usize,
    profile: TriviaProfile,
    b: u8,
) -> Option<usize> {
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
            // A line comment ends at any ECMAScript line terminator — LF, CR, or
            // the UTF-8 line/paragraph separators U+2028/U+2029 (`e2 80 a8`/`a9`)
            // — matching the lexer (a `\n`-only stop would run the comment past a
            // `\r`/U+2028 and swallow following code). The terminator is consumed
            // (it's whitespace for the next scan).
            let mut j = i + 2;
            while j < end {
                match bytes[j] {
                    b'\n' | b'\r' => return Some(j + 1),
                    0xe2 if j + 2 < end
                        && bytes[j + 1] == 0x80
                        && (bytes[j + 2] == 0xa8 || bytes[j + 2] == 0xa9) =>
                    {
                        return Some(j + 3);
                    }
                    _ => j += 1,
                }
            }
            return Some(end);
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

/// Whether `keyword` occurs at `i` as a **whole word** — present byte-for-byte
/// and not flanked by a JS/TS identifier byte (alphanumeric, `_`, or `$`), so
/// `export` does not match inside `exported` or `$export`. The boundary check is
/// against the full `bytes`, not any `[start, end)` window. Caller ensures `i +
/// keyword.len() <= bytes.len()`.
#[inline]
fn whole_word_at(bytes: &[u8], i: usize, keyword: &[u8]) -> bool {
    let kw_len = keyword.len();
    if &bytes[i..i + kw_len] != keyword {
        return false;
    }
    let before_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
    let after_ok = i + kw_len >= bytes.len() || !is_identifier_byte(bytes[i + kw_len]);
    before_ok && after_ok
}

/// Like [`whole_word_at`], but matching `keyword` ASCII-case-insensitively.
fn whole_word_at_ignore_ascii_case(bytes: &[u8], i: usize, keyword: &[u8]) -> bool {
    let kw_len = keyword.len();
    if !bytes[i..i + kw_len].eq_ignore_ascii_case(keyword) {
        return false;
    }
    let before_ok = i == 0 || !is_identifier_byte(bytes[i - 1]);
    let after_ok = i + kw_len >= bytes.len() || !is_identifier_byte(bytes[i + kw_len]);
    before_ok && after_ok
}

/// Whether `b` is an ASCII byte that can appear inside a JS/TS identifier —
/// alphanumeric, `_`, or `$`. Used for whole-word keyword boundaries.
#[inline]
fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Find the **first** whole-word occurrence of `keyword` in `bytes[start..end]`,
/// skipping trivia per `profile`. Returns the keyword's start position, or `None`.
///
/// The trivia skip is what makes this safe against a keyword that appears inside
/// a comment or string (e.g. `@dec /* class */ class C {}` finds the real
/// `class`, not the one in the comment).
#[inline]
pub fn find_keyword(
    bytes: &[u8],
    start: usize,
    end: usize,
    keyword: &[u8],
    profile: TriviaProfile,
) -> Option<usize> {
    let kw_len = keyword.len();
    let mut i = start;
    while i + kw_len <= end {
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        if whole_word_at(bytes, i, keyword) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Like [`find_keyword`], but matching the keyword **ASCII-case-insensitively**.
///
/// CSS grammar keywords (`and`/`or`/`not`/...) are ASCII case-insensitive (CSS
/// Syntax 3 §"tokenizing"), so a connector buried-comment-aware scan must match
/// `AND` as well as `and`. JS/TS keywords are case-sensitive — they use
/// [`find_keyword`]. Pass an already-lowercase `keyword`.
pub fn find_keyword_ascii_case_insensitive(
    bytes: &[u8],
    start: usize,
    end: usize,
    keyword: &[u8],
    profile: TriviaProfile,
) -> Option<usize> {
    let kw_len = keyword.len();
    let mut i = start;
    while i + kw_len <= end {
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        if whole_word_at_ignore_ascii_case(bytes, i, keyword) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the **last** whole-word occurrence of `keyword` in `bytes[start..end]`,
/// skipping trivia per `profile`. Returns its start position, or `None`.
///
/// The forward scan with skip-trivia gives the rightmost match that is **not**
/// inside a comment or string, so it both (a) skips a keyword buried in a
/// comment (`from /* from */ 'x'` finds the real `from`) and (b) prefers a later
/// real keyword over an earlier identifier that merely contains it (`import
/// { from } from 'x'` — the specifier `from` loses to the keyword). A plain
/// reverse `rfind` gets neither right.
#[inline]
pub fn rfind_keyword(
    bytes: &[u8],
    start: usize,
    end: usize,
    keyword: &[u8],
    profile: TriviaProfile,
) -> Option<usize> {
    let kw_len = keyword.len();
    let mut found = None;
    let mut i = start;
    while i + kw_len <= end {
        if let Some(past) = skip_trivia(bytes, i, end, profile) {
            i = past;
            continue;
        }
        if whole_word_at(bytes, i, keyword) {
            found = Some(i);
        }
        i += 1;
    }
    found
}

/// Whether the `/` at `slash_pos` starts a regex literal (rather than a division
/// operator). Decided by the previous significant byte, walking back to
/// `lower_bound`: a `/` after something that *ends* an expression (identifier
/// char, `)`, `]`) is division; after anything else (or at the start) it is a
/// regex.
///
/// This is the one piece of `/`-disambiguation the trivia cursor deliberately
/// leaves out of [`skip_trivia`]/[`TriviaProfile`]: it needs a *backward*
/// raw-byte walk, which a stateless forward scan can't honor as a flag. So it
/// lives here as a standalone helper that the depth-tracking scanners
/// (Svelte's brace matcher, the TS arrow-vs-paren lookahead) call alongside
/// `skip_trivia`, lower-bounding the walk at their own scan start.
#[inline]
pub fn is_regex_start(bytes: &[u8], slash_pos: usize, lower_bound: usize) -> bool {
    let mut j = slash_pos;
    while j > lower_bound {
        j -= 1;
        let b = bytes[j];
        if !b.is_ascii_whitespace() {
            // Bytes that END an expression — a `/` after these is DIVISION.
            return !(b.is_ascii_alphanumeric() || b == b'_' || b == b')' || b == b']');
        }
    }
    // Nothing significant before it (start of the scanned region) → regex.
    true
}

/// Skip past a regex literal whose opening `/` is at `start`, returning the
/// position just after the closing `/` and any trailing flags (bounded by
/// `end`). Backslash-escape aware, and aware that a `/` inside a `[…]`
/// character class is a literal, not the terminator. An unterminated literal
/// returns `end`.
///
/// Pairs with [`is_regex_start`] — the caller confirms the `/` is a regex
/// before skipping. Caller must ensure `start < end <= bytes.len()`.
#[inline]
pub fn skip_regex_literal(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut i = start + 1; // past the opening `/`
    while i < end {
        match bytes[i] {
            b'\\' if i + 1 < end => i += 2, // escape — skip the next byte
            b'/' => {
                // Closing `/`; consume trailing flags (ASCII lowercase).
                i += 1;
                while i < end && bytes[i].is_ascii_lowercase() {
                    i += 1;
                }
                return i;
            }
            b'[' => {
                // Character class — a `/` inside is literal; skip to `]`.
                i += 1;
                while i < end {
                    match bytes[i] {
                        b'\\' if i + 1 < end => i += 2,
                        b']' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            _ => i += 1,
        }
    }
    end
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
    fn skip_trivia_line_comment_stops_at_all_terminators() {
        // CR ends a line comment (not just LF) — past the `\r` is index 5.
        assert_eq!(skip_trivia(b"// x\ry", 0, 6, TriviaProfile::JS), Some(5));
        // U+2028 (e2 80 a8) ends a line comment — past its 3 bytes.
        let src = b"// x\xe2\x80\xa8y"; // `// x` + U+2028 + `y`
        assert_eq!(skip_trivia(src, 0, src.len(), TriviaProfile::JS), Some(7));
        // A delimiter after a CR-terminated line comment is then found, not
        // swallowed: the `,` at index 6 follows `// x\r`.
        assert_eq!(
            find_char(b"// x\r, y", 0, 8, b',', TriviaProfile::JS),
            Some(5)
        );
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

    #[test]
    fn find_keyword_skips_comments_and_respects_word_boundaries() {
        // The `export` inside the comment is skipped; the real one is found.
        let src = b"/* export */ export class C";
        assert_eq!(
            find_keyword(src, 0, src.len(), b"export", TriviaProfile::JS),
            Some(13)
        );
        // Whole-word only: `export` inside `exported` is not a match.
        let src = b"exported = 1";
        assert_eq!(
            find_keyword(src, 0, src.len(), b"export", TriviaProfile::JS),
            None
        );
        // `$` is an identifier byte, so a keyword flanked by it is not a word
        // (`$from`/`from$` are identifiers, not the `from` keyword).
        assert_eq!(
            find_keyword(b"$from x", 0, 7, b"from", TriviaProfile::JS),
            None
        );
        assert_eq!(
            find_keyword(b"from$ x", 0, 7, b"from", TriviaProfile::JS),
            None
        );
        // Plain match at a boundary.
        assert_eq!(
            find_keyword(b"a class C", 0, 9, b"class", TriviaProfile::JS),
            Some(2)
        );
        // A keyword inside a string is skipped under JS.
        let src = b"'class' class C";
        assert_eq!(
            find_keyword(src, 0, src.len(), b"class", TriviaProfile::JS),
            Some(8)
        );
    }

    #[test]
    fn find_keyword_ascii_case_insensitive_matches_mixed_case_and_skips_comments() {
        // Uppercase/mixed-case connector matches (CSS grammar keywords are
        // ASCII case-insensitive).
        let src = b"(a: b) AND (c: d)";
        assert_eq!(
            find_keyword_ascii_case_insensitive(src, 0, src.len(), b"and", TriviaProfile::CSS),
            Some(7)
        );
        // A connector buried in a comment is skipped; the real (uppercase) one
        // after it is found — the coupling that makes gap-comment splitting sound.
        let src = b"(a: b) /* and */ Or (c: d)";
        assert_eq!(
            find_keyword_ascii_case_insensitive(src, 0, src.len(), b"or", TriviaProfile::CSS),
            Some(17)
        );
        // Whole-word only: `and` inside `understand` is not a match.
        let src = b"understand";
        assert_eq!(
            find_keyword_ascii_case_insensitive(src, 0, src.len(), b"and", TriviaProfile::CSS),
            None
        );
    }

    #[test]
    fn rfind_keyword_skips_comments_and_prefers_the_real_keyword() {
        // `from /* from */ 'x'` — the real `from` (index 0), not the comment's.
        let src = b"from /* from */ 'x'";
        assert_eq!(
            rfind_keyword(src, 0, src.len(), b"from", TriviaProfile::COMMENTS),
            Some(0)
        );
        // `{ from } from` — the specifier `from` (index 2) loses to the keyword
        // `from` (index 9); rfind picks the later REAL one.
        let src = b"{ from } from";
        assert_eq!(
            rfind_keyword(src, 0, src.len(), b"from", TriviaProfile::COMMENTS),
            Some(9)
        );
        // A specifier `from`, the real `from`, then a comment `from`: real wins.
        let src = b"{ from } from /* from */";
        assert_eq!(
            rfind_keyword(src, 0, src.len(), b"from", TriviaProfile::COMMENTS),
            Some(9)
        );
        // Whole-word only.
        assert_eq!(
            rfind_keyword(b"fromage", 0, 7, b"from", TriviaProfile::COMMENTS),
            None
        );
    }

    #[test]
    fn is_regex_start_uses_previous_significant_byte() {
        // `= /re/` — `/` after `=` (and whitespace) is a regex.
        assert!(is_regex_start(b"a = /re/", 4, 0));
        // `a / b` — `/` after identifier `a` is division.
        assert!(!is_regex_start(b"a / b", 2, 0));
        // `) / b` — `/` after `)` is division; `] / b` likewise.
        assert!(!is_regex_start(b") / b", 2, 0));
        assert!(!is_regex_start(b"] / b", 2, 0));
        // At the lower bound (nothing significant before) → regex.
        assert!(is_regex_start(b"/re/", 0, 0));
        // The lower bound is honored: even though `(` precedes, a walk bounded
        // at the `/` itself sees nothing before it → regex.
        assert!(is_regex_start(b"(/re/", 1, 1));
    }

    #[test]
    fn skip_regex_literal_handles_escapes_classes_and_flags() {
        // Plain literal: past the closing `/`.
        let src = b"/re/ x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), 4);
        // Trailing flags are consumed.
        let src = b"/re/gi x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), 6);
        // Escaped `/` does not terminate.
        let src = br"/a\/b/ x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), 6);
        // A `/` inside a character class is literal, not the terminator.
        let src = b"/[/)]/ x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), 6);
        // Parens inside are opaque — the returned slice covers the whole literal.
        let src = br"/\)/ y";
        assert_eq!(skip_regex_literal(src, 0, src.len()), 4);
        // Unterminated → end.
        let src = b"/abc";
        assert_eq!(skip_regex_literal(src, 0, src.len()), src.len());
    }
}
