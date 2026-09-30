use std::fmt;
// Shared lexer-error constructor: used by the unterminated/unexpected sites in the token scan.
use tsv_lang::{ParseError, lex_err};

use crate::parser::scan_to_matching_brace;
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

/// A lexed Svelte markup token: a small size-asserted POD with `u32` document offsets,
/// lexed in place into the parser's current-token and lookahead slots
/// (`Lexer::next_token_into`) — the lexer's only token entry point, so there is no
/// by-value form. `Clone` (not `Copy`) mirrors the `tsv_ts::Token` / `tsv_css::Token`
/// convention — the parser is the single owner of `current` / `peek`, consuming via
/// `.take()` / move rather than implicit copies.
/// There is **no out-of-band decoded value**: markup tokens are pure spans (the
/// embedded TS/CSS/expression content is lexed by the other crates).
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: u32,
    pub end: u32,
}

// Guards the hot-path invariant: `Token` is a small POD with no heap-owning field, so the
// in-place write (`next_token_into`) stays a few plain stores per token. 12 bytes (not the
// TS/CSS 16): the fieldless `TokenKind` is 1 byte, whereas theirs carries a `char` payload.
const _: () = assert!(size_of::<Token>() == 12);

/// The ASCII half of [`Lexer::scan_name_run`]'s character class, one entry per byte: the
/// ASCII letters and digits, `_`, `$`, `-`, `:`, `|` and `.`. Every entry at or above 0x80
/// is `false`, so a non-ASCII byte stops the table walk and the character question goes to
/// [`name_run_end_from_non_ascii`].
///
/// The whole ASCII class, not a fast subset of it: the two Unicode classes the non-ASCII
/// arm asks add nothing under U+0080 — `char::is_alphanumeric` agrees with
/// `is_ascii_alphanumeric` there, and `is_pcen_char`'s only other ASCII members are `-`,
/// `.` and `_` — so every other ASCII byte ends the run, Svelte whitespace included.
const NAME_RUN_ASCII: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 0x80 {
        let b = i as u8;
        t[i] = b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'-' | b':' | b'|' | b'.');
        i += 1;
    }
    t
};

/// The first `<` or `{` at or after `from` — the end of `bytes` when none follows: where a
/// template-mode token can start ([`Lexer::skip_to_special_char`]).
///
/// Both needles are ASCII and UTF-8 is self-synchronising — every byte of a multi-byte
/// character is at or above 0x80 — so a byte scan stops exactly where a character scan stops,
/// and every stop is a character boundary.
///
/// The byte at `from` is tested first: the dispatch reaches this through `skip_to_special_char`
/// with the cursor already on a token's first byte whenever the front handed it on, so the gap
/// is usually empty there, and two compares retire it where the word hop's entry costs its
/// constants. Past it the run is text, eight
/// bytes a word (`tsv_lang::swar::next_byte_of`) — over the whole document, so only the last
/// word of the file falls to the hop's byte tail.
#[inline]
fn special_byte_at_or_after(bytes: &[u8], from: usize) -> usize {
    match bytes.get(from) {
        None | Some(b'<' | b'{') => from,
        Some(_) => tsv_lang::swar::next_byte_of(bytes, from + 1, [b'<', b'{']),
    }
}

/// One past the first `-->` at or after `from` — the end of the HTML comment whose body starts
/// there — or `None` when the comment never closes.
///
/// The body is hopped a word at a time to each `-` (`tsv_lang::swar::next_byte_of`), and only
/// there are the next two bytes asked. Every byte of the needle is ASCII, so — as in
/// [`special_byte_at_or_after`] — no stop falls inside a character.
#[inline]
fn html_comment_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    loop {
        i = tsv_lang::swar::next_byte_of(bytes, i, [b'-']);
        if i >= bytes.len() {
            return None;
        }
        if bytes[i..].starts_with(b"-->") {
            return Some(i + b"-->".len());
        }
        i += 1;
    }
}

/// The ASCII half of [`is_svelte_ws`], one entry per byte: `<TAB>` through `<CR>` and `<SP>`.
/// Every entry at or above 0x80 is `false`, so a non-ASCII byte stops the walk.
const SVELTE_WS_ASCII: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = 0;
    while i < 0x80 {
        t[i] = is_svelte_ws(i as u8 as char);
        i += 1;
    }
    t
};

/// The end of the run of ASCII Svelte whitespace bytes ([`SVELTE_WS_ASCII`]) from `from` on —
/// `from` itself when none is there. A table walk, one load and one compare a byte, where
/// the class's own spelling is a subtract and two compares.
#[inline]
fn ascii_svelte_ws_run_end(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && SVELTE_WS_ASCII[usize::from(bytes[i])] {
        i += 1;
    }
    i
}

/// The end of the run of [`NAME_RUN_ASCII`] bytes from `from` on — `from` itself when none is
/// there. The run stops on any other byte, so a non-ASCII byte at the end it returns is where
/// the name may go on.
#[inline]
fn ascii_name_run_end(bytes: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < bytes.len() && NAME_RUN_ASCII[usize::from(bytes[i])] {
        i += 1;
    }
    i
}

