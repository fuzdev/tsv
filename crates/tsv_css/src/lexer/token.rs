use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Identifiers and keywords
    Identifier, // div, color, red, etc.

    // Braces and delimiters
    LeftBrace,    // {
    RightBrace,   // }
    LeftBracket,  // [
    RightBracket, // ]
    LeftParen,    // (
    RightParen,   // )

    // Punctuation
    Colon,            // :
    Semicolon,        // ;
    Comma,            // ,
    Dot,              // .
    Hash,             // #
    GreaterThan,      // >
    LessThan,         // <
    Plus,             // +
    Tilde,            // ~
    Asterisk,         // *
    Ampersand,        // &
    AtSign,           // @
    Slash,            // / (division operator)
    Equals,           // =
    Percent,          // % (for percent-encoding in URLs like %20)
    Caret,            // ^ (for attribute selectors: ^=)
    Question,         // ? (for query strings in unquoted url(), e.g. url(a.ttf?x=1))
    Dollar,           // $ (for attribute selectors: $=)
    Pipe,             // | (for attribute selectors: |=, namespace selectors)
    ColumnCombinator, // || (CSS Grid column combinator)
    Bang,             // ! (for !important)

    // Values - no String allocations, extract from source via start/end positions
    String { quote: char },     // content: source[start+1..end-1]
    Number,                     // value: source[start..end]
    Percentage,                 // value: source[start..end-1] (excludes %)
    Dimension { unit_len: u8 }, // value: source[start..end-unit_len], unit: source[end-unit_len..end]
    // Opaque `<url-token>`: an *unquoted* `url(...)` spanning `url(` … `)` verbatim
    // (css-syntax §4.3.6). Consumed whole by the lexer so `/*`, `:`, etc. inside are
    // literal content, not a comment / colon. Quoted `url("…")` stays an identifier +
    // `(` + string (a function-token). value: source[start..end].
    Url,

    // Comments - content: source[start+2..end-2] (excludes /* */)
    Comment,

    // Whitespace
    Whitespace, // spaces, tabs, newlines

    Eof,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::LeftBrace => write!(f, "'{{'"),
            TokenKind::RightBrace => write!(f, "'}}'"),
            TokenKind::LeftBracket => write!(f, "'['"),
            TokenKind::RightBracket => write!(f, "']'"),
            TokenKind::LeftParen => write!(f, "'('"),
            TokenKind::RightParen => write!(f, "')'"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::Dot => write!(f, "'.'"),
            TokenKind::Hash => write!(f, "'#'"),
            TokenKind::GreaterThan => write!(f, "'>'"),
            TokenKind::LessThan => write!(f, "'<'"),
            TokenKind::Plus => write!(f, "'+'"),
            TokenKind::Tilde => write!(f, "'~'"),
            TokenKind::Asterisk => write!(f, "'*'"),
            TokenKind::Ampersand => write!(f, "'&'"),
            TokenKind::AtSign => write!(f, "'@'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::Percent => write!(f, "'%'"),
            TokenKind::Caret => write!(f, "'^'"),
            TokenKind::Question => write!(f, "'?'"),
            TokenKind::Dollar => write!(f, "'$'"),
            TokenKind::Pipe => write!(f, "'|'"),
            TokenKind::ColumnCombinator => write!(f, "'||'"),
            TokenKind::Bang => write!(f, "'!'"),
            TokenKind::String { .. } => write!(f, "string"),
            TokenKind::Number => write!(f, "number"),
            TokenKind::Percentage => write!(f, "percentage"),
            TokenKind::Dimension { .. } => write!(f, "dimension"),
            TokenKind::Url => write!(f, "url"),
            TokenKind::Comment => write!(f, "comment"),
            TokenKind::Whitespace => write!(f, "whitespace"),
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

/// A lexed CSS token: a 16-byte POD (`kind` + two `u32` spans) returned by value
/// from `next_token`. The decoded value of an escaped identifier is kept
/// **out-of-band** on the `Lexer` (`take_decoded`) rather than inline here, so the
/// common no-escape identifier costs no allocation and the hot token does not carry
/// a `String`. `Clone` (not `Copy`) mirrors `tsv_ts::Token`.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub start: u32,
    pub end: u32,
}

// The 16-byte size is load-bearing: it keeps `next_token`'s by-value return in
// registers. `TokenKind`'s widest payload is `String { quote: char }` (align 4),
// so `kind` is 8 bytes + two `u32` spans = 16.
const _: () = assert!(size_of::<Token>() == 16);
