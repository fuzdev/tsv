// Internal AST - CSS-specific types optimized for traversal and manipulation
//
// ARCHITECTURE: This is our internal AST representation, optimized for traversal,
// manipulation, and formatting. It gets converted to public AST for JSON serialization.
//
// POC Status: Intentionally minimal to validate multi-language architecture.
// Selectors stored as strings, not parsed structure - this was the RIGHT decision
// for POC to avoid getting bogged down in CSS complexity.
//
// See CSS_SPEC.md for full expansion roadmap.

use tsv_lang::Span;

/// CSS AST node types
#[derive(Debug, Clone)]
pub enum CssNode {
    Rule(CssRule),
    Comment(CssComment),
    // TODO: Phase 3 - Add AtRule variant (see CSS_SPEC.md "Phase 3: At-Rules")
    // AtRule(AtRule),
}

impl CssNode {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
            CssNode::Comment(comment) => comment.span,
        }
    }
}

/// CSS Rule - selector with declaration block
#[derive(Debug, Clone)]
pub struct CssRule {
    // TODO: Phase 2 - Replace String with parsed selector AST (see CSS_SPEC.md "Phase 2: Selectors")
    //
    // Current: pub selector: String,
    // Future:  pub selector: SelectorList,
    //
    // Target structure (matching Svelte's AST from SVELTE_CSS_PARSING.md):
    //
    // pub struct SelectorList {
    //     pub selectors: Vec<ComplexSelector>,
    //     pub span: Span,
    // }
    //
    // pub struct ComplexSelector {
    //     pub children: Vec<RelativeSelector>,
    //     pub span: Span,
    // }
    //
    // pub struct RelativeSelector {
    //     pub combinator: Option<Combinator>,  // None, Descendant, Child, NextSibling, etc.
    //     pub selectors: Vec<SimpleSelector>,
    //     pub span: Span,
    // }
    //
    // pub enum SimpleSelector {
    //     Type { namespace: Option<String>, name: String, span: Span },
    //     Universal { namespace: Option<String>, span: Span },
    //     Class { name: String, span: Span },
    //     Id { name: String, span: Span },
    //     Attribute { name: String, matcher: Option<AttrMatcher>, value: Option<String>, flags: Option<String>, span: Span },
    //     PseudoClass { name: String, args: Option<Vec<SelectorList>>, span: Span },
    //     PseudoElement { name: String, args: Option<Vec<SimpleSelector>>, span: Span },
    //     Nesting { span: Span },  // & selector
    // }
    //
    // This structure enables:
    //   - Specificity calculation
    //   - Selector matching (for Svelte's unused CSS detection)
    //   - CSS scoping (adding .svelte-{hash} classes)
    //   - Proper formatting with correct precedence
    //
    // Implementation priority: P1 (after Phase 1 tokenization)
    pub selector: String,

    pub selector_span: Span, // Span of just the selector
    pub block_span: Span,    // Span of the block including braces
    pub declarations: Vec<CssDeclaration>,
    pub span: Span, // Full rule span
}

/// CSS Declaration - property: value pair
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    pub property: String,

    // TODO: Phase 4 - Replace String with parsed value AST (see CSS_SPEC.md "Phase 4: Values and Functions")
    //
    // Current: pub value: String,
    // Future:  pub value: CssValue,
    //
    // Target structure:
    //
    // pub enum CssValue {
    //     Identifier(String),
    //     String(String),
    //     Number { value: f64, unit: Option<String> },  // 10px, 1.5em, 50%
    //     Percentage(f64),
    //     Color(Color),  // rgb(), hsl(), hex, named
    //     Function { name: String, args: Vec<CssValue> },  // calc(), var(), etc.
    //     List(Vec<CssValue>),              // Space-separated
    //     CommaSeparated(Vec<CssValue>),
    // }
    //
    // This structure enables:
    //   - Value validation (type checking)
    //   - Value transformation (calc evaluation, var substitution)
    //   - Minification (removing unnecessary spaces)
    //   - Pretty-printing with correct precedence
    //
    // Implementation priority: P2 (after selectors and at-rules)
    pub value: String,

    pub span: Span,
}

/// CSS Comment - /* ... */
#[derive(Debug, Clone)]
pub struct CssComment {
    /// Comment content without /* */ delimiters
    pub content: String,
    pub span: Span,
}
