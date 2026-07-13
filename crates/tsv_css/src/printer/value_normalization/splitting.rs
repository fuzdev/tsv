// Value splitting and whitespace normalization: top-level splits that respect
// parens/quotes/comments, plus prettier-style whitespace collapsing.

use std::borrow::Cow;

use crate::whitespace::is_css_whitespace;

/// Normalize CSS whitespace in extracted source text
///
/// Single-pass normalization that:
/// - Collapses consecutive whitespace to single spaces
/// - Removes spaces after opening parentheses: `( expr` → `(expr`
/// - Removes spaces before closing parentheses: `expr )` → `expr)`
/// - Preserves content inside quoted strings (`'...'` and `"..."`)
/// - Preserves content inside CSS comments (`/* ... */`)
///
/// This matches Prettier's normalization for calc(), var(), and other CSS functions.
///
/// # Example
/// ```ignore
/// assert_eq!(
///     normalize_css_whitespace("10px  /* test */  20px"),
///     "10px /* test */ 20px"
/// );
/// assert_eq!(
///     normalize_css_whitespace("var( --a, /* comment */ red )"),
///     "var(--a, /* comment */ red)"
/// );
/// assert_eq!(
///     normalize_css_whitespace("url( 'path with spaces' )"),
///     "url('path with spaces')"
/// );
/// ```
pub(crate) fn normalize_css_whitespace(s: &str) -> Cow<'_, str> {
    // Fast path: input with no byte the normalizer acts on normalizes to itself.
    // With no ASCII whitespace (to collapse), no `(`/`)` (paren-space stripping),
    // no `,` (comma spacing), no `/` (comment spacing) and no quote (string
    // handling), every char takes the loop's regular-character branch, so
    // `pending_space` never sets and the final `trim()` is a no-op — the output
    // equals the input. Return the source slice *borrowed* — no allocation, no
    // char scan, no per-char push, no trim; a caller holding the value's span can
    // then emit it as a zero-allocation `DocText::SourceSpan` (see
    // `build_identifier_doc`). The overwhelmingly-common `CssValue::Identifier`
    // value (`red`, `flex`, `1px`) hits this. Non-ASCII bytes still bail to the
    // slow path (conservative), but there they are preserved: CSS whitespace is
    // ASCII-only, so NBSP/U+2028/… are value content, not separators to collapse.
    if s.bytes().all(is_normalize_noop_byte) {
        return Cow::Borrowed(s);
    }

    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut in_comment = false;
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        // Check for comment start (outside strings)
        if !in_string && !in_comment && ch == '/' && chars.peek() == Some(&'*') {
            // Add space before comment if preceded by non-whitespace (except `(`)
            // This normalizes `foo,/*` → `foo, /*`
            if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            result.push('/');
            chars.next(); // consume '*'
            result.push('*');
            in_comment = true;
            continue;
        }

        // Check for comment end
        if in_comment && ch == '*' && chars.peek() == Some(&'/') {
            result.push('*');
            chars.next(); // consume '/'
            result.push('/');
            in_comment = false;
            pending_space = true; // Space needed before next token
            continue;
        }

        // Inside comment - preserve everything
        if in_comment {
            result.push(ch);
            continue;
        }

        // String delimiter handling (outside comments)
        if !in_string && (ch == '\'' || ch == '"') {
            if pending_space && !result.is_empty() && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            in_string = true;
            string_delim = ch;
            result.push(ch);
            continue;
        }

        if in_string && ch == string_delim {
            in_string = false;
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Inside string - preserve everything
        if in_string {
            result.push(ch);
            continue;
        }

        // A CSS escape (`\` + code point). Its payload is value **content**, never a
        // separator — `\` followed by whitespace is a valid escape whose escaped code
        // point IS that whitespace (CSS Syntax 3 §4.3.4 / §4.3.7). Copy the pair
        // verbatim so it can't reach the whitespace branch below: collapsing or
        // trimming the payload strands the backslash, which then escapes the next
        // delimiter (`;`, `)`, `!`) and the output no longer parses.
        if ch == '\\' {
            if pending_space && !result.is_empty() {
                result.push(' ');
            }
            pending_space = false;
            result.push(ch);
            if let Some(escaped) = chars.next() {
                result.push(escaped);
            }
            continue;
        }

        // Opening paren - skip following whitespace
        if ch == '(' {
            if pending_space && !result.is_empty() {
                result.push(' ');
            }
            pending_space = false;
            result.push(ch);
            // Skip whitespace after opening paren (ASCII only — NBSP and other
            // Unicode whitespace are value content, not separators).
            while chars.peek().is_some_and(|&c| is_css_whitespace(c)) {
                chars.next();
            }
            continue;
        }

        // Closing paren - remove trailing whitespace
        if ch == ')' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Comma - no space before, single space after (CSS never wants a space
        // before a comma, e.g. a media-query list `projection, tv`).
        if ch == ',' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = true;
            continue;
        }

        // Whitespace - mark pending (collapse consecutive). ASCII-only: NBSP and
        // other Unicode whitespace fall through to the regular-character branch
        // and are preserved verbatim as value content.
        if is_css_whitespace(ch) {
            if !result.is_empty() && !result.ends_with('(') {
                pending_space = true;
            }
            continue;
        }

        // Regular character - add pending space if needed
        if pending_space && !result.is_empty() {
            result.push(' ');
            pending_space = false;
        }
        result.push(ch);
    }

    // The char-loop above already suppresses leading/trailing whitespace in the
    // common case, so the trim is usually a no-op; return the owned buffer as-is
    // rather than cloning it. Only the rare edge cases that preserve verbatim
    // boundary whitespace (e.g. an unterminated string/comment) pay the copy —
    // byte-identical either way. The trailing trim spares an escape's payload
    // (`50px\ `), which the loop copied verbatim as content.
    let trimmed = crate::escapes::trim_end_preserving_escape(result.trim_start());
    if trimmed.len() == result.len() {
        Cow::Owned(result)
    } else {
        Cow::Owned(trimmed.to_string())
    }
}

