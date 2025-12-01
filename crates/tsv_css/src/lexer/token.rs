#[derive(Debug, Clone, PartialEq, Eq)]
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
    Dollar,           // $ (for attribute selectors: $=)
    Pipe,             // | (for attribute selectors: |=, namespace selectors)
    ColumnCombinator, // || (CSS Grid column combinator)
    Bang,             // ! (for !important)

    // Values - composite tokens per CSS Syntax Level 3
    String {
        content: String, // Raw content with escapes preserved: hel\"lo
        quote: char,     // ' or "
    },
    Number(String),            // 123, 1.5, .5, 007 - preserve source representation
    Percentage(String),        // 50% - preserve source representation
    Dimension(String, String), // 16px, 1.5em - preserve source representation (value, unit)

    // Comments - preserved for printers
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
    /// Decoded value for tokens that require escape sequence processing
    /// - For Identifier: decoded CSS identifier (escapes resolved)
    /// - For other tokens: None
    pub decoded: Option<String>,
}
