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
    Atrule(CssAtrule), // Phase 3: @media, @keyframes, @supports, etc.
}

impl CssNode {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
            CssNode::Comment(comment) => comment.span,
            CssNode::Atrule(atrule) => atrule.span,
        }
    }
}

/// CSS Rule - selector with declaration block
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: SelectorList,
    pub block_span: Span, // Span of the block including braces
    pub declarations: Vec<CssDeclaration>,
    pub span: Span, // Full rule span
}

// ============================================================================
// Selector AST (Phase 2)
// ============================================================================
//
// Implements Selectors Level 4 specification:
// https://drafts.csswg.org/selectors-4/
//
// Structure:
//   SelectorList → ComplexSelector → RelativeSelector → SimpleSelector
//
// This enables:
//   - Specificity calculation
//   - Selector matching (for Svelte's unused CSS detection)
//   - CSS scoping (adding .svelte-{hash} classes)
//   - Proper formatting with correct precedence

/// Selector list - comma-separated selectors
#[derive(Debug, Clone)]
pub struct SelectorList {
    pub selectors: Vec<ComplexSelector>,
    pub span: Span,
}

/// Complex selector - one or more relative selectors connected by combinators
#[derive(Debug, Clone)]
pub struct ComplexSelector {
    pub children: Vec<RelativeSelector>,
    pub span: Span,
}

/// Relative selector - combinator + simple selectors
#[derive(Debug, Clone)]
pub struct RelativeSelector {
    pub combinator: Option<Combinator>,
    pub selectors: Vec<SimpleSelector>,
    pub span: Span,
}

/// Combinator between selectors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,        // space (ancestor-descendant)
    Child,             // > (parent-child)
    NextSibling,       // + (adjacent sibling)
    SubsequentSibling, // ~ (general sibling)
    Column,            // || (column combinator)
}

/// Simple selector - the atomic units that make up a complex selector
#[derive(Debug, Clone)]
pub enum SimpleSelector {
    Type {
        namespace: Option<String>,
        name: String,
        span: Span,
    },
    Universal {
        namespace: Option<String>,
        span: Span,
    },
    Class {
        name: String,
        span: Span,
    },
    Id {
        name: String,
        span: Span,
    },
    Attribute {
        namespace: Option<String>,
        name: String,
        matcher: Option<AttributeMatcher>,
        value: Option<String>,
        flags: Option<String>, // i (case-insensitive), s (case-sensitive)
        span: Span,
    },
    PseudoClass {
        name: String,
        raw_args: Option<String>, // TODO: Parse into Vec<SelectorList> for :is(), :not(), etc.
        span: Span,
    },
    PseudoElement {
        name: String,
        raw_args: Option<String>, // TODO: Parse into Vec<SimpleSelector> for ::slotted(), etc.
        span: Span,
    },
    Nesting {
        span: Span, // & selector (CSS Nesting spec)
    },
    Percentage {
        value: f64,
        span: Span, // Phase 3b: @keyframes percentage selectors (0%, 50%, 100%)
    },
}

/// Attribute selector matcher type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeMatcher {
    Exact,     // [attr="value"] - exact match
    Contains,  // [attr~="value"] - whitespace-separated list contains value
    DashMatch, // [attr|="value"] - exact or starts with value followed by -
    Prefix,    // [attr^="value"] - starts with
    Suffix,    // [attr$="value"] - ends with
    Substring, // [attr*="value"] - contains substring
}

/// CSS Declaration - property: value pair
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    pub property: String,
    pub value: CssValue,
    pub span: Span,
}

// ============================================================================
// CSS Value AST (Phase 4)
// ============================================================================
//
// Implements CSS Values and Units Level 4 specification:
// https://drafts.csswg.org/css-values-4/
//
// Structure enables:
//   - Value validation (type checking)
//   - Value transformation (calc evaluation, var substitution)
//   - Minification (removing unnecessary spaces)
//   - Pretty-printing with correct precedence