/// Collapse each maximal whitespace run (spaces, tabs, and line breaks alike) to a
/// single space — the whitespace normalization tsv applies at every other
/// selector-argument position, and what prettier applies inside a selector.
/// String- and comment-aware: whitespace inside a `'…'`/`"…"` string (`\` escapes
/// honored) or a `/* … */` comment is content and stays verbatim.
///
/// Used for a dropped forgiving-selector item (`SimpleSelector::Invalid` — e.g.
/// `:is(.a > . > .b)`, whose empty `.` class makes the complex selector invalid):
/// the item is kept in the output verbatim except for this whitespace collapse, so
/// it renders on one line and matches prettier (see conformance_svelte.md §CSS
/// Corrections). Idempotent by construction — the collapsed form is a fixed point.
pub(crate) fn collapse_whitespace_runs(s: &str) -> Cow<'_, str> {
    // Fast path: nothing to collapse — no run needs rewriting when every whitespace
    // byte is already a lone ASCII space with non-whitespace on both sides (the
    // common single-spaced form). Whitespace inside a `'…'`/`/*…*/` interior only
    // trips this into the (correct, interior-preserving) slow path — a harmless
    // extra allocation, never a wrong answer.
    if !needs_whitespace_collapse(s) {
        return Cow::Borrowed(s);
    }

    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut in_comment = false;

    while let Some(ch) = chars.next() {
        // Comment interior: copy verbatim (its whitespace is content).
        if in_comment {
            result.push(ch);
            if ch == '*' && chars.peek() == Some(&'/') {
                result.push('/');
                chars.next();
                in_comment = false;
            }
            continue;
        }
        // String interior: copy verbatim, honoring `\` escapes so an escaped quote
        // (or a `\`-continuation newline) doesn't end the string early.
        if in_string {
            result.push(ch);
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    result.push(next);
                }
            } else if ch == string_delim {
                in_string = false;
            }
            continue;
        }
        // Comment / string starts.
        if ch == '/' && chars.peek() == Some(&'*') {
            result.push('/');
            result.push('*');
            chars.next();
            in_comment = true;
            continue;
        }
        if ch == '\'' || ch == '"' {
            in_string = true;
            string_delim = ch;
            result.push(ch);
            continue;
        }
        // A whitespace run outside strings/comments: consume it whole and emit one
        // space in its place.
        if is_css_whitespace(ch) {
            while chars.peek().is_some_and(|&next| is_css_whitespace(next)) {
                chars.next();
            }
            result.push(' ');
            continue;
        }
        result.push(ch);
    }

    Cow::Owned(result)
}

