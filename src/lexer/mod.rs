// Lexer module - contains TypeScript, Svelte, and CSS lexers

pub mod css;
pub mod svelte;
pub mod typescript;

// Re-export TypeScript lexer types for backward compatibility
pub use typescript::{KeywordKind, Lexer, TokenKind};
