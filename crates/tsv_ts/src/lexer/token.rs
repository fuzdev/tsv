// Token types for TypeScript/JavaScript lexer

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeywordKind {
    Const,
    Let,
    Var,
    Number,
    // TODO: String, Boolean, etc.
}

impl fmt::Display for KeywordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeywordKind::Const => write!(f, "const"),
            KeywordKind::Let => write!(f, "let"),
            KeywordKind::Var => write!(f, "var"),
            KeywordKind::Number => write!(f, "number"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number,
    String,
    Identifier,
    Keyword(KeywordKind),
    Equals,
    Colon,
    Semicolon,
    Comma,
    BraceOpen,   // {
    BraceClose,  // }
    Comment { content: String, is_block: bool },
    Eof,
}

// TODO: Consider refining Display implementation for better error messages
// Current approach: Quoted tokens like '=', lowercase for others
// Alternative: Could match TypeScript/JavaScript terminology more closely
// Examples:
// - "identifier token" instead of "identifier"
// - "number literal" instead of "number"
// - "string literal" instead of "string"
// Trade-off: Current is concise, alternative is more descriptive
// Usage in errors: "Expected property key, found {token_kind}"
impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Number => write!(f, "number"),
            TokenKind::String => write!(f, "string"),
            TokenKind::Identifier => write!(f, "identifier"),
            TokenKind::Keyword(kw) => write!(f, "'{}'", kw),
            TokenKind::Equals => write!(f, "'='"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Semicolon => write!(f, "';'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::BraceOpen => write!(f, "'{{'"),
            TokenKind::BraceClose => write!(f, "'}}'"),
            TokenKind::Comment { is_block, .. } => {
                if *is_block {
                    write!(f, "block comment")
                } else {
                    write!(f, "line comment")
                }
            }
            TokenKind::Eof => write!(f, "end of file"),
        }
    }
}

// Token design with escape handling
// - `decoded`: owned string for escape-processed values (only allocated when needed)
// - Raw text: extracted via source[start..end] on demand (zero duplication)
//
// TODO: REMOVE `raw` FIELD - Code smell / redundant duplication
// We consistently use source[start..end] everywhere for raw text.
// The `raw` field duplicates data and violates our architecture principle:
// "Raw strings are NEVER duplicated in the AST" (see CLAUDE.md).
// This applies to tokens too - tokens are just pre-AST.
//
// Removal steps:
// 1. Remove `raw` field and lifetime parameter from Token struct
// 2. Update all token construction sites to not pass raw
// 3. Update parser to use source[token.start..token.end] if needed
// 4. Verify no performance regression (source slicing is already what we do)
#[derive(Debug, Clone)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
    /// Raw token value (borrowed from source)
    ///
    /// TODO: REMOVE THIS - redundant with start/end + source. See comment above.
    #[allow(dead_code)]
    pub raw: &'a str,
    /// Decoded value (for strings with escape sequences)
    /// None for non-string tokens or strings without escapes
    pub decoded: Option<String>,
}

// TODO: Expand keyword list for:
// - Type keywords: interface, type, enum, namespace, etc.
// - Control flow: if, else, while, for, switch, case, break, continue, return
// - Other: function, class, import, export, async, await, etc.
pub fn keyword_kind(s: &str) -> Option<KeywordKind> {
    match s {
        "const" => Some(KeywordKind::Const),
        "let" => Some(KeywordKind::Let),
        "var" => Some(KeywordKind::Var),
        "number" => Some(KeywordKind::Number),
        _ => None,
    }
}
