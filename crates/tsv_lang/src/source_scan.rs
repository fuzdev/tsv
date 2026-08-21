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
/// previous-token context to tell it from division, which a stateless forward
/// `skip_trivia` can't carry as a flag. The disambiguation lives in the separate
/// [`is_regex_start_after`] / [`skip_regex_literal`] helpers below, which the
/// depth-tracking scanners that *do* sit at a regex boundary (the printer's paren
/// scan, the Svelte brace matcher, the TS arrow-vs-paren lookahead) call alongside
/// `skip_trivia`, threading the operand-end anchor themselves. A plain inter-node
/// delimiter scan never sits at a regex boundary in practice, matching the
/// historical `skip_string_or_comment`.
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

/// Skip the whole RUN of whitespace and trivia starting at `from`, returning the byte
/// position of the first significant character — or `source.len()` if the run reaches the
/// end.
///
/// [`skip_trivia`] answers "does trivia START here"; this answers "where does the trivia
/// END", which is what a caller sitting *between two tokens* actually asks. One
/// `skip_trivia` call sees only the first span, and between two tokens there can be
/// whitespace, a comment, more whitespace and another comment — so every such caller hand
/// -rolled the same alternating loop, each with its own copy of two easy-to-miss
/// obligations: that `skip_trivia` must not be called at `end` (it indexes `bytes[i]`, and
/// running out of source mid-run is the ordinary case, not a caller error), and that the
/// whitespace step must move by whole **characters** — a byte cursor that advances by one
/// past a non-ASCII member lands on a continuation byte, which both misreads the text and
/// panics as a `&str` index.
///
/// `is_whitespace` is the caller's own language class rather than `char::is_whitespace`:
/// JS `\s` and Rust's `White_Space` disagree at `U+0085` and `U+FEFF`, and a scan using
/// the wrong one stops early — under-reporting, the direction these scans exist to avoid.
///
/// Total: a `from` past the end, or off a character boundary, returns it unchanged rather
/// than panicking.
#[inline]
pub fn skip_trivia_run(
    source: &str,
    from: usize,
    profile: TriviaProfile,
    is_whitespace: impl Fn(char) -> bool,
) -> usize {
    let end = source.len();
    let mut pos = from.min(end);
    loop {
        let Some(rest) = source.get(pos..) else {
            return pos;
        };
        pos = end - rest.trim_start_matches(&is_whitespace).len();
        if pos == end {
            return end;
        }
        let Some(past) = skip_trivia(source.as_bytes(), pos, end, profile) else {
            return pos;
        };
        // `skip_trivia` never reports a position at or before its own — every arm returns
        // at least `i + 2`, or `end`, which is `> i` because `i < end` — so the run always
        // advances and the loop always terminates.
        pos = past;
    }
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

/// Find the **last** occurrence of `target` in `bytes[start..end]`, skipping comments.
/// Returns the byte's position, or `None`.
///
/// The single-byte counterpart of [`rfind_keyword`], and a forward scan for the same
/// reason: only a forward walk can skip trivia, so it is what yields the rightmost match
/// that is **not** inside a comment. A plain reverse `rfind` would happily return a byte
/// written inside a trailing comment.
///
/// `target` must not itself be a trivia-introducing byte (`/`, `'`, `"`, `` ` ``)
/// — those are consumed as trivia and would never match.
#[inline]
pub fn rfind_char_skipping_comments(
    bytes: &[u8],
    start: usize,
    end: usize,
    target: u8,
) -> Option<usize> {
    let mut found = None;
    let mut i = start;
    while i < end {
        if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::COMMENTS) {
            i = past;
            continue;
        }
        if bytes[i] == target {
            found = Some(i);
        }
        i += 1;
    }
    found
}

/// Whether `keyword` occurs at `i` as a **whole word** — present byte-for-byte
/// and not flanked by a JS/TS identifier byte (alphanumeric, `_`, or `$`), so
/// `export` does not match inside `exported` or `$export`. The boundary check is
/// against the full `bytes`, not any `[start, end)` window. Caller ensures `i +
/// keyword.len() <= bytes.len()`.
#[inline]
fn whole_word_at(bytes: &[u8], i: usize, keyword: &[u8]) -> bool {
    &bytes[i..i + keyword.len()] == keyword && word_boundaries_ok(bytes, i, keyword.len())
}

