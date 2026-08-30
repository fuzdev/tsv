use std::fmt;
// Shared lexer-error constructor: used by the unterminated/unexpected sites in `next_token`.
use tsv_lang::{ParseError, lex_err, source_scan};

use crate::whitespace::{brace_interior_start, char_at, is_svelte_ws, skip_svelte_ws};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    LeftAngle,     // <
    RightAngle,    // >
    Slash,         // /
    LeftBrace,     // {
    RightBrace,    // }
    BlockOpen,     // {#
    BlockClose,    // {/
    BlockContinue, // {:
    TagOpen,       // {@
    Equals,        // =
    String,        // "..." attribute values
    Identifier,    // Tag names, attribute names
    Comment,       // <!-- ... -->
    Eof,
}

impl TokenKind {
    /// Does this token begin at a `{`?
    ///
    /// The lexer classifies a brace-led construct at the brace, so the answer is a fact
    /// about the enum rather than about any one reader — and the match is deliberately
    /// **exhaustive, with no wildcard**: a new brace-led variant must then be classified
    /// here or the crate stops compiling. Enumerating a subset by hand is how `{#`, `{:`
    /// and `{/` came to miss the attribute dispatch (`SvelteParser::parse_attributes_inner`).
    pub(crate) const fn starts_with_brace(self) -> bool {
        match self {
            Self::LeftBrace
            | Self::BlockOpen
            | Self::BlockClose
            | Self::BlockContinue
            | Self::TagOpen => true,
            Self::LeftAngle
            | Self::RightAngle
            | Self::Slash
            | Self::RightBrace
            | Self::Equals
            | Self::String
            | Self::Identifier
            | Self::Comment
            | Self::Eof => false,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::LeftAngle => write!(f, "'<'"),
            TokenKind::RightAngle => write!(f, "'>'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::LeftBrace => write!(f, "'{{'"),
            TokenKind::RightBrace => write!(f, "'}}'"),
            TokenKind::BlockOpen => write!(f, "'{{#'"),
            TokenKind::BlockClose => write!(f, "'{{/'"),
            TokenKind::BlockContinue => write!(f, "'{{:'"),
            TokenKind::TagOpen => write!(f, "'{{@'"),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Comment => write!(f, "comment"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

/// A lexed Svelte markup token: a small size-asserted POD with `u32` spans returned
/// by value from `next_token`, like `tsv_ts::Token` / `tsv_css::Token`. `Clone` (not
/// `Copy`) mirrors those crates' convention — the parser is the single owner of
/// `current` / `peek`, consuming via `.take()` / move rather than implicit copies.
/// There is **no out-of-band decoded value**: markup tokens are pure spans (the
/// embedded TS/CSS/expression content is lexed by the other crates).
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: u32,
    pub end: u32,
}

// Compact POD — keeps `next_token`'s by-value return cheap. 12 bytes (not the TS/CSS
// 16): the fieldless `TokenKind` is 1 byte, whereas theirs carries a `char` payload.
const _: () = assert!(size_of::<Token>() == 12);

pub struct Lexer<'a> {
    source: &'a str,
    /// The cursor, as a byte offset into `source` — always on a character boundary.
    ///
    /// ⚠️ **The only cursor, and it is a BYTE cursor.** Every construct this lexer
    /// recognises opens with an ASCII byte and UTF-8 is self-synchronising, so the dispatch
    /// ([`Lexer::cur_byte`]) and every needle scan read raw bytes; where a *character* class
    /// is the question (Svelte whitespace, a Unicode name char) the byte is tested first and
    /// [`char_at`] decodes only past U+007F — the discipline that helper exists for, and
    /// `tsv_ts`'s lexer's too. A second cursor carrying a decoded `char` alongside this one
    /// charges every byte of the document a UTF-8 decode and a `len_utf8` to answer a
    /// question the byte already answers: don't reintroduce one.
    position: usize,
    pub inside_tag: bool,    // Track if we're inside <...>
    initial_position: usize, // Position after BOM skip (0 or 3)
    /// Byte offset of `source` within the document this lexer's ERRORS are rendered
    /// against — zero for a whole component, `pos` for the slice the parser rebuilds this
    /// lexer over after a jumped scan (`SvelteParser::advance_to_position`). Token
    /// positions are unaffected (the parser shifts those by its own `base_offset`).
    /// See [`Lexer::host_err`].
    base_offset: usize,
}

/// The `#` or `@` that makes a brace a `{#…}` block or a `{@…}` tag rather than an
/// expression, whitespace between the two allowed.
///
/// ⚠️ **Wider than Svelte's own sequence rule, deliberately.** Svelte's `read_sequence` asks
/// `parser.match('#')` immediately after `eat('{')`, so on that side a separated `{ #if}` is
/// not a placement question at all — it goes to the expression parser and dies there as JS.
/// tsv reports the placement error at **both** spellings. The verdict is the same either way
/// (both parsers reject), so what the widening costs is wording, and what it buys is three
/// things the glued reading left broken:
///
/// - `{ @html x}` reached the TypeScript expression parser and came back
///   `Expected 'class' after 'decorator'` — another language's question, which is the whole
///   reason the placement guard exists.
/// - `a="{ #if c}a{/if}"` reached the `{/if}`, whose `/` opens a regex that never closes, so
///   the scan ran to EOF and the value died as `Unterminated string literal` — the lexer
///   accident the sequence stop below exists to prevent.
/// - `{ #x in y}` *parsed*: the brand check is the one production where a private name is an
///   operand, and its binding rule is a whole-`Script` early error tsv defers. The printer
///   then normalizes the brace, so `{ #x in y}` became `{#x in y}` — which the glued reading
///   rejected. `tsv format` emitted what `tsv parse` refused.
///
/// The third is why the offset could not simply be nudged: any fixed distance from the brace
/// assumes a gap of one particular width — the principle, and the helper that answers it once,
/// are [`brace_interior_start`].
///
/// **Sequence positions only.** Both callers are inside a tag or an RCDATA body; a template
/// `{ #each items as item}` is a genuine block there (Svelte's `tag()` runs
/// `allow_whitespace()`) and never reaches this.
///
/// `{:` and `{/` are deliberately absent — `read_sequence` does not guard them either, and they
/// fall through to the expression parser on both sides.
///
/// Two callers, and they must agree **exactly**: the placement guard
/// (`SvelteParser::check_sequence_placement`), which turns a marker into Svelte's own error, and
/// the quoted-attribute-value scan below, which stops treating the value as a sequence at one.
/// A wider set here breaks a valid value; a narrower one hands the guard's error back to the
/// scan accident it replaced.
#[derive(Debug, Clone, Copy)]
pub(crate) enum BlockOrTagMarker {
    /// `{#` — a block, e.g. `{#if …}`.
    Block,
    /// `{@` — a tag, e.g. `{@html …}`.
    Tag,
}

impl BlockOrTagMarker {
    /// The marker opening the brace at `brace_pos`, and its own byte offset — `None` when the
    /// brace opens neither a block nor a tag.
    ///
    /// The offset is returned rather than re-derived because the marker no longer sits at a
    /// fixed distance from the brace: [`brace_interior_start`] is Svelte's
    /// `allow_whitespace()`, so the gap is any width, and a caller that recomputes
    /// `brace_pos + 2` reads the author's whitespace as the construct's name.
    #[inline]
    pub(crate) fn in_sequence_at(source: &str, brace_pos: usize) -> Option<(Self, usize)> {
        let marker_pos = brace_interior_start(source, brace_pos);
        let marker = match source.as_bytes().get(marker_pos)? {
            b'#' => Self::Block,
            b'@' => Self::Tag,
            _ => return None,
        };
        Some((marker, marker_pos))
    }

    /// The marker byte itself, for spelling the construct back to the author.
    pub(crate) const fn sigil(self) -> char {
        match self {
            Self::Block => '#',
            Self::Tag => '@',
        }
    }

    /// What Svelte calls it in `block_invalid_placement` / `tag_invalid_placement`.
    pub(crate) const fn construct(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Tag => "tag",
        }
    }
}

impl<'a> Lexer<'a> {
    /// A lexer over `source`, which sits at `base_offset` in the document its errors will
    /// be rendered against.
    ///
    /// The offset is a required argument rather than a `new(source)` default because a
    /// silent zero is the failure mode: the parser rebuilds this lexer over `source[pos..]`
    /// whenever it jumps the cursor, so after any such jump a zero-offset lexer reports
    /// every error in the coordinates of that slice rather than the component's.
    pub fn at_offset(source: &'a str, base_offset: usize) -> Self {
        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes.
        let position = if source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };

        Self {
            source,
            position,
            inside_tag: false,
            initial_position: position,
            base_offset,
        }
    }

    /// Lift an error this lexer produced into the coordinates of the document it will be
    /// rendered against (`ParseError::shift_position`).
    ///
    /// Applied once, at [`Lexer::next_token`] — this lexer's only fallible entry point,
    /// and so its only producer.
    #[cold]
    #[inline(never)]
    fn host_err(&self, err: ParseError) -> ParseError {
        err.shift_position(self.base_offset)
    }

    /// Returns the initial position after BOM skip (0 if no BOM, 3 if BOM was skipped).
    /// Used by parser to initialize gap tracking.
    pub fn initial_position(&self) -> usize {
        self.initial_position
    }

    /// The byte at the cursor, or `None` at end of input.
    ///
    /// The dispatch primitive: every token this lexer recognises opens with an ASCII byte,
    /// so the common path never decodes. A caller whose question is a *character* class
    /// tests the byte first and reaches for [`char_at`] only past U+007F.
    #[inline]
    fn cur_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    /// The character at the cursor, or `None` at end of input — for the non-ASCII branches
    /// alone ([`char_at`] itself is ASCII-fast, so this costs a decode only where one is
    /// genuinely owed).
    #[inline]
    fn cur_char(&self) -> Option<char> {
        char_at(self.source, self.position).map(|(c, _)| c)
    }

    /// Advance the cursor past the character at the cursor — one byte for ASCII, its full
    /// UTF-8 width otherwise. No-op at end of input.
    #[inline]
    fn advance(&mut self) {
        if let Some((_, width)) = char_at(self.source, self.position) {
            self.position += width;
        }
    }

    /// Create a token with the current position as end.
    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        }
    }

    /// Whether the source from the current position starts with `needle`.
    /// Used for the ASCII comment delimiters (`<!--` / `-->`); a byte compare is
    /// exact for ASCII needles and avoids the per-call UTF-8 char counting.
    #[inline]
    fn starts_with(&self, needle: &[u8]) -> bool {
        self.source.as_bytes()[self.position..].starts_with(needle)
    }

    fn skip_whitespace(&mut self) {
        self.position = self.peek_past_whitespace();
    }

    /// Move the cursor to byte offset `pos`, which must be a char boundary at or after the
    /// current position. Lets a scan delegate a span to a byte-level helper
    /// (`tsv_lang::source_scan`) and resume lexing just past it, instead of re-walking the
    /// span char by char through `advance`.
    #[inline]
    fn seek_to(&mut self, pos: usize) {
        debug_assert!(pos >= self.position && self.source.is_char_boundary(pos));
        self.position = pos;
    }

    /// Byte offset of the first non-whitespace char at or after the cursor, without
    /// consuming input. Whitespace matches `skip_whitespace` ([`is_svelte_ws`]), so a
    /// follow-up `skip_whitespace()` lands exactly here.
    #[inline]
    fn peek_past_whitespace(&self) -> usize {
        // The ASCII half of `is_svelte_ws` inline, the rest delegated. This runs once per
        // in-tag token over a mean of well under one byte of whitespace, so a plain
        // `skip_svelte_ws(self.source, self.position)` — which is what this is, and reads
        // better — spends most of its time on a per-CHARACTER call into that scan:
        // `instructions:u` +0.294% on a 1,695-file Svelte corpus, and it loses cycles in
        // every replicate of the layout group. The CLASS is still the one definition —
        // only its ASCII arm is peeled, and the first byte at or above U+007F hands the
        // rest of the run straight back to `skip_svelte_ws`.
        let bytes = self.source.as_bytes();
        let mut i = self.position;
        while let Some(&b) = bytes.get(i) {
            if b >= 0x80 {
                return skip_svelte_ws(self.source, i);
            }
            if !is_svelte_ws(b as char) {
                break;
            }
            i += 1;
        }
        i
    }

    /// Skip everything until we hit a special character (<, {)
    /// Used in template mode to treat text content as gaps
    /// Note: '}' is NOT special in template mode - it's only consumed directly
    /// during expression tag parsing. This allows '}' in text (e.g., after {'{'}text})
    /// to be treated as plain text, matching Svelte's parser behavior.
    fn skip_to_special_char(&mut self) {
        // Both needles are ASCII and UTF-8 is self-synchronising — every byte of a
        // multi-byte character is at or above 0x80 — so a byte scan stops exactly where a
        // character scan stops, and every stop is a character boundary. The step is an
        // unconditional `+= 1`, which is the point: a width that depends on the byte's value
        // puts the loop's own cursor downstream of the load.
        let bytes = self.source.as_bytes();
        let mut i = self.position;
        while let Some(&b) = bytes.get(i) {
            if b == b'<' || b == b'{' {
                break;
            }
            i += 1;
        }
        self.position = i;
    }

    /// Advance past the continuation characters of a tag/attribute name, the cursor
    /// already past the name's first character.
    ///
    /// Svelte's `read_tag` name run, and the one place its character class is spelled —
    /// both name-opening arms of [`Lexer::next_token_local`] (ASCII-led and non-ASCII-led)
    /// reach it, so the class cannot drift between them. ⚠️ Not the *unquoted numeric value*
    /// run, which that function scans inline: a narrower class (`is_alphanumeric`, `_`, `-`)
    /// answering to HTML's unquoted-attribute-value grammar rather than to `read_tag`.
    ///
    /// NOTE: for attribute/directive *names* this is only the LEADING run — the parser's
    /// `attribute_name_run_end` extends it past special chars (`a%b`) to Svelte's
    /// `read_tag` terminator set (`[\s=/>"']`), which differs from the tag-name set.
    /// Widen attribute-name coverage there, not this char class.
    fn scan_name_run(&mut self) {
        let bytes = self.source.as_bytes();
        let mut i = self.position;
        while let Some(&b) = bytes.get(i) {
            // The overwhelmingly common name char, and disjoint from every
            // terminator below — so taking it first keeps the whitespace guard
            // off the hot path without changing what the loop accepts.
            if b.is_ascii_alphanumeric() {
                i += 1;
                continue;
            }
            if b < 0x80 {
                // The rest of the ASCII name class, and the whole of it: the two
                // Unicode classes below add nothing under U+0080, since
                // `char::is_alphanumeric` agrees with `is_ascii_alphanumeric`
                // there and `is_pcen_char`'s only other ASCII members are `-`,
                // `.` and `_`. So every other ASCII byte ends the run — Svelte
                // whitespace included, which is what the guard below states for
                // the characters that still reach it.
                if matches!(b, b'_' | b'$' | b'-' | b':' | b'|' | b'.') {
                    i += 1;
                    continue;
                }
                break;
            }
            let Some((ch, width)) = char_at(self.source, i) else {
                break;
            };
            // Whitespace ends a name run before any name-char test, mirroring
            // `read_until(regex)`, where the terminator wins over what the name
            // grammar would otherwise admit. Not redundant with the classes below:
            // PCENChar spans `[#xFDF0-#xFFFD]`, which contains U+FEFF — the one
            // character that is both Svelte whitespace and a custom-element name
            // char. Without this guard `</div\u{feff}>` lexes as the name
            // `div\u{feff}` and fails to close its `div`.
            if is_svelte_ws(ch) {
                break;
            }
            // `is_alphanumeric` covers the non-ASCII Unicode *letters* the fast
            // path above leaves (so `<my-café>` works); `is_pcen_char` adds the
            // non-alphanumeric members of the HTML custom-element name grammar
            // (`·`, ZWNJ/ZWJ, astral emoji) so a whole custom-element name stays in
            // one token. Both are asked only past U+007F, which is the whole of
            // what either adds. Over-admitting (e.g. a PCENChar with no preceding
            // hyphen) is harmless — the parser's `is_valid_tag_name` gate rejects any
            // name that isn't valid.
            if ch.is_alphanumeric() || tsv_html::is_pcen_char(ch) {
                i += width;
            } else {
                break;
            }
        }
        self.position = i;
    }

    /// The lexer's one fallible entry point — so the error path is lifted into host
    /// coordinates here ([`Lexer::host_err`]); the scan itself reports in the lexer's own.
    #[inline]
    pub fn next_token(&mut self) -> Result<Token, ParseError> {
        self.next_token_local().map_err(|err| self.host_err(err))
    }

    /// [`Lexer::next_token`]'s scan, reporting an error at its position in `self.source`.
    fn next_token_local(&mut self) -> Result<Token, ParseError> {
        // Template mode (outside tags): skip text content, only tokenize special chars
        // Tag mode (inside <...>): tokenize everything including identifiers
        if self.inside_tag {
            self.skip_whitespace();
        } else {
            self.skip_to_special_char();
        }

        let start = self.position;

        match self.cur_byte() {
            None => Ok(Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            }),
            Some(b'<') => {
                // Check for HTML comment: <!--
                if self.starts_with(b"<!--") {
                    self.position += b"<!--".len();

                    // Scan until "-->". An ASCII needle again, so the scan steps a byte at
                    // a time and still cannot stop inside a character.
                    while self.position < self.source.len() {
                        if self.starts_with(b"-->") {
                            self.position += b"-->".len();
                            return Ok(self.make_token(TokenKind::Comment, start));
                        }
                        self.position += 1;
                    }

                    // Unterminated comment
                    return Err(lex_err("Unterminated HTML comment", start));
                }

                self.inside_tag = true; // Enter tag mode
                self.advance();
                Ok(self.make_token(TokenKind::LeftAngle, start))
            }
            Some(b'>') => {
                self.inside_tag = false; // Exit tag mode, back to template mode
                self.advance();
                Ok(self.make_token(TokenKind::RightAngle, start))
            }
            Some(b'/') => {
                self.advance();
                Ok(self.make_token(TokenKind::Slash, start))
            }
            Some(b'{') => {
                self.advance();
                // Check for block tokens: {#, {:, {/, {@ — Svelte's `tag()` runs
                // `allow_whitespace()` right after `{`, so the marker may be separated
                // from the brace by whitespace: `{ #if}` tokenizes like `{#if}`. (The
                // runes-mode "no whitespace" rule is a phase-2 validator early-error
                // tsv defers.) Peek past whitespace for a marker; only consume it when
                // one follows, so a bare `{` expression/declaration tag keeps its exact
                // offsets (the block/tag parsers read the keyword from the token end,
                // so absorbing leading whitespace into the marker token is transparent).
                let marker = self.peek_past_whitespace();
                match self.source.as_bytes().get(marker) {
                    Some(b'#') => {
                        self.skip_whitespace();
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockOpen, start))
                    }
                    Some(b':') => {
                        self.skip_whitespace();
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockContinue, start))
                    }
                    // `{/if}` close vs `{/* */}` / `{// }` comment expression: a `*`/`/`
                    // after the marker `/` means a comment, so fall through to LeftBrace.
                    Some(b'/')
                        if !matches!(
                            self.source.as_bytes().get(marker + 1),
                            Some(b'*') | Some(b'/')
                        ) =>
                    {
                        // Block close: {/if}, {/each}, etc
                        self.skip_whitespace();
                        self.advance();
                        Ok(self.make_token(TokenKind::BlockClose, start))
                    }
                    Some(b'@') => {
                        self.skip_whitespace();
                        self.advance();
                        Ok(self.make_token(TokenKind::TagOpen, start))
                    }
                    _ => Ok(self.make_token(TokenKind::LeftBrace, start)),
                }
            }
            Some(b'}') => {
                self.advance();
                Ok(self.make_token(TokenKind::RightBrace, start))
            }
            Some(b'=') => {
                self.advance();
                Ok(self.make_token(TokenKind::Equals, start))
            }
            Some(quote @ (b'\'' | b'"')) => {
                // Quoted attribute value. Only two things matter here: the closing quote,
                // and any `{expr}` tag — whose interior is JS, where the attribute's quote
                // character is just an ordinary byte (`title="{a['\"']}"`).
                //
                // The expression is skipped WHOLE via the shared trivia-aware brace
                // matcher rather than re-lexed here. It already knows every construct in
                // which a `}` or a quote is not code — nested braces, strings (escape
                // aware), template literals including `${…}` interpolation, comments, and
                // regex literals — so no delimiter buried in one can be mistaken for the
                // end of the expression or of the attribute. Hand-tracking a subset of
                // those is the "comment-aware delimiter scan" bug class (see
                // `tsv_debug scan_audit`): a scan tracking braces and strings but
                // not comments or regex is desynced by `title="{/* ` */ b}"` and
                // `title="{f(/"/)}"` and runs to EOF — an over-rejection of Svelte-valid input.
                //
                // `parse_attribute_value` (attribute.rs) re-walks the same value to split
                // it into Text and ExpressionTag parts, and reaches the same answer the
                // same way (via `parse_expression_tag_at`); this is the tokenizing half.
                self.advance(); // consume opening quote

                // A `{#`/`{@` opening the brace ends the value's life as a *sequence*:
                // Svelte's `read_sequence` rejects a block or tag in an attribute value before
                // it reads an expression, and so does `SvelteParser::check_sequence_placement`.
                // The marker need not be glued — `BlockOrTagMarker::in_sequence_at` skips the
                // gap, and must, or the accident below survives one space
                // (`a="{ #if c}a{/if}"`).
                // From that marker on there is no expression to skip, and pretending otherwise
                // loses the error: `style="{#if c}a{/if}"` reaches the `{/if}`, whose `/` opens
                // a regex literal that never closes, so the scan runs to EOF and the whole
                // value dies as `Unterminated string literal` — a lexer accident standing in
                // for the placement rule the author actually broke. Reading the rest as plain
                // bytes closes the string at its real quote, which is the HTML-level delimiter
                // the static reader uses anyway, and hands the parser the position where the
                // rule lives.
                let mut sequence_is_invalid = false;
                // Borrowed from the immutable source, so both outlive the `&mut self` `seek_to`
                // below rather than being re-taken per brace.
                let source = self.source;
                let bytes = source.as_bytes();

                while let Some(&b) = bytes.get(self.position) {
                    if b == quote {
                        self.position += 1; // consume closing quote
                        return Ok(self.make_token(TokenKind::String, start));
                    }
                    if b == b'{' && !sequence_is_invalid {
                        if BlockOrTagMarker::in_sequence_at(source, self.position).is_some() {
                            sequence_is_invalid = true;
                        } else {
                            let Some(close) = source_scan::scan_to_matching_brace(
                                bytes,
                                self.position + 1,
                                source.len(),
                            ) else {
                                break; // unterminated `{` — the value can't close
                            };
                            self.seek_to(close + '}'.len_utf8());
                            continue;
                        }
                    }
                    // Attribute-value text. HTML/Svelte attribute values have NO backslash
                    // escapes (unlike a JS string inside `{expr}`, skipped above), so `\`
                    // is a literal char: `a="{x}\"` closes at the `"` with value `{x}\`,
                    // matching Svelte's parser. Treating `\` as an escape here read `\"` as
                    // an escaped quote and ran past the close → "Unterminated string
                    // literal" (an over-rejection of valid Svelte; the `fuzz` gate).
                    //
                    // Both needles are ASCII, so — as in `skip_to_special_char` — stepping
                    // one byte cannot stop inside a character.
                    self.position += 1;
                }
                // Unterminated string
                Err(lex_err("Unterminated string literal in template", start))
            }
            Some(b) if b.is_ascii_alphabetic() || matches!(b, b'_' | b'$' | b'-' | b'!') => {
                // Tag names and identifiers.
                // NOTE: for attribute/directive *names* this token is only the LEADING run —
                // the parser's `attribute_name_run_end` extends it past special chars (`a%b`)
                // to Svelte's `read_tag` terminator set (`[\s=/>"']`), which differs from the
                // tag-name set. Widen attribute-name coverage there, not this char class.
                // Also include - as a start character for CSS custom property attributes (--margin)
                // and include : and | for directive syntax (on:click|preventDefault)
                // and -- for CSS custom properties (style:--custom)
                // and . for dot notation components (ns.Comp)
                // and ! for <!DOCTYPE> (Svelte treats !DOCTYPE as the element name)
                // Advance past first char — ! is a valid start but not a continuation char.
                // One byte: this arm matched an ASCII one (the non-ASCII-led name arm below
                // steps its first character whole).
                self.position += 1;
                self.scan_name_run();
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            Some(b) if b.is_ascii_digit() => {
                // Unquoted numeric attribute values (e.g., data-count=123)
                // HTML allows unquoted values that are alphanumeric
                let bytes = self.source.as_bytes();
                let mut i = self.position;
                while let Some(&b) = bytes.get(i) {
                    if b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-') {
                        i += 1;
                        continue;
                    }
                    if b < 0x80 {
                        break;
                    }
                    // `_` and `-` are ASCII, so past U+007F the class is `is_alphanumeric`
                    // alone.
                    let Some((ch, width)) = char_at(self.source, i) else {
                        break;
                    };
                    if ch.is_alphanumeric() {
                        i += width;
                    } else {
                        break;
                    }
                }
                self.position = i;
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            // A non-ASCII letter opens a name run too (`<café>`, `<Ωmega>`). This arm and
            // the ASCII-led one above are the two halves of a single `is_alphabetic` test,
            // split on U+007F so the common path never decodes; everything else past U+007F
            // is a single-character Identifier, per the arm below.
            Some(b) if b >= 0x80 && self.cur_char().is_some_and(char::is_alphabetic) => {
                self.advance();
                self.scan_name_run();
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            // Any other char inside a tag is a name char per Svelte's `read_tag`
            // (a name run is anything but `/[\s=/>"']/`, and every one of those
            // terminators is handled by an arm above). Emit it as a single-char
            // Identifier; the parser's `attribute_name_run_end` extends it into the
            // full name, so a symbol-led attribute name (`<div %foo>`, `[innerHTML]`)
            // parses as Svelte's `read_static_attribute` reads it. This arm is
            // reached only inside a tag (template mode stops at `<`/`{`), and it only
            // ever converts a former hard error into a token — so it cannot regress a
            // previously-valid parse. A symbol-led *tag* name (`<%foo>`, `<_foo>`) is then
            // rejected by the element parser's `is_valid_tag_name` gate (`parser/element.rs`),
            // which validates the whole name against Svelte's element/component grammar — so
            // this arm never turns an invalid tag name into an accepted element.
            Some(_) => {
                self.advance();
                Ok(self.make_token(TokenKind::Identifier, start))
            }
        }
    }
}