/// CSS value - right-hand side of a declaration
#[derive(Debug, Clone, serde::Serialize)]
pub enum CssValue {
    /// Identifier: auto, bold, inherit, currentColor, etc.
    Identifier(String),

    /// String literal: "Arial", 'font.woff'
    String(String),

    /// Number with optional unit: 10, 10px, 1.5em, 50%, etc.
    Dimension {
        value: f64,
        unit: String, // empty string for unitless numbers, "px", "%", etc.
        source: String, // original source representation (preserves leading zeros)
    },

    /// Color - various formats (rgb, hsl, hex, named)
    Color(Color),

    /// Function call: calc(), var(), rgb(), url(), etc.
    Function { name: String, args: Vec<CssValue> },

    /// Space-separated list of values
    List { values: Vec<CssValue> },

    /// Comma-separated list of values
    CommaSeparated { values: Vec<CssValue> },
}

/// CSS color value
#[derive(Debug, Clone, serde::Serialize)]
pub enum Color {
    /// Named color: red, blue, currentColor, etc.
    Named(String),

    /// Hex color: #ff0000, #f00, etc.
    Hex(String),

    /// RGB color: rgb(255, 0, 0) or rgb(255 0 0 / 1)
    Rgb {
        r: u8,
        g: u8,
        b: u8,
        alpha: Option<f64>,
    },

    /// HSL color: hsl(0, 100%, 50%)
    Hsl {
        hue: f64,
        saturation: f64,
        lightness: f64,
        alpha: Option<f64>,
    },
}

/// CSS Comment - /* ... */
#[derive(Debug, Clone)]
pub struct CssComment {
    /// Comment content without /* */ delimiters
    pub content: String,
    pub span: Span,
}

// ============================================================================
// At-Rule AST (Phase 3)
// ============================================================================
//
// Implements CSS Syntax Module Level 3 at-rules:
// https://drafts.csswg.org/css-syntax-3/#at-rules
//
// Examples:
//   @media screen and (min-width: 768px) { ... }
//   @keyframes slide { ... }
//   @supports (display: grid) { ... }
//   @font-face { ... }
//   @import url('styles.css');
//
// Prelude is kept as raw string (not parsed) - matches Svelte's approach.
// Block content varies by at-rule type:
//   - Conditional at-rules (@media, @supports, @layer): Contains rules
//   - Descriptor at-rules (@font-face, @page): Contains declarations
//   - Statement at-rules (@import, @charset): No block

/// At-rule (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
#[derive(Debug, Clone)]
pub struct CssAtrule {
    /// At-rule name without @ (e.g., "media", "keyframes")
    pub name: String,

    /// Raw unparsed prelude string
    /// Examples: "screen and (min-width: 768px)", "slide", "url('file.css')"
    /// Deferred to Phase 4 for structured parsing
    pub prelude: String,

    /// Block contents (Some for conditional/descriptor, None for statement at-rules)
    pub block: Option<CssAtruleBlock>,

    pub span: Span,
}

/// At-rule block - can contain rules, declarations, or nested at-rules
#[derive(Debug, Clone)]
pub struct CssAtruleBlock {
    /// Block children - mixture depends on at-rule type
    ///
    /// @media, @supports, @layer: Vec<CssNode> (rules + nested at-rules)
    /// @font-face, @page: Vec<CssDeclaration> (declarations only)
    ///
    /// We store as Vec<CssBlockChild> to support both:
    pub children: Vec<CssBlockChild>,
    pub span: Span,
}

/// At-rule block child - can be rule, declaration, or nested at-rule
#[derive(Debug, Clone)]
pub enum CssBlockChild {
    Rule(CssRule),
    Declaration(CssDeclaration),
    Atrule(CssAtrule),
    Comment(CssComment),
}

impl CssBlockChild {
    pub fn span(&self) -> Span {
        match self {
            CssBlockChild::Rule(rule) => rule.span,
            CssBlockChild::Declaration(decl) => decl.span,
            CssBlockChild::Atrule(atrule) => atrule.span,
            CssBlockChild::Comment(comment) => comment.span,
        }
    }
}
