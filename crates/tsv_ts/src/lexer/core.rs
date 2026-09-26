// Core lexer implementation

use super::comments;
use super::escapes;
use super::ident::{is_id_continue, is_id_start};
use super::lex_err;
use super::token::{Token, TokenKind, keyword_at};
use tsv_lang::ParseError;

/// The one message for a `NumericLiteralSeparator` inside a leading-zero literal —
/// raised both for a `_` directly after the `0` (`0_0`) and for one inside the
/// integer run of either legacy form (`08_1`, `010_1`). The
/// `LegacyOctalLikeDecimalIntegerLiteral` productions carry no `[Sep]` parameter,
/// so neither spelling is grammatical in any mode.
const LEGACY_SEPARATOR_MSG: &str =
    "Numeric separators are not allowed in legacy octal-like literals";

/// Byte length of the UTF-8 sequence whose lead byte is `lead`. Used to advance
/// the byte cursor past one character without decoding it.
#[inline]
const fn utf8_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// 256-entry lookup tables for the ASCII identifier-class fast paths. Each entry is
/// computed from the same predicate the byte tests below expand to, so the tables are
/// exact — a lookup replaces the range/eq OR-chain with one L1 load on the hot
/// per-character identifier-body loop.
const ID_START_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphabetic() || b == b'_' || b == b'$';
        i += 1;
    }
    t
};
const ID_CONTINUE_LUT: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        t[i] = b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        i += 1;
    }
    t
};

/// ASCII subset of `ID_Start` (`a-z A-Z _ $`) — the byte-cursor fast path before
/// falling back to the full Unicode `is_id_start` on a decoded char.
#[inline]
const fn is_ascii_id_start(b: u8) -> bool {
    ID_START_LUT[b as usize]
}

/// ASCII subset of `ID_Continue` (`a-z A-Z 0-9 _ $`) — the byte-cursor fast path
/// for an identifier body before falling back to the full Unicode `is_id_continue`.
#[inline]
const fn is_ascii_id_continue(b: u8) -> bool {
    ID_CONTINUE_LUT[b as usize]
}

/// The byte length of the `UnicodeEscapeSequence` beginning at `bytes[pos]`, or `0` when none
/// does — the length-only view of [`try_decode_unicode_escape`], for the parser's lookahead
/// byte scans, which need to step over an escaped name without caring what it decodes to.
///
/// Deliberately the lexer's own predicate rather than a shape test of its own: an escape the
/// lexer rejects (a value past `U+10FFFF`, a lone surrogate — neither is an identifier
/// character) must end a lookahead's name run too, or the scan reports a name the lexer will
/// not produce. What it does NOT judge is the decoded character's `ID_Start` / `ID_Continue`
/// class, matching the lookahead tables' standing looseness about every byte `> 127`.
#[inline]
pub(crate) fn unicode_escape_len_at(bytes: &[u8], pos: usize) -> usize {
    match try_decode_unicode_escape(bytes, pos) {
        Some((_, len)) => len,
        None => 0,
    }
}

/// Try to decode a unicode escape sequence at the given position.
/// Returns Some((decoded_char, bytes_consumed)) if valid, None otherwise.
///
/// Handles both `\uXXXX` (4-digit) and `\u{X...}` (braced) formats.
///
/// The **one** answer to "is there an escape here, and how far does it run" — the parser's
/// lookahead byte scans ask it through [`unicode_escape_len_at`], which drops the decoded
/// `char` and keeps the length. They must not restate the shape: a lookahead that reads an
/// escape the lexer does not (or stops short of one it does) mis-classifies the construct it
/// is looking at, which is how an escaped name came to be an identifier here and not one
/// there. Takes bytes rather than `&str` so both callers can share it without a re-slice —
/// the lexer passes its own cached `Lexer::bytes`, the scans the slice they already hold.
///
/// Stays private: the crate-visible surface is [`unicode_escape_len_at`] alone, so a caller
/// outside this module cannot reach for the decoded `char` and re-derive an identifier-class
/// verdict beside the lexer's.
fn try_decode_unicode_escape(bytes: &[u8], start: usize) -> Option<(char, usize)> {
    // Need at least \u
    if start + 2 > bytes.len() || bytes[start] != b'\\' || bytes[start + 1] != b'u' {
        return None;
    }

    let after_u = start + 2;

    if after_u < bytes.len() && bytes[after_u] == b'{' {
        // Braced format: \u{XXXX}
        let content_start = after_u + 1;
        let mut end = content_start;
        while end < bytes.len() && bytes[end] != b'}' {
            if !bytes[end].is_ascii_hexdigit() {
                return None;
            }
            end += 1;
        }
        if end >= bytes.len() || end == content_start {
            return None;
        }
        // `CodePoint :: HexDigits but only if MV of HexDigits ≤ 0x10FFFF` caps the
        // VALUE, not the digit count — leading zeros are unbounded, so
        // `\u{0000000000000061}` is a valid `a`. Accumulate in `u64` and stop once
        // past the cap: an arbitrarily long escape then can't overflow, while the
        // check below still reads it as out of range. `u32::from_str_radix` over the
        // whole run cannot do this — it overflows to `None` at 9 digits, so dropping
        // the digit-count test alone would have moved the over-rejection, not fixed
        // it. Mirrors the string-literal path in `lexer/escapes.rs`.
        let mut code: u64 = 0;
        for &b in &bytes[content_start..end] {
            if code <= 0x10FFFF {
                code = code * 16 + u64::from((b as char).to_digit(16)?);
            }
        }
        if code > 0x10FFFF {
            return None;
        }
        // Unlike the string path, a lone surrogate is NOT substituted here: it is not
        // an identifier character, so `None` (reject) is what acorn does too.
        let ch = char::from_u32(code as u32)?;
        Some((ch, end + 1 - start)) // +1 for closing brace
    } else {
        // 4-digit format: \uXXXX
        if after_u + 4 > bytes.len() {
            return None;
        }
        // `to_digit` IS the hex validation — a non-hex byte, ASCII or not, yields `None` and
        // returns — so a separate `is_ascii_hexdigit` pass would only restate it. The braced
        // branch above keeps its own pass for two reasons this fixed-width one has neither of:
        // it BOUNDS the `}` search (a stray `\u{` in arbitrary source would otherwise scan to
        // the next `}` anywhere in the file — this is called at every `\` a lookahead crosses),
        // and its accumulation stops once the value is past the cap, so the digits after that
        // point reach no `to_digit` of their own.
        let mut code: u32 = 0;
        for &b in &bytes[after_u..after_u + 4] {
            code = code * 16 + (b as char).to_digit(16)?;
        }
        let ch = char::from_u32(code)?;
        Some((ch, 6)) // \uXXXX is 6 bytes
    }
}

/// How `scan_template_body` stopped: at a closing `` ` ``, at the `${` of an
/// interpolation, or at EOF (unterminated).
enum TemplateStop {
    Backtick,
    Interpolation,
    Eof,
}

/// A [`Lexer`] cursor position with the state that travels with it, taken by
/// [`Lexer::checkpoint`] and restored by [`Lexer::rewind`]. Opaque: only the lexer
/// reads it, and only to put itself back.
#[derive(Clone, Copy)]
pub struct LexerCheckpoint {
    position: usize,
    template_depth: u32,
    had_line_terminator: bool,
    /// Restored rather than left: a slot the abandoned tokens filled names a real
    /// non-plain start and would never be *wrong*, but the two-slot window's
    /// "at most one token ahead of the parser" reading is simplest when the
    /// window is exactly what it was.
    nonplain_ident_starts: [u32; 2],
}

pub struct Lexer<'a> {
    source: &'a str,
    /// The source as raw bytes (`source.as_bytes()`), cached so the hot dispatch
    /// peeks a byte without re-deriving the slice. Char decoding (non-ASCII branches)
    /// goes through `source` at `position`.
    bytes: &'a [u8],
    position: usize,
    /// Stack for tracking template literal nesting depth.
    /// When we enter a template interpolation `${`, we push to this stack.
    /// When we see `}`, if the stack is non-empty, we continue template reading.
    template_depth: u32,
    /// True if a line terminator was encountered while skipping whitespace to reach
    /// the current token. Used for Automatic Semicolon Insertion (ASI).
    /// Reset at start of skip_whitespace(), set when line terminators are found.
    had_line_terminator: bool,
    /// Out-of-band decoded value for the token just produced — populated only on the
    /// rare escape path (strings/templates with escapes, escaped identifiers). Kept
    /// off `Token` so the hot per-token value stays a 16-byte POD.
    ///
    /// The decoded bytes live in `decode_scratch`, a buffer parked on the lexer and
    /// **reused across the file** (cleared per escape, capacity retained), so no
    /// per-literal `String` allocates — the escaped-string decode churn a fresh
    /// `String` (plus its `Box`) produced per token is gone. `has_decoded` is the
    /// presence flag `decoded_str` reads; it is cleared at the top of every
    /// token-producing entry point (`next_token_into_local`, `continue_template_from_brace`,
    /// `read_regex_literal`) so it reflects only the current token, and set by the
    /// escape paths. The scratch is never read while `has_decoded` is false, so its
    /// stale contents are inert.
    decode_scratch: String,
    has_decoded: bool,
    /// The start offsets of the last two identifier-or-keyword tokens whose raw bytes
    /// were NOT plain one-column ASCII — a byte at or above `0x80` somewhere in the
    /// token — most recent first, `u32::MAX` (no token starts there) while unset.
    /// Written only by the identifier scan's general reader ([`Lexer::scan_identifier_into`]
    /// and [`Lexer::scan_identifier_tail`]), and only on the two branches that consume a
    /// non-ASCII char, so the common identifier's path
    /// ([`Lexer::scan_ascii_identifier_into`]) records nothing.
    /// Three names in 483,358 take those branches on a real corpus; carrying the same
    /// fact as a per-identifier `bool` instead — seeded, kept live across the scan
    /// loop, stored, and saved past the parser's lookahead — measured **+0.47%** of a
    /// parse run for work the parse path never spends. Keying the RARE event makes the
    /// record free.
    ///
    /// Read by [`Lexer::ident_is_plain_ascii`]: a token is plain iff its start is in
    /// neither slot. Two slots, not one, because the parser reads a name while the lexer
    /// may already have produced ONE further token (its single-token lookahead), and that
    /// token may itself be non-plain — it takes slot 0 and the current name's start
    /// survives in slot 1. The lexer is never more than one token ahead of the parser's
    /// `current` (every relex re-seats it at a non-identifier token and clears or keeps
    /// the one peek), which is the whole invariant. A stale offset is never wrong: a key
    /// names a byte offset whose identifier scan saw a non-ASCII char, and the same bytes
    /// scan the same way under any tokenization.
    ///
    /// The printer spends it: a plain-ASCII name has a visual width equal to its byte
    /// length — an identifier can hold no `\t` or `\n` — so the width scan
    /// `DocArena::source_span` runs for every other source slice is skipped entirely.
    nonplain_ident_starts: [u32; 2],
    /// Byte offset of `source` within the document this lexer's ERRORS are rendered
    /// against — zero for a standalone file, the island start for a Svelte `<script>`.
    /// Token positions are unaffected (the parser shifts those itself when it builds
    /// spans); this exists solely so an error position names a place in the document the
    /// reader is looking at. See [`Lexer::host_err`].
    base_offset: usize,
}

/// Returns true if `c` is an ECMAScript **WhiteSpace** code point (ES spec
/// `sec-white-space`, `table-white-space-code-points`): `<TAB>`, `<VT>`, `<FF>`,
/// `<ZWNBSP>` (U+FEFF), and every `Space_Separator` (Unicode category `Zs`,
/// which includes `<SP>` and `<NBSP>`).
///
/// This is deliberately **not** Rust's `char::is_whitespace()` (the Unicode
/// `White_Space` property), which differs in both directions: it omits U+FEFF
/// and includes U+0085 (`<NEL>`), neither of which ECMAScript treats as
/// WhiteSpace. The spec says so explicitly — WhiteSpace "intentionally excludes
/// all code points that have the Unicode 'White_Space' property but which are
/// not classified in general category 'Space_Separator'".
///
/// LineTerminators are the separate [`is_es_line_terminator`] production,
/// matched ahead of this in [`skip_whitespace_general`] because a newline drives
/// ASI — so they are intentionally absent here. A scan that wants "the next
/// token starts where?" wants the **union** of the two, which is what JS's `\s`
/// means; the parser's lookahead (`parser::scan::skip_whitespace`) asks for both.
pub(crate) const fn is_es_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'              // <TAB>
        | '\u{000B}'            // <VT>
        | '\u{000C}'            // <FF>
        | '\u{FEFF}'            // <ZWNBSP>
        // <USP>: Unicode category Zs (Space_Separator)
        | '\u{0020}'            // SPACE
        | '\u{00A0}'            // NO-BREAK SPACE
        | '\u{1680}'
        | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// Returns true if `c` is an ECMAScript **LineTerminator** (ES spec