/// Like [`whole_word_at`], but matching `keyword` ASCII-case-insensitively.
fn whole_word_at_ignore_ascii_case(bytes: &[u8], i: usize, keyword: &[u8]) -> bool {
    bytes[i..i + keyword.len()].eq_ignore_ascii_case(keyword)
        && word_boundaries_ok(bytes, i, keyword.len())
}

/// The shared boundary half of the whole-word tests: neither flank of
/// `[i, i + kw_len)` is an identifier byte.
#[inline]
fn word_boundaries_ok(bytes: &[u8], i: usize, kw_len: usize) -> bool {
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

/// Whether a trivia span opening at `i` **ends an operand** — a string or
/// template literal does (`'ab' / 2` divides), a comment does not (it is
/// transparent, and the operand before it still governs).
///
/// The discriminator is the opener byte, because [`skip_trivia`] returns the
/// same `Some(past)` for both kinds. Depth-tracking scanners call this to
/// maintain the `operand_end` anchor [`is_regex_start_after`] reads; stating the
/// rule once here is what keeps a scanner from silently treating a skipped
/// string as if it were a comment.
#[inline]
pub fn trivia_ends_operand(bytes: &[u8], i: usize) -> bool {
    matches!(bytes[i], b'"' | b'\'' | b'`')
}

/// The operand-end anchor after a scan consumes the significant byte at `i` —
/// unchanged when that byte is whitespace, which ends no operand.
///
/// The whitespace case is the whole reason this is a named helper rather than a
/// bare `operand_end = i + 1`: an anchor advanced over the space *after* a
/// comment sits above the comment's own bytes, which is precisely the position a
/// backward reader must never be handed.
#[inline]
#[must_use]
pub fn operand_end_after(bytes: &[u8], i: usize, operand_end: usize) -> usize {
    if bytes[i].is_ascii_whitespace() {
        operand_end
    } else {
        i + 1
    }
}

/// Whether a `/` starts a regex literal (rather than a division operator), given
/// `operand_end` — the caller's scan position just past the last **non-trivia**
/// byte it consumed before reaching the `/`.
///
/// Decided by the last non-whitespace byte at or below `operand_end`: a `/`
/// after something that *ends* an expression (identifier char, `)`, `]`, a
/// postfix `++`/`--`, or a string/template closing quote `'` `"` `` ` ``) is
/// division; after anything else — or with nothing significant before it — it is
/// a regex. `lower_bound` bounds both walks.
///
/// The anchor is read directly — there is no backward walk, which is the point:
/// `bytes[operand_end - 1]` is a byte the caller's scan already classified as
/// significant, so no reader here can wander into a comment. Callers maintain it
/// with [`operand_end_after`] and [`trivia_ends_operand`].
///
/// This is the one piece of `/`-disambiguation the trivia cursor deliberately
/// leaves out of [`skip_trivia`]/[`TriviaProfile`]: it needs previous-**token**
/// context, which a stateless forward scan can't carry as a flag. So the
/// depth-tracking scanners that sit at a regex boundary (the printer's paren
/// scan, Svelte's brace matcher, the TS arrow-vs-paren lookahead) thread the
/// anchor themselves — updating it past every significant byte, past a skipped
/// regex or template, and past a skipped string but **not** a comment
/// ([`trivia_ends_operand`]).
///
/// ⚠️ It takes the anchor rather than the `/`'s own position because deriving
/// one by walking *backward* cannot see trivia: a block comment before the
/// slash (`fn() /* c */ / bb`) puts the `/` of its `*/` in the lookback slot,
/// which ends no operand, so the division read as a regex and the scan ran on
/// to some unrelated delimiter — losing the `)` a paren scan was looking for,
/// and rejecting a Svelte `{…}` tag outright.
#[inline]
pub fn is_regex_start_after(bytes: &[u8], operand_end: usize, lower_bound: usize) -> bool {
    // Nothing significant before it (start of the scanned region) → regex.
    if operand_end <= lower_bound {
        return true;
    }
    let j = operand_end - 1;
    let b = bytes[j];
    // An identifier byte usually ends an operand (`a / 2` divides), but a
    // whole RESERVED word ending here is an operator, and a `/` after an
    // operator opens a regex (`typeof /re/`, `void /re/`, `'a' in /re/`).
    if is_identifier_byte(b) {
        return word_before_regex(bytes, operand_end, lower_bound);
    }
    // A postfix `++`/`--` ends an operand, so the `/` after it DIVIDES
    // (`aa++ / bb`). A lone `+`/`-` is a binary or unary operator, after
    // which a regex may start (`aa + /re/.test(b)`), so the doubling is
    // the whole discriminator.
    if matches!(b, b'+' | b'-') && j > lower_bound && bytes[j - 1] == b {
        return false;
    }
    // Bytes that END an expression — a `/` after these is DIVISION. The
    // string/template closing quotes (`'` `"` `` ` ``) belong here: after a
    // literal like `'ab' / 2`, the `/` divides (the anchor sits past the whole
    // string, so this quote can only be its close).
    !(b == b')' || b == b']' || b == b'\'' || b == b'"' || b == b'`')
}

/// Whether the identifier ending at `word_end` (exclusive) is a reserved word an
/// expression — and so a regex literal — may follow (`typeof /re/`, `void /re/`,
/// `'a' in /re/`).
///
/// Only **reserved** words qualify: a reserved word can never be a variable, so
/// reading one as an operator can never misclassify a real division. Contextual
/// keywords are deliberately absent — `of` and `as` are legal identifiers, so
/// `of / 2` must stay division. `await` is the one judgment call: reserved at
/// `Goal::Module` (the default) and inside every async function, which is where
/// `await /re/.test(x)` occurs; only at the rare `Goal::Script` is it an ordinary
/// identifier whose `await / 2` would be misread.
///
/// A reserved word used as a PROPERTY NAME is an operand, not an operator
/// (`a.in / bb` and `a.return / bb` both divide), so a `.` before the word
/// disqualifies it.
fn word_before_regex(bytes: &[u8], word_end: usize, lower_bound: usize) -> bool {
    let mut start = word_end;
    while start > lower_bound && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }
    // Matched as a pattern rather than scanned from a list so the compiler can
    // switch on length first — this is the hot path for ordinary division, where
    // the word is some identifier that matches nothing. A numeric literal
    // (`1e3 / 2`) falls out here too, since no literal spells a keyword.
    if !matches!(
        &bytes[start..word_end],
        b"return"
            | b"typeof"
            | b"instanceof"
            | b"in"
            | b"void"
            | b"delete"
            | b"case"
            | b"do"
            | b"else"
            | b"throw"
            | b"new"
            | b"extends"
            | b"yield"
            | b"await"
    ) {
        return false;
    }
    // `.name` / `?.name` — a member access, so the word is an operand.
    //
    // This is the one lookback the caller's forward anchor can't supply: it asks
    // about the token *before* the word, not the one the anchor marks. So it
    // walks back, and must step over a block comment written in that gap
    // (`a./* c */in / bb`) — landing on the `/` of a `*/` would read as "no dot"
    // and turn the member access into an operator, i.e. the division into a
    // regex. A line comment can't sit here: it would swallow the word.
    let mut j = start;
    while j > lower_bound {
        j -= 1;
        if let Some(open) = block_comment_start_before(bytes, j, lower_bound) {
            j = open;
            continue;
        }
        if !bytes[j].is_ascii_whitespace() {
            return bytes[j] != b'.';
        }
    }
    true
}