/// Whether `collapse_whitespace_runs` would rewrite `s`: true if any whitespace
/// byte is not a lone ASCII space (a tab/newline/CR/FF, or a space adjacent to
/// more whitespace). A conservative superset — it counts whitespace inside a
/// string/comment interior too, so a true answer there just routes to the slow
/// path (which preserves the interior); it never returns false when a collapse is
/// actually needed.
fn needs_whitespace_collapse(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.iter().enumerate().any(|(i, &b)| {
        if b == b' ' {
            bytes.get(i + 1) == Some(&b' ')
        } else {
            b.is_ascii_whitespace()
        }
    })
}

/// Whether `normalize_css_whitespace` leaves byte `b` untouched: an ASCII,
/// non-whitespace byte that is none of the structural bytes the normalizer acts on
/// (`(` `)` `,` `/` and the two quotes). A string composed only of such bytes
/// normalizes to itself, enabling the scan-skip fast path. Non-ASCII bytes (≥0x80)
/// are conservatively treated as "acted on" so they bail to the slow path — where
/// they are preserved (CSS whitespace is ASCII-only, so Unicode whitespace is
/// content). The acted-on ASCII whitespace set (`\t \n \x0C \r` space) is exactly
/// `char::is_ascii_whitespace` / `is_css_whitespace` — note U+000B (VT) is *not*
/// CSS whitespace, so it is a no-op byte the normalizer preserves.
#[inline]
fn is_normalize_noop_byte(b: u8) -> bool {
    b.is_ascii()
        && !matches!(
            b,
            b'\t' | b'\n' | 0x0C | b'\r' | b' ' | b'(' | b')' | b',' | b'/' | b'\'' | b'"'
        )
}

/// Extract the content between a function's parentheses from source
///
/// Given source like `property: func_name(arg1, arg2)` and func_name `func_name`,
/// returns `Some("arg1, arg2")`. Returns `None` if the function can't be found.
pub(crate) fn extract_function_args<'a>(source: &'a str, func_name: &str) -> Option<&'a str> {
    let func_start = source.find(func_name)?;
    let after_name = &source[func_start + func_name.len()..];
    let open_paren = after_name.find('(')?;

    let inner_start = func_start + func_name.len() + open_paren + 1;
    let inner_content = &source[inner_start..];

    // Find closing paren (handle nested parens)
    let mut depth = 1;
    for (i, c) in inner_content.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&inner_content[..i]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Split by top-level spaces, preserving content inside parentheses, quotes, and comments
///
/// Used for space-separated values like `var(--b) color-mix(...)`.
/// Returns individual values that can be wrapped independently.
pub(crate) fn split_by_space_preserving_parens(content: &str) -> Vec<&str> {
    split_top_level(content, |b| b == b' ' || b == b'\t', true)
}

/// Split function arguments by top-level commas, preserving nested parens, quotes, and comments
///
/// Used when extracting function arguments from source while preserving comments.
/// Handles nested parentheses correctly so `func(a, b)` inside an arg isn't split.
/// Skips over block comments so commas inside `/* a, b */` aren't treated as separators.
/// Skips over quoted strings so commas inside `"a, b"` aren't treated as separators.
pub(crate) fn split_args_by_comma(content: &str) -> Vec<&str> {
    split_top_level(content, |b| b == b',', false)
}

