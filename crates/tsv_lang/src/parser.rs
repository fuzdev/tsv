// Parser utilities shared across all language parsers

/// Cached lookahead data for parsers
///
/// This struct is used by all language parsers (CSS, TypeScript, Svelte) to cache
/// the next token during lookahead operations. The `peek()` methods populate this
/// struct to avoid re-lexing the same token multiple times.
///
/// # Type Parameters
///
/// * `K` - The token kind type (e.g., `TokenKind` enum for each language)
///
/// # Fields
///
/// * `kind` - The token kind
/// * `start` - Byte offset where the token starts in the source
/// * `end` - Byte offset where the token ends in the source (exclusive)
/// * `decoded` - Optional decoded value for tokens with escape sequences
///   - Used by TypeScript parser for strings/identifiers with escapes
///   - Always `None` for CSS and Svelte parsers (not needed for their peek logic)
///
/// # Examples
///
/// ```rust,ignore
/// // CSS/Svelte usage (no decoded value)
/// let peek = PeekData {
///     kind: TokenKind::Identifier,
///     start: 0,
///     end: 5,
///     decoded: None,
/// };
///
/// // TypeScript usage (with decoded value for escape handling)
/// let peek = PeekData {
///     kind: TokenKind::StringLiteral,
///     start: 0,
///     end: 10,
///     decoded: Some("hello\n".to_string()),  // Escaped \n decoded
/// };
/// ```
#[derive(Debug)]
pub struct PeekData<K> {
    pub kind: K,
    pub start: usize,
    pub end: usize,
    pub decoded: Option<String>,
}

impl<K> PeekData<K> {
    /// Create a new PeekData without a decoded value
    ///
    /// This is the common case for CSS and Svelte parsers.
    pub fn new(kind: K, start: usize, end: usize) -> Self {
        Self {
            kind,
            start,
            end,
            decoded: None,
        }
    }

    /// Create a new PeekData with a decoded value
    ///
    /// This is used by the TypeScript parser for tokens with escape sequences.
    pub fn with_decoded(kind: K, start: usize, end: usize, decoded: Option<String>) -> Self {
        Self {
            kind,
            start,
            end,
            decoded,
        }
    }
}