/// If `bytes[j]` is the `/` closing a block comment, the index of that comment's
/// opening `/` — so a backward walk can step over it. `None` when `j` isn't a
/// comment close, or no opener is found above `lower_bound`.
///
/// Backward matching is only sound because JS block comments don't nest; a `/*`
/// inside a comment *body* can still be found first, which leaves the walk
/// inside the comment rather than past it — no worse than not stepping at all,
/// and the reason forward anchoring ([`is_regex_start_after`]) is preferred
/// wherever the caller can supply one.
fn block_comment_start_before(bytes: &[u8], j: usize, lower_bound: usize) -> Option<usize> {
    if bytes[j] != b'/' || j < lower_bound + 2 || bytes[j - 1] != b'*' {
        return None;
    }
    let mut k = j - 1;
    while k > lower_bound {
        k -= 1;
        if bytes[k] == b'*' && k > lower_bound && bytes[k - 1] == b'/' {
            return Some(k - 1);
        }
    }
    None
}

/// Skip past a regex literal whose opening `/` is at `start`, returning the
/// position just after the closing `/` and any trailing flags (bounded by
/// `end`). Backslash-escape aware, and aware that a `/` inside a `[…]`
/// character class is a literal, not the terminator. An unterminated literal
/// returns `end`.
///
/// Pairs with [`is_regex_start_after`] — the caller confirms the `/` is a regex
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

