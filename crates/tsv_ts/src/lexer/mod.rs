// TypeScript/JS lexer
//
// Tokenizes TypeScript source code into a stream of tokens.
// Supports TypeScript-specific syntax like type annotations.

mod comments;
mod core;
pub mod escapes;
pub mod ident;
mod token;

// Re-export public API
pub(crate) use core::{
    ES_LINE_TERMINATOR_LEADS, is_es_line_terminator, is_es_line_terminator_at, is_es_whitespace,
    unicode_escape_len_at,
};
pub use core::{Lexer, LexerCheckpoint};
pub(crate) use token::keyword_at;
pub use token::{KeywordKind, Token, TokenKind};
