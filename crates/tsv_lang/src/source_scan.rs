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
// The one piece of state such a scan carries beyond its own depth counters is the
// regex-vs-division anchor, and it carries it as an `OperandAnchor` — which derives
// the answer where a `/` asks for it instead of maintaining it on every byte.
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

/// The four bytes that can open trivia — the set [`skip_trivia`] tests before it
/// commits to a scan, and therefore the set every *hopping* scan must also stop
/// at. A scan that walks byte by byte gets this for free by asking `skip_trivia`
/// at every position; one that hops to its next significant byte does not, and a
/// hop whose needles omit an opener steps straight over a string or comment and
/// reads its body as significant.
///
/// Pair every hop's needle array with [`covers_trivia_openers`] in a `const _`, so
/// adding a fifth opener here is a compile error at each hop rather than a silent
/// misparse.
pub const TRIVIA_OPENERS: [u8; 4] = [b'"', b'\'', b'`', b'/'];

/// Whether `needles` stops at every [`TRIVIA_OPENERS`] byte — the precondition for
/// hopping past bytes instead of asking [`skip_trivia`] about each one.
///
/// `const`-evaluable so a hop can prove it at compile time:
///
/// ```
/// # use tsv_lang::source_scan::{covers_trivia_openers, TRIVIA_OPENERS};
/// const NEEDLES: [u8; 6] = [b'{', b'}', b'"', b'\'', b'`', b'/'];
/// const _: () = assert!(covers_trivia_openers(&NEEDLES));
/// ```
#[must_use]
pub const fn covers_trivia_openers(needles: &[u8]) -> bool {
    let mut opener = 0;
    while opener < TRIVIA_OPENERS.len() {
        let mut found = false;
        let mut k = 0;
        while k < needles.len() {
            if needles[k] == TRIVIA_OPENERS[opener] {
                found = true;
            }
            k += 1;
        }
        if !found {
            return false;
        }
        opener += 1;
    }
    true
}

/// [`TRIVIA_OPENERS`] plus one caller-supplied significant byte — the hop needle
/// set for a scan whose own byte is not a compile-time constant (a keyword's
/// first byte).
///
/// A hop with a const needle array states its coverage with a
/// [`covers_trivia_openers`] assert; this one satisfies the same obligation *by
/// construction*, and it is built by copying `TRIVIA_OPENERS` rather than
/// restating its members, so a fifth opener widens every array built here
/// instead of silently escaping one.
///
/// `pub` for the same reason [`TRIVIA_OPENERS`] and [`covers_trivia_openers`] are,
/// though the only callers today are in this module: the hop's precondition is a
/// contract the *other* crates' scans have to satisfy, and the filed follow-up
/// sites are in `tsv_svelte`. ⚠️ One of those — `match_bracket` — takes **two**
/// runtime bytes (`open`/`close`), which this shape does not cover; widen it there
/// rather than open-coding an array that restates the openers.
///
/// ⚠️ The runtime byte costs about four more instructions a word than a
/// compile-time one — `!(v ^ splat) & HIGH_BITS` folds to a single shared `!v &
/// HIGH_BITS` only for constants below `0x80` (see [`crate::swar::next_byte_of`]).
/// That is a rounding error against the ~15 instructions a byte the walk it
/// replaces costs, not a reason to specialize per keyword.
#[must_use]
pub const fn trivia_hop_needles(significant: u8) -> [u8; TRIVIA_OPENERS.len() + 1] {
    let mut out = [significant; TRIVIA_OPENERS.len() + 1];
    let mut k = 0;
    while k < TRIVIA_OPENERS.len() {
        out[k + 1] = TRIVIA_OPENERS[k];
        k += 1;
    }
    out
}

/// Tautological while the constructor above copies [`TRIVIA_OPENERS`] — which is the
/// point: it is the tripwire on a rewrite that stops copying it.
const _: () = assert!(covers_trivia_openers(&trivia_hop_needles(b'k')));

/// The bytes a paren-depth scan hops between: its own parens, plus every
/// [`TRIVIA_OPENERS`] byte. Shared by the two such scans in `tsv_ts` — the
/// printer's `find_closing_paren` and the parser's `matching_paren_close` —
/// which ask one question of one byte class and must not drift apart in it.
pub const PAREN_HOP_NEEDLES: [u8; 6] = [b'(', b')', b'"', b'\'', b'`', b'/'];

/// A hop may only skip bytes that cannot open trivia — see [`covers_trivia_openers`].
const _: () = assert!(covers_trivia_openers(&PAREN_HOP_NEEDLES));