/// Scan from `scan_start` — the first byte inside an already-open `{` (counted as
/// depth 1) — to that brace's matching `}`, returning the `}`'s offset, or `None`
/// if the braces don't balance before `end`.
///
/// Expression-context aware: strings and line/block comments are skipped via
/// [`skip_trivia`] (JS), regex literals via [`is_regex_start_after`] / [`skip_regex_literal`],
/// and template literals — interpolation and all — via [`skip_template_literal`], so
/// a `}` inside any of them is inert. The shared core behind Svelte's `{…}`-tag
/// matcher (`tsv_svelte`'s `scan_to_matching_brace`) and the `${…}` interpolation
/// skip below. A binding-PATTERN scanner (`match_bracket`) deliberately does **not**
/// route through here — Svelte rejects a regex in that position, so the pattern
/// scan stays regex-unaware — but it *does* share [`skip_template_literal`].
pub fn scan_to_matching_brace(bytes: &[u8], scan_start: usize, end: usize) -> Option<usize> {
    let mut depth: u32 = 1;
    let mut i = scan_start;
    // Just past the last significant byte — the anchor `is_regex_start_after`
    // reads. A template literal ends an operand, as does a skipped regex.
    let mut operand_end = scan_start;
    while i < end {
        if bytes[i] == b'`' {
            i = skip_template_literal(bytes, i, end);
            operand_end = i;
            continue;
        }
        if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
            if trivia_ends_operand(bytes, i) {
                operand_end = past;
            }
            i = past;
            continue;
        }
        if bytes[i] == b'/' && i + 1 < end && is_regex_start_after(bytes, operand_end, scan_start) {
            i = skip_regex_literal(bytes, i, end);
            operand_end = i;
            continue;
        }
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        operand_end = operand_end_after(bytes, i, operand_end);
        i += 1;
    }
    None
}

/// Skip a template literal whose opening `` ` `` is at `start`, returning the
/// position just past the closing `` ` `` (bounded by `end`; an unterminated
/// literal returns `end`).
///
/// **Interpolation-aware**, unlike [`skip_trivia`]'s opaque quote-to-quote
/// template handling: a `${…}` region is scanned with *balanced braces* (via
/// [`scan_to_matching_brace`]), so a `}` inside it — and any nested template /
/// string / regex / object literal — doesn't end the template early. `skip_trivia`
/// scans `` ` `` to the next `` ` ``, which mis-pairs across a nested template
/// (`` `${`x`}` `` pairs the outer and inner opening backticks), swallowing the rest
/// of the input. So the brace matchers that need *exact* template extents (Svelte's
/// `{…}` tag scanner and binding-pattern scanner) intercept `` ` `` and call this
/// instead of delegating it to `skip_trivia`.
pub fn skip_template_literal(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut i = start + 1; // past the opening backtick
    while i < end {
        match bytes[i] {
            b'\\' if i + 1 < end => i += 2, // escape — skip the next byte
            b'`' => return i + 1,           // closing backtick
            b'$' if i + 1 < end && bytes[i + 1] == b'{' => {
                // `${…}` interpolation — skip its balanced-brace body (which may
                // itself hold nested templates, strings, regex, and braces). Runs
                // just past the matching `}`, or to `end` if unterminated.
                i = scan_to_matching_brace(bytes, i + 2, end).map_or(end, |close| close + 1);
            }
            _ => i += 1,
        }
    }
    end // unterminated template literal
}

/// How much whitespace may sit between a block comment's `*/` and the token it precedes for
/// the two to still count as adjacent — the one axis on which the callers of
/// [`block_comment_end_before`] differ, so it is named rather than re-derived per copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentGlue {
    /// Horizontal whitespace only (spaces/tabs). A comment the author put on its own line
    /// leads the *line*, not the token, so a newline breaks the glue.
    SameLine,
    /// Any whitespace, newlines included — the comment is adjacent even from its own line.
    AnyLine,
}

