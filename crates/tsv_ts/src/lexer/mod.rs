// TypeScript/JS lexer
//
// Tokenizes TypeScript source code into a stream of tokens.
// Supports TypeScript-specific syntax like type annotations.

mod comments;
mod core;
pub mod escapes;
pub mod ident;
mod token;

// Shared lexer-error constructor: `core` / `comments` reach it via `super::lex_err`.
use tsv_lang::lex_err;

// Re-export public API
pub use core::Lexer;
pub(crate) use core::{
    is_es_line_terminator, is_es_line_terminator_at, is_es_whitespace, unicode_escape_len_at,
};
pub use token::{KeywordKind, Token, TokenKind};