/// [`Lexer::scan_name_run`] from a non-ASCII character at `i` onward: the end of the name
/// run, the whole class asked a character at a time.
///
/// Cold and out of line — a non-ASCII name character is rare, and this arm's decode and
/// Unicode tables would otherwise set up their constants and save their registers on every
/// name the lexer scans.
#[cold]
#[inline(never)]
fn name_run_end_from_non_ascii(source: &str, mut i: usize) -> usize {
    let bytes = source.as_bytes();
    while let Some(&b) = bytes.get(i) {
        if NAME_RUN_ASCII[usize::from(b)] {
            i += 1;
            continue;
        }
        if b < 0x80 {
            break;
        }
        let Some((ch, width)) = char_at(source, i) else {
            break;
        };
        // Whitespace ends a name run before any name-char test, mirroring
        // `read_until(regex)`, where the terminator wins over what the name
        // grammar would otherwise admit. Not redundant with the classes below:
        // two characters are both Svelte whitespace and custom-element name
        // chars — U+FEFF, inside PCENChar's `[#xFDF0-#xFFFD]`, and U+1680 (the
        // Ogham space mark), inside its `[#x37F-#x1FFF]`. Without this guard
        // `</div\u{feff}>` lexes as the name `div\u{feff}` and fails to close its
        // `div`.
        if is_svelte_ws(ch) {
            break;
        }
        // `is_alphanumeric` covers the non-ASCII Unicode *letters* the ASCII table
        // leaves (so `<my-café>` works); `is_pcen_char` adds the non-alphanumeric
        // members of the HTML custom-element name grammar (`·`, ZWNJ/ZWJ, astral
        // emoji) so a whole custom-element name stays in one token. Both are asked
        // only past U+007F, which is the whole of what either adds. Over-admitting
        // (e.g. a PCENChar with no preceding hyphen) is harmless — the parser's
        // `is_valid_tag_name` gate rejects any name that isn't valid.
        if ch.is_alphanumeric() || tsv_html::is_pcen_char(ch) {
            i += width;
        } else {
            break;
        }
    }
    i
}

/// The end of a quoted attribute value — one past its closing `quote` — from the first `{`
/// in it, at `brace`; `None` when the value never closes.
///
/// The expression is skipped WHOLE via the shared trivia-aware brace matcher rather than
/// re-lexed here. It already knows every construct in which a `}` or a quote is not code —
/// nested braces, strings (escape aware), template literals including `${…}` interpolation,
/// comments, and regex literals — so no delimiter buried in one can be mistaken for the end
/// of the expression or of the attribute. Hand-tracking a subset of those is the
/// "comment-aware delimiter scan" bug class (see `tsv_debug scan_audit`): a scan tracking
/// braces and strings but not comments or regex is desynced by `title="{/* ` */ b}"` and
/// `title="{f(/"/)}"` and runs to EOF — an over-rejection of Svelte-valid input.
///
/// `parse_attribute_value` (attribute.rs) re-walks the same value to split it into Text and
/// ExpressionTag parts, and reaches the same answer the same way (via
/// `parse_expression_tag_at`); this is the tokenizing half.
///
/// A `{#`/`{@` opening the brace ends the value's life as a *sequence*: Svelte's
/// `read_sequence` rejects a block or tag in an attribute value before it reads an
/// expression, and so does `SvelteParser::check_sequence_placement`. The marker need not be
/// glued — `BlockOrTagMarker::in_sequence_at` skips the gap, and must, or the accident below
/// survives one space (`a="{ #if c}a{/if}"`). From that marker on there is no expression to
/// skip, and pretending otherwise loses the error: `style="{#if c}a{/if}"` reaches the
/// `{/if}`, whose `/` opens a regex literal that never closes, so the scan runs to EOF and
/// the whole value dies as `Unterminated string literal` — a lexer accident standing in for
/// the placement rule the author actually broke. Reading the rest as plain bytes closes the
/// string at its real quote, which is the HTML-level delimiter the static reader uses
/// anyway, and hands the parser the position where the rule lives.
///
/// Cold and out of line: few values hold a `{` at all, and this arm's matcher and marker
/// test would otherwise set up their state on every value the lexer walks.
#[cold]
#[inline(never)]
fn quoted_value_end_from_brace(source: &str, brace: usize, quote: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut i = brace;
    let mut sequence_is_invalid = false;
    loop {
        let b = *bytes.get(i)?;
        if b == quote {
            return Some(i + 1);
        }
        if b == b'{' && !sequence_is_invalid {
            if BlockOrTagMarker::in_sequence_at(source, i).is_some() {
                sequence_is_invalid = true;
            } else {
                // `None` — an unterminated `{` — and the value can't close.
                i = scan_to_matching_brace(bytes, i + 1)? + '}'.len_utf8();
                continue;
            }
        }
        i += 1;
    }
}

