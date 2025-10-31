// Parser module - coordinates parsing logic

mod css;
mod svelte;
mod typescript;

pub use css::parse_css;
pub use svelte::parse_svelte;
pub use typescript::parse_typescript;

// Shared peek token data (not storing Token<'a> to avoid lifetime issues)
// Used by both TypeScript and Svelte parsers for single-token lookahead
// Generic over TokenKind type since each parser has its own TokenKind enum
pub(crate) struct PeekData<K> {
    pub(super) kind: K,
    pub(super) start: usize,
    pub(super) end: usize,
}
