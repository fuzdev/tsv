// Internal AST - CSS-specific types optimized for traversal and manipulation

use tsv_lang::Span;

/// CSS AST node types
#[derive(Debug, Clone)]
pub enum CssNode {
    Rule(CssRule),
    // TODO: Add more node types as needed (AtRule, Comment, etc.)
}

impl CssNode {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
        }
    }
}

/// CSS Rule - selector with declaration block
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: String, // TODO: Parse selector structure (SelectorList, ComplexSelector, etc.)
    pub selector_span: Span, // Span of just the selector
    pub block_span: Span, // Span of the block including braces
    pub declarations: Vec<CssDeclaration>,
    pub span: Span, // Full rule span
}

/// CSS Declaration - property: value pair
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
    pub span: Span,
}