/// Split `content` at top-level bytes matching `is_sep`, preserving content inside
/// parentheses, quotes (`'`/`"`), and block comments (`/* */`).
///
/// The shared scanner behind `split_by_space_preserving_parens` and
/// `split_args_by_comma`: a byte state machine tracking paren depth, quote state,
/// and comment state, splitting only on a separator byte found at depth 0 outside
/// quotes/comments. `is_sep` selects the separator(s); `trim` selects the emit
/// policy via `push_segment` — `true` trims each segment and drops empties
/// (whitespace-value splitting), `false` keeps segments raw including empties
/// (comma-arg splitting).
fn split_top_level(content: &str, is_sep: impl Fn(u8) -> bool, trim: bool) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;
    let mut in_comment = false;
    let mut in_quote = false;
    let mut quote_char = b'\0';
    let bytes = content.as_bytes();

    let mut i = 0;
    while i < content.len() {
        // Check for comment start (outside quotes)
        if !in_quote
            && !in_comment
            && i + 1 < content.len()
            && bytes[i] == b'/'
            && bytes[i + 1] == b'*'
        {
            in_comment = true;
            i += 2;
            continue;
        }
        // Check for comment end
        if in_comment && i + 1 < content.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }
        if in_comment {
            i += 1;
            continue;
        }

        // Handle quotes
        if !in_quote && (bytes[i] == b'\'' || bytes[i] == b'"') {
            in_quote = true;
            quote_char = bytes[i];
            i += 1;
            continue;
        }
        if in_quote && bytes[i] == quote_char {
            in_quote = false;
            i += 1;
            continue;
        }
        if in_quote {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b if depth == 0 && is_sep(b) => {
                push_segment(&mut parts, &content[start..i], trim);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Don't forget the last part
    if start < content.len() {
        push_segment(&mut parts, &content[start..], trim);
    }

    parts
}

/// Emit one segment under the active policy: when `trim`, trim it and skip if empty;
/// otherwise push it verbatim (including empty segments).
fn push_segment<'a>(parts: &mut Vec<&'a str>, segment: &'a str, trim: bool) {
    if trim {
        let trimmed = segment.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    } else {
        parts.push(segment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_css_whitespace_fast_path_equivalence() {
        // Bare ASCII tokens (the fast path) are returned verbatim.
        for s in [
            "red", "1px", "#fff", "flex", "inherit", "-0.5em", "bold!", "a-b_c",
        ] {
            assert_eq!(
                normalize_css_whitespace(s),
                s,
                "bare token {s:?} must pass through"
            );
        }
        assert_eq!(normalize_css_whitespace(""), "");

        // Inputs the normalizer acts on still take the slow path and normalize.
        assert_eq!(normalize_css_whitespace("a  b"), "a b"); // whitespace collapse
        assert_eq!(normalize_css_whitespace("(  a  )"), "(a)"); // paren spacing
        assert_eq!(normalize_css_whitespace("a , b"), "a, b"); // comma spacing
        assert_eq!(normalize_css_whitespace("a,b"), "a, b");
        assert_eq!(normalize_css_whitespace(" red "), "red"); // trim
        assert_eq!(normalize_css_whitespace("a\tb"), "a b"); // tab is whitespace

        // U+000B (VT) is Unicode whitespace but *not* CSS whitespace, so it is
        // preserved as value content rather than collapsed to a space. Prettier
        // agrees (it keeps the VT inside the token), so this is a match, not a
        // divergence.
        assert_eq!(normalize_css_whitespace("u\u{000B}v"), "u\u{000B}v"); // VT preserved
        assert_eq!(
            normalize_css_whitespace("u\u{000B}\u{000B}v"),
            "u\u{000B}\u{000B}v"
        ); // not collapsed

        // Non-ASCII bails to the slow path (conservative), where it is preserved:
        // CSS whitespace is ASCII-only, so NBSP and other Unicode whitespace are
        // value content, not separators to collapse.
        assert_eq!(normalize_css_whitespace("a\u{00A0}b"), "a\u{00A0}b"); // NBSP preserved
        assert_eq!(
            normalize_css_whitespace("a\u{00A0}\u{00A0}b"),
            "a\u{00A0}\u{00A0}b"
        ); // not collapsed
        assert_eq!(normalize_css_whitespace("émotion"), "émotion"); // non-ws non-ASCII verbatim
    }

    #[test]
    fn test_collapse_whitespace_runs() {
        // Every whitespace run — spaces, tabs, newlines, and mixed — collapses to a
        // single space. The newline-free multi-space case is the one a newline-only
        // collapse used to leave verbatim, diverging from prettier.
        assert_eq!(collapse_whitespace_runs(".a  >  .  >  .b"), ".a > . > .b");
        assert_eq!(collapse_whitespace_runs(".a\t>\t.b"), ".a > .b");
        assert_eq!(collapse_whitespace_runs(".a >\n> .b"), ".a > > .b");
        assert_eq!(collapse_whitespace_runs(".a \t\n .b"), ".a .b");
        // A lone tab (run of length 1) still becomes a space.
        assert_eq!(collapse_whitespace_runs(".a\t.b"), ".a .b");

        // Already single-spaced: unchanged and borrowed (no allocation).
        let out = collapse_whitespace_runs(".a > .b");
        assert_eq!(out, ".a > .b");
        assert!(matches!(out, Cow::Borrowed(_)));
        assert!(matches!(collapse_whitespace_runs(".a.b"), Cow::Borrowed(_)));

        // String and comment interiors keep their whitespace verbatim.
        assert_eq!(
            collapse_whitespace_runs("[data-x='a  b']  .c"),
            "[data-x='a  b'] .c"
        );
        assert_eq!(
            collapse_whitespace_runs("[data-x=\"a\tb\"]"),
            "[data-x=\"a\tb\"]"
        );
        assert_eq!(
            collapse_whitespace_runs(".a  /* c  d */  .b"),
            ".a /* c  d */ .b"
        );
        // An escaped quote does not end the string early.
        assert_eq!(
            collapse_whitespace_runs(r"[x='a\'  b']  .c"),
            r"[x='a\'  b'] .c"
        );

        // Idempotent: the collapsed form is a fixed point.
        let once = collapse_whitespace_runs(".a  >\n>  .b").into_owned();
        assert_eq!(collapse_whitespace_runs(&once), once);
    }

    #[test]
    fn test_extract_function_args() {
        assert_eq!(
            extract_function_args("prop: var(--a, red)", "var"),
            Some("--a, red")
        );
        assert_eq!(
            extract_function_args("prop: var(--a, /* comment */ red)", "var"),
            Some("--a, /* comment */ red")
        );
        // Nested parens
        assert_eq!(
            extract_function_args("prop: var(--a, calc(1 + 2))", "var"),
            Some("--a, calc(1 + 2)")
        );
        // Function not found
        assert_eq!(extract_function_args("prop: red", "var"), None);
    }

    #[test]
    fn test_split_args_by_comma() {
        assert_eq!(split_args_by_comma("a, b, c"), vec!["a", " b", " c"]);
        assert_eq!(split_args_by_comma("--a, red"), vec!["--a", " red"]);
        // Nested parens preserved
        assert_eq!(
            split_args_by_comma("--a, calc(1, 2)"),
            vec!["--a", " calc(1, 2)"]
        );
        // Single arg
        assert_eq!(split_args_by_comma("--a"), vec!["--a"]);
        // Empty
        assert_eq!(split_args_by_comma(""), Vec::<&str>::new());
        // Empty segments are KEPT (raw policy, unlike the space splitter): a
        // doubled separator yields an empty segment, a leading separator yields
        // a leading empty, and a lone separator yields one empty segment.
        assert_eq!(split_args_by_comma("a,,b"), vec!["a", "", "b"]);
        assert_eq!(split_args_by_comma(",a"), vec!["", "a"]);
        assert_eq!(split_args_by_comma(","), vec![""]);
        // A trailing separator does NOT produce a trailing empty (the
        // `start < content.len()` final-segment guard).
        assert_eq!(split_args_by_comma("a,b,"), vec!["a", "b"]);
        // Commas inside comments are NOT separators
        assert_eq!(
            split_args_by_comma("--a, /* with, comma */ red"),
            vec!["--a", " /* with, comma */ red"]
        );
        assert_eq!(
            split_args_by_comma("/* a, b */ value"),
            vec!["/* a, b */ value"]
        );
        // Commas inside quotes are NOT separators
        assert_eq!(
            split_args_by_comma(r#"--font, "Font, Name""#),
            vec!["--font", r#" "Font, Name""#]
        );
        assert_eq!(
            split_args_by_comma(r"'a, b', 'c, d'"),
            vec!["'a, b'", " 'c, d'"]
        );
    }

    #[test]
    fn test_split_by_space_preserving_parens() {
        assert_eq!(
            split_by_space_preserving_parens("var(--b) color-mix(in srgb, red, blue)"),
            vec!["var(--b)", "color-mix(in srgb, red, blue)"]
        );
        // Each segment is trimmed and empties are DROPPED (unlike the comma
        // splitter): consecutive/leading/trailing whitespace collapses away.
        assert_eq!(split_by_space_preserving_parens("a  b"), vec!["a", "b"]);
        assert_eq!(split_by_space_preserving_parens("  a  "), vec!["a"]);
        // Tabs are separators too.
        assert_eq!(split_by_space_preserving_parens("a\tb"), vec!["a", "b"]);
        // Single token, and all-whitespace yields nothing.
        assert_eq!(split_by_space_preserving_parens("solid"), vec!["solid"]);
        assert_eq!(split_by_space_preserving_parens("   "), Vec::<&str>::new());
        // Spaces inside parens are NOT separators.
        assert_eq!(
            split_by_space_preserving_parens("calc(1px + 2px) red"),
            vec!["calc(1px + 2px)", "red"]
        );
        // Spaces inside comments are NOT separators (comment stays one atom).
        assert_eq!(
            split_by_space_preserving_parens("a /* x y */ b"),
            vec!["a", "/* x y */", "b"]
        );
        // Spaces inside quotes are NOT separators.
        assert_eq!(
            split_by_space_preserving_parens(r"'Font Name' serif"),
            vec!["'Font Name'", "serif"]
        );
    }
}
