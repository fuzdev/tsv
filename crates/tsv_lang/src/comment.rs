// Shared comment type used across languages
use crate::Span;

#[derive(Debug, Clone)]
pub struct Comment {
    pub content: String,
    pub is_block: bool, // true for /* */ or <!-- -->, false for //
    pub span: Span,
}