pub struct Lexer<'a> {
    source: &'a str,
    /// The cursor, as a byte offset into `source` — always on a character boundary. `source`
    /// is the whole document, so the cursor and every token span are document offsets.
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
    pub inside_tag: bool, // Track if we're inside <...>
    /// The start of the last String token lexed whose value holds no `{` — `u32::MAX`
    /// before there is one. What [`Lexer::string_holds_no_brace`] reads.
    ///
    /// It can never be stale: a String token is a function of its start alone (the scan
    /// reads the immutable document from there and nothing else), so "the String at `S`
    /// holds no `{`" stays true however the cursor moves afterwards. Only a later brace-free
    /// String replaces it; an asker whose String is not the last brace-free one lexed simply
    /// misses.
    plain_string_start: u32,
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
    /// A lexer over the whole document `source`, its cursor past a leading byte-order mark
    /// ([`tsv_lang::leading_bom_len`]).
    pub fn new(source: &'a str) -> Self {
        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes; the WIRE
        // elides it at emission (`LeadingBom::Elided` in the writer), since Svelte's
        // `parse` strips it before parsing and its offsets index the BOM-less string.
        Self {
            source,
            position: tsv_lang::leading_bom_len(source),
            inside_tag: false,
            plain_string_start: u32::MAX,
        }
    }

    /// Move the cursor to the document offset `pos`, a character boundary, in either
    /// direction — the parser's resume after a scan it ran over the source itself
    /// (`SvelteParser::advance_to_position`). The mode (`inside_tag`) is kept.
    ///
    /// No byte-order-mark skip at `pos`: the only BOM is at byte 0 ([`Lexer::new`]). A
    /// U+FEFF at a resume point is skipped by the next token's scan either way — it is
    /// Svelte whitespace ([`is_svelte_ws`]) in tag mode, and not `<` or `{` in template mode.
    pub fn seek(&mut self, pos: usize) {
        assert!(
            self.source.is_char_boundary(pos),
            "seek to {pos}, not a character boundary of the document"
        );
        self.position = pos;
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

    /// Whether the source from the current position starts with `needle`.
    /// Used for the ASCII comment opener (`<!--`); a byte compare is exact for ASCII
    /// needles and avoids the per-call UTF-8 char counting.
    #[inline]
    fn starts_with(&self, needle: &[u8]) -> bool {
        self.source.as_bytes()[self.position..].starts_with(needle)
    }

    /// Whether the String token starting at document offset `start` holds no `{` between
    /// its quotes — so its whole content is one attribute-value Text, with no expression
    /// tag to split out. `true` only for the last brace-free String lexed — the parser's
    /// current token unless a lookahead has lexed another brace-free String past it;
    /// `false` is "not known", never "holds a `{`".
    #[inline]
    pub(crate) fn string_holds_no_brace(&self, start: usize) -> bool {
        self.plain_string_start as usize == start
    }

    fn skip_whitespace(&mut self) {
        self.position = self.peek_past_whitespace();
    }

    /// Byte offset of the first non-whitespace char at or after the cursor, without
    /// consuming input. Whitespace matches `skip_whitespace` ([`is_svelte_ws`]), so a
    /// follow-up `skip_whitespace()` lands exactly here.
    #[inline]
    fn peek_past_whitespace(&self) -> usize {
        // `skip_svelte_ws(self.source, self.position)` with its ASCII half peeled into a
        // table walk: the run is ASCII on nearly every token, and that scan asks a character
        // at a time. The CLASS is still the one definition — the first byte at or above
        // U+007F hands the rest of the run straight back to `skip_svelte_ws`.
        let bytes = self.source.as_bytes();
        let i = ascii_svelte_ws_run_end(bytes, self.position);
        if bytes.get(i).is_some_and(|&b| b >= 0x80) {
            skip_svelte_ws(self.source, i)
        } else {
            i
        }
    }

    /// Skip everything until we hit a special character (<, {)
    /// Used in template mode to treat text content as gaps
    /// Note: '}' is NOT special in template mode - it's only consumed directly
    /// during expression tag parsing. This allows '}' in text (e.g., after {'{'}text})
    /// to be treated as plain text, matching Svelte's parser behavior.
    fn skip_to_special_char(&mut self) {
        self.position = special_byte_at_or_after(self.source.as_bytes(), self.position);
    }

    /// Advance past the continuation characters of a tag/attribute name, the cursor
    /// already past the name's first character.
    ///
    /// Svelte's `read_tag` name run, and the one place its character class is spelled —
    /// both name-opening arms of [`Lexer::next_token_dispatch`] (ASCII-led and non-ASCII-led)
    /// reach it, and the front's ASCII-led arm ([`Lexer::next_token_into`]) walks the same
    /// [`ascii_name_run_end`] and hands on a run that reaches a non-ASCII byte, so the class
    /// cannot drift between them. ⚠️ Not the *unquoted numeric value* run, which the dispatch
    /// scans inline: a narrower class (`is_alphanumeric`, `_`, `-`) answering to HTML's
    /// unquoted-attribute-value grammar rather than to `read_tag`.
    ///
    /// NOTE: for attribute/directive *names* this is only the LEADING run — the parser's
    /// `attribute_name_run_end` extends it past special chars (`a%b`) to Svelte's
    /// `read_tag` terminator set (`[\s=/>"']`), which differs from the tag-name set.
    /// Widen attribute-name coverage there, not this char class.
    ///
    /// The ASCII run is one [`NAME_RUN_ASCII`] load a byte; the first byte at or above
    /// U+0080 hands the rest of the run to [`name_run_end_from_non_ascii`], out of line,
    /// so the common path neither decodes nor pays that arm's constants and register saves.
    fn scan_name_run(&mut self) {
        let bytes = self.source.as_bytes();
        let i = ascii_name_run_end(bytes, self.position);
        self.position = if bytes.get(i).is_some_and(|&b| b >= 0x80) {
            name_run_end_from_non_ascii(self.source, i)
        } else {
            i
        };
    }

    /// Lex the next token straight into `*dst` — the parser's current-token slot or its
    /// lookahead slot. A `Result<Token, ParseError>` does not come back in registers: it is
    /// returned through a stack slot the caller then reloads and re-scatters, once per
    /// token, so writing through the caller's slot leaves only the error pointer to return.
    /// `*dst` is written only on success.
    ///
    /// This is the scan's **front**: it lexes every token whose path makes no call — the gap
    /// before it (an empty one in template mode, or an ASCII whitespace run in a tag), end
    /// of input, the punctuation, a bare `{` and a marker glued to its brace, and a name whose
    /// run stays ASCII, which together are most tokens — and hands the rest to a
    /// function that finishes the token, entered as the front's last act so the handoff
    /// compiles to a jump: [`Lexer::text_gap_then_token_into`] for template text ahead of the
    /// token, which hops the text and re-enters the front at its end;
    /// [`Lexer::string_token_into`] for a quoted value; and
    /// [`Lexer::next_token_dispatch`] for a comment or `<!` declaration, a marker separated
    /// from its brace, an unquoted numeric value, a non-ASCII byte anywhere the front reads
    /// one, and the errors. Each finisher lexes the token from the cursor the front leaves
    /// it, which is the token's first byte, a byte inside the template text before it (the
    /// text finisher's hop finishes that text), or, in a tag, a non-ASCII byte inside the
    /// whitespace gap before it (the dispatch's gap skip finishes that gap). The split is what
    /// keeps the front free of a frame: a function making the scanners' calls saves registers
    /// on entry and restores them at every exit — on every token, the ones that call nothing
    /// included.
    ///
    /// ⚠️ So nothing the front reaches may make a call it then continues past. A handoff must
    /// stay in tail position and return this function's own `Result` (a one-word niche,
    /// returned in a register); a callee that returns anything else puts the call and the
    /// frame back. And the front runs on the caller-saved registers alone, so what it keeps
    /// live is budgeted too: the template text's and the quoted value's word loops are
    /// finishers of their own for that reason, a marker behind a gap is read by the dispatch
    /// rather than here (walking the gap kept two registers more live than the front has), and
    /// the whitespace and name runs are table walks over one cursor
    /// ([`ascii_svelte_ws_run_end`], [`ascii_name_run_end`]).
    ///
    /// `#[inline(never)]`: small and frame-free, the front would otherwise be a candidate for
    /// inlining into its hot callers — `SvelteParser::advance`, `fill_peek` — and a lexer inlined
    /// into a frame the parser's recursion stacks grows that frame.
    ///
    /// The match yields only the token's KIND — every token spans `start` to the cursor — so
    /// the one store at the end is the only write of `*dst`.
    #[inline(never)]
    pub fn next_token_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        let bytes = self.source.as_bytes();
        let mut i = self.position;
        if self.inside_tag {
            // Tag mode: step over the whitespace between tokens, its ASCII half here. A
            // non-ASCII byte may be Svelte whitespace too: it reaches the match below as a
            // token's first byte, which hands it on, and the dispatch's own gap skip finishes
            // the gap from it.
            i = ascii_svelte_ws_run_end(bytes, i);
        } else if !matches!(bytes.get(i), None | Some(b'<' | b'{')) {
            // Template mode, and text before the next token: the gap `skip_to_special_char`
            // skips, hopped out of line (`text_gap_then_token_into`). A token with text before
            // it — most template tokens, formatted markup putting a newline and indent between
            // them — hops the text out of line; one with none never leaves the front.
            self.position = i + 1;
            return self.text_gap_then_token_into(dst);
        }

        let start = i;
        let Some(&b) = bytes.get(start) else {
            // The empty token at `start`.
            self.position = start;
            *dst = Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            };
            return Ok(());
        };
        // Every arm the front settles leaves `i` at the token's end; every handoff leaves the
        // cursor at `start`.
        macro_rules! hand_on {
            () => {{
                self.position = start;
                return self.next_token_dispatch(dst);
            }};
        }
        let kind = match b {
            // `<!--` opens a comment and `<!` a declaration name; the dispatch reads both.
            b'<' if bytes.get(start + 1) == Some(&b'!') => hand_on!(),
            b'<' => {
                self.inside_tag = true; // Enter tag mode
                i = start + 1;
                TokenKind::LeftAngle
            }
            b'>' => {
                self.inside_tag = false; // Exit tag mode, back to template mode
                i = start + 1;
                TokenKind::RightAngle
            }
            b'/' => {
                i = start + 1;
                TokenKind::Slash
            }
            // A marker glued to its brace: the token runs one past the marker byte. A gap
            // between the two goes to the dispatch, which reads the marker past it. End of
            // input reads as a NUL, which is neither a marker nor whitespace — a bare `{`, as
            // the dispatch has it — so no path of its own merges into the others.
            b'{' => match bytes.get(start + 1).copied().unwrap_or(0) {
                b'#' => {
                    i = start + 2;
                    TokenKind::BlockOpen
                }
                b':' => {
                    i = start + 2;
                    TokenKind::BlockContinue
                }
                b'/' if !matches!(bytes.get(start + 2), Some(b'*' | b'/')) => {
                    i = start + 2;
                    TokenKind::BlockClose
                }
                b'@' => {
                    i = start + 2;
                    TokenKind::TagOpen
                }
                c if c >= 0x80 || is_svelte_ws(c as char) => hand_on!(),
                _ => {
                    i = start + 1;
                    TokenKind::LeftBrace
                }
            },
            b'}' => {
                i = start + 1;
                TokenKind::RightBrace
            }
            b'=' => {
                i = start + 1;
                TokenKind::Equals
            }
            // A quoted value: `string_token_into`'s hop keeps more constants and cursors live
            // than the front has caller-saved registers for.
            b'\'' | b'"' => {
                self.position = start;
                return self.string_token_into(dst);
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | b'-' | b'!' => {
                // The dispatch's ASCII-led name, when its run stays ASCII (`scan_name_run`'s
                // table walk, past the first byte — `!` opens a name but does not continue one).
                i = ascii_name_run_end(bytes, start + 1);
                if bytes.get(i).is_some_and(|&c| c >= 0x80) {
                    hand_on!();
                }
                TokenKind::Identifier
            }
            // An unquoted numeric value, and every non-ASCII lead byte.
            b'0'..=b'9' | 0x80..=0xFF => hand_on!(),
            // Any other ASCII byte: the dispatch's single-character name.
            _ => {
                i = start + 1;
                TokenKind::Identifier
            }
        };
        self.position = i;
        *dst = Token {
            kind,
            start: start as u32,
            end: i as u32,
        };
        Ok(())
    }

    /// Skip the template text from the cursor — a byte past the first byte of the gap the front
    /// ([`Lexer::next_token_into`]) found — to the next `<` or `{`, then lex the token there
    /// into `*dst` by re-entering the front, which finds the gap empty.
    ///
    /// The text is hopped eight bytes a word (`tsv_lang::swar::next_byte_of`), whose constants
    /// and cursors are more than the front can hold on the caller-saved registers, so the hop
    /// lives here and the front enters it in tail position, as this function re-enters the
    /// front: each handoff compiles to a jump, and only a token with text before it pays this
    /// function's register saves.
    #[inline(never)]
    fn text_gap_then_token_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        self.position =
            tsv_lang::swar::next_byte_of(self.source.as_bytes(), self.position, [b'<', b'{']);
        self.next_token_into(dst)
    }

    /// Lex the quoted attribute value whose opening quote is at the cursor into `*dst` — a
    /// String token, from the quote through the closing one.
    ///
    /// Only two things matter here: the closing quote, and any `{expr}` tag — whose interior
    /// is JS, where the attribute's quote character is just an ordinary byte
    /// (`title="{a['\"']}"`). The first `{` hands the rest of the value to
    /// `quoted_value_end_from_brace`, out of line: most values hold none, so the value is one
    /// hop to the first of the two bytes, eight bytes a word (`tsv_lang::swar::next_byte_of`),
    /// instantiated per quote kind so both needles reach the word loop as constants.
    ///
    /// Attribute-value text. HTML/Svelte attribute values have NO backslash escapes (unlike a
    /// JS string inside `{expr}`), so `\` is a literal char: `a="{x}\"` closes at the `"` with
    /// value `{x}\`, matching Svelte's parser. Treating `\` as an escape here read `\"` as an
    /// escaped quote and ran past the close → "Unterminated string literal" (an
    /// over-rejection of valid Svelte; the `fuzz` gate).
    ///
    /// Both needles are ASCII, so — as in [`special_byte_at_or_after`] — the hop cannot stop inside
    /// a character.
    ///
    /// Out of line, and entered by both scans in tail position: the word loop's constants and
    /// cursors are more than the front ([`Lexer::next_token_into`]) can hold without saving
    /// registers, which it would then do on every token.
    #[inline(never)]
    fn string_token_into(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        let bytes = self.source.as_bytes();
        let start = self.position;
        let quote = bytes[start];
        debug_assert!(matches!(quote, b'\'' | b'"'));
        let from = start + 1; // past the opening quote, an ASCII byte
        let stop = if quote == b'"' {
            tsv_lang::swar::next_byte_of(bytes, from, [b'"', b'{'])
        } else {
            tsv_lang::swar::next_byte_of(bytes, from, [b'\'', b'{'])
        };
        let end = match bytes.get(stop) {
            Some(b'{') => quoted_value_end_from_brace(self.source, stop, quote),
            // The closing quote, and a value with no `{` in it.
            Some(_) => {
                self.plain_string_start = start as u32;
                Some(stop + 1)
            }
            None => None,
        };
        let Some(end) = end else {
            return Err(lex_err("Unterminated string literal in template", start));
        };
        self.position = end;
        *dst = Token {
            kind: TokenKind::String,
            start: start as u32,
            end: end as u32,
        };
        Ok(())
    }

    /// Lex the token at the cursor into `*dst` — every token the front
    /// ([`Lexer::next_token_into`]) hands on, and the whole scan, gap included, from any
    /// cursor: entered at a token's first byte the gap skip below is a no-op (a tag-mode token
    /// does not open with whitespace, and a template-mode one opens with the `<` or `{` the
    /// skip stops on), and entered inside a tag's whitespace gap it finishes the gap.
    ///
    /// `#[inline(never)]` because the front's frame-free shape depends on it: inlined, its
    /// calls would put their register saves back on every token.
    #[inline(never)]
    fn next_token_dispatch(&mut self, dst: &mut Token) -> Result<(), ParseError> {
        // Template mode (outside tags): skip text content, only tokenize special chars
        // Tag mode (inside <...>): tokenize everything including identifiers
        if self.inside_tag {
            self.skip_whitespace();
        } else {
            self.skip_to_special_char();
        }

        let start = self.position;

        let kind = match self.cur_byte() {
            // The cursor has not moved, so this is the empty token at `start`.
            None => TokenKind::Eof,
            Some(b'<') => {
                // Check for HTML comment: <!--
                if self.starts_with(b"<!--") {
                    let Some(end) = html_comment_end(self.source.as_bytes(), start + b"<!--".len())
                    else {
                        return Err(lex_err("Unterminated HTML comment", start));
                    };
                    self.position = end;
                    TokenKind::Comment
                } else {
                    self.inside_tag = true; // Enter tag mode
                    self.position = start + 1;
                    TokenKind::LeftAngle
                }
            }
            Some(b'>') => {
                self.inside_tag = false; // Exit tag mode, back to template mode
                self.position = start + 1;
                TokenKind::RightAngle
            }
            Some(b'/') => {
                self.position = start + 1;
                TokenKind::Slash
            }
            Some(b'{') => {
                // Every byte this arm steps over is ASCII — the brace, and the marker the
                // whitespace skip lands on — so each step is one byte.
                self.position = start + 1;
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
                        self.position = marker + 1;
                        TokenKind::BlockOpen
                    }
                    Some(b':') => {
                        self.position = marker + 1;
                        TokenKind::BlockContinue
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
                        self.position = marker + 1;
                        TokenKind::BlockClose
                    }
                    Some(b'@') => {
                        self.position = marker + 1;
                        TokenKind::TagOpen
                    }
                    _ => TokenKind::LeftBrace,
                }
            }
            Some(b'}') => {
                self.position = start + 1;
                TokenKind::RightBrace
            }
            Some(b'=') => {
                self.position = start + 1;
                TokenKind::Equals
            }
            Some(b'\'' | b'"') => return self.string_token_into(dst),
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
                TokenKind::Identifier
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
                TokenKind::Identifier
            }
            // A non-ASCII letter opens a name run too (`<café>`, `<Ωmega>`). This arm and
            // the ASCII-led one above are the two halves of a single `is_alphabetic` test,
            // split on U+007F so the common path never decodes; everything else past U+007F
            // is a single-character Identifier, per the arm below.
            Some(b) if b >= 0x80 && self.cur_char().is_some_and(char::is_alphabetic) => {
                self.advance();
                self.scan_name_run();
                TokenKind::Identifier
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
                TokenKind::Identifier
            }
        };
        *dst = Token {
            kind,
            start: start as u32,
            end: self.position as u32,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Lexer, ParseError, Token, TokenKind, char_at, html_comment_end, is_svelte_ws, lex_err,
        special_byte_at_or_after,
    };

    /// The brace-free fact [`Lexer::string_holds_no_brace`] answers: asked right after each
    /// String, it holds for the brace-free values and not for the one holding an expression
    /// tag, and a later brace-free String replaces an earlier one's.
    #[test]
    fn string_holds_no_brace_names_the_last_brace_free_string() {
        let source = r#"<a b="x" c="{y}" d='z' e="">"#;
        let mut lexer = Lexer::new(source);
        let mut token = Token {
            kind: TokenKind::Eof,
            start: 0,
            end: 0,
        };
        let mut answers = Vec::new();
        let mut first_string = None;
        loop {
            lexer.next_token_into(&mut token).unwrap();
            match token.kind {
                TokenKind::Eof => break,
                TokenKind::String => {
                    first_string.get_or_insert(token.start as usize);
                    answers.push(lexer.string_holds_no_brace(token.start as usize));
                }
                _ => {}
            }
        }
        assert_eq!(answers, [true, false, true, true]);
        assert!(!lexer.string_holds_no_brace(first_string.unwrap()));
    }

    /// What one step leaves observable: the token's kind and span, then the cursor, the mode
    /// and the brace-free String fact.
    type Step = (TokenKind, u32, u32, usize, bool, u32);

    /// Lex `source` to its end (or its first error) through the front or through the dispatch
    /// alone, one step a token, recording each token with the lexer state the step leaves.
    fn lex_all(source: &str, front: bool) -> Vec<Result<Step, ParseError>> {
        let mut lexer = Lexer::new(source);
        let mut token = Token {
            kind: TokenKind::Eof,
            start: 0,
            end: 0,
        };
        let mut steps = Vec::new();
        for _ in 0..=source.len() {
            let step = if front {
                lexer.next_token_into(&mut token)
            } else {
                lexer.next_token_dispatch(&mut token)
            };
            let failed = step.is_err();
            steps.push(step.map(|()| {
                (
                    token.kind,
                    token.start,
                    token.end,
                    lexer.position,
                    lexer.inside_tag,
                    lexer.plain_string_start,
                )
            }));
            if failed {
                break;
            }
            if token.kind == TokenKind::Eof {
                break;
            }
        }
        steps
    }

    /// The front ([`Lexer::next_token_into`]) lexes every source to the token stream the
    /// dispatch alone produces — the tokens, the cursor, the mode and the brace-free String
    /// fact after each step, and the error — message and position — a step stops at — whichever arm it settles and
    /// wherever it hands on. Sources of up to three fragments over every byte class the front
    /// branches on, each fragment reached in template mode and in a tag.
    #[test]
    fn front_matches_the_dispatch_on_every_fragment_sequence() {
        let fragments = [
            "<",
            ">",
            "/",
            "=",
            "}",
            "{",
            "{#",
            "{ #",
            "{:",
            "{\t:",
            "{/",
            "{/*",
            "{//",
            "{@",
            "{\u{a0}#",
            "{\u{feff}",
            "{é",
            " ",
            "\n\t",
            "\u{a0}",
            "\u{2028}",
            "\u{feff}x",
            "a",
            "div",
            "on:click|once",
            "a.b",
            "!DOCTYPE",
            "<!--c-->",
            "<!--",
            "<!x",
            "\"x\"",
            "'y'",
            "\"{x}\"",
            "'{#if c}'",
            "\"",
            "'",
            "\"a",
            "12",
            "3é",
            "é",
            "Ωmega",
            "a\u{b7}b",
            "x-café",
            "%",
            "[",
            "\\",
            "\u{1}",
            "\0",
            "{\0",
            "text",
        ];
        for a in fragments {
            for b in fragments {
                for c in ["", "<", " ", "a", "{", "é", "\""] {
                    for prefix in ["", "<x "] {
                        let source = format!("{prefix}{a}{b}{c}");
                        assert_eq!(
                            lex_all(&source, true),
                            lex_all(&source, false),
                            "{source:?}"
                        );
                    }
                }
            }
        }
    }

    /// The name-run class as one character loop — the predicate the table walk and its
    /// cold non-ASCII arm split between them, spelled whole so the split is graded
    /// against something that never made it.
    fn name_run_end_model(source: &str, mut i: usize) -> usize {
        while let Some((ch, width)) = char_at(source, i) {
            let continues = if ch.is_ascii() {
                ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '-' | ':' | '|' | '.')
            } else {
                !is_svelte_ws(ch) && (ch.is_alphanumeric() || tsv_html::is_pcen_char(ch))
            };
            if !continues {
                break;
            }
            i += width;
        }
        i
    }

    fn scan_from(source: &str, start: usize) -> usize {
        let mut lexer = Lexer::new(source);
        lexer.seek(start);
        lexer.scan_name_run();
        lexer.position
    }

    /// Every Unicode scalar — so every byte a UTF-8 character opens with — at each position
    /// a name run meets it: first, behind an ASCII run, behind a non-ASCII one (the cold
    /// arm's own loop), doubled, and followed by an ASCII name byte and by a terminator. Each
    /// source starts behind a prefix, so no scan begins at offset 0.
    #[test]
    fn scan_name_run_matches_the_class_for_every_scalar() {
        let mut checked = 0usize;
        for cp in 0..=0x10_ffff_u32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            for (prefix, body) in [
                ("<", format!("{c}")),
                ("<", format!("a{c}")),
                ("<", format!("ab-{c}x")),
                ("<", format!("é{c}")),
                ("<", format!("\u{e9}a{c}{c}b")),
                ("< ", format!("{c}=")),
                ("<x ", format!("on:a|{c}>")),
            ] {
                let source = format!("{prefix}{body}");
                let start = prefix.len();
                assert_eq!(
                    scan_from(&source, start),
                    name_run_end_model(&source, start),
                    "U+{cp:04X} in {source:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 7_000_000, "the scalar walk ran ({checked} cases)");
    }

    /// The characters the class distinguishes by more than one rule, named: U+FEFF (Svelte
    /// whitespace AND a custom-element name char — whitespace wins), NBSP and U+1680 (both
    /// whitespace, the latter inside a PCENChar range), LS / PS, NEL (not JS `\s`, yet outside
    /// this class too — the parser's longer name run is what carries it), a combining mark,
    /// ZWNJ / ZWJ, `·` and an astral emoji.
    #[test]
    fn scan_name_run_named_boundary_characters() {
        for (name, tail) in [
            ("div", "\u{feff}>"),
            ("div", "\u{a0}x"),
            ("div", "\u{1680}x"),
            ("div", "\u{2028}x"),
            ("div", "\u{2029}x"),
            ("div", "\u{85}x"),
            ("e\u{301}t\u{301}e", "="),
            ("a\u{200c}b\u{200d}c", "/"),
            ("a\u{b7}b", " "),
            ("my-\u{1f600}", ">"),
            ("x-caf\u{e9}", ""),
            ("on:click|once", "={f}"),
            ("A.b$c_d", "\"x\""),
            ("a", "\\b"),
            ("a", "\u{1}b"),
            ("a", "\"b"),
        ] {
            let source = format!("<{name}{tail}");
            assert_eq!(scan_from(&source, 1), 1 + name.len(), "{source:?}");
            assert_eq!(
                name_run_end_model(&source, 1),
                1 + name.len(),
                "model: {source:?}"
            );
        }
    }

    /// Every character boundary of `source`, its end included.
    fn boundaries(source: &str) -> Vec<usize> {
        (0..=source.len())
            .filter(|&i| source.is_char_boundary(i))
            .collect()
    }

    /// Text runs around the word hop's edges — every length from empty past two words — over
    /// backgrounds of one-, two-, three- and four-byte characters and of the ASCII bytes the
    /// lexer reads in template mode's neighbourhood (`}`, `>`, `-`, a newline), each run
    /// followed by nothing (end of input), by a `<` or by a `{`.
    fn text_run_sources() -> Vec<String> {
        let mut sources = Vec::new();
        for background in ["a", "é", "€", "\u{1f600}", "}", ">", "-", "\n", "\0"] {
            for len in 0..=18 {
                let run = background.repeat(len);
                for tail in ["", "<", "{", "<{", "{<", "}"] {
                    sources.push(format!("{run}{tail}"));
                    sources.push(format!("{run}{tail}{run}"));
                }
            }
        }
        sources
    }

    /// [`special_byte_at_or_after`] against a plain byte loop, from every character boundary
    /// of every text-run source ([`text_run_sources`]) — so every distance to the next `<` or
    /// `{`, and to the end of input, on both sides of each word boundary.
    #[test]
    fn special_byte_at_or_after_matches_a_scalar_scan() {
        fn scalar(bytes: &[u8], from: usize) -> usize {
            let mut i = from;
            while i < bytes.len() && !matches!(bytes[i], b'<' | b'{') {
                i += 1;
            }
            i
        }
        for source in text_run_sources() {
            let bytes = source.as_bytes();
            for from in boundaries(&source) {
                assert_eq!(
                    special_byte_at_or_after(bytes, from),
                    scalar(bytes, from),
                    "{source:?} from {from}"
                );
            }
        }
    }

    /// The template-mode token stream over text runs: through the front (which hops a
    /// non-empty run out of line and re-enters) and through the dispatch alone, each token
    /// starts at the first `<` or `{` the plain byte loop finds from the previous token's
    /// end, and the stream ends in the empty `Eof` at the end of input. The sources hold
    /// only text and `{`, so every token is a template-mode one.
    #[test]
    fn template_text_tokens_start_at_the_next_special_byte() {
        for source in text_run_sources() {
            if source.contains('<') {
                continue;
            }
            let bytes = source.as_bytes();
            let mut expected = Vec::new();
            let mut i = 0;
            loop {
                while i < bytes.len() && bytes[i] != b'{' {
                    i += 1;
                }
                if i == bytes.len() {
                    expected.push((TokenKind::Eof, i, i));
                    break;
                }
                expected.push((TokenKind::LeftBrace, i, i + 1));
                i += 1;
            }
            for front in [true, false] {
                let got: Vec<_> = lex_all(&source, front)
                    .into_iter()
                    .map(|step| {
                        let (kind, start, end, ..) = step.unwrap();
                        (kind, start as usize, end as usize)
                    })
                    .collect();
                assert_eq!(got, expected, "{source:?} (front: {front})");
            }
        }
    }

    /// [`html_comment_end`] against a plain search for `-->`, over comment bodies built from
    /// the fragments a `-`-hop could misread — lone and doubled dashes, a `->` and a `>`
    /// with no dashes before them, the closer itself, non-ASCII characters — padded to put
    /// each fragment on either side of a word boundary, from every character boundary.
    /// Then the lexer: `<!--` followed by each body is one Comment token ending one past the
    /// first `-->`, or the unterminated-comment error at the `<`, through the front and the
    /// dispatch alike.
    #[test]
    fn html_comment_end_matches_a_scalar_search() {
        fn scalar(bytes: &[u8], from: usize) -> Option<usize> {
            bytes[from..]
                .windows(3)
                .position(|w| w == b"-->")
                .map(|at| from + at + 3)
        }
        let fragments = [
            "",
            "-",
            "--",
            "->",
            ">",
            "-->",
            "--->",
            "a",
            "é",
            "<",
            "\u{1f600}",
        ];
        let mut bodies = Vec::new();
        for pad in [0, 1, 5, 6, 7, 8, 9, 13, 14, 15, 16, 17] {
            for padding in ["a", "é", "-"] {
                let lead = padding.repeat(pad);
                for a in fragments {
                    for b in fragments {
                        for c in ["", "-", "-->", "a"] {
                            bodies.push(format!("{lead}{a}{b}{c}"));
                        }
                    }
                }
            }
        }
        for body in &bodies {
            let bytes = body.as_bytes();
            for from in boundaries(body) {
                assert_eq!(
                    html_comment_end(bytes, from),
                    scalar(bytes, from),
                    "{body:?} from {from}"
                );
            }
            for prefix in ["", "x", "<p>"] {
                let source = format!("{prefix}<!--{body}");
                let start = prefix.len();
                let expected = scalar(source.as_bytes(), start + 4)
                    .ok_or_else(|| lex_err("Unterminated HTML comment", start));
                for front in [true, false] {
                    let steps = lex_all(&source, front);
                    let got = steps
                        .iter()
                        .find_map(|step| match step {
                            Ok((TokenKind::Comment, s, e, ..)) => {
                                assert_eq!(*s as usize, start, "{source:?}");
                                Some(Ok(*e as usize))
                            }
                            Err(err) => Some(Err(err.clone())),
                            Ok(_) => None,
                        })
                        .expect("a comment or an error");
                    assert_eq!(got, expected, "{source:?} (front: {front})");
                }
            }
        }
    }
}
