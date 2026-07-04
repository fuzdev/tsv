// TypeScript/JS lexer
//
// Tokenizes TypeScript source code into a stream of tokens.
// Supports TypeScript-specific syntax like type annotations.

mod comments;
mod core;
pub mod escapes;
pub(crate) mod ident;
mod token;

// Shared lexer-error constructor: `core` / `comments` reach it via `super::lex_err`.
use tsv_lang::lex_err;

// Re-export public API
pub use core::Lexer;
pub use token::{KeywordKind, Token, TokenKind};