/// `sec-line-terminators`, `table-line-terminator-code-points`): `<LF>`, `<CR>`,
/// `<LS>` (U+2028) and `<PS>` (U+2029) — and nothing else. The spec is explicit
/// that "other new line or line breaking Unicode code points are not treated as
/// line terminators", so U+0085 (`<NEL>`) is not one (nor is it WhiteSpace).
///
/// The sibling of [`is_es_whitespace`], and the reason the two are separate: a
/// line terminator ends a `SingleLineComment`, drives ASI, and is forbidden
/// outright at a `[no LineTerminator here]` restriction (`ArrowParameters [no
/// LineTerminator here] '=>'`), so the lexer must record that it crossed one
/// rather than merely skip it. Callers that only need "where does the next token
/// start" take the union of both productions.
pub(crate) const fn is_es_line_terminator(c: char) -> bool {
    matches!(c, '\u{000A}' | '\u{000D}' | '\u{2028}' | '\u{2029}')
}

/// [`is_es_line_terminator`] over a **byte cursor**: whether a LineTerminator
/// starts at `bytes[pos]`, without decoding a `char`.
///
/// The byte-level form of the same production, living beside the char-level one
/// so the two cannot drift — three scanners had hand-rolled the `e2 80 a8`/`a9`
/// peek separately, which is the duplication that let the sibling whitespace
/// class go narrow unnoticed.
///
/// Byte-at-a-time is sound: neither LF, CR nor `0xe2` ever appears as a UTF-8
/// continuation byte (those are `0x80..=0xbf`), and in valid UTF-8 `0xe2` is
/// always a 3-byte lead — so a scan that steps one byte at a time still lands
/// this peek on a character boundary, and callers need no decode.
///
/// `pos` must be in bounds; the LS/PS peek does its own bounds check.
#[inline]
pub(crate) fn is_es_line_terminator_at(bytes: &[u8], pos: usize) -> bool {
    match bytes[pos] {
        b'\n' | b'\r' => true,
        // <LS> U+2028 / <PS> U+2029 — the only non-ASCII LineTerminators, and
        // the only `e2 80 a8`/`a9` sequences.
        0xE2 => {
            pos + 2 < bytes.len() && bytes[pos + 1] == 0x80 && matches!(bytes[pos + 2], 0xA8 | 0xA9)
        }
        _ => false,
    }
}

/// The lead bytes of [`is_es_line_terminator_at`]'s class — the loose superset a
/// word-at-a-time scan searches for, one byte per production: `<LF>`, `<CR>`, and
/// the `0xE2` that opens `<LS>` / `<PS>`.
///
/// A hit on `0xE2` is a **candidate**, not a terminator, so every caller re-tests it
/// with [`is_es_line_terminator_at`] and resumes the scan when it fails — the
/// loose-class-plus-exact-fallback shape `tsv_lang::swar` documents. The pair lives
/// here, next to the exact class, because a second spelling of either is how the three
/// hand-rolled `<LS>` / `<PS>` peeks drifted apart before; the relation is asserted by
/// a test beside them.
pub(crate) const ES_LINE_TERMINATOR_LEADS: [u8; 3] = [b'\n', b'\r', 0xE2];

/// [`Lexer::skip_whitespace`] from the non-ASCII byte at `pos` of `source` on: every
/// character is decoded and classified against the Unicode rules — LS / PS are the
/// non-ASCII LineTerminators, NBSP / U+FEFF / the `Zs` spaces the non-ASCII WhiteSpace —
/// until one is neither. Returns where that character starts and whether a LineTerminator
/// was crossed on the way.
///
/// A free function over the source rather than a `&mut self` method, so the call cannot
/// write the lexer as far as the optimizer knows and the handoff keeps its slice and
/// cursor across it; the caller records the crossing.
#[cold]
#[inline(never)]
fn skip_whitespace_general(source: &str, mut pos: usize) -> (usize, bool) {
    let mut crossed = false;
    loop {
        match source[pos..].chars().next() {
            Some(c) if is_es_line_terminator(c) => {
                crossed = true;
                pos += c.len_utf8();
            }
            Some(c) if is_es_whitespace(c) => pos += c.len_utf8(),
            _ => return (pos, crossed),
        }
    }
}

impl<'a> Lexer<'a> {
    /// A lexer over `source`, which sits at `base_offset` in the document its errors will
    /// be rendered against — `0` when that document *is* `source`, the island start for a
    /// Svelte `<script>`. See [`Lexer::host_err`].
    ///
    /// The offset is a required argument rather than a `new(source)` default because a
    /// silent zero is the failure mode: a slice lexer that reports its own coordinates
    /// hands the renderer a position it reads as the document's, pointing the caret at an
    /// unrelated line.
    pub fn at_offset(source: &'a str, base_offset: usize) -> Self {
        let bytes = source.as_bytes();
        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes, and the
        // WIRE keeps them (`LeadingBom::Counted` in the writer): acorn reads the BOM as
        // whitespace, so its offsets index the author's string, BOM included.
        let position = tsv_lang::leading_bom_len(source);

        Self {
            source,
            bytes,
            position,
            template_depth: 0,
            had_line_terminator: false,
            decode_scratch: String::new(),
            has_decoded: false,
            nonplain_ident_starts: [u32::MAX; 2],
            base_offset,
        }
    }

    /// Lift an error this lexer produced into the coordinates of the document it will be
    /// rendered against (`ParseError::shift_position`).
    ///
    /// Applied at each entry point that PRODUCES an error — [`Lexer::next_token_into`],
    /// [`Lexer::read_regex_literal`], [`Lexer::continue_template_from_brace`] — and
    /// never in a wrapper that delegates to one of them ([`Lexer::next_token`],
    /// [`Lexer::seek_and_next_token`]), which would shift twice and lose the caret
    /// altogether.
    #[cold]
    #[inline(never)]
    fn host_err(&self, err: ParseError) -> ParseError {
        err.shift_position(self.base_offset)
    }

    /// The byte at the cursor, or `None` at EOF.
    #[inline]
    fn cur_byte(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /// The byte `offset` bytes ahead of the cursor, or `None` past EOF.
    #[inline]
    fn byte_ahead(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.position + offset).copied()
    }

    /// Decode the full character at the cursor (for the non-ASCII branches);
    /// `None` at EOF. ASCII paths use `cur_byte` and never call this.
    #[inline]
    fn cur_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    /// Whether the cursor is on a Unicode line separator — LS (U+2028) or PS
    /// (U+2029), the two non-ASCII LineTerminators. Both lead with byte `0xE2`,
    /// so callers gate on `byte >= 0x80` before this decodes.
    #[inline]
    fn at_line_separator(&self) -> bool {
        matches!(self.cur_char(), Some('\u{2028}' | '\u{2029}'))
    }

    /// The decoded value produced for the most recently lexed token (escape paths
    /// only); `None` for the common escape-free token. Borrows the parked
    /// `decode_scratch` — valid until the next token is lexed (which may overwrite
    /// it), so the parser copies it into its AST arena immediately after each lex
    /// (`Parser::decoded_to_arena`) rather than holding the borrow.
    #[inline]
    pub fn decoded_str(&self) -> Option<&str> {
        if self.has_decoded {
            Some(&self.decode_scratch)
        } else {
            None
        }
    }

    /// Whether the raw bytes of the identifier-or-keyword token starting at `start`
    /// (lexer-relative, as token spans are) are plain one-column ASCII. Exact for the
    /// parser's current token and for its one lookahead token — see
    /// `nonplain_ident_starts` for why two slots suffice and what would break it.
    #[inline]
    pub fn ident_is_plain_ascii(&self, start: u32) -> bool {
        let [k0, k1] = self.nonplain_ident_starts;
        start != k0 && start != k1
    }

    /// Record that the identifier scan starting at `start` consumed a non-ASCII char.
    /// Idempotent per token: a token whose first char AND a later char are non-ASCII
    /// calls this twice, and pushing it twice would evict the previous token's record
    /// while the parser may still be reading that token's name.
    #[cold]
    #[inline(never)]
    fn note_nonplain_ident(&mut self, start: usize) {
        let start = start as u32;
        if self.nonplain_ident_starts[0] != start {
            self.nonplain_ident_starts[1] = self.nonplain_ident_starts[0];
            self.nonplain_ident_starts[0] = start;
        }
    }

    /// Returns true if a line terminator was encountered while skipping to the current token.
    /// Used for ASI (Automatic Semicolon Insertion).
    pub fn had_line_terminator(&self) -> bool {
        self.had_line_terminator
    }

    /// The cursor state a later [`Lexer::rewind`] returns to — the parser's one
    /// speculative parse (`Parser::parse_arrow_or_rewind`) takes it before the arrow
    /// head it may have to un-read. Everything a token advances except the decode
    /// scratch, which the rewind marks stale instead (see `rewind`).
    #[must_use]
    pub fn checkpoint(&self) -> LexerCheckpoint {
        LexerCheckpoint {
            position: self.position,
            template_depth: self.template_depth,
            had_line_terminator: self.had_line_terminator,
            nonplain_ident_starts: self.nonplain_ident_starts,
        }
    }

    /// Return the cursor to `checkpoint`, un-reading every token lexed since. The
    /// decode scratch is left as the abandoned tokens overwrote it and flagged
    /// absent: the parser drains a decoded value into its arena at the lex that
    /// produced it, so nothing reads the scratch for a token it did not just lex,
    /// and the next token-producing call resets the flag either way.
    pub fn rewind(&mut self, checkpoint: LexerCheckpoint) {
        let LexerCheckpoint {
            position,
            template_depth,
            had_line_terminator,
            nonplain_ident_starts,
        } = checkpoint;
        self.position = position;
        self.template_depth = template_depth;
        self.had_line_terminator = had_line_terminator;
        self.nonplain_ident_starts = nonplain_ident_starts;
        self.has_decoded = false;
    }

    /// Seek to a specific position and re-lex from there.
    /// Used when splitting compound tokens like `>=` into `>` + `=`.
    pub fn seek_and_next_token(&mut self, position: usize) -> Result<Token, ParseError> {
        self.set_position(position);
        self.next_token()
    }

    /// Reset the cursor to an absolute byte position (must be a char boundary).
    #[inline]
    fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    /// Advance the cursor past the current character (1 byte for ASCII, more for
    /// a multi-byte UTF-8 sequence). No-op at EOF.
    ///
    /// ⚠️ **`inline(always)`, not `inline`.** Five instructions in the lexer's
    /// innermost loop, and the plain hint still left an out-of-line copy that
    /// the scanners *called* — visible as its own board symbol. Forcing it in
    /// measures `instructions:u` **−0.33…−0.39%** on the TS-heavy corpora and
    /// −0.14% on pure `.svelte`, against a pure-CSS null control at **−0.000%**
    /// (`tsv_css` lexes with its own scanner and never reaches this), and it
    /// makes `.text` **528 B smaller** — the out-of-line copy and every call
    /// site's argument setup both go away.
    #[expect(clippy::inline_always)]
    #[inline(always)]
    fn advance(&mut self) {
        if let Some(&b) = self.bytes.get(self.position) {
            self.position += utf8_len(b);
        }
    }

