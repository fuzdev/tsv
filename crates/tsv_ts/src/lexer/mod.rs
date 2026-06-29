// TypeScript/JS lexer
//
// Tokenizes TypeScript source code into a stream of tokens.
// Supports TypeScript-specific syntax like type annotations.

mod comments;
mod core;
pub mod escapes;
pub(crate) mod ident;
mod token;

use tsv_lang::ParseError;

// Re-export public API
pub use core::Lexer;
pub use token::{KeywordKind, Token, TokenKind};

/// Construct a boxed lexer error. The lexer returns `Result<_, Box<ParseError>>`
/// (see `From<Box<ParseError>>` in `tsv_lang`): boxing keeps the hot `next_token`
/// Ok path pointer-sized. `#[cold]` / `#[inline(never)]` outlines the error
/// construction so it never bloats the inlined token-scan fast path. Shared by the
/// `core` and `comments` scanners (both descendant modules reach it via `super`).
#[cold]
#[inline(never)]
#[allow(clippy::unnecessary_box_returns)] // the box is the point — keeps the hot Result pointer-sized
fn lex_err(message: impl Into<String>, position: usize) -> Box<ParseError> {
    Box::new(ParseError::InvalidSyntax {
        message: message.into(),
        position,
        context: None,
    })
}
