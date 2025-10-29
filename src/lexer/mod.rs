// Lexer module - contains TypeScript and Svelte lexers

pub mod typescript;
pub mod svelte;

// Re-export TypeScript lexer types for backward compatibility
pub use typescript::{KeywordKind, Lexer, TokenKind};