    /// Advance the cursor past the ASCII byte under it — the dispatch's form of
    /// [`Lexer::advance`], for a byte it has already matched. The dispatch matches the byte
    /// [`Lexer::skip_whitespace`] handed it, not one it read through `self`, so `advance`'s
    /// own read of the byte would no longer fold into the match: every punctuator would
    /// re-read it and decode the length of a UTF-8 sequence it knows is one byte.
    #[inline]
    fn advance_ascii(&mut self) {
        debug_assert!(self.bytes[self.position].is_ascii());
        self.position += 1;
    }

    /// Create a token with the current position as end
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        }
    }

    /// Lex the identifier-or-keyword token opened by the ASCII `IdentifierStart` byte at
    /// `start` (`a-z A-Z _ $`) into `*dst` — the whole path of the common identifier.
    ///
    /// A table walk takes the ASCII `IdentifierPart` run (`[a-zA-Z0-9_$]`); its stop byte
    /// then settles the name: at end of input, or on an ASCII byte other than `\`, the
    /// name ends at the run, escape-free (nothing to decode — the dispatch cleared
    /// `has_decoded` before it got here), and one keyword lookup over its bytes gives the
    /// kind. Only a `\` or a non-ASCII byte can continue it, and those go to the cold
    /// [`Lexer::continue_ascii_identifier_into`]; when that byte turns out not to continue
    /// the name either (`if\x`, `return` + NBSP), the name is still the ASCII run and is
    /// finished here, keyword lookup included — so the reserved-word compare tree is
    /// reached from this one place, and inlines into the dispatch with the
    /// letter-and-length pre-filter. (Outlining the compare tree behind the inline
    /// pre-filter measured slower: the pre-filter admits most names, and each admitted
    /// name then paid a call.) The path relies on the plain `#[inline]` being honored:
    /// it inlines into the dispatch, and a call here is the first thing to check if the
    /// dispatch grows.
    ///
    /// `bytes` is the slice [`Lexer::skip_whitespace`] read the start byte from, passed
    /// down rather than re-read from `self` so the run is walked against the same length
    /// the handoff checked.
    #[inline]
    fn scan_ascii_identifier_into(&mut self, bytes: &'a [u8], start: usize, dst: &mut Token) {
        // An index, not a `get`: the handoff proved `start` in bounds where the run's trip
        // count below cannot see it, and without the proof the run takes an extra
        // instruction a byte and the keyword pre-filter re-checks its read of
        // `bytes[start]`. The index restates it once, ahead of both.
        let first = bytes[start];
        debug_assert!(is_ascii_id_start(first));
        let mut end = start + 1;
        while end < bytes.len() && is_ascii_id_continue(bytes[end]) {
            end += 1;
        }
        if bytes
            .get(end)
            .is_some_and(|&stop| stop >= 0x80 || stop == b'\\')
            && self.continue_ascii_identifier_into(start, end, dst)
        {
            return;
        }
        self.position = end;
        *dst = Token {
            kind: match keyword_at(bytes, start, end - start) {
                Some(kw) => TokenKind::Keyword(kw),
                None => TokenKind::Identifier,
            },
            start: start as u32,
            end: end as u32,
        };
    }

    /// [`Lexer::scan_ascii_identifier_into`]'s general reader, for an ASCII run
    /// `start..end` whose stop byte is a `\` or a non-ASCII byte. When that byte continues
    /// the name (a unicode escape decoding to an `IdentifierPart`, or a non-ASCII
    /// `IdentifierPart` char), reads the rest of it through
    /// [`Lexer::scan_identifier_tail`], writes the token and returns `true`; otherwise
    /// returns `false` with the cursor at `end`, leaving the caller to finish the ASCII
    /// name.
    #[cold]
    #[inline(never)]
    fn continue_ascii_identifier_into(
        &mut self,
        start: usize,
        end: usize,
        dst: &mut Token,
    ) -> bool {
        self.set_position(end);
        let decoded = self.scan_identifier_tail(start, None);
        if self.position == end {
            return false;
        }
        self.finish_general_identifier_into(start, decoded, dst);
        true
    }

    /// Scan an identifier that begins with a unicode escape or a non-ASCII
    /// `IdentifierStart` char — the dispatch's two identifier arms other than the ASCII
    /// one ([`Lexer::scan_ascii_identifier_into`]).
    ///
    /// ECMAScript allows unicode escapes in identifiers:
    /// - `\u0066oo` → identifier `foo`
    /// - `\u{41}` → identifier `A`
    /// - `b\u0061r` → identifier `bar`
    ///
    /// The decoded name is parked in the lexer's `decode_scratch` when escapes are present
    /// (read back through `decoded_str`). Prettier normalizes these to their decoded form.
    fn scan_identifier_into(
        &mut self,
        first_char: char,
        dst: &mut Token,
    ) -> Result<(), ParseError> {
        let start = self.position;
        // None until an actual escape is decoded; `decoded.is_some()` ⇔ has-escapes.
        let mut decoded: Option<String> = None;

        if first_char == '\\' {
            // First char is a unicode escape
            if let Some((ch, len)) = try_decode_unicode_escape(self.bytes, self.position) {
                if !is_id_start(ch) {
                    return Err(lex_err(
                        format!("Invalid identifier start character from unicode escape: '{ch}'"),
                        start,
                    ));
                }
                decoded = Some(String::from(ch));
                // Advance by the escape sequence length
                for _ in 0..len {
                    self.advance();
                }
            } else {
                return Err(lex_err("Invalid unicode escape in identifier", start));
            }
        } else {
            // A non-ASCII start char: the raw token is not plain ASCII.
            self.advance();
            self.note_nonplain_ident(start);
        }

        let decoded = self.scan_identifier_tail(start, decoded);
        self.finish_general_identifier_into(start, decoded, dst);
        Ok(())
    }

    /// The general identifier loop: consume `IdentifierPart` chars and unicode escapes from
    /// the cursor, for the identifier that began at `start`, and return its decoded name —
    /// `decoded` as passed, extended by every char read, or materialized from the literal
    /// prefix `start..` at the first escape (`None` while no escape has been read).
    ///
    /// Whether the raw token is plain ASCII is recorded on the one branch here that
    /// consumes a non-ASCII char (`note_nonplain_ident`) — the other is
    /// [`Lexer::scan_identifier_into`]'s non-ASCII start — so the common identifier's
    /// path, [`Lexer::scan_ascii_identifier_into`], carries no flag, no register and no
    /// store for it.
    fn scan_identifier_tail(
        &mut self,
        start: usize,
        mut decoded: Option<String>,
    ) -> Option<String> {
        loop {
            match self.cur_byte() {
                Some(b'\\') => {
                    // Potential unicode escape in identifier
                    if let Some((ch, len)) = try_decode_unicode_escape(self.bytes, self.position) {
                        if !is_id_continue(ch) {
                            // Not a valid identifier continue char, stop here
                            break;
                        }
                        // First escape: everything consumed so far was literal.
                        decoded
                            .get_or_insert_with(|| self.source[start..self.position].to_string())
                            .push(ch);
                        for _ in 0..len {
                            self.advance();
                        }
                    } else {
                        // Not a valid escape, stop identifier scanning
                        break;
                    }
                }
                // ASCII byte: settle it from the byte LUT alone — no char decode, no
                // cross-crate Unicode `is_id_continue` dispatch. The LUT equals
                // `is_id_continue` on ASCII, so this stays exact. After an escape has
                // materialized the decoded buffer, ASCII identifier parts are pushed here.
                Some(b) if b < 0x80 => {
                    if is_ascii_id_continue(b) {
                        if let Some(d) = &mut decoded {
                            d.push(b as char);
                        }
                        self.advance();
                    } else {
                        break;
                    }
                }
                // Non-ASCII lead byte: decode the full char and continue while it is an
                // IdentifierPart.
                Some(_) => match self.cur_char() {
                    Some(ch) if is_id_continue(ch) => {
                        self.note_nonplain_ident(start);
                        if let Some(d) = &mut decoded {
                            d.push(ch);
                        }
                        self.advance();
                    }
                    _ => break,
                },
                None => break,
            }
        }
        decoded
    }

    /// Write the identifier `start..self.position` read by the general reader into
    /// `*dst`, parking a decoded name in the lexer's scratch. Always an
    /// `Identifier`, never a keyword: an escaped spelling of a reserved word is an
    /// identifier (`\u0063lass` is the identifier `class`), and an unescaped name that
    /// reached the general reader holds a non-ASCII char, which no reserved word does.
    /// (The one general-reader name that CAN be a keyword — an ASCII run whose `\` or
    /// non-ASCII stop byte does not continue it — is finished by
    /// [`Lexer::scan_ascii_identifier_into`].)
    fn finish_general_identifier_into(
        &mut self,
        start: usize,
        decoded: Option<String>,
        dst: &mut Token,
    ) {
        // Escaped identifiers are near-zero in real code; funnel the rare local
        // buffer into the parked scratch so `decoded_str` reads it uniformly. The
        // dispatch cleared `has_decoded` for the escape-free name.
        if let Some(s) = decoded {
            self.decode_scratch.clear();
            self.decode_scratch.push_str(&s);
            self.has_decoded = true;
        }
        *dst = Token {
            kind: TokenKind::Identifier,
            start: start as u32,
            end: self.position as u32,
        };
    }

    /// Scan digits matching a predicate, validating numeric separators (`_`).
    /// Per the ECMAScript lexical grammar a `NumericLiteralSeparator` must sit
    /// *between two digits*, so a `_` is rejected at the start of the group, at
    /// the end, when doubled, or adjacent to a prefix/`.`/`e` — the placement
    /// over-acceptances acorn flags as "Numeric separator is not allowed …".
    /// Returns whether at least one digit was consumed (callers enforce the
    /// "≥1 digit after a radix prefix" rule).
    fn scan_digits(&mut self, is_valid_digit: impl Fn(char) -> bool) -> Result<bool, ParseError> {
        // Digits and `_` are ASCII, so a byte scan suffices: a non-ASCII byte
        // (`b as char` ∈ U+0080..=U+00FF) is never a valid digit, so the predicate
        // breaks the loop just as it would on any other terminator.
        let mut saw_digit = false;
        let mut prev_was_digit = false;
        while let Some(b) = self.cur_byte() {
            if b == b'_' {
                // A separator is valid only with a digit on each side.
                let next_is_digit = self
                    .byte_ahead(1)
                    .is_some_and(|n| is_valid_digit(n as char));
                if !prev_was_digit || !next_is_digit {
                    return Err(lex_err(
                        "Numeric separator '_' must appear between two digits",
                        self.position,
                    ));
                }
                self.advance();
                prev_was_digit = false;
            } else if is_valid_digit(b as char) {
                self.advance();
                saw_digit = true;
                prev_was_digit = true;
            } else {
                break;
            }
        }
        Ok(saw_digit)
    }

    /// Scan a decimal number (integer, float, or scientific notation).
    /// Handles: 123, 1.5, 1e3, 1.5e-2, 1_000, 1.e1. Returns whether the literal
    /// is integer-form (no fractional part and no exponent) — the only decimal
    /// shape a BigInt `n` suffix may follow (`1.5n` / `1e3n` are rejected).
    fn scan_decimal_number(&mut self) -> Result<bool, ParseError> {
        // `s` starts at the char after a `.`. Returns true when `s` begins a valid
        // exponent (`e`/`E`, optional sign, then a digit) — i.e. `1.e1` is one number.
        fn is_exponent_start(s: &str) -> bool {
            let mut chars = s.chars();
            if !matches!(chars.next(), Some('e' | 'E')) {
                return false;
            }
            let mut c = chars.next();
            if matches!(c, Some('+' | '-')) {
                c = chars.next();
            }
            c.is_some_and(|c| c.is_ascii_digit())
        }

        let mut is_integer = true;

        // Integer part (with optional separators)
        self.scan_digits(|c| c.is_ascii_digit())?;

        // Decimal point and fractional part
        if self.cur_byte() == Some(b'.') {
            // Peek ahead: the common `3.14` case only needs the next byte — an ASCII
            // digit is single-byte, so the byte test equals decoding the char, with no
            // subslice/decode. The rarer trailing-dot-exponent (`1.e1`) path below still
            // reads the `&str` tail.
            if self.byte_ahead(1).is_some_and(|b| b.is_ascii_digit()) {
                // Normal decimal: 3.14
                is_integer = false;
                self.advance(); // consume '.'
                self.scan_digits(|c| c.is_ascii_digit())?;
            } else if is_exponent_start(&self.source[self.position + 1..]) {
                // Trailing-dot exponent: `1.e1` is a single numeric literal (= 1e1).
                // Consume the '.'; the exponent block below consumes `e1`.
                // Without this, `1.e1` would lex as `1.` followed by member access `.e1`.
                is_integer = false;
                self.advance(); // consume '.'
            } else {
                // Trailing decimal: `5.` / `0.` (operator, punctuation, or end),
                // `5..foo` / `0..toString()` (the next `.` is member access). The
                // `.` is greedily the decimal point (maximal munch), so consume it.
                // The boundary check at the end of `scan_number_into` then rejects
                // an IdentifierStart abutting the `.` (`5.foo` / `10._1` / `5.in`).
                is_integer = false;
                self.advance(); // consume '.'
            }
        }

        // Exponent part: e+10, E-3, e10
        if matches!(self.cur_byte(), Some(b'e' | b'E')) {
            is_integer = false;
            self.advance(); // consume 'e' or 'E'
            // Optional sign
            if matches!(self.cur_byte(), Some(b'+' | b'-')) {
                self.advance();
            }
            self.scan_digits(|c| c.is_ascii_digit())?;
        }

        Ok(is_integer)
    }

    /// Scan a numeric literal — decimal, `0x`/`0b`/`0o` radix, float, exponent,
    /// or `BigInt` suffix — writing the `Number` token into `*dst`. `first` is the
    /// byte at `start` the dispatch matched; it is read only to detect a leading-
    /// `0` radix prefix, so it is a digit for `5`/`0x…` or `.` for a leading-dot
    /// fraction (`.5`) — both non-`0`, both routing to `scan_decimal_number`. The
    /// single number entry point, so the "identifier directly after a number"
    /// boundary rule (ecma262 12.9.3) lives here once. Mirrors the `_into`
    /// write-through of the other large scanners so the dispatch arm is one
    /// `return`. **Mode-free**: the two leading-zero forms (legacy octal `0777`,
    /// non-octal decimal `08`/`09`) lex under every strictness, and the strict-mode
    /// rejection is the parser's, taken where the token becomes a node. The
    /// separator forms (`0_0`) are the exception — they are a syntax error in every
    /// mode, so they error here.
    fn scan_number_into(
        &mut self,
        start: usize,
        first: u8,
        dst: &mut Token,
    ) -> Result<(), ParseError> {
        // Radix literals (`0x`/`0b`/`0o`) are always integer-form, so a BigInt
        // `n` suffix is always allowed after them; a decimal literal allows `n`
        // only when it carries no fraction and no exponent.
        let mut bigint_allowed = true;
        if first == b'0' {
            match self.byte_ahead(1) {
                Some(b'x' | b'X') => {
                    // Hex: 0xff, 0xFF
                    self.advance(); // consume '0'
                    self.advance(); // consume 'x'
                    if !self.scan_digits(|c| c.is_ascii_hexdigit())? {
                        return Err(lex_err("Missing hexadecimal digits after '0x'", start));
                    }
                }
                Some(b'b' | b'B') => {
                    // Binary: 0b1010
                    self.advance(); // consume '0'
                    self.advance(); // consume 'b'
                    if !self.scan_digits(|c| c == '0' || c == '1')? {
                        return Err(lex_err("Missing binary digits after '0b'", start));
                    }
                }
                Some(b'o' | b'O') => {
                    // Octal: 0o77
                    self.advance(); // consume '0'
                    self.advance(); // consume 'o'
                    if !self.scan_digits(|c| ('0'..='7').contains(&c))? {
                        return Err(lex_err("Missing octal digits after '0o'", start));
                    }
                }
                Some(b'0'..=b'9') => {
                    // The two leading-zero forms ECMAScript keeps alive for Script
                    // code: `LegacyOctalIntegerLiteral` (`010`, octal digits only)
                    // and `NonOctalDecimalIntegerLiteral` (`08`/`09`/`089`, an `8`
                    // or `9` in the run). Both are disallowed in strict code — a
                    // *production disallowance*, ecma262 sec-strict-mode-of-ecmascript
                    // — so they are legal only in a sloppy Script. The lexer reads
                    // both and records nothing: the strictness a leading-zero literal
                    // is graded under is the parser's to know, at the point the token
                    // becomes a node (`parse_number_or_bigint_literal`), which also
                    // keeps a *peeked* token on the far side of a strictness boundary
                    // from ever being rejected under the wrong mode.
                    self.advance(); // consume '0'
                    let mut all_octal = true;
                    while let Some(b) = self.cur_byte() {
                        if b.is_ascii_digit() {
                            if b >= b'8' {
                                all_octal = false;
                            }
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    if self.cur_byte() == Some(b'_') {
                        return Err(lex_err(LEGACY_SEPARATOR_MSG, start));
                    }
                    if !all_octal {
                        // Demoted to decimal, so a fraction and an exponent are
                        // grammatical (`08.5e1` = 850) — and a separator inside the
                        // *fraction* is ordinary (`08.5_1`), which is why the
                        // integer run above is scanned separator-free by hand.
                        self.scan_decimal_number()?;
                    }
                    // `LegacyOctalIntegerLiteral` ends at its digit run: the
                    // production admits no fraction and no exponent, so a following
                    // `.` is the next token (`010.toString()` is a member access) and
                    // a following `e` hits the identifier-boundary check below
                    // (`0777e2`). Neither form takes a BigInt `n` (`010n`, `08n`),
                    // which the same check rejects.
                    bigint_allowed = false;
                }
                Some(b'_') => {
                    // A `NumericLiteralSeparator` cannot appear in a leading-zero
                    // legacy form (`0_0`/`0_8` — the `LegacyOctalLikeDecimalInteger`
                    // productions carry no `[Sep]` parameter), so a `_` immediately
                    // after a leading `0` is rejected. Unlike the leading-zero digit
                    // forms above this holds in *every* mode, so it stays a lex error.
                    return Err(lex_err(LEGACY_SEPARATOR_MSG, start));
                }
                _ => {
                    // Regular number or float starting with 0 (e.g. `0`, `0.5`,
                    // `0e1`, `0n`) — the leading `0` is followed by `.`/`e`/`n`/a
                    // radix prefix/end, none of which are a digit or `_`.
                    bigint_allowed = self.scan_decimal_number()?;
                }
            }
        } else {
            // Regular decimal number
            bigint_allowed = self.scan_decimal_number()?;
        }

        // BigInt suffix `n` attaches only to an integer-form literal. When `n`
        // follows a float/exponent we leave it unconsumed so the adjacent
        // identifier triggers the normal parse-level rejection (as for `5abc`),
        // matching acorn's "Identifier directly after number".
        if bigint_allowed && self.cur_byte() == Some(b'n') {
            self.advance();
        }

        // ecma262 12.9.3: "The SourceCharacter immediately following a
        // NumericLiteral must not be an IdentifierStart or DecimalDigit" — the
        // spec's own example is that `3in` is an error, not the two tokens `3` and
        // `in`. Enforcing the IdentifierStart half here (the single number entry)
        // rejects a number abutting a keyword-operator (`5in` / `1.5in` / `0xffin`
        // / `5nin` / `.5in`) rather than reading it as `5 in y`; the parser's
        // number→primary path only catches a following *identifier* (`5foo`), not
        // an infix keyword. A DecimalDigit can only follow a complete number after
        // an out-of-range radix digit (`0b12`) or a BigInt suffix (`5n3`), both of
        // which the parser already rejects as adjacent number tokens.
        // Byte-gate the IdentifierStart check: the byte after a number is almost
        // always an ASCII terminator (`;`, `)`, whitespace, an operator), settled from
        // the LUT with no char decode + no cross-crate `is_id_start` dispatch. The LUT
        // equals `is_id_start` on ASCII (the same tested invariant the identifier fast
        // paths rely on); only a non-ASCII lead byte decodes the full char.
        let id_start_follows = match self.cur_byte() {
            Some(b) if b < 0x80 => is_ascii_id_start(b),
            Some(_) => self.cur_char().is_some_and(is_id_start),
            None => false,
        };
        if id_start_follows {
            return Err(lex_err("Identifier directly after number", self.position));
        }

        *dst = self.make_token(TokenKind::Number, start);
        Ok(())
    }

    /// Scan a single- or double-quoted string literal (cursor on the opening
    /// `quote` byte), writing the `String` token into `*dst` and the decoded value
    /// out-of-band via the parked `decode_scratch` only when it contains escapes. Mirrors the
    /// `_into` write-through of the other large scanners.
    ///
    /// The inner run skips everything that is none of the close quote, a backslash, and the
    /// two raw line terminators a `StringLiteral` may not contain — a 4-byte search, run
    /// word-at-a-time through [`tsv_lang::swar::next_byte_of`]. A byte-at-a-time form is
    /// sound (all four are ASCII, so none can appear as a UTF-8 continuation byte) and
    /// costs twelve instructions and five branches per byte, vectorizing at none of them
    /// — the escape arm resumes the run, which makes the stride data-dependent.
    /// `has_escapes` gates the (rare) decode pass.
    fn scan_string_into(
        &mut self,
        start: usize,
        quote: u8,
        dst: &mut Token,
    ) -> Result<(), ParseError> {
        self.advance(); // consume opening quote
        let content_start = self.position;

        let bytes = self.bytes;
        let len = bytes.len();
        let mut p = content_start;
        let mut has_escapes = false;
        loop {
            p = tsv_lang::swar::next_byte_of(bytes, p, [quote, b'\\', b'\n', b'\r']);
            // Unterminated: the source ran out, or a raw `<LF>` / `<CR>` ended the line the
            // literal opened on. `StringLiteral` excludes a raw LineTerminator (the character
            // reaches one only as an escape), and acorn rejects both — skipping them would
            // swallow the rest of the file up to some later quote. `<LS>` / `<PS>` are NOT
            // here: ES2019's JSON-superset change made them legal in a string literal, and
            // being multi-byte they never match these ASCII tests anyway.
            if p >= len || bytes[p] == b'\n' || bytes[p] == b'\r' {
                self.position = p;
                return Err(lex_err("Unterminated string literal", start));
            }
            if bytes[p] == quote {
                let content_end = p;
                p += 1; // consume closing quote
                self.position = p;

                let content = &self.source[content_start..content_end];

                // Decode escape sequences into the parked scratch (no per-literal
                // String/Box allocation); `has_decoded` was cleared at entry and is
                // set only when this token actually carries escapes.
                if has_escapes {
                    escapes::decode_string_escapes_into(content, &mut self.decode_scratch)?;
                    self.has_decoded = true;
                }
                *dst = Token {
                    kind: TokenKind::String,
                    start: start as u32,
                    end: p as u32,
                };
                return Ok(());
            }
            // bytes[p] == b'\\': escape — skip the backslash and the next
            // character (decode_string_escapes validates it later). Advance
            // past a full char so a multibyte escaped char resumes the inner
            // scan on a char boundary.
            has_escapes = true;
            p += 1;
            if p < len {
                if bytes[p] == b'\r' && bytes.get(p + 1) == Some(&b'\n') {
                    // A LineContinuation's terminator is a whole LineTerminatorSequence, so
                    // `<CR><LF>` goes as one: leaving the `<LF>` behind would meet the
                    // raw-terminator gate above and reject a legal continuation.
                    p += 2;
                } else {
                    p += utf8_len(bytes[p]);
                }
            }
        }
    }

    /// Skip the WhiteSpace and LineTerminators ahead of the next token, recording in
    /// `had_line_terminator` whether a LineTerminator was among them, and hand back the
    /// byte the token starts with (`None` at end of input), the cursor left on it.
    ///
    /// The run is ASCII on essentially every token, and empty on most, so it walks a local
    /// cursor that is stored once and hands the dispatch the stop byte it already holds
    /// rather than have it load the byte again. `<CR><LF>` needs no pairing here: each of
    /// the two is a LineTerminator, and each sets the one flag. A non-ASCII byte — NBSP,
    /// U+FEFF, a `Zs` space, LS / PS, or the first byte of the token itself — goes to the
    /// cold [`skip_whitespace_general`], so the hot loop keeps nothing live for a decode.
    ///
    /// Taking the byte from here rather than reading it again has two consumers that must
    /// know it: the dispatch's arms step over it with [`Lexer::advance_ascii`], and the
    /// identifier arm restates the bound (see [`Lexer::scan_ascii_identifier_into`]).
    #[inline]
    fn skip_whitespace(&mut self, bytes: &'a [u8]) -> Option<u8> {
        self.had_line_terminator = false;
        let mut pos = self.position;
        while let Some(&b) = bytes.get(pos) {
            match b {
                // SPACE / TAB / VT / FF — the ASCII subset of WhiteSpace
                b' ' | b'\t' | 0x0B | 0x0C => {}
                // LF / CR — the ASCII LineTerminators (ES spec 12.3)
                b'\n' | b'\r' => self.had_line_terminator = true,
                0x80.. => {
                    let (end, crossed) = skip_whitespace_general(self.source, pos);
                    if crossed {
                        self.had_line_terminator = true;
                    }
                    self.position = end;
                    return bytes.get(end).copied();
                }
                // Any other ASCII byte is not whitespace — the token starts here.
                _ => {
                    self.position = pos;
                    return Some(b);
                }
            }
            pos += 1;
        }
        self.position = pos;
        None
    }

    /// Lex the next token directly into `*dst` — the hot advance path. Writing
    /// through the caller's slot (`&mut self.current`) instead of returning a
    /// `Result<Token, ParseError>` elides the round trip the by-value return
    /// makes through the caller's frame (the intermediate `Token` built,
    /// returned through a slot, then reloaded and re-scattered into the
    /// parser's field). The match yields a `Token` value only for the short
    /// punctuation/operator paths, whose kinds carry no payload; the
    /// identifier/number/string/template/comment/hashbang scanners and the error
    /// paths write `dst` (or propagate the error) via an early `return`. A payload
    /// kind yielded by the match is not free even where it is rare: the arms merge
    /// into one value, so every punctuation token then copies the payload bytes
    /// through the merged stack temporary into `dst`. [`Lexer::next_token`] is the
    /// thin by-value wrapper every other caller takes.
    ///
    /// The error path is lifted into host coordinates here, at the producer
    /// ([`Lexer::host_err`]); the scan itself works in — and reports in — the lexer's own.
    #[inline]
    pub fn next_token_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        self.next_token_into_local(dst)
            .map_err(|err| self.host_err(err))
    }

    /// [`Lexer::next_token_into`]'s scan, reporting an error at its position in
    /// `self.source` — never called directly, so the lift above is applied exactly once.
    fn next_token_into_local(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        // Clear the decoded-value flag from the previous token so `decoded_str`
        // reflects only the token produced by this call (set by the escape paths below).
        self.has_decoded = false;
        let bytes = self.bytes;
        let first = self.skip_whitespace(bytes);
        let start = self.position;

        *dst = match first {
            None => Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            },
            // ECMAScript identifiers: start with ID_Start, _, or $; continue with ID_Continue or $
            // Note: _ is in ID_Continue but not ID_Start, so we check it explicitly for start
            // Identifiers may contain unicode escapes: \u0066oo → foo, b\u0061r → bar
            // Tested ahead of every other byte arm, the identifier being the most common
            // token: none of the arms it precedes matches an ASCII `IdentifierStart` byte.
            Some(b) if is_ascii_id_start(b) => {
                self.scan_ascii_identifier_into(bytes, start, dst);
                return Ok(());
            }
            Some(b';') => {
                self.advance_ascii();
                self.make_token(TokenKind::Semicolon, start)
            }
            Some(b':') => {
                self.advance_ascii();
                self.make_token(TokenKind::Colon, start)
            }
            Some(b'=') => {
                self.advance_ascii();
                match self.cur_byte() {
                    Some(b'>') => {
                        // =>
                        self.advance_ascii();
                        self.make_token(TokenKind::Arrow, start)
                    }
                    Some(b'=') => {
                        self.advance_ascii();
                        if self.cur_byte() == Some(b'=') {
                            // ===
                            self.advance_ascii();
                            self.make_token(TokenKind::EqualsEqualsEquals, start)
                        } else {
                            // ==
                            self.make_token(TokenKind::EqualsEquals, start)
                        }
                    }
                    _ => {
                        // =
                        self.make_token(TokenKind::Equals, start)
                    }
                }
            }
            Some(b) if b.is_ascii_digit() => return self.scan_number_into(start, b, dst),
            // Unicode escape at start of identifier: \u0066oo → foo
            Some(b'\\') => {
                // Check if this is a valid unicode escape that decodes to an identifier start
                if let Some((ch, _)) = try_decode_unicode_escape(self.bytes, self.position)
                    && is_id_start(ch)
                {
                    return self.scan_identifier_into('\\', dst);
                }
                // Not a valid identifier start - error.
                return Err(lex_err("Unexpected character: '\\'", start));
            }
            Some(quote @ (b'\'' | b'"')) => return self.scan_string_into(start, quote, dst),
            Some(b',') => {
                self.advance_ascii();
                self.make_token(TokenKind::Comma, start)
            }
            Some(b'{') => {
                self.advance_ascii();
                self.make_token(TokenKind::BraceOpen, start)
            }
            Some(b'}') => {
                self.advance_ascii();
                self.make_token(TokenKind::BraceClose, start)
            }
            Some(b'[') => {
                self.advance_ascii();
                self.make_token(TokenKind::BracketOpen, start)
            }
            Some(b']') => {
                self.advance_ascii();
                self.make_token(TokenKind::BracketClose, start)
            }
            Some(b'(') => {
                self.advance_ascii();
                self.make_token(TokenKind::ParenOpen, start)
            }
            Some(b')') => {
                self.advance_ascii();
                self.make_token(TokenKind::ParenClose, start)
            }
            Some(b'.') => {
                // `.`, `..`, `...` and digits are all ASCII, so peek the next two bytes.
                if self.byte_ahead(1) == Some(b'.') && self.byte_ahead(2) == Some(b'.') {
                    // Spread operator: ...
                    self.advance_ascii(); // consume first .
                    self.advance_ascii(); // consume second .
                    self.advance_ascii(); // consume third .
                    self.make_token(TokenKind::DotDotDot, start)
                } else if self.byte_ahead(1).is_some_and(|b| b.is_ascii_digit()) {
                    // Number starting with a decimal point (`.5`, `.5e3`). Route it
                    // through the one number entry with `.` as `first` (a non-`0`
                    // byte → empty integer part → fraction/exponent), so leading-dot
                    // fractions share `scan_number_into`'s separator/exponent and
                    // boundary handling instead of a parallel scan that can drift.
                    return self.scan_number_into(start, b'.', dst);
                } else {
                    // Single dot: member access operator
                    self.advance_ascii();
                    self.make_token(TokenKind::Dot, start)
                }
            }
            Some(b'-') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'-') {
                    self.advance_ascii();
                    self.make_token(TokenKind::MinusMinus, start)
                } else if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::MinusEquals, start)
                } else {
                    self.make_token(TokenKind::Minus, start)
                }
            }
            Some(b'+') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'+') {
                    self.advance_ascii();
                    self.make_token(TokenKind::PlusPlus, start)
                } else if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::PlusEquals, start)
                } else {
                    self.make_token(TokenKind::Plus, start)
                }
            }
            Some(b'/') => {
                // Could be: // line comment, /* block comment */, or / division operator
                // Peek ahead to determine which
                let peek = self.byte_ahead(1);
                match peek {
                    Some(b'/') => {
                        // Line comment
                        let end = comments::line_comment_end(bytes, start);
                        self.comment_into(start, end, false, dst);
                        return Ok(());
                    }
                    Some(b'*') => {
                        // Block comment
                        let end = comments::block_comment_end(bytes, start)?;
                        self.comment_into(start, end, true, dst);
                        return Ok(());
                    }
                    Some(b'=') => {
                        // Division assignment operator /=
                        self.advance_ascii();
                        self.advance_ascii();
                        self.make_token(TokenKind::SlashEquals, start)
                    }
                    _ => {
                        // Division operator /
                        self.advance_ascii();
                        self.make_token(TokenKind::Slash, start)
                    }
                }
            }
            Some(b'*') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'*') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::StarStarEquals, start)
                    } else {
                        self.make_token(TokenKind::StarStar, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::StarEquals, start)
                } else {
                    self.make_token(TokenKind::Star, start)
                }
            }
            Some(b'%') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::PercentEquals, start)
                } else {
                    self.make_token(TokenKind::Percent, start)
                }
            }
            Some(b'^') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::CaretEquals, start)
                } else {
                    self.make_token(TokenKind::Caret, start)
                }
            }
            Some(b'~') => {
                self.advance_ascii();
                self.make_token(TokenKind::Tilde, start)
            }
            Some(b'<') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::LessThanEquals, start)
                } else if self.cur_byte() == Some(b'<') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::LeftShiftEquals, start)
                    } else {
                        self.make_token(TokenKind::LeftShift, start)
                    }
                } else {
                    self.make_token(TokenKind::LessThan, start)
                }
            }
            Some(b'>') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::GreaterThanEquals, start)
                } else if self.cur_byte() == Some(b'>') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'>') {
                        // >>> or >>>=
                        self.advance_ascii();
                        if self.cur_byte() == Some(b'=') {
                            self.advance_ascii();
                            self.make_token(TokenKind::UnsignedRightShiftEquals, start)
                        } else {
                            self.make_token(TokenKind::UnsignedRightShift, start)
                        }
                    } else if self.cur_byte() == Some(b'=') {
                        // >>=
                        self.advance_ascii();
                        self.make_token(TokenKind::RightShiftEquals, start)
                    } else {
                        // >>
                        self.make_token(TokenKind::RightShift, start)
                    }
                } else {
                    self.make_token(TokenKind::GreaterThan, start)
                }
            }
            Some(b'!') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::BangEqualsEquals, start)
                    } else {
                        self.make_token(TokenKind::BangEquals, start)
                    }
                } else {
                    self.make_token(TokenKind::Bang, start)
                }
            }
            Some(b'&') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'&') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::AmpersandAmpersandEquals, start)
                    } else {
                        self.make_token(TokenKind::AmpersandAmpersand, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::AmpersandEquals, start)
                } else {
                    self.make_token(TokenKind::Ampersand, start)
                }
            }
            Some(b'|') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'|') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::PipePipeEquals, start)
                    } else {
                        self.make_token(TokenKind::PipePipe, start)
                    }
                } else if self.cur_byte() == Some(b'=') {
                    self.advance_ascii();
                    self.make_token(TokenKind::PipeEquals, start)
                } else {
                    self.make_token(TokenKind::Pipe, start)
                }
            }
            Some(b'?') => {
                self.advance_ascii();
                if self.cur_byte() == Some(b'?') {
                    self.advance_ascii();
                    if self.cur_byte() == Some(b'=') {
                        self.advance_ascii();
                        self.make_token(TokenKind::QuestionQuestionEquals, start)
                    } else {
                        self.make_token(TokenKind::QuestionQuestion, start)
                    }
                } else if self.cur_byte() == Some(b'.') {
                    // Check for optional chaining `?.`
                    // Must not be followed by a digit (to avoid ambiguity with `?.0` which should be `?` `.0`)
                    // Cursor is on `.`; the byte after it is `position + 1`.
                    let next = self.byte_ahead(1);
                    if next.is_none_or(|b| !b.is_ascii_digit()) {
                        self.advance_ascii();
                        self.make_token(TokenKind::QuestionDot, start)
                    } else {
                        // `?.0` should be `?` followed by `.0` (number)
                        self.make_token(TokenKind::Question, start)
                    }
                } else {
                    self.make_token(TokenKind::Question, start)
                }
            }
            Some(b'`') => {
                // Template literal starting with backtick
                return self.read_template_into(start, dst);
            }
            Some(b'@') => {
                // @ for decorators
                self.advance_ascii();
                self.make_token(TokenKind::At, start)
            }
            Some(b'#') => {
                // Check for hashbang at start of file: #!/usr/bin/env node
                if start == 0 {
                    let next = self.source.get(1..2);
                    if next == Some("!") {
                        // Hashbang comment - read until end of line
                        return self.read_hashbang_into(start, dst);
                    }
                }
                // # for private identifiers
                self.advance_ascii();
                self.make_token(TokenKind::Hash, start)
            }
            // Non-ASCII lead byte: a Unicode IdentifierStart, otherwise an error.
            // (The ASCII id-start arm above handles `a-z A-Z _ $`; this decodes the
            // char for the Unicode `is_id_start` check — the one token-start decode.)
            Some(b) if b >= 0x80 => match self.cur_char() {
                Some(ch) if is_id_start(ch) => return self.scan_identifier_into(ch, dst),
                Some(ch) => {
                    return Err(lex_err(format!("Unexpected character: '{ch}'"), start));
                }
                None => return Err(lex_err("Unexpected character", start)),
            },
            Some(b) => {
                return Err(lex_err(
                    format!("Unexpected character: '{}'", b as char),
                    start,
                ));
            }
        };
        Ok(())
    }

    /// Write the `//` (`is_block: false`) or `/* */` comment token spanning `start..end`
    /// into `*dst` and move the cursor past it; the content starts after the two-byte
    /// opener.
    #[inline]
    fn comment_into(&mut self, start: usize, end: usize, is_block: bool, dst: &mut Token) {
        self.position = end;
        *dst = Token {
            kind: TokenKind::Comment {
                is_block,
                content_start: (start + 2) as u32,
            },
            start: start as u32,
            end: end as u32,
        };
    }

    /// By-value next-token for every caller but the hot advance path: the parser's
    /// bootstrap, its `peek_kind` lookahead, its cold re-lexes (the comment drain,
    /// the regex relex, [`Lexer::seek_and_next_token`]'s compound-token split), and
    /// `debug_token_stream`. The advance path uses [`Lexer::next_token_into`] to
    /// write the parser's current token in place.
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        let mut tok = Token {
            kind: TokenKind::Eof,
            start: 0,
            end: 0,
        };
        self.next_token_into(&mut tok)?;
        Ok(tok)
    }

    /// Scan one template segment body over raw bytes, starting at `content_start`
    /// (just past the opening `` ` `` or `}`). Returns `(content_end, stop,
    /// has_escapes)`: `content_end` is the segment's content boundary and `stop`
    /// is what terminated it. On a non-EOF stop `self.position` is left just past
    /// the consumed terminator (the closing `` ` ``, or the `{` of `${`); on EOF it
    /// is left at the end.
    ///
    /// The inner run skips everything that is not `` ` `` / `$` / `\` — a 3-byte
    /// search, run word-at-a-time through [`tsv_lang::swar::next_byte_of`]; a lone `$`
    /// and an escape both resume it, which is what keeps the compare-chain spelling
    /// scalar. Byte-at-a-time is sound: all three are
    /// ASCII (`< 0x80`) and so never appear as a UTF-8 continuation byte. A `\`
    /// skips itself plus the next full char (a multibyte escaped char resumes the
    /// scan on a char boundary); the escape is validated later when the segment is
    /// decoded (`decode_string_escapes_into`). Depth tracking (`${` push) stays with the caller.
    fn scan_template_body(&mut self, content_start: usize) -> (usize, TemplateStop, bool) {
        let bytes = self.bytes;
        let len = bytes.len();
        let mut p = content_start;
        let mut has_escapes = false;
        loop {
            p = tsv_lang::swar::next_byte_of(bytes, p, [b'`', b'$', b'\\']);
            if p >= len {
                self.position = p;
                return (p, TemplateStop::Eof, has_escapes);
            }
            match bytes[p] {
                b'`' => {
                    let content_end = p;
                    self.position = p + 1; // consume closing `
                    return (content_end, TemplateStop::Backtick, has_escapes);
                }
                b'$' if bytes.get(p + 1) == Some(&b'{') => {
                    let content_end = p;
                    self.position = p + 2; // consume ${
                    return (content_end, TemplateStop::Interpolation, has_escapes);
                }
                b'$' => p += 1, // lone $
                _ => {
                    // backslash — skip it and the escaped char (full width)
                    has_escapes = true;
                    p += 1;
                    if p < len {
                        p += utf8_len(bytes[p]);
                    }
                }
            }
        }
    }

    /// Read template literal content.
    ///
    /// Called when we see a backtick (start of template) or after reading `}` in template context.
    /// Returns one of:
    /// - NoSubstitutionTemplate: Complete template with no interpolation
    /// - TemplateHead: Start of template with `${` interpolation
    /// - TemplateMiddle: Middle section between interpolations (}...${)
    /// - TemplateTail: End section after last interpolation (}...`)
    fn read_template_into(&mut self, start: usize, dst: &mut Token) -> Result<(), ParseError> {
        self.advance(); // consume opening ` or }

        let content_start = self.position;
        let (content_end, stop, has_escapes) = self.scan_template_body(content_start);

        let kind = match stop {
            TemplateStop::Eof => {
                return Err(lex_err("Unterminated template literal", start));
            }
            // Determine token type based on whether we started with ` or }. (This
            // entry point is only reached on a leading `` ` ``, so `is_head` is
            // always true here; the check is kept exact for clarity/robustness.)
            TemplateStop::Backtick => {
                if self.source[start..].starts_with('`') {
                    TokenKind::NoSubstitutionTemplate
                } else {
                    TokenKind::TemplateTail
                }
            }
            TemplateStop::Interpolation => {
                self.template_depth += 1;
                if self.source[start..].starts_with('`') {
                    TokenKind::TemplateHead
                } else {
                    TokenKind::TemplateMiddle
                }
            }
        };

        let content = &self.source[content_start..content_end];
        // Decode into the parked scratch. A bad escape (deferred for tagged
        // templates) leaves `has_decoded` false — same as a no-escape segment; the
        // parser distinguishes them by the presence of a backslash in `content`.
        if has_escapes
            && escapes::decode_string_escapes_into(content, &mut self.decode_scratch).is_ok()
        {
            self.has_decoded = true;
        }

        *dst = Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }

    /// Read a regex literal starting from a `/` or `/=` token.
    ///
    /// Called by the parser when it determines that `/` or `/=` should be a regex, not division.
    /// The parser passes the start position of the token it received.
    ///
    /// The lexer syncs to that position and reads `/pattern/flags`.
    /// For `/=` tokens, the `=` becomes the first character of the pattern (e.g., `/=\s*/`).
    ///
    /// Pattern and flags are verbatim source slices (escapes preserved), so the
    /// parser recovers them from spans rather than the token. Returns the token
    /// plus the position of the closing `/` (the pattern/flags boundary), letting
    /// the parser slice `[slash_start+1, close]` and `[close+1, end]` without the
    /// caller ever materializing the strings (`token.decoded` is `None`).
    ///
    /// A producer, so it lifts its error into host coordinates ([`Lexer::host_err`]).
    #[inline]
    pub fn read_regex_literal(&mut self, slash_start: usize) -> Result<(Token, usize), ParseError> {
        self.read_regex_literal_local(slash_start)
            .map_err(|err| self.host_err(err))
    }

    /// [`Lexer::read_regex_literal`]'s scan, reporting at its position in `self.source`.
    fn read_regex_literal_local(
        &mut self,
        slash_start: usize,
    ) -> Result<(Token, usize), ParseError> {
        // A regex token never carries a decoded value; clear any left from the
        // previous token so the parser's `decoded_str()` after this lex sees `None`.
        self.has_decoded = false;
        // Sync to just after the opening /
        self.set_position(slash_start + 1);

        let pattern_start = self.position;
        let mut in_class = false; // Inside character class [...]
        let mut escaped = false; // Previous char was \

        // Read pattern until unescaped / outside character class.
        //
        // The pattern body is deliberately NOT validated here, and this loose scan is
        // exactly what the spec asks a lexer for: the `RegularExpressionBody` productions
        // exist so "the input element scanner [can] find the end of the regular expression
        // literal", and the body/flags "are subsequently parsed again using the more
        // stringent ECMAScript Regular Expression grammar" (ecma262
        // sec-literals-regular-expression-literals). That second parse failing is an
        // *early error* — `IsValidRegularExpressionLiteral` — not a grammar error, so it
        // belongs to the diagnostics layer with the other deferred early errors, and the
        // formatter (which only ever re-emits the body verbatim) never needs it.
        loop {
            match self.cur_byte() {
                None => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                // Line terminators are not allowed in a regex — checked BEFORE the
                // `escaped` arm so even `\<LS>` errors (matching the original order).
                Some(b'\n' | b'\r') => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                Some(b) if b >= 0x80 && self.at_line_separator() => {
                    return Err(lex_err(
                        "Unterminated regular expression literal",
                        slash_start,
                    ));
                }
                Some(_) if escaped => {
                    // Escaped character - consume and continue
                    escaped = false;
                    self.advance();
                }
                Some(b'\\') => {
                    escaped = true;
                    self.advance();
                }
                Some(b'[') if !in_class => {
                    in_class = true;
                    self.advance();
                }
                Some(b']') if in_class => {
                    in_class = false;
                    self.advance();
                }
                Some(b'/') if !in_class => {
                    // End of pattern
                    break;
                }
                Some(_) => {
                    self.advance();
                }
            }
        }

        let pattern_end = self.position;
        let pattern = &self.source[pattern_start..pattern_end];

        // Check for empty pattern (would be a comment)
        if pattern.is_empty() {
            return Err(lex_err(
                "Regular expression literal cannot be empty (use /(?:)/ for empty pattern)",
                slash_start,
            ));
        }

        self.advance(); // Consume closing /

        // Read flags. This IS the whole spec production — `RegularExpressionFlags ::
        // [empty] | RegularExpressionFlags IdentifierPartChar`, and `IdentifierPartChar ::
        // UnicodeIDContinue | $`. Note what that excludes: there is no backslash, so an
        // escaped flag is not a flags production at all and correctly fails here, as it
        // does in acorn — there is nothing to "support".
        //
        // Which flags are *legal* (`d`/`g`/`i`/`m`/`s`/`u`/`v`/`y`, each at most once) is
        // not this loop's business: that is the first two steps of the early error
        // `IsValidRegularExpressionLiteral`, deferred to the diagnostics layer along with
        // the pattern parse (see the body scan above). So `/a/qqq` and `/a/gg` lex fine.
        // The flags text is recovered from the span by the parser, not sliced here;
        // this loop only advances `self.position` to the token end.
        while let Some(b) = self.cur_byte() {
            // Flags are IdentifierPart (mostly ASCII d/g/i/m/s/u/v/y); decode only
            // for a non-ASCII byte before the Unicode `is_id_continue` check.
            let Some(ch) = (if b < 0x80 {
                Some(b as char)
            } else {
                self.cur_char()
            }) else {
                break;
            };
            if is_id_continue(ch) {
                self.advance();
            } else {
                break;
            }
        }

        Ok((
            Token {
                kind: TokenKind::RegexLiteral,
                start: slash_start as u32,
                end: self.position as u32,
            },
            pattern_end,
        ))
    }

    /// Continue reading template after an interpolation expression.
    ///
    /// Called by the parser after parsing the expression inside `${}`.
    /// The parser has seen the closing `}` but hasn't called advance().
    ///
    /// `brace_end` is the position just after the `}` where template content starts.
    /// The lexer will sync to this position and read the rest of the template.
    ///
    /// A producer, so it lifts its error into host coordinates ([`Lexer::host_err`]).
    #[inline]
    pub fn continue_template_from_brace(&mut self, brace_end: usize) -> Result<Token, ParseError> {
        self.continue_template_from_brace_local(brace_end)
            .map_err(|err| self.host_err(err))
    }

    /// [`Lexer::continue_template_from_brace`]'s scan, reporting at its position in
    /// `self.source`.
    fn continue_template_from_brace_local(
        &mut self,
        brace_end: usize,
    ) -> Result<Token, ParseError> {
        // Standalone token-producing entry point — clear the out-of-band decoded slot
        // so `decoded_str()` reflects only the segment produced here (set below on escapes).
        self.has_decoded = false;
        if self.template_depth == 0 {
            return Err(lex_err(
                "continue_template called outside template context",
                self.position,
            ));
        }
        self.template_depth -= 1;

        // Sync lexer position to just after the }
        // brace_end is where template content starts
        self.set_position(brace_end);

        let content_start = brace_end;
        let brace_start = brace_end - 1; // for span tracking, } is 1 char before
        let (content_end, stop, has_escapes) = self.scan_template_body(content_start);

        let kind = match stop {
            TemplateStop::Eof => {
                return Err(lex_err("Unterminated template literal", brace_start));
            }
            TemplateStop::Backtick => TokenKind::TemplateTail,
            TemplateStop::Interpolation => {
                self.template_depth += 1;
                TokenKind::TemplateMiddle
            }
        };

        let content = &self.source[content_start..content_end];
        // Decode into the parked scratch. A bad escape (deferred for tagged
        // templates) leaves `has_decoded` false — same as a no-escape segment; the
        // parser distinguishes them by the presence of a backslash in `content`.
        if has_escapes
            && escapes::decode_string_escapes_into(content, &mut self.decode_scratch).is_ok()
        {
            self.has_decoded = true;
        }

        Ok(Token {
            kind,
            start: brace_start as u32,
            end: self.position as u32,
        })
    }

    /// Read a hashbang comment: #!...
    /// Only valid at the start of the file (position 0).
    /// Reads until end of line or end of file.
    /// Returns as a Comment token with is_block: false.
    fn read_hashbang_into(&mut self, start: usize, dst: &mut Token) -> Result<(), ParseError> {
        // Skip #!
        self.advance(); // #
        self.advance(); // !

        // Read until newline or EOF without copying. Unlike `//`, the hashbang's
        // content includes the `#!` prefix, so its content starts at `start`
        // (no delimiter stripping) — recovered on demand as a source slice.
        loop {
            match self.cur_byte() {
                // End of hashbang comment at the first LineTerminator (LF, CR,
                // LS, PS) or EOF. Don't consume the terminator - it's whitespace
                // for the next token. (Mirrors the `//` line-comment reader.)
                None | Some(b'\n' | b'\r') => break,
                Some(b) if b >= 0x80 && self.at_line_separator() => {
                    break;
                }
                Some(_) => {
                    self.advance();
                }
            }
        }

        *dst = Token {
            kind: TokenKind::Comment {
                is_block: false,
                content_start: start as u32,
            },
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::token::KEYWORDS;

    // The byte-cursor fast path and the identifier-scan terminator arm both decide
    // ASCII identifier bytes from the `[bool; 256]` LUTs instead of decoding a char
    // and calling the Unicode predicates. That is only sound if the LUTs agree with
    // `is_id_start`/`is_id_continue` on every ASCII byte.
    #[test]
    fn ascii_id_luts_match_unicode_predicates() {
        for b in 0u8..0x80 {
            assert_eq!(
                is_ascii_id_start(b),
                is_id_start(b as char),
                "id_start mismatch at byte {b:#x}"
            );
            assert_eq!(
                is_ascii_id_continue(b),
                is_id_continue(b as char),
                "id_continue mismatch at byte {b:#x}"
            );
        }
    }

    // `ES_LINE_TERMINATOR_LEADS` is the loose class a word-at-a-time scan searches
    // for, and the scans that use it only ever re-test a HIT. So it must be a
    // superset of the exact production's first bytes: a terminator whose lead is
    // missing from the array is skipped silently, and no corpus carries `<LS>` /
    // `<PS>` to show it. (The reverse — a lead that is not always a terminator —
    // is the point, and `0xE2` is exactly that.)
    #[test]
    fn line_terminator_leads_cover_every_first_byte_of_the_exact_class() {
        // Every ASCII byte, then the two non-ASCII terminators and a `0xE2` lead
        // that is not one, each as the whole source.
        for b in 0u8..0x80 {
            let bytes = [b];
            assert!(
                !is_es_line_terminator_at(&bytes, 0) || ES_LINE_TERMINATOR_LEADS.contains(&b),
                "byte {b:#x} terminates a line but is not a scan lead"
            );
        }
        for s in ["\u{2028}", "\u{2029}", "\u{2014}"] {
            let bytes = s.as_bytes();
            assert!(
                !is_es_line_terminator_at(bytes, 0) || ES_LINE_TERMINATOR_LEADS.contains(&bytes[0]),
                "{s:?} terminates a line but its lead is not a scan lead"
            );
        }
        // And the loose class is genuinely loose, which is what the callers' resume
        // arm exists for.
        assert!(!is_es_line_terminator_at("\u{2014}".as_bytes(), 0));
    }

    /// The identifier ECMAScript reads at byte 0 of `src`, restated from the grammar's
    /// primitives alone (`is_id_start` / `is_id_continue`, the one escape decoder, and the
    /// `KEYWORDS` oracle) with none of the lexer's byte fast paths: the token kind, its end,
    /// the decoded name when an escape was read, and whether its raw bytes are plain ASCII.
    fn model_identifier(src: &str) -> (TokenKind, usize, Option<String>, bool) {
        let bytes = src.as_bytes();
        let mut decoded = None;
        let mut end = if bytes[0] == b'\\' {
            let (ch, len) = try_decode_unicode_escape(bytes, 0).expect("an escape-led case");
            assert!(
                is_id_start(ch),
                "an escape-led case must start an identifier"
            );
            decoded = Some(String::from(ch));
            len
        } else {
            let ch = src.chars().next().expect("a non-empty case");
            assert!(is_id_start(ch), "a case must start an identifier");
            ch.len_utf8()
        };
        loop {
            if bytes.get(end) == Some(&b'\\') {
                match try_decode_unicode_escape(bytes, end) {
                    Some((ch, len)) if is_id_continue(ch) => {
                        decoded
                            .get_or_insert_with(|| src[..end].to_string())
                            .push(ch);
                        end += len;
                    }
                    _ => break,
                }
            } else {
                match src[end..].chars().next() {
                    Some(ch) if is_id_continue(ch) => {
                        if let Some(d) = &mut decoded {
                            d.push(ch);
                        }
                        end += ch.len_utf8();
                    }
                    _ => break,
                }
            }
        }
        let kind = match decoded {
            Some(_) => TokenKind::Identifier,
            None => KEYWORDS
                .iter()
                .find(|(kw, _)| *kw == &src[..end])
                .map_or(TokenKind::Identifier, |&(_, kw)| TokenKind::Keyword(kw)),
        };
        (kind, end, decoded, src[..end].is_ascii())
    }

    /// Lex `src` through the public `next_token` and check its first token, and the lexer
    /// state that token leaves, against [`model_identifier`] — once at byte 0 and once
    /// behind a `; ` prefix, whose token is lexed first and asserted to be the `;`, so a
    /// scan that reads from the source's start rather than the token's is caught too.
    #[track_caller]
    fn assert_identifier_matches_model(src: &str) {
        const PREFIX: &str = "; ";
        let (kind, len, decoded, plain) = model_identifier(src);
        for prefix in ["", PREFIX] {
            let source = format!("{prefix}{src}");
            let start = prefix.len();
            let end = start + len;
            let mut lexer = Lexer::at_offset(&source, 0);
            if !prefix.is_empty() {
                let semicolon = lexer.next_token().expect("the prefix lexes");
                assert_eq!(semicolon.kind, TokenKind::Semicolon, "prefix of {source:?}");
            }
            let token = lexer
                .next_token()
                .unwrap_or_else(|e| panic!("{source:?} failed to lex: {e}"));
            assert_eq!(
                (token.kind, token.start, token.end),
                (kind.clone(), start as u32, end as u32),
                "token of {source:?}"
            );
            assert_eq!(lexer.position, end, "cursor after {source:?}");
            assert_eq!(
                lexer.decoded_str(),
                decoded.as_deref(),
                "decoded name of {source:?}"
            );
            assert_eq!(
                lexer.ident_is_plain_ascii(start as u32),
                plain,
                "plain-ASCII record of {source:?}"
            );
        }
    }

    /// A character for every byte that can follow an identifier's first run, for the
    /// exhaustive identifier sweep: every ASCII byte, then code points under each valid UTF-8
    /// lead byte `0xC2..=0xF4` — all of the two-byte range, a stride through the three- and
    /// four-byte ranges, and the code points the identifier and whitespace rules single out
    /// (ZWNJ / ZWJ continue a name; NBSP, U+FEFF, U+2028 / U+2029 end one). No other byte
    /// can follow an ASCII byte in a `&str`: `0x80..=0xBF` are continuation bytes and
    /// `0xC0`, `0xC1`, `0xF5..=0xFF` never occur.
    fn identifier_followers() -> Vec<String> {
        let mut out: Vec<String> = (0u8..0x80).map(|b| char::from(b).to_string()).collect();
        let strided = (0x80..0x800)
            .chain((0x800..0x1_0000).step_by(61))
            .chain((0x1_0000..0x11_0000).step_by(1021));
        let special = [
            0x200C, 0x200D, 0x00A0, 0xFEFF, 0x2028, 0x2029, 0x3000, 0x1680, 0x0301, 0x00B7, 0x2118,
            0x309B, 0x0085, 0x2014, 0xFFFF, 0x10_FFFF,
        ];
        out.extend(
            strided
                .chain(special)
                .filter_map(char::from_u32)
                .map(String::from),
        );
        out
    }

    /// The common identifier — an ASCII run ended by any ASCII byte but `\` — is read by a
    /// fast path that settles the name at the run's stop byte without the general reader;
    /// everything else (an escape, a non-ASCII continuation, a `\` or non-ASCII byte that
    /// does NOT continue the name) goes on to the general reader. Both halves must agree
    /// with the grammar on every stop, and no corpus holds most of them, so every ASCII
    /// identifier start is lexed with three bodies (none, one byte, five bytes), followed by
    /// every character of [`identifier_followers`], then by end of input or by a tail that
    /// an escape or a continuation could run into.
    #[test]
    fn every_ascii_led_identifier_stop_matches_the_grammar() {
        let followers = identifier_followers();
        let tails = ["", "x", " ", "u0061", "u{62}", "u0020", "u00", "u{110000}"];
        let mut cases = 0_u32;
        for first in (0u8..0x80).filter(|&b| is_ascii_id_start(b)) {
            for body in ["", "b", "b9$_Z"] {
                let head = format!("{}{body}", char::from(first));
                assert_identifier_matches_model(&head);
                cases += 1;
                for follower in &followers {
                    // An escape tail is only interesting after a `\`.
                    let tails: &[&str] = if follower == "\\" {
                        &tails
                    } else {
                        &tails[..2]
                    };
                    for tail in tails {
                        assert_identifier_matches_model(&format!("{head}{follower}{tail}"));
                        cases += 1;
                    }
                }
            }
        }
        assert!(
            cases > 500_000,
            "the identifier sweep collapsed to {cases} cases"
        );
    }

    /// Every reserved word, and its near misses one byte short and one byte long, followed
    /// by every character of [`identifier_followers`]: the stop byte is settled on the fast
    /// path when it is an ASCII byte other than `\`, and by the cold general reader when it
    /// is a `\` or non-ASCII; a stop that does not continue the name comes back to the fast
    /// path's one keyword lookup (`if\x` is the keyword `if`; `return` + NBSP is the keyword
    /// `return`). Every verdict must match the `KEYWORDS` oracle.
    #[test]
    fn every_keyword_stop_matches_the_oracle() {
        let followers = identifier_followers();
        let mut cases = 0_u32;
        for &(kw, _) in KEYWORDS {
            for word in [
                kw.to_string(),
                kw[..kw.len() - 1].to_string(),
                format!("{kw}s"),
            ] {
                for follower in &followers {
                    let tails: &[&str] = if follower == "\\" {
                        &["", "x", "u0061", "u0020"]
                    } else {
                        &["", "x"]
                    };
                    for tail in tails {
                        assert_identifier_matches_model(&format!("{word}{follower}{tail}"));
                        cases += 1;
                    }
                }
            }
        }
        assert!(
            cases > 100_000,
            "the keyword sweep collapsed to {cases} cases"
        );
    }

    /// The identifier arms the fast path does not take — an escape-led name and a
    /// non-ASCII-led one — and the shapes the sweeps name only in passing.
    #[test]
    fn identifier_special_cases_match_the_grammar() {
        for src in [
            "ab",
            "if\\x",
            "if\\u0061",
            "i\\u0066",
            "\\u0069f",
            "aé",
            "a\u{200C}b",
            "a\u{200D}b",
            "return\u{00A0}x",
            "return\u{2028}x",
            "return\u{FEFF}x",
            "returné",
            "$a",
            "_a",
            "$",
            "_",
            "é",
            "éa",
            "é\\u0061",
            "\\u0061",
            "\\u{61}bc",
            "\\u0061\\u0062",
            "\u{2118}x",
        ] {
            assert_identifier_matches_model(src);
        }
        // An escaped name leaves its decoded value in the scratch; the plain name after it
        // must not read it back — the dispatch clears the flag for every token, and the
        // common identifier's path relies on that rather than clearing it itself.
        for (src, first_end, second) in [("\\u0061 b", 6, 7..8), ("a\\u0062 c", 7, 8..9)] {
            let mut lexer = Lexer::at_offset(src, 0);
            let first = lexer.next_token().expect("the escaped name lexes");
            assert_eq!((first.kind, first.end), (TokenKind::Identifier, first_end));
            assert!(
                lexer.decoded_str().is_some(),
                "{src:?}: the escape is decoded"
            );
            let plain = lexer.next_token().expect("the plain name lexes");
            assert_eq!(
                (plain.kind, plain.start, plain.end),
                (TokenKind::Identifier, second.start, second.end),
                "{src:?}"
            );
            assert_eq!(
                lexer.decoded_str(),
                None,
                "{src:?}: the plain name decodes nothing"
            );
        }
        // `#private` is a `#` token and then the name.
        let mut lexer = Lexer::at_offset("#private", 0);
        let hash = lexer.next_token().expect("`#` lexes");
        assert_eq!((hash.kind, hash.start, hash.end), (TokenKind::Hash, 0, 1));
        let name = lexer.next_token().expect("the private name lexes");
        assert_eq!(
            (name.kind, name.start, name.end),
            (model_identifier("private").0, 1, 8)
        );
    }

    /// One comment token [`model_trivia`] reads — kind, span, and whether a line terminator
    /// came between it and the token before it — or the error that ends the stream.
    type TriviaItem = Result<(TokenKind, u32, u32, bool), ParseError>;

    /// The trivia ECMAScript reads from byte `pos` of `src` — WhiteSpace, LineTerminators,
    /// `//` and `/* */` comments, and a hashbang at byte 0 — restated from the grammar's
    /// char-level predicates alone ([`is_es_whitespace`], [`is_es_line_terminator`]) with
    /// none of the lexer's byte fast paths. Returns each comment, then where the next token
    /// starts and whether a line terminator came ahead of it; an unterminated block comment
    /// ends the stream with its error.
    fn model_trivia(src: &str, mut pos: usize) -> (Vec<TriviaItem>, usize, bool) {
        let line_end = |from: usize| {
            src[from..]
                .char_indices()
                .find(|&(_, c)| is_es_line_terminator(c))
                .map_or(src.len(), |(i, _)| from + i)
        };
        let mut items = Vec::new();
        if pos == 0 && src.starts_with("#!") {
            let kind = TokenKind::Comment {
                is_block: false,
                content_start: 0,
            };
            pos = line_end(0);
            items.push(Ok((kind, 0, pos as u32, false)));
        }
        loop {
            let mut crossed = false;
            while let Some(c) = src[pos..].chars().next() {
                if is_es_line_terminator(c) {
                    crossed = true;
                } else if !is_es_whitespace(c) {
                    break;
                }
                pos += c.len_utf8();
            }
            let (is_block, end) = if src[pos..].starts_with("//") {
                (false, line_end(pos))
            } else if src[pos..].starts_with("/*") {
                let Some(close) = src[pos + 2..].find("*/") else {
                    items.push(Err(lex_err("Unterminated block comment", pos)));
                    return (items, pos, crossed);
                };
                (true, pos + 2 + close + 2)
            } else {
                return (items, pos, crossed);
            };
            let kind = TokenKind::Comment {
                is_block,
                content_start: (pos + 2) as u32,
            };
            items.push(Ok((kind, pos as u32, end as u32, crossed)));
            pos = end;
        }
    }

    /// The pieces the trivia sweep composes: every ASCII WhiteSpace byte, LF / CR / CRLF,
    /// every non-ASCII WhiteSpace code point (NBSP, U+FEFF and each `Zs`), LS / PS, and line
    /// and block comments — empty, single- and multi-line, holding a `*` or a `/`, and
    /// unterminated.
    const TRIVIA_PIECES: &[&str] = &[
        " ",
        "\t",
        "\u{0B}",
        "\u{0C}",
        "\n",
        "\r",
        "\r\n",
        "\u{A0}",
        "\u{FEFF}",
        "\u{1680}",
        "\u{2000}",
        "\u{2001}",
        "\u{2002}",
        "\u{2003}",
        "\u{2004}",
        "\u{2005}",
        "\u{2006}",
        "\u{2007}",
        "\u{2008}",
        "\u{2009}",
        "\u{200A}",
        "\u{202F}",
        "\u{205F}",
        "\u{3000}",
        "\u{2028}",
        "\u{2029}",
        "//",
        "//x",
        "/**/",
        "/*x*/",
        "/*\n*/",
        "/*\u{2028}* /*/",
        "/*",
    ];

    /// What may follow the trivia, with the first token it opens and that token's length, or
    /// `None` where the lexer rejects the character: end of input, identifiers and a keyword
    /// (ASCII- and non-ASCII-led), `/` and `/=` (division — a regex is the parser's relex,
    /// never the lexer's reading), `=>`, the HTML-like comment openers (plain operators:
    /// Annex B is out), `#`, and non-ASCII code points that are neither WhiteSpace nor an
    /// identifier start (NEL, ZWSP, U+180E).
    fn trivia_followers() -> Vec<(&'static str, Option<(TokenKind, usize)>)> {
        vec![
            ("", Some((TokenKind::Eof, 0))),
            ("a", Some((TokenKind::Identifier, 1))),
            ("if", Some((model_identifier("if").0, 2))),
            ("é", Some((TokenKind::Identifier, 2))),
            (";", Some((TokenKind::Semicolon, 1))),
            ("/", Some((TokenKind::Slash, 1))),
            ("/=", Some((TokenKind::SlashEquals, 2))),
            ("=>", Some((TokenKind::Arrow, 2))),
            ("<!--", Some((TokenKind::LessThan, 1))),
            ("-->", Some((TokenKind::MinusMinus, 2))),
            ("#", Some((TokenKind::Hash, 1))),
            ("\u{85}", None),
            ("\u{200B}", None),
            ("\u{180E}", None),
        ]
    }

    /// Lex `prefix` + `trivia` + `follower` through the public `next_token` — the prefix's
    /// one token first, when there is one — and check every token the trivia yields, and the
    /// token after it, against [`model_trivia`]: kind, span, the line-terminator flag, and
    /// the error (text and position) of an unterminated block comment or a rejected
    /// character.
    #[track_caller]
    fn assert_trivia_matches_model(
        prefix: &str,
        trivia: &str,
        follower: &str,
        token: Option<&(TokenKind, usize)>,
    ) {
        let source = format!("{prefix}{trivia}{follower}");
        let mut lexer = Lexer::at_offset(&source, 0);
        let from = if prefix.is_empty() {
            tsv_lang::leading_bom_len(&source)
        } else {
            let first = lexer.next_token().expect("the prefix lexes");
            assert_eq!(first.end as usize, prefix.len(), "prefix of {source:?}");
            prefix.len()
        };
        let (items, pos, crossed) = model_trivia(&source, from);
        for item in items {
            match item {
                Ok((kind, start, end, crossed)) => {
                    let comment = lexer
                        .next_token()
                        .unwrap_or_else(|e| panic!("{source:?} failed to lex: {e}"));
                    assert_eq!(
                        (comment.kind, comment.start, comment.end),
                        (kind, start, end),
                        "comment of {source:?}"
                    );
                    assert_eq!(
                        lexer.had_line_terminator(),
                        crossed,
                        "line terminator ahead of the comment at {start} in {source:?}"
                    );
                }
                Err(expected) => {
                    assert_eq!(
                        lexer.next_token().map(|t| t.kind),
                        Err(expected),
                        "{source:?}"
                    );
                    return;
                }
            }
        }
        let result = lexer.next_token();
        // A line comment may run into the follower and leave nothing after it; a run whose
        // pieces spell a token of their own (`//` then `/*\n*/` leaves `*/`) stops inside
        // the trivia, where only the token's start and flag are the trivia's to decide.
        let token = if pos == source.len() {
            Some(&(TokenKind::Eof, 0))
        } else if &source[pos..] == follower {
            token
        } else {
            let next = result.unwrap_or_else(|e| panic!("{source:?} failed to lex: {e}"));
            assert_eq!(
                next.start as usize, pos,
                "token inside the trivia of {source:?}"
            );
            assert_eq!(
                lexer.had_line_terminator(),
                crossed,
                "line terminator ahead of the token in {source:?}"
            );
            return;
        };
        match token {
            Some((kind, len)) => {
                let next = result.unwrap_or_else(|e| panic!("{source:?} failed to lex: {e}"));
                assert_eq!(
                    (next.kind, next.start as usize, next.end as usize),
                    (kind.clone(), pos, pos + len),
                    "token after the trivia of {source:?}"
                );
                assert_eq!(
                    lexer.had_line_terminator(),
                    crossed,
                    "line terminator ahead of the token in {source:?}"
                );
                assert_eq!(lexer.position, pos + len, "cursor after {source:?}");
            }
            None => {
                let ch = follower
                    .chars()
                    .next()
                    .expect("a rejected follower is a char");
                assert_eq!(
                    result.map(|t| t.kind),
                    Err(lex_err(format!("Unexpected character: '{ch}'"), pos)),
                    "{source:?}"
                );
            }
        }
    }

    /// The whitespace ahead of a token is skipped by an ASCII fast path that hands the
    /// dispatch its stop byte, and a comment is written into the token slot by the
    /// dispatch's `/` arm; no corpus holds most of what either must get right. Every run of
    /// zero to three [`TRIVIA_PIECES`] — so every pairing of CR, LF and CRLF, every
    /// whitespace code point beside every comment shape, and every comment against end of
    /// input — is lexed ahead of every one of [`trivia_followers`], at byte 0 and behind a
    /// `;` (so a scan that reads from the source's start rather than the token's is caught).
    #[test]
    fn every_trivia_run_matches_the_grammar() {
        let followers = trivia_followers();
        let mut runs = vec![String::new()];
        let mut frontier = runs.clone();
        for _ in 0..3 {
            frontier = frontier
                .iter()
                .flat_map(|run| {
                    TRIVIA_PIECES
                        .iter()
                        .map(move |piece| format!("{run}{piece}"))
                })
                .collect();
            runs.extend(frontier.iter().cloned());
        }
        let mut cases = 0_u32;
        for run in &runs {
            for (follower, token) in &followers {
                for prefix in ["", ";"] {
                    assert_trivia_matches_model(prefix, run, follower, token.as_ref());
                    cases += 1;
                }
            }
        }
        assert!(
            cases > 1_000_000,
            "the trivia sweep collapsed to {cases} cases"
        );
    }

    /// The byte-0 forms the sweep's `;` prefix cannot reach: a hashbang (a comment token
    /// whose content includes the `#!`, only at byte 0), a BOM ahead of it (which makes the
    /// `#` a plain token), and a BOM ahead of trivia (skipped before the first token).
    #[test]
    fn byte_zero_trivia_matches_the_grammar() {
        let followers = trivia_followers();
        for head in [
            "#!",
            "#!x",
            "#!x\n",
            "#!x\r\n",
            "#!x\u{2028}",
            "\u{FEFF}",
            "\u{FEFF}#!x\n",
        ] {
            for piece in ["", " ", "\n", "//x", "/**/", "/*"] {
                for (follower, token) in &followers {
                    assert_trivia_matches_model(
                        "",
                        &format!("{head}{piece}"),
                        follower,
                        token.as_ref(),
                    );
                }
            }
        }
    }
}
