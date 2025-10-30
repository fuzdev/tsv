// Lexer module - contains TypeScript, Svelte, and CSS lexers

pub mod typescript;
pub mod svelte;
pub mod css;

// Re-export TypeScript lexer types for backward compatibility
pub use typescript::{KeywordKind, Lexer, TokenKind};