/// Whether `b` is one of `needles` — the byte-scan ladder's BOTTOM rung, in front
/// of a hop whose runs are routinely empty.
///
/// [`crate::swar::next_byte_of`]'s entry costs about fifteen instructions and an
/// empty run saves none of them, so a site whose hops are mostly adjacent pays
/// the entry to learn what one compare already knew. Branchless by construction
/// (an OR-fold, not a short-circuit chain), and derived from the same array the
/// hop passes, so the two cannot drift.
///
/// ⚠️ Conditional on the EMPTY-RUN share, and not free to add: at 8.6% adjacency
/// (`tsv_css`'s `extract_function_parts`) the same test cost instructions and
/// bought no cycles. Read the census's zero bucket first.
///
/// **The width of the test is not what decides it, because the width is not what
/// gets emitted.** At the two `tsv_ts` paren scans this is a SIX-byte membership
/// test — not the two compares the rung was first priced at — and `objdump` shows
/// LLVM lowering it to a 63-wide window check plus one bit test:
/// `add $-0x22` / `cmp $0x3e` / `ja` / `bt %rax, $0x40000000000020e1` / `jae`.
/// Every byte class in this module fits that shape (`skip_trivia`'s own four
/// openers are `$0x4000000000002021` at the same base), so **a membership test
/// over punctuation costs about five instructions regardless of how many members
/// it has** — check the span before pricing one by its arity.
///
/// Measured, at 74–80% adjacency the pre-test is worth `instructions:u` −0.13
/// points of the TypeScript format run *on top of* the hop, which without it is
/// worth **nothing at all** there. The two axes (run length, needle count) do not
/// trade off against each other the way the per-word cost table suggests; the
/// empty-run share dominates both.
#[inline]
#[must_use]
pub fn is_hop_needle<const N: usize>(b: u8, needles: [u8; N]) -> bool {
    let mut hit = false;
    let mut k = 0;
    while k < N {
        hit |= needles[k] == b;
        k += 1;
    }
    hit
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
    // the per-byte finder loops. Only the four [`TRIVIA_OPENERS`] can begin a
    // string/comment; their scans live in the `#[cold]` `skip_trivia_scan` below,
    // kept out of line so the rare branch can't bloat the callers — the scan loops
    // made the old single function too big to inline, leaving its call/return
    // overhead the bulk of its `perf` self-time.
    //
    // ⚠️ Left as the bare compare chain, deliberately: spelling it through
    // [`TRIVIA_OPENERS`] (or as a `matches!`) is semantically identical and moves
    // this function's codegen — 112 bytes of `.text` across the ~20 scans it
    // inlines into — for no functional gain. The const names the set for the
    // *hopping* scans, which cannot ask this function at all;
    // `covers_trivia_openers` is what keeps those honest.
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
/// against the full `bytes`, not any `[start, end)` window. A keyword that would
/// run past the end of `bytes` is not there.
#[inline]
pub fn whole_word_at(bytes: &[u8], i: usize, keyword: &[u8]) -> bool {
    bytes[i..].starts_with(keyword) && word_boundaries_ok(bytes, i, keyword.len())
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
/// skipping trivia per `profile`. Returns the keyword's start position, or `None`
/// — including for an empty `keyword`, which has no occurrence to find.
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
    let &first = keyword.first()?;
    let kw_len = keyword.len();
    let needles = trivia_hop_needles(first);
    let mut i = start;
    while i + kw_len <= end {
        // Hop to the next byte that can matter — see the note on
        // [`rfind_keyword`], which shares this loop's shape and its census.
        i = crate::swar::next_byte_of(&bytes[..end], i, needles);
        if i + kw_len > end {
            break;
        }
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
/// skipping trivia per `profile`. Returns its start position, or `None` —
/// including for an empty `keyword`, which has no occurrence to find.
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
    let &first = keyword.first()?;
    let kw_len = keyword.len();
    let needles = trivia_hop_needles(first);
    let mut found = None;
    let mut i = start;
    while i + kw_len <= end {
        // A byte that is neither the keyword's first byte nor a trivia opener can
        // do nothing here: `whole_word_at` compares `keyword` from `i`, so it
        // cannot match, and `skip_trivia` cannot claim the position. So the walk
        // hops between the bytes that *can* act, a word at a time, instead of
        // asking about each one — the byte-scan ladder's top rung
        // ([`crate::swar::next_byte_of`]), chosen on the INERT fraction rather
        // than on whether the loop reads as a search.
        //
        // This scan has the family's longest runs by a wide margin, because it
        // never exits early: it wants the LAST match, so every call walks its
        // whole range. Per pass it crosses a mean run of 14.6 bytes over 1,666
        // TypeScript files (10.8 over 1,695 Svelte ones), with 85% of those bytes
        // in runs of eight or more and under 1% of its hops adjacent — far too
        // few for the one-byte pre-test that pays where a construct is routinely
        // empty (`tsv_css`'s `string_end`).
        //
        // Bounded at `end` for COST, not for correctness — the guard below retires
        // any landing past the window either way. A printer scan routinely passes
        // an `end` a few tokens ahead of `start` while `bytes` is the whole
        // document, and a full-slice hop over a needle-free window would read to
        // the end of the file to learn what `end` already said.
        i = crate::swar::next_byte_of(&bytes[..end], i, needles);
        if i + kw_len > end {
            break;
        }
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
fn trivia_ends_operand(bytes: &[u8], i: usize) -> bool {
    matches!(bytes[i], b'"' | b'\'' | b'`')
}

/// The `operand_end` anchor [`is_regex_start_after`] reads, derived **on demand**
/// instead of maintained per byte.
///
/// A depth-tracking scan needs the anchor only where it meets a `/`, which is a rare
/// byte — but stated as a running variable it charges every *significant* byte an
/// `is_ascii_whitespace` test and a select to keep current. This carries the two
/// facts the anchor can be rebuilt from instead: its value as of the last boundary
/// the scan crossed, and where the run of significant bytes since that boundary
/// began. Both move only at a boundary, so the per-byte cost is nothing at all.
///
/// **The backward walk is sound because it never leaves that run.** Every byte in
/// `segment_start..pos` is one the scan already classified as significant, so the
/// walk cannot wander into a comment — which is precisely the hazard
/// [`is_regex_start_after`]'s doc warns a naive lookback falls into (`fn() /* c */ /
/// bb` puts the `/` of the `*/` in the lookback slot). It stops at `segment_start`,
/// and the boundary value answers whenever the whole run is whitespace.
/// Deliberately NOT `Copy`: this is a scan's live cursor, not a config value like
/// [`TriviaProfile`], and a silent fork of it would advance one copy while a stale
/// one answered.
#[derive(Debug)]
pub struct OperandAnchor {
    /// The anchor as of the last boundary crossed — already resolved, never lazy.
    at_boundary: usize,
    /// Where the current run of significant bytes begins; the backward walk's floor.
    segment_start: usize,
    /// Where the last literal [`Self::skipped_operand`] recorded ends. The anchor
    /// landing exactly there means the literal is the operand before the `/` — which
    /// the byte there cannot say for a regex without flags, whose last byte is its own
    /// closing `/` (`/a/ / 2` divides).
    literal_end: Option<usize>,
}

impl OperandAnchor {
    /// A scan that begins at `start` with nothing significant consumed yet.
    #[inline]
    #[must_use]
    pub fn new(start: usize) -> Self {
        Self {
            at_boundary: start,
            segment_start: start,
            literal_end: None,
        }
    }

    /// Record a [`skip_trivia`] span that opened at `i` and ended at `past`.
    ///
    /// A string or template ends an operand, so the anchor lands past it; a comment
    /// is transparent, so the operand *before* it still governs — and that pending
    /// lazy answer must be resolved here, while the run it belongs to is still
    /// reachable — by the same `lower_bound` and `grammar` the scan's
    /// [`Self::starts_regex`] asks with (a `!` glued to a `)` before the comment may be a
    /// statement header's, [`OperandGrammar::closes_statement_header`]).
    ///
    /// ⚠️ **The two assignments below are ordered**: resolving the comment case reads
    /// `segment_start`, so moving the floor first would resolve it against the wrong
    /// run and report the anchor from before the *previous* boundary — `abc /* c */
    /// /x/` would then read its division as a regex.
    #[inline]
    pub fn skipped_trivia(
        &mut self,
        bytes: &[u8],
        i: usize,
        past: usize,
        lower_bound: usize,
        grammar: OperandGrammar,
    ) {
        self.at_boundary = if trivia_ends_operand(bytes, i) {
            past
        } else {
            self.at(bytes, i, lower_bound, grammar)
        };
        self.segment_start = past;
    }

    /// Record a span that both **ends an operand** and is opaque to the backward
    /// walk — a regex literal or a template literal — ending at `past`.
    #[inline]
    pub fn skipped_operand(&mut self, past: usize) {
        self.at_boundary = past;
        self.segment_start = past;
        self.literal_end = Some(past);
    }

    /// The anchor at `pos`: just past the last significant byte the scan consumed
    /// before it that is not whitespace — stepping back over a `!` run that is a
    /// postfix non-null ([`Self::postfix_non_null_start`]).
    ///
    /// Private on purpose: [`Self::starts_regex`] is the only question this answers,
    /// and routing every caller through it is what keeps a scan from reconstructing
    /// the eager anchor by hand.
    #[inline]
    #[must_use]
    fn at(&self, bytes: &[u8], pos: usize, lower_bound: usize, grammar: OperandGrammar) -> usize {
        let mut j = pos;
        while j > self.segment_start && bytes[j - 1].is_ascii_whitespace() {
            j -= 1;
        }
        if j > self.segment_start && bytes[j - 1] == b'!' {
            j = self.postfix_non_null_start(bytes, j, lower_bound, grammar);
        }
        if j > self.segment_start {
            j
        } else {
            self.at_boundary
        }
    }

    /// Where the `!` run ending at `run_end` begins when it is a POSTFIX non-null
    /// (`a! / 2`, `f()!! / 2`), so the operand before it is the anchor and the `/`
    /// divides; `run_end` itself when it is a PREFIX logical not (`!/re/`), which leaves
    /// the `!` as the anchor's last byte and the `/` opening a regex.
    ///
    /// **Glued means postfix.** Both readings put the run right after something, so the
    /// bytes cannot say which `!` it is — but a non-null is written against its operand,
    /// and a prefix `!` after an operand is always separated from it: `if (c) !/re/`,
    /// `a⏎!/re/` (ASI), a block keyword `{#if !/re/…}`. So the run steps only when nothing
    /// but more `!`s sits between it and the byte before — an operand's end, an operator
    /// (`a &&!/re/`, where stepping reads the operator and the answer is the same), or,
    /// when the run opens the scan's current segment, the trivia or literal the scan just
    /// stepped over (`a /* c */! / 2`, `'s'! / 2`). Whitespace before the run is never
    /// crossed; the spaced `a ! / 2`, which no formatter prints, reads as the regex.
    ///
    /// **A line break is never glued.** TypeScript takes no postfix `!` after one (its
    /// `hasPrecedingLineBreak`), so the run begins a new statement there (ASI). That is a
    /// line comment, whose terminator the scan consumes (`a // c⏎!/re/`), and a block
    /// comment that spans lines (`a /* c⏎*/!/re/`), which counts as the line break it
    /// holds.
    ///
    /// **Nor is a statement header's `)`.** The run glued to it is a prefix `!` — a
    /// statement begins after the header (`if (c)!/re/`) — where after any other `)` it is
    /// postfix (`f()! / 2`). The bytes cannot tell the two `)`s apart, so the scan's
    /// grammar is asked ([`OperandGrammar::closes_statement_header`]), for that glued `)`
    /// alone.
    ///
    /// The step lives in this walk rather than in [`is_regex_start_after`] because only
    /// the walk knows where the scan's significant bytes stop: a raw backward walk from a
    /// `!` glued to a comment would read the comment's `*/` as the operator before it.
    #[cold]
    fn postfix_non_null_start(
        &self,
        bytes: &[u8],
        run_end: usize,
        lower_bound: usize,
        grammar: OperandGrammar,
    ) -> usize {
        let mut k = run_end - 1;
        while k > self.segment_start && bytes[k - 1] == b'!' {
            k -= 1;
        }
        // The byte before the run is whitespace inside the segment, and — when the run opens
        // it — the last byte of whatever the scan stepped over, which is a line break exactly
        // when that was a line comment or a block comment holding one.
        let glued = if k > self.segment_start {
            !bytes[k - 1].is_ascii_whitespace()
        } else {
            k == 0
                || !(bytes[k - 1].is_ascii_whitespace()
                    || line_terminator_ends_at(bytes, k)
                    || self.multi_line_comment_ends_at(bytes, k, lower_bound))
        };
        if !glued {
            return run_end;
        }
        // The operand the run is glued to ends at `k` inside the segment, or at the value
        // the boundary resolved to when the run opens it.
        let operand_end = if k > self.segment_start {
            k
        } else {
            self.at_boundary
        };
        if operand_end > lower_bound
            && bytes[operand_end - 1] == b')'
            && (grammar.closes_statement_header)(bytes, operand_end - 1, lower_bound)
        {
            return run_end;
        }
        k
    }

    /// Whether a block comment spanning more than one line ends just before `end` — the
    /// trivia the scan stepped over, when a segment begins at `end`. A regex literal can
    /// end on the same `*/` bytes (`/a*/`), and is told apart by the literal record.
    fn multi_line_comment_ends_at(&self, bytes: &[u8], end: usize, lower_bound: usize) -> bool {
        end >= 2
            && self.literal_end != Some(end)
            && block_comment_start_before(bytes, end - 1, lower_bound)
                .is_some_and(|open| (open..end).any(|i| is_line_terminator_at(bytes, i)))
    }

    /// Whether the `/` at `pos` starts a regex literal — [`is_regex_start_after`]
    /// over the anchor this rebuilds, so a caller never handles the two separately.
    /// `lower_bound` (the scan's start) bounds every walk, and `grammar` answers the
    /// operand ends the bytes cannot ([`OperandGrammar`]).
    #[inline]
    #[must_use]
    pub fn starts_regex(
        &self,
        bytes: &[u8],
        pos: usize,
        lower_bound: usize,
        grammar: OperandGrammar,
    ) -> bool {
        let anchor = self.at(bytes, pos, lower_bound, grammar);
        // A skipped literal ends an operand, whatever its last byte reads as.
        if self.literal_end == Some(anchor) {
            return false;
        }
        is_regex_start_after(bytes, anchor, lower_bound, grammar.closes_type_arguments)
    }
}

/// The two operand ends a raw byte scan cannot read from bytes, answered by the grammar
/// that owns them — so a scan that finds where an expression ENDS before it is parsed
/// reads each `/` the way the parser that follows it will.
///
/// The scanning crate does not answer either: the language crate that owns the grammar
/// supplies both (`tsv_ts::OPERAND_GRAMMAR`), and every scan at a regex boundary carries
/// them ([`OperandAnchor::starts_regex`], [`scan_to_matching_brace`],
/// [`scan_to_matching_paren`], [`skip_template_literal`]). A resolver must read the region
/// the way its parser will, so the scan and the parse agree on where an expression ends by
/// construction.
///
/// Handed in at each ask rather than stored in the anchor: the anchor sits in the hot
/// scan loops, and keeping it the three words it needs measured cheaper.
#[derive(Clone, Copy, Debug)]
pub struct OperandGrammar {
    /// Whether a `>` closes a type-argument list ([`ClosesTypeArguments`]).
    pub closes_type_arguments: ClosesTypeArguments,
    /// Whether a `)` closes a statement header ([`ClosesStatementHeader`]).
    pub closes_statement_header: ClosesStatementHeader,
}

impl OperandGrammar {
    /// The answers the bytes alone give: every `>` is an operator (no type arguments),
    /// and every `)` ends an operand (no statement header is looked for).
    pub const BYTES_ONLY: Self = Self {
        closes_type_arguments: never,
        closes_statement_header: never,
    };
}

/// The resolver that answers no to everything ([`OperandGrammar::BYTES_ONLY`]).
fn never(_bytes: &[u8], _at: usize, _lower_bound: usize) -> bool {
    false
}

/// Whether the `>` at `gt` closes a **type-argument list** — `f<T> / 2`, `a.b<T> / 2`,
/// `new C<T> / 2` — so that the `/` after it divides, where after every other `>` (a
/// comparison, a shift, an arrow's `=>`) it opens a regex (`a > /re/.test(b)`).
///
/// The one byte [`is_regex_start_after`] cannot read: the two `>`s are the same byte, and
/// telling them apart is a question about the `<` that opened the list, which only a
/// grammar with type arguments can answer ([`OperandGrammar`]).
///
/// Its arguments are the scanned bytes, the `>`'s offset, and the scan's lower bound, which
/// bounds any backward reach.
pub type ClosesTypeArguments = fn(bytes: &[u8], gt: usize, lower_bound: usize) -> bool;

/// Whether the `)` at `rparen` closes a **statement header** — `if (c)`, `while (c)`,
/// `for (…)`, `with (o)` — after which a statement begins, so a `!` glued to it is a
/// prefix logical not (`if (c)!/re/`), where after every other `)` it is a postfix
/// non-null (`f()! / 2`).
///
/// Asked only of a `)` a glued `!` run follows ([`OperandAnchor`]'s postfix step), never
/// of a plain `) /` pair. Its arguments are the scanned bytes, the `)`'s offset, and the
/// scan's lower bound, where a forward walk to the `)` may begin.
pub type ClosesStatementHeader = fn(bytes: &[u8], rparen: usize, lower_bound: usize) -> bool;

/// Whether a `/` starts a regex literal (rather than a division operator), given
/// `operand_end` — the caller's scan position just past the last **non-trivia**
/// byte it consumed before reaching the `/`.
///
/// Decided by the last non-whitespace byte at or below `operand_end`: a `/`
/// after something that *ends* an expression (an identifier character — ASCII or
/// not — `)`, `]`, a postfix `++`/`--`, a string/template closing quote `'` `"`
/// `` ` ``, a numeric literal's trailing `.`, or a `>` closing a type-argument list)
/// is division; after anything else — or with nothing significant before it — it
/// is a regex. `lower_bound` bounds every walk. A `!` never reaches here: the anchor
/// steps over it ([`OperandAnchor`]'s `at`).
///
/// The `>` is the one byte whose answer the bytes do not hold — `f<T> / 2` divides
/// where `a > /re/` does not — so `closes_type_arguments` is asked about it
/// ([`ClosesTypeArguments`]). `=>` is settled first: an arrow's body is an
/// expression, so a `/` after it opens a regex whatever grammar is scanned.
///
/// The anchor is **handed in, never derived here by looking backward from the `/`** —
/// which is the point: `bytes[operand_end - 1]` is a byte the caller's scan already
/// classified as significant, so neither this function nor the reserved-word walk it
/// delegates to can wander into a comment. (Those walks exist and are what
/// `lower_bound` bounds; what does *not* exist is a search for the anchor itself.)
/// Callers derive the anchor with [`OperandAnchor`], whose own walk is bounded by the
/// last boundary its scan crossed and is sound for exactly the same reason.
///
/// This is the one piece of `/`-disambiguation the trivia cursor deliberately
/// leaves out of [`skip_trivia`]/[`TriviaProfile`]: it needs previous-**token**
/// context, which a stateless forward scan can't carry as a flag. So the
/// depth-tracking scanners that sit at a regex boundary (the printer's paren
/// scan, Svelte's brace matcher, the TS arrow-vs-paren lookahead) carry the
/// anchor themselves, as an [`OperandAnchor`] — which moves past a skipped
/// regex or template, and past a skipped string but **not** a comment
/// ([`trivia_ends_operand`]).
///
/// ⚠️ It takes the anchor rather than the `/`'s own position because an
/// **unbounded** backward walk cannot see trivia: a block comment before the
/// slash (`fn() /* c */ / bb`) puts the `/` of its `*/` in the lookback slot,
/// which ends no operand, so the division read as a regex and the scan ran on
/// to some unrelated delimiter — losing the `)` a paren scan was looking for,
/// and rejecting a Svelte `{…}` tag outright. [`OperandAnchor`]'s walk is not
/// that walk: it stops at the last boundary the scan crossed, so the bytes it
/// reads are only ones the scan already called significant, and the comment
/// case is answered by the stored boundary value instead of by looking.
#[inline]
fn is_regex_start_after(
    bytes: &[u8],
    operand_end: usize,
    lower_bound: usize,
    closes_type_arguments: ClosesTypeArguments,
) -> bool {
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
    match b {
        // A postfix `++`/`--` ends an operand, so the `/` after it DIVIDES
        // (`aa++ / bb`). A lone `+`/`-` is a binary or unary operator, after
        // which a regex may start (`aa + /re/.test(b)`), so the doubling is
        // the whole discriminator.
        b'+' | b'-' => !(j > lower_bound && bytes[j - 1] == b),
        // Bytes that END an expression — a `/` after these is DIVISION. The
        // string/template closing quotes (`'` `"` `` ` ``) belong here: after a
        // literal like `'ab' / 2`, the `/` divides (the anchor sits past the whole
        // string, so this quote can only be its close).
        b')' | b']' | b'\'' | b'"' | b'`' => false,
        // `=>` opens an arrow's expression body (`s => /re/.test(s)`); any other `>`
        // divides only when it closes a type-argument list (`f<T> / 2`), which the
        // grammar-owning resolver says — a comparison or shift (`a > /re/`) does not.
        b'>' => {
            (j > lower_bound && bytes[j - 1] == b'=')
                || !closes_type_arguments(bytes, j, lower_bound)
        }
        // A numeric literal may end in its `.` (`1. / 2`), which no other token
        // can: past a digit the `.` is the literal's, and anything else before
        // it (`...` spread, a member `.` still waiting for its name) is no operand.
        b'.' => !(j > lower_bound && bytes[j - 1].is_ascii_digit()),
        // A non-ASCII identifier character ends an operand exactly as an ASCII one
        // does (`é / 2`). No non-ASCII word is reserved, so no keyword walk follows.
        0x80.. => !ends_with_identifier_char(bytes, operand_end, lower_bound),
        _ => true,
    }
}

/// Whether the character ending at `end` (exclusive) is a **non-ASCII** identifier
/// character — Unicode `XID_Continue`, plus ZWNJ and ZWJ, which ECMAScript admits in
/// an `IdentifierPart` explicitly.
///
/// ⚠️ Conservative by a handful of code points: ECMAScript uses `ID_Continue`, and the
/// few characters in it but not in `XID_Continue` (the NFKC-unstable letters `tsv_ts`'s
/// lexer lists in `is_id_start_not_xid`) read as no identifier here, so a `/` after one
/// still opens a regex — the pre-existing answer, and an over-rejection only.
fn ends_with_identifier_char(bytes: &[u8], end: usize, lower_bound: usize) -> bool {
    // Step back to the character's lead byte: at most three continuation bytes.
    let mut lead = end - 1;
    while lead > lower_bound && end - lead < 4 && bytes[lead] & 0xC0 == 0x80 {
        lead -= 1;
    }
    std::str::from_utf8(&bytes[lead..end])
        .ok()
        .and_then(|s| s.chars().next())
        .is_some_and(|ch| {
            unicode_ident::is_xid_continue(ch) || matches!(ch, '\u{200C}' | '\u{200D}')
        })
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
    // A non-ASCII identifier character before the ASCII tail makes it one longer
    // word (`éin / 2`, whose `in` is no keyword), and no such word is reserved.
    if start > lower_bound
        && bytes[start - 1] >= 0x80
        && ends_with_identifier_char(bytes, start, lower_bound)
    {
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
/// `None` when the body reaches a **line terminator** first: a regex literal
/// cannot hold one — not in its body, its class, or behind a `\`
/// (ecma262 `RegularExpressionNonTerminator`) — so the `/` opened no regex, and
/// the caller reads it as the division it must then be and scans on from the
/// byte after it. That bounds a misread slash to its own line: without it, a `/`
/// the anchor rule wrongly calls a regex's opener swallows everything up to the
/// next `/` in the source, closing brackets and braces included.
///
/// Pairs with [`is_regex_start_after`] — the caller confirms the `/` is a regex
/// before skipping. Caller must ensure `start < end <= bytes.len()`.
#[inline]
#[must_use]
pub fn skip_regex_literal(bytes: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut i = start + 1; // past the opening `/`
    while i < end {
        if is_line_terminator_at(bytes, i) {
            return None;
        }
        match bytes[i] {
            b'\\' if i + 1 < end => {
                // Escape — skip the next byte, which may not be a line terminator.
                if is_line_terminator_at(bytes, i + 1) {
                    return None;
                }
                i += 2;
            }
            b'/' => {
                // Closing `/`; consume trailing flags (ASCII lowercase).
                i += 1;
                while i < end && bytes[i].is_ascii_lowercase() {
                    i += 1;
                }
                return Some(i);
            }
            b'[' => {
                // Character class — a `/` inside is literal; skip to `]`.
                i += 1;
                while i < end {
                    if is_line_terminator_at(bytes, i) {
                        return None;
                    }
                    match bytes[i] {
                        b'\\' if i + 1 < end => {
                            if is_line_terminator_at(bytes, i + 1) {
                                return None;
                            }
                            i += 2;
                        }
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
    Some(end)
}

/// Whether an ECMAScript `LineTerminator` ENDS just before `end` — the backward twin of
/// [`is_line_terminator_at`]. LF and CR are ASCII whitespace too; this adds the three-byte
/// U+2028 / U+2029.
#[inline]
fn line_terminator_ends_at(bytes: &[u8], end: usize) -> bool {
    end >= 3 && matches!(bytes[end - 3..end], [0xE2, 0x80, 0xA8 | 0xA9])
        || end >= 1 && matches!(bytes[end - 1], b'\n' | b'\r')
}

/// Whether an ECMAScript `LineTerminator` begins at `i` — LF, CR, or the UTF-8
/// encodings of U+2028 / U+2029 (`E2 80 A8` / `E2 80 A9`).
#[inline]
fn is_line_terminator_at(bytes: &[u8], i: usize) -> bool {
    match bytes[i] {
        b'\n' | b'\r' => true,
        0xE2 => matches!(bytes.get(i + 1..i + 3), Some([0x80, 0xA8 | 0xA9])),
        _ => false,
    }
}

/// The bytes [`scan_to_matching_close`] hops between: its own delimiter pair, plus every
/// [`TRIVIA_OPENERS`] byte (`` ` `` doubles as the template-literal opener).
const fn hop_needles(open: u8, close: u8) -> [u8; 6] {
    [open, close, b'"', b'\'', b'`', b'/']
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
///
/// `grammar` settles the operand ends the bytes cannot ([`OperandGrammar`]); it is
/// handed on to every nested template.
pub fn scan_to_matching_brace(
    bytes: &[u8],
    scan_start: usize,
    end: usize,
    grammar: OperandGrammar,
) -> Option<usize> {
    scan_to_matching_close::<b'{', b'}'>(bytes, scan_start, end, grammar)
}

/// [`scan_to_matching_brace`] for a `(` — the same expression-context walk (strings,
/// comments, templates and regex literals opaque) matching parens instead: the close of
/// a paren-delimited EXPRESSION a caller must find before parsing it, such as a Svelte
/// `{#each}` key. `scan_start` is the first byte inside the already-open `(`.
pub fn scan_to_matching_paren(
    bytes: &[u8],
    scan_start: usize,
    end: usize,
    grammar: OperandGrammar,
) -> Option<usize> {
    scan_to_matching_close::<b'(', b')'>(bytes, scan_start, end, grammar)
}

/// The walk behind [`scan_to_matching_brace`] and [`scan_to_matching_paren`], one body
/// over its delimiter pair so the two cannot drift.
///
/// Kept out of line (`inline(never)`), chosen by measurement: where this body lands moves
/// the layout of the code around its callers, and out of line it measured the flattest
/// whole run (parse and format together) against the single-delimiter scan it replaced —
/// inlined, the format phase lost more than the parse phase gained.
#[inline(never)]
fn scan_to_matching_close<const OPEN: u8, const CLOSE: u8>(
    bytes: &[u8],
    scan_start: usize,
    end: usize,
    grammar: OperandGrammar,
) -> Option<usize> {
    // A hop may only skip bytes that cannot open trivia — see [`covers_trivia_openers`].
    const { assert!(covers_trivia_openers(&hop_needles(OPEN, CLOSE))) };
    let mut depth: u32 = 1;
    let mut i = scan_start;
    // The anchor `is_regex_start_after` reads, rebuilt where a `/` asks for it. A
    // template literal ends an operand, as does a skipped regex.
    let mut anchor = OperandAnchor::new(scan_start);
    while i < end {
        // Only the hop needles can move this scan; every other byte reaches the
        // `_ => {}` arm below and is stepped over one at a time, so the walk hops
        // between them a word at a time instead of reading each one. That is the
        // byte-scan ladder's top rung (`swar::next_byte_of`), chosen because the
        // *inert* fraction picks a rung — not whether the loop is spelled as a
        // search. Across the Svelte corpus this crosses a mean run of 14.8 bytes
        // with 92% of them in runs of eight or more, and only 8.1% of its hops are
        // adjacent — too few for the one-byte pre-test that pays at `tsv_css`'s
        // `string_end`, whose runs are half empty.
        //
        // The hop is invisible to `anchor`, which is why it is sound: an eager
        // per-byte anchor could not be skipped past, but `OperandAnchor` resolves
        // lazily and *backwards* from the `/` that asks, and its `skipped_*` calls
        // fire only at positions a hop lands on. The bytes crossed here were never
        // its to see.
        //
        // Bounded at `end` for COST, not for correctness: the `i >= end` guard
        // below retires a landing past the window either way (the two hops agree
        // wherever a needle exists inside it, and disagree only above `end`).
        // Callers routinely pass an `end` far short of the source end, and a
        // full-slice hop over a needle-free window would read to the end of the
        // file to learn what `end` already said.
        i = crate::swar::next_byte_of(&bytes[..end], i, hop_needles(OPEN, CLOSE));
        if i >= end {
            break;
        }
        if bytes[i] == b'`' {
            i = skip_template_literal(bytes, i, end, grammar);
            anchor.skipped_operand(i);
            continue;
        }
        if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
            anchor.skipped_trivia(bytes, i, past, scan_start, grammar);
            i = past;
            continue;
        }
        if bytes[i] == b'/'
            && i + 1 < end
            && anchor.starts_regex(bytes, i, scan_start, grammar)
            && let Some(past) = skip_regex_literal(bytes, i, end)
        {
            i = past;
            anchor.skipped_operand(i);
            continue;
        }
        match bytes[i] {
            b if b == OPEN => depth += 1,
            b if b == CLOSE => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
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
///
/// `grammar` is handed on to each interpolation's scan ([`scan_to_matching_brace`]).
pub fn skip_template_literal(
    bytes: &[u8],
    start: usize,
    end: usize,
    grammar: OperandGrammar,
) -> usize {
    let mut i = start + 1; // past the opening backtick
    while i < end {
        match bytes[i] {
            b'\\' if i + 1 < end => i += 2, // escape — skip the next byte
            b'`' => return i + 1,           // closing backtick
            b'$' if i + 1 < end && bytes[i + 1] == b'{' => {
                // `${…}` interpolation — skip its balanced-brace body (which may
                // itself hold nested templates, strings, regex, and braces). Runs
                // just past the matching `}`, or to `end` if unterminated.
                i = scan_to_matching_brace(bytes, i + 2, end, grammar)
                    .map_or(end, |close| close + 1);
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
/// callers that treat a document boundary as a line boundary test for it themselves.
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
        let s = |src: &str| {
            skip_template_literal(src.as_bytes(), 0, src.len(), OperandGrammar::BYTES_ONLY)
        };
        assert_eq!(s("`abc`"), 5); // whole literal
        assert_eq!(s("`abc`de"), 5); // stops at the close, not EOF
        assert_eq!(s(r"`a\`b`"), 6); // an escaped backtick is not the close
    }

    #[test]
    fn skip_template_literal_interpolation_balances_braces() {
        let s = |src: &str| {
            skip_template_literal(src.as_bytes(), 0, src.len(), OperandGrammar::BYTES_ONLY)
        };
        assert_eq!(s("`a${b}c`"), 8); // simple `${…}`
        assert_eq!(s("`${ {x: 1} }`"), 13); // an object literal `}` inside doesn't end it
        assert_eq!(s("`${ `}` }`"), 10); // a `}` inside a NESTED template isn't the close
    }

    #[test]
    fn skip_template_literal_nested_template() {
        // The bug this fixes: `skip_trivia`'s opaque `` ` ``-to-`` ` `` scan mis-pairs
        // across a nested template. `skip_template_literal` recurses through `${…}`,
        // so a nested template — even one holding a lone quote — is skipped whole.
        let s = |src: &str| {
            skip_template_literal(src.as_bytes(), 0, src.len(), OperandGrammar::BYTES_ONLY)
        };
        assert_eq!(s("`${`x`}`"), 8);
        assert_eq!(s(r#"`${`"`}`"#), 8); // nested template holding a `"`
        assert_eq!(s("`${`${`y`}`}`"), 13); // doubly nested
    }

    #[test]
    fn skip_template_literal_unterminated_returns_end() {
        let s = |src: &str| {
            skip_template_literal(src.as_bytes(), 0, src.len(), OperandGrammar::BYTES_ONLY)
        };
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
            // The EAGER rule, stated here rather than borrowed from production: an
            // anchor advances past every significant byte that is not whitespace or
            // a GLUED `!` run's `!`, and those leave it alone. `OperandAnchor` rebuilds
            // the same value lazily, so this loop is its independent oracle.
            if !bytes[i].is_ascii_whitespace() && !is_glued_bang(bytes, i, lower_bound) {
                operand_end = i + 1;
            }
            i += 1;
        }
        is_regex_start_after(bytes, operand_end, lower_bound, never)
    }

    /// The eager statement of a glued `!` run: the `!` at `i` belongs to a run that
    /// either opens the scan or sits right against a byte that is neither whitespace, nor
    /// the end of a line terminator (a line comment's, which the trivia skip consumes),
    /// nor the close of a block comment spanning lines.
    fn is_glued_bang(bytes: &[u8], i: usize, lower_bound: usize) -> bool {
        if bytes[i] != b'!' {
            return false;
        }
        let mut k = i;
        while k > lower_bound && bytes[k - 1] == b'!' {
            k -= 1;
        }
        let multi_line_comment = k >= 2
            && block_comment_start_before(bytes, k - 1, lower_bound)
                .is_some_and(|open| bytes[open..k].contains(&b'\n'));
        k == lower_bound
            || !(bytes[k - 1].is_ascii_whitespace()
                || line_terminator_ends_at(bytes, k)
                || multi_line_comment)
    }

    /// The lazy anchor must agree with the eager rule at every `/` in a source —
    /// the property the per-byte maintenance used to make true by construction.
    #[test]
    fn operand_anchor_matches_the_eager_rule_at_every_slash() {
        let sources = [
            "(a) / b",
            "(a /* c */ / b)",
            "(a // c\n / b)",
            "('s' / b)",
            "(`t` / b)",
            "(   /re/ )",
            "(a = /\\)/)",
            "(a) /* x */ /re/",
            "(/* c */ /re/)",
            "(a /*c*/ /*d*/ / b)",
            "(a\n\t/ b)",
            "(f(x) / y)",
            "(x++ / y)",
            "()/re/",
            "(`${a}` / b)",
            "(a! / b)",
            "(a /* c */! / b)",
            "(a! /* c */ / b)",
            "(!/re/)",
            "(a && !!/re/)",
            "(if (c) !/re/)",
            "(a\n!/re/)",
            "(a ! / b)",
            "(a // c\n!/re/)",
            "(a // c\u{2028}!/re/)",
            "(a /* c */\n!/re/)",
            "(a /* c\n*/!/re/)",
            "(a /** d\n */!/re/)",
            "(a /* c */!/re/)",
            "('s'! / b)",
            "(/a/ / b)",
            "(x + /a/ /* c */ / b)",
        ];
        for src in sources {
            let bytes = src.as_bytes();
            let end = bytes.len();
            let mut anchor = OperandAnchor::new(0);
            let mut eager = 0usize;
            let mut i = 0usize;
            while i < end {
                if let Some(past) = skip_trivia(bytes, i, end, TriviaProfile::JS) {
                    assert_eq!(
                        anchor.at(bytes, i, 0, OperandGrammar::BYTES_ONLY),
                        eager,
                        "{src:?}: anchor disagrees before trivia at {i}"
                    );
                    if trivia_ends_operand(bytes, i) {
                        eager = past;
                    }
                    anchor.skipped_trivia(bytes, i, past, 0, OperandGrammar::BYTES_ONLY);
                    i = past;
                    continue;
                }
                if bytes[i] == b'/' {
                    assert_eq!(
                        anchor.at(bytes, i, 0, OperandGrammar::BYTES_ONLY),
                        eager,
                        "{src:?}: anchor disagrees at the `/` at {i}"
                    );
                    if is_regex_start_after(bytes, eager, 0, never)
                        && let Some(past) = skip_regex_literal(bytes, i, end)
                    {
                        i = past;
                        eager = i;
                        anchor.skipped_operand(i);
                        continue;
                    }
                }
                if !bytes[i].is_ascii_whitespace() && !is_glued_bang(bytes, i, 0) {
                    eager = i + 1;
                }
                i += 1;
            }
        }
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
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(4));
        // Trailing flags are consumed.
        let src = b"/re/gi x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(6));
        // Escaped `/` does not terminate.
        let src = br"/a\/b/ x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(6));
        // A `/` inside a character class is literal, not the terminator.
        let src = b"/[/)]/ x";
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(6));
        // Parens inside are opaque — the returned slice covers the whole literal.
        let src = br"/\)/ y";
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(4));
        // Unterminated → end.
        let src = b"/abc";
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(src.len()));
    }

    #[test]
    fn skip_regex_literal_refuses_a_line_terminator() {
        // A regex body, class or escape cannot hold a line terminator, so a `/` whose
        // "body" reaches one opened no regex at all.
        for src in [
            "/ 2\n/",
            "/ 2\r/",
            "/ 2\u{2028}/",
            "/ 2\u{2029}/",
            "/[ 2\n]/",
            "/\\\n/",
            "/[\\\n]/",
        ] {
            let bytes = src.as_bytes();
            assert_eq!(skip_regex_literal(bytes, 0, bytes.len()), None, "{src:?}");
        }
        // Any other non-ASCII character is ordinary body.
        let src = "/\u{2027}/ x".as_bytes();
        assert_eq!(skip_regex_literal(src, 0, src.len()), Some(5));
    }

    #[test]
    fn a_skipped_regex_literal_ends_an_operand() {
        // After a regex literal with no flags its closing `/` is the last byte, which
        // alone reads as an operator; the anchor knows it closed a literal.
        let src = b"x + /a/ /* c */ / 2 / 3";
        let end = src.len();
        let mut anchor = OperandAnchor::new(0);
        assert!(anchor.starts_regex(src, 4, 0, OperandGrammar::BYTES_ONLY));
        let past = skip_regex_literal(src, 4, end).unwrap();
        assert_eq!(past, 7);
        anchor.skipped_operand(past);
        anchor.skipped_trivia(src, 8, 15, 0, OperandGrammar::BYTES_ONLY);
        assert!(!anchor.starts_regex(src, 16, 0, OperandGrammar::BYTES_ONLY));
    }

    #[test]
    fn is_regex_start_after_the_new_operand_ends() {
        let div = |src: &str| !regex_at(src, src.len() - 1, 0);
        // A postfix non-null ends no operand of its own — the operand before it does.
        assert!(div("a! /"));
        assert!(div("f()!! /"));
        assert!(div("a /* c */! /"));
        assert!(div("a! /* c */ /"));
        // A prefix logical not starts none — the operator before it governs — and a `!`
        // run with whitespace before it is prefix, whatever precedes the whitespace.
        assert!(!div("!/"));
        assert!(!div("a && !/"));
        assert!(!div("return !/"));
        assert!(!div("if (c) !/"));
        assert!(!div("a\n!/"));
        assert!(!div("a ! /"));
        // A line comment ends a line, so a run after it is not glued to anything.
        assert!(!div("a // c\n!/"));
        assert!(!div("return a // c\n!/"));
        assert!(!div("a // c\u{2028}!/"));
        // A block comment does not, so a run glued to its close is postfix.
        assert!(div("a /* c */! /"));
        // A numeric literal's trailing dot ends an operand; a spread does not.
        assert!(div("1. /"));
        assert!(!div("[.../"));
        // A non-ASCII identifier character ends an operand; a word it prefixes is no
        // keyword.
        assert!(div("é /"));
        assert!(div("aé /"));
        assert!(div("éin /"));
        assert!(!div("in /"));
        // A non-ASCII character that is no identifier part leaves the regex answer.
        assert!(!div("\u{a0}/"));
    }

    #[test]
    fn a_bang_glued_to_a_multi_line_block_comment_is_prefix() {
        // The anchor as a scan leaves it after stepping over the comment opening at 2.
        let regex = |src: &str| {
            let bytes = src.as_bytes();
            let open = 2;
            let close = skip_trivia(bytes, open, bytes.len(), TriviaProfile::JS).unwrap();
            let mut anchor = OperandAnchor::new(0);
            anchor.skipped_trivia(bytes, open, close, 0, OperandGrammar::BYTES_ONLY);
            anchor.starts_regex(bytes, bytes.len() - 1, 0, OperandGrammar::BYTES_ONLY)
        };
        // A comment that spans lines is a line break: the run begins a statement.
        assert!(regex("a /* c\n*/!/"));
        assert!(regex("a /** d\n */!/"));
        assert!(regex("a /* c\u{2028}*/!/"));
        // One that does not leaves the run glued to the operand before it.
        assert!(!regex("a /* c */! /"));
    }

    #[test]
    fn a_bang_glued_to_a_statement_headers_paren_asks_the_grammar() {
        // A toy grammar whose only header is `if (c)`.
        let header: ClosesStatementHeader =
            |bytes, rparen, lower_bound| bytes[lower_bound..rparen].ends_with(b"if (c");
        let grammar = OperandGrammar {
            closes_type_arguments: never,
            closes_statement_header: header,
        };
        let regex = |src: &str, grammar: OperandGrammar| {
            let bytes = src.as_bytes();
            OperandAnchor::new(0).starts_regex(bytes, bytes.len() - 1, 0, grammar)
        };
        // After a header's `)`, a glued run is a prefix `!` and the `/` opens a regex.
        assert!(regex("if (c)!/", grammar));
        assert!(regex("if (c)!!/", grammar));
        // After any other `)` it is postfix, and the `/` divides.
        assert!(!regex("f(c)! /", grammar));
        assert!(!regex("f()! /", grammar));
        // A grammar that finds no header reads every glued run as postfix.
        assert!(!regex("if (c)!/", OperandGrammar::BYTES_ONLY));
    }

    #[test]
    fn is_regex_start_after_a_gt_asks_the_resolver() {
        let bytes = b"f<T> /";
        let closes: ClosesTypeArguments = |_, gt, _| gt == 3;
        // Without a type-argument grammar every `>` is an operator.
        assert!(is_regex_start_after(bytes, 4, 0, never));
        assert!(!is_regex_start_after(bytes, 4, 0, closes));
        // An arrow's `=>` opens an expression whatever the resolver says.
        let arrow = b"s => /";
        let always: ClosesTypeArguments = |_, _, _| true;
        assert!(is_regex_start_after(arrow, 4, 0, always));
    }
}