/// The end offset of a **block** comment (`… */`) preceding the token at `pos`, with nothing
/// but `glue` whitespace between them. `None` when no block comment is adjacent.
///
/// Byte-level only: it locates the `*/`, never the `/*`. A `/*` can appear inside the
/// comment's own body, in a preceding line comment, or in a string literal, and byte scanning
/// cannot tell those apart from the real opener — mis-slicing the content would drop a real
/// comment or fabricate one. Callers resolve the actual comment through the lexer's spans by
/// matching the end offset this returns; the spans, not the bytes, are authoritative.
///
/// A `*/` inside a string literal can therefore reach a caller's lookup, which then simply
/// finds no comment ending there.
#[must_use]
pub fn block_comment_end_before(bytes: &[u8], pos: usize, glue: CommentGlue) -> Option<usize> {
    let mut i = pos.min(bytes.len());
    while i > 0
        && match glue {
            CommentGlue::SameLine => matches!(bytes[i - 1], b' ' | b'\t'),
            CommentGlue::AnyLine => bytes[i - 1].is_ascii_whitespace(),
        }
    {
        i -= 1;
    }
    // The shortest block comment is `/**/` (4 bytes), so a `*/` before offset 4 cannot be one.
    (i >= 4 && bytes.get(i - 2..i) == Some(b"*/".as_slice())).then_some(i)
}

/// Whether a newline sits immediately before `pos`, skipping horizontal whitespace.
///
/// Walks backwards from `pos`, skipping spaces and tabs; `true` when a newline is
/// reached before any other byte. The start of the source is **not** a newline —
/// callers that treat a file boundary as a line boundary test `pos == 0` themselves
/// (`crate::directive_alone_on_line` does).
///
/// Mirrors prettier's `hasNewline(text, index, { backwards: true })`.
#[must_use]
pub fn has_newline_before_position(source: &str, pos: u32) -> bool {
    source.as_bytes()[..pos as usize]
        .iter()
        .rev()
        .find(|b| !matches!(b, b' ' | b'\t'))
        .is_some_and(|b| matches!(b, b'\n' | b'\r'))
}

