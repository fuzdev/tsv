#[derive(Debug, Clone, PartialEq)]
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
    Colon,       // :
    Semicolon,   // ;
    Comma,       // ,
    Dot,         // .
    Hash,        // #
    GreaterThan, // >
    Plus,        // +
    Tilde,       // ~
    Asterisk,    // *
    Ampersand,   // &
    AtSign,      // @
    Slash,       // / (division operator)
    Equals,      // =

    // Values - composite tokens per CSS Syntax Level 3
    String {
        content: String, // Raw content with escapes preserved: hel\"lo
        quote: char,     // ' or "
    },
    Number(String),         // 123, 1.5, .5, 007 - preserve source representation
    Percentage(String),     // 50% - preserve source representation
    Dimension(String, String), // 16px, 1.5em - preserve source representation (value, unit)

    // Comments - preserved for formatters
    Comment(String), // /* ... */ - content without delimiters

    // Whitespace
    Whitespace, // spaces, tabs, newlines

    Eof,
}

#[derive(Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}
