use std::fmt;
use std::str::Chars;
// Shared lexer-error constructor: used by the unterminated/unexpected sites in `next_token`.
use tsv_lang::{ParseError, lex_err, source_scan};

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
    chars: Chars<'a>,
    position: usize,
    current: Option<char>,
    pub inside_tag: bool,    // Track if we're inside <...>
    initial_position: usize, // Position after BOM skip (0 or 3)
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut chars = source.chars();
        let mut current = chars.next();
        let mut position = 0;

        // Skip UTF-8 BOM (U+FEFF) at start of file if present.
        // BOM is a legacy artifact; we strip it (like deno fmt, VS Code).
        // Position starts after BOM so token spans reflect actual file bytes.
        if current == Some('\u{feff}') {
            position = '\u{feff}'.len_utf8();
            current = chars.next();
        }

        Self {
            source,
            chars,
            position,
            current,
            inside_tag: false,
            initial_position: position,
        }
    }

    /// Returns the initial position after BOM skip (0 if no BOM, 3 if BOM was skipped).
    /// Used by parser to initialize gap tracking.
    pub fn initial_position(&self) -> usize {
        self.initial_position
    }

    #[inline]
    fn advance(&mut self) {
        if let Some(ch) = self.current {
            self.position += ch.len_utf8();
            self.current = self.chars.next();
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
        while let Some(ch) = self.current {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Move the cursor to byte offset `pos`, which must be a char boundary at or after the
    /// current position. Lets a scan delegate a span to a byte-level helper
    /// (`tsv_lang::source_scan`) and resume lexing just past it, instead of re-walking the
    /// span char by char through `advance`.
    #[inline]
    fn seek_to(&mut self, pos: usize) {
        debug_assert!(pos >= self.position && self.source.is_char_boundary(pos));
        self.chars = self.source[pos..].chars();
        self.current = self.chars.next();
        self.position = pos;
    }

    /// Byte offset of the first non-whitespace char at or after the cursor, without
    /// consuming input. Whitespace matches `skip_whitespace` (`char::is_whitespace`),
    /// so a follow-up `skip_whitespace()` lands exactly here.
    #[inline]
    fn peek_past_whitespace(&self) -> usize {
        let mut pos = self.position;
        for ch in self.source[self.position..].chars() {
            if ch.is_whitespace() {
                pos += ch.len_utf8();
            } else {
                break;
            }
        }
        pos
    }

    /// Skip everything until we hit a special character (<, {)
    /// Used in template mode to treat text content as gaps
    /// Note: '}' is NOT special in template mode - it's only consumed directly
    /// during expression tag parsing. This allows '}' in text (e.g., after {'{'}text})
    /// to be treated as plain text, matching Svelte's parser behavior.
    fn skip_to_special_char(&mut self) {
        while let Some(ch) = self.current {
            match ch {
                '<' | '{' => break,
                _ => self.advance(),
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, Box<ParseError>> {
        // Template mode (outside tags): skip text content, only tokenize special chars
        // Tag mode (inside <...>): tokenize everything including identifiers
        if self.inside_tag {
            self.skip_whitespace();
        } else {
            self.skip_to_special_char();
        }

        let start = self.position;

        match self.current {
            None => Ok(Token {
                kind: TokenKind::Eof,
                start: start as u32,
                end: start as u32,
            }),
            Some('<') => {
                // Check for HTML comment: <!--
                if self.starts_with(b"<!--") {
                    // Consume "<!--"
                    self.advance(); // <
                    self.advance(); // !
                    self.advance(); // -
                    self.advance(); // -

                    // Scan until "-->"
                    while self.current.is_some() {
                        if self.starts_with(b"-->") {
                            // Consume "-->"
                            self.advance();
                            self.advance();
                            self.advance();
                            return Ok(self.make_token(TokenKind::Comment, start));
                        }
                        self.advance();
                    }

                    // Unterminated comment
                    return Err(lex_err("Unterminated HTML comment", start));
                }

                self.inside_tag = true; // Enter tag mode
                self.advance();
                Ok(self.make_token(TokenKind::LeftAngle, start))
            }
            Some('>') => {
                self.inside_tag = false; // Exit tag mode, back to template mode
                self.advance();
                Ok(self.make_token(TokenKind::RightAngle, start))
            }
            Some('/') => {
                self.advance();
                Ok(self.make_token(TokenKind::Slash, start))
            }
            Some('{') => {
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
            Some('}') => {
                self.advance();
                Ok(self.make_token(TokenKind::RightBrace, start))
            }
            Some('=') => {
                self.advance();
                Ok(self.make_token(TokenKind::Equals, start))
            }
            Some(quote @ '\'' | quote @ '"') => {
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
                // `tsv_debug scan_audit`): this scan used to track braces and strings but
                // not comments or regex, so `title="{/* ` */ b}"` and `title="{f(/"/)}"`
                // desynced it and it ran to EOF — an over-rejection of Svelte-valid input.
                //
                // `parse_attribute_value` (attribute.rs) re-walks the same value to split
                // it into Text and ExpressionTag parts, and reaches the same answer the
                // same way (via `parse_expression_tag_at`); this is the tokenizing half.
                self.advance(); // consume opening quote

                while let Some(ch) = self.current {
                    if ch == quote {
                        self.advance(); // consume closing quote
                        return Ok(self.make_token(TokenKind::String, start));
                    }
                    if ch == '{' {
                        let Some(close) = source_scan::scan_to_matching_brace(
                            self.source.as_bytes(),
                            self.position + 1,
                            self.source.len(),
                        ) else {
                            break; // unterminated `{` — the value can't close
                        };
                        self.seek_to(close + '}'.len_utf8());
                        continue;
                    }
                    // Attribute-value text. HTML/Svelte attribute values have NO backslash
                    // escapes (unlike a JS string inside `{expr}`, skipped above), so `\`
                    // is a literal char: `a="{x}\"` closes at the `"` with value `{x}\`,
                    // matching Svelte's parser. Treating `\` as an escape here read `\"` as
                    // an escaped quote and ran past the close → "Unterminated string
                    // literal" (an over-rejection of valid Svelte; the `fuzz` gate).
                    self.advance();
                }
                // Unterminated string
                Err(lex_err("Unterminated string literal in template", start))
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '-' || ch == '!' => {
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
                // Advance past first char — ! is a valid start but not a continuation char
                self.advance();
                while let Some(ch) = self.current {
                    // `is_alphanumeric` already covers Unicode *letters* (so `<my-café>`
                    // works); `is_pcen_char` adds the non-alphanumeric members of the HTML
                    // custom-element name grammar (`·`, ZWNJ/ZWJ, astral emoji) so a whole
                    // custom-element name stays in one token. It sits last: ASCII
                    // short-circuits on `is_alphanumeric` before reaching it. Over-admitting
                    // (e.g. a PCENChar with no preceding hyphen) is harmless — the parser's
                    // `is_valid_tag_name` gate rejects any name that isn't valid.
                    if ch.is_alphanumeric()
                        || ch == '_'
                        || ch == '$'
                        || ch == '-'
                        || ch == ':'
                        || ch == '|'
                        || ch == '.'
                        || tsv_html::is_pcen_char(ch)
                    {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Ok(self.make_token(TokenKind::Identifier, start))
            }
            Some(ch) if ch.is_ascii_digit() => {
                // Unquoted numeric attribute values (e.g., data-count=123)
                // HTML allows unquoted values that are alphanumeric
                while let Some(ch) = self.current {
                    if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                        self.advance();
                    } else {
                        break;
                    }
                }
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