/// Whether a newline sits immediately after `pos`, skipping horizontal whitespace.
///
/// The forward twin of [`has_newline_before_position`]; end-of-source is likewise not
/// a newline.
///
/// Mirrors prettier's `hasNewline(text, index)`.
#[must_use]
pub fn has_newline_after_position(source: &str, pos: u32) -> bool {
    source.as_bytes()[pos as usize..]
        .iter()
        .find(|b| !matches!(b, b' ' | b'\t'))
        .is_some_and(|b| matches!(b, b'\n' | b'\r'))
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

    /// The whole ALTERNATING run, not one trivia span: between two tokens there can be
    /// whitespace, a comment, more whitespace and another comment, and a single
    /// [`skip_trivia`] call stops after the first.
    ///
    /// Two obligations each hand-rolled copy of this loop had to remember, both graded
    /// here. A run reaching the END of the source is ordinary, not a caller error — every
    /// case below whose trivia runs to EOF would have called `skip_trivia` at `end`, where
    /// it indexes out of bounds. And the whitespace step must move by whole CHARACTERS: a
    /// non-ASCII member (JS `\s` has several) leaves a byte cursor on a continuation byte,
    /// which misreads the text and panics as a `&str` index.
    #[test]
    fn skip_trivia_run_crosses_the_whole_alternating_run() {
        // JS `\s`, narrowed to the members this test needs — deliberately not
        // `char::is_whitespace`, which disagrees with it at `U+0085` and `U+FEFF`.
        let ws = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{a0}' | '\u{feff}');
        let run = |src: &str| skip_trivia_run(src, 0, TriviaProfile::COMMENTS, ws);

        for (src, first_significant) in [
            ("x", "x"),
            ("  x", "x"),
            ("/* c */x", "x"),
            ("  /* c1 */ /* c2 */  x", "x"),
            ("// c\nx", "x"),
            ("/* c1 */\n// c2\n\t/* c3 */x", "x"),
            // A non-ASCII whitespace member: the run must step over the whole character.
            ("\u{a0}\u{feff}/* c */\u{a0}x", "x"),
            // Trivia all the way to the end — the case that calls for the end guard.
            ("", ""),
            ("   ", ""),
            ("/* c */", ""),
            ("// c", ""),
            ("  /* c */  ", ""),
            // An unterminated comment ends at the source end rather than running past it.
            ("/* c", ""),
        ] {
            let pos = run(src);
            assert_eq!(&src[pos..], first_significant, "{src:?}");
        }

        // Strings are not trivia under `COMMENTS`, and a lone `/` is no comment at all.
        assert_eq!(&"  'a b' x"[run("  'a b' x")..], "'a b' x");
        assert_eq!(&" / x"[run(" / x")..], "/ x");

        // Total: a `from` past the end, or off a character boundary, comes back unchanged.
        assert_eq!(skip_trivia_run("ab", 9, TriviaProfile::COMMENTS, ws), 2);
        assert_eq!(skip_trivia_run("é x", 1, TriviaProfile::COMMENTS, ws), 1);
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
    fn skip_template_literal_basic() {
        let s = |src: &str| skip_template_literal(src.as_bytes(), 0, src.len());
        assert_eq!(s("`abc`"), 5); // whole literal
        assert_eq!(s("`abc`de"), 5); // stops at the close, not EOF
        assert_eq!(s(r"`a\`b`"), 6); // an escaped backtick is not the close
    }

    #[test]
    fn skip_template_literal_interpolation_balances_braces() {
        let s = |src: &str| skip_template_literal(src.as_bytes(), 0, src.len());
        assert_eq!(s("`a${b}c`"), 8); // simple `${…}`
        assert_eq!(s("`${ {x: 1} }`"), 13); // an object literal `}` inside doesn't end it
        assert_eq!(s("`${ `}` }`"), 10); // a `}` inside a NESTED template isn't the close
    }

    #[test]
    fn skip_template_literal_nested_template() {
        // The bug this fixes: `skip_trivia`'s opaque `` ` ``-to-`` ` `` scan mis-pairs
        // across a nested template. `skip_template_literal` recurses through `${…}`,
        // so a nested template — even one holding a lone quote — is skipped whole.
        let s = |src: &str| skip_template_literal(src.as_bytes(), 0, src.len());
        assert_eq!(s("`${`x`}`"), 8);
        assert_eq!(s(r#"`${`"`}`"#), 8); // nested template holding a `"`
        assert_eq!(s("`${`${`y`}`}`"), 13); // doubly nested
    }

    #[test]
    fn skip_template_literal_unterminated_returns_end() {
        let s = |src: &str| skip_template_literal(src.as_bytes(), 0, src.len());
        assert_eq!(s("`abc"), 4); // no closing backtick
        assert_eq!(s("`${abc"), 6); // unterminated interpolation
    }

    /// What a depth-tracking scanner does, in miniature: walk forward to the `/`
    /// at `slash_pos`, maintaining the `operand_end` anchor, then ask. The tests
    /// grade this composition rather than a hand-computed anchor, because the
    /// bug class lives in the hand-off — a scanner that lets a skipped *comment*
    /// advance the anchor (or fails to advance it past a skipped *string*) gets
    /// the right answer from a wrong premise.
    fn regex_at(src: &str, slash_pos: usize, lower_bound: usize) -> bool {
        let bytes = src.as_bytes();
        let end = bytes.len();
        let mut i = lower_bound;
        let mut operand_end = lower_bound;
        while i < slash_pos {
            if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
                if trivia_ends_operand(bytes, i) {
                    operand_end = past;
                }
                i = past;
                continue;
            }
            operand_end = operand_end_after(bytes, i, operand_end);
            i += 1;
        }
        is_regex_start_after(bytes, operand_end, lower_bound)
    }

    #[test]
    fn is_regex_start_division_after_string_close() {
        // A `/` after a string/template closing quote is DIVISION, not a regex.
        // `lower_bound` = 0; the `/` position is the last byte.
        let div = |src: &str| !regex_at(src, src.len() - 1, 0);
        assert!(div("'ab' /")); // single-quote close
        assert!(div("\"ab\" /")); // double-quote close
        assert!(div("`ab` /")); // template close
        assert!(div("x /")); // identifier — already division
        // ...but a `/` after an operator is a regex start (not division).
        assert!(!div("= /"));
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
    fn rfind_char_skipping_comments_takes_the_last_real_occurrence() {
        // Two real occurrences: the LAST wins (where `find` would take the first).
        assert_eq!(rfind_char_skipping_comments(b"a)b);", 0, 5, b')'), Some(3));
        assert_eq!(find_char_skipping_comments(b"a)b);", 0, 5, b')'), Some(1));
        // A comment-borne occurrence never wins, even though it is last...
        assert_eq!(
            rfind_char_skipping_comments(b"a) /* ) */ ;", 0, 12, b')'),
            Some(1)
        );
        // ...which is exactly what a reverse byte scan would get wrong (it lands on the
        // `)` at index 6, inside the comment).
        assert_eq!(b"a) /* ) */ ;".iter().rposition(|&b| b == b')'), Some(6));
        // No occurrence outside a comment.
        assert_eq!(
            rfind_char_skipping_comments(b"a /* ) */ b", 0, 11, b')'),
            None
        );
        // Empty range.
        assert_eq!(rfind_char_skipping_comments(b"a)b", 1, 1, b')'), None);
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
        assert!(regex_at("a = /re/", 4, 0));
        // `a / b` — `/` after identifier `a` is division.
        assert!(!regex_at("a / b", 2, 0));
        // `) / b` — `/` after `)` is division; `] / b` likewise.
        assert!(!regex_at(") / b", 2, 0));
        assert!(!regex_at("] / b", 2, 0));
        // At the lower bound (nothing significant before) → regex.
        assert!(regex_at("/re/", 0, 0));
        // The lower bound is honored: even though `(` precedes, a scan bounded
        // at the `/` itself sees nothing before it → regex.
        assert!(regex_at("(/re/", 1, 1));
    }

    #[test]
    fn is_regex_start_sees_through_a_comment_to_the_operand() {
        // A comment is transparent: the operand BEFORE it still decides, so each
        // of these `/`s divides. Walking backward from the slash instead lands on
        // the `/` of the `*/`, which ends no operand — and read the division as a
        // regex, running the enclosing scan on to some unrelated delimiter.
        for src in [
            "aa++ /* c */ /",
            "aa-- /* c */ /",
            "fn() /* c */ /",
            "arr[0] /* c */ /",
            "aa /* c */ /",
            "'ab' /* c */ /",
            "`ab` /* c */ /",
            "aa /* c1 */ /* c2 */ /",
            "aa // c\n/",
        ] {
            assert!(
                !regex_at(src, src.len() - 1, 0),
                "expected division: {src:?}"
            );
        }

        // ...and the operator cases stay regexes through a comment.
        for src in [
            "= /* c */ /",
            "aa + /* c */ /",
            "typeof /* c */ /",
            "( /* c */ /",
        ] {
            assert!(regex_at(src, src.len() - 1, 0), "expected regex: {src:?}");
        }

        // A reserved word used as a PROPERTY NAME is an operand even with the
        // comment between the `.` and the name — the one lookback the forward
        // anchor can't supply, so it steps back over the comment itself.
        assert!(!regex_at("a./* c */in /", 12, 0));
        // ...while a comment before a genuine operator keyword leaves it one.
        assert!(regex_at("/* c */ typeof /", 15, 0));
    }

    #[test]
    fn is_regex_start_reads_a_reserved_word_as_an_operator() {
        // The prefix's length IS the `/` offset, so each case names its own
        // position — no scan needed to locate it.
        let at_slash = |prefix: &str, rest: &str| {
            let src = format!("{prefix}{rest}");
            regex_at(&src, prefix.len(), 0)
        };

        // A reserved word is an operator, so the `/` after it opens a regex.
        for prefix in [
            "typeof ",
            "void ",
            "'a' in ",
            "return ",
            "case ",
            "throw ",
            "yield ",
            "await ",
            "a instanceof ",
        ] {
            assert!(at_slash(prefix, "/re/"), "expected regex after `{prefix}`");
        }

        // An ordinary identifier is an operand — including one that merely ENDS
        // with a keyword, one ending in `$` (an identifier byte the old exclusion
        // list missed), and the contextual keywords that are legal variables.
        for prefix in ["aa ", "notreturn ", "a$ ", "of ", "as "] {
            assert!(
                !at_slash(prefix, "/ bb"),
                "expected division after `{prefix}`"
            );
        }

        // A reserved word is a legal PROPERTY name, and a property is an operand.
        for prefix in ["a.in ", "a.return ", "a?.typeof "] {
            assert!(
                !at_slash(prefix, "/ bb"),
                "expected division after `{prefix}`"
            );
        }

        // A postfix update ends an operand; a lone `+`/`-` does not.
        for prefix in ["aa++ ", "aa-- ", "aa++"] {
            assert!(
                !at_slash(prefix, "/ bb"),
                "expected division after `{prefix}`"
            );
        }
        for prefix in ["aa + ", "aa - ", "aa = -", "aa = +"] {
            assert!(at_slash(prefix, "/re/"), "expected regex after `{prefix}`");
        }
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
