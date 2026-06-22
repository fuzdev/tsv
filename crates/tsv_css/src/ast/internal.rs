// Internal AST - CSS-specific types optimized for traversal and manipulation
//
// ARCHITECTURE: This is our internal AST representation, optimized for traversal,
// manipulation, and formatting. It gets converted to public AST for JSON serialization.

pub use tsv_lang::Comment;
use tsv_lang::Span;

/// CSS Stylesheet - top-level container for CSS nodes and comments
///
/// Comments are stored in a separate Vec, sorted by span.start.
/// This matches the TS/Svelte pattern and enables efficient range queries
/// using `comments_in_range()` from tsv_lang.
#[derive(Debug, Clone)]
pub struct CssStyleSheet {
    /// CSS nodes (rules, at-rules) - no longer includes Comment variant
    pub nodes: Vec<CssNode>,

    /// All comments sorted by span.start (top-level and value comments)
    ///
    /// Includes:
    /// - Top-level comments (between rules)
    /// - Value comments (inside property values like `font-size: /* comment */ 12px;`)
    ///
    /// Use `tsv_lang::comments_in_range()` for efficient range lookups.
    pub comments: Vec<Comment>,

    /// Precomputed line break positions (byte offsets of newlines).
    /// Used for O(log n) line boundary lookups during printing.
    pub line_breaks: Vec<u32>,
}

impl CssStyleSheet {
    /// Create a new empty stylesheet
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            comments: Vec::new(),
            line_breaks: Vec::new(),
        }
    }

    /// Create a stylesheet with nodes (no comments)
    pub fn with_nodes(nodes: Vec<CssNode>) -> Self {
        Self {
            nodes,
            comments: Vec::new(),
            line_breaks: Vec::new(),
        }
    }
}

impl Default for CssStyleSheet {
    fn default() -> Self {
        Self::new()
    }
}

/// CSS AST node types
///
/// Comments are stored separately in `CssStyleSheet.comments` and looked up by position.
#[derive(Debug, Clone)]
pub enum CssNode {
    Rule(CssRule),
    Atrule(CssAtrule), // @media, @keyframes, @supports, etc.
}

impl CssNode {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
            CssNode::Atrule(atrule) => atrule.span,
        }
    }
}

/// CSS Rule - selector with declaration block
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: SelectorList,
    pub block_span: Span,                 // Span of the block including braces
    pub declarations: Vec<CssBlockChild>, // Declarations and comments
    pub span: Span,                       // Full rule span
}

//
// Selector AST
//
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
    pub combinator_span: Option<Span>, // Position of the combinator symbol
    pub selectors: Vec<SimpleSelector>,
    pub span: Span,
}

/// Combinator between selectors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Combinator {
    Descendant = 0,        // space (ancestor-descendant)
    Child = 1,             // > (parent-child)
    NextSibling = 2,       // + (adjacent sibling)
    SubsequentSibling = 3, // ~ (general sibling)
    Column = 4,            // || (column combinator)
}

impl Combinator {
    /// Returns the combinator symbol
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Combinator::Descendant => " ",
            Combinator::Child => ">",
            Combinator::NextSibling => "+",
            Combinator::SubsequentSibling => "~",
            Combinator::Column => "||",
        }
    }
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
        args: Option<PseudoClassArgs>,
        span: Span,
    },
    PseudoElement {
        name: String,
        args: Option<PseudoClassArgs>,
        span: Span,
    },
    Nesting {
        span: Span, // & selector (CSS Nesting spec)
    },
    Percentage {
        value: f64,
        span: Span, // @keyframes percentage selectors (0%, 50%, 100%)
    },
    /// Invalid selector - unparseable syntax preserved for forgiving parsing
    ///
    /// Used in :is() and :where() pseudo-classes which use forgiving selector lists.
    /// Per CSS Selectors Level 4, invalid selectors are ignored for matching purposes
    /// but preserved in the source for formatter output.
    ///
    /// Examples: `.` (incomplete class), `[` (incomplete attribute), etc.
    Invalid {
        raw: String, // Raw selector text as written
        span: Span,
    },
}

/// Pseudo-class/pseudo-element argument types (semantic representation)
///
/// Stores semantic data (what the args mean), not output structure.
/// Conversion layer generates Svelte's wrapper format.
#[derive(Debug, Clone)]
pub enum PseudoClassArgs {
    /// Nth expression for :nth-child(), :nth-of-type(), :nth-last-child(), :nth-last-of-type()
    ///
    /// Values: "2n + 1", "odd", "even", "3", "-n+6", etc.
    /// Optional selector list for `:nth-child(An+B of S)` syntax (CSS Selectors Level 4)
    /// Span covers the argument content (inside the parentheses)
    Nth {
        value: String,
        of_selector: Option<SelectorList>,
        span: Span,
    },

    /// Selector list for :is(), :not(), :where(), :has()
    ///
    /// Contains a full SelectorList that can include multiple complex selectors.
    /// Used for logical combinations and relational selectors.
    SelectorList { selectors: SelectorList, span: Span },

    /// Compound selector for ::slotted() pseudo-element
    ///
    /// Per CSS Scoping Module Level 1: `::slotted( <compound-selector> )`
    /// A compound selector is a sequence of simple selectors without combinators.
    ///
    /// Examples: `*`, `div`, `.foo`, `div.foo#bar:hover`, `[slot]`
    /// Invalid: `div span` (combinator), `div > span` (combinator)
    Slotted {
        selectors: Vec<SimpleSelector>, // Compound selector (no combinators)
        span: Span,
    },

    /// Part names for ::part() pseudo-element
    ///
    /// Per CSS Shadow Parts Specification: `::part( <ident>+ )`
    /// One or more space-separated identifiers (NOT selectors).
    ///
    /// Multiple idents = intersection semantics (element must have ALL part names).
    /// Order-independent: `::part(tab active)` = `::part(active tab)`
    ///
    /// Examples: `label`, `tab`, `tab active`, `button primary`
    Part {
        idents: Vec<String>, // Space-separated part names
        span: Span,
    },

    /// Identifier argument for spec-compliant pseudo-classes/elements
    ///
    /// Used for pseudo-classes and pseudo-elements that take a single identifier per spec:
    /// - :dir() - takes direction identifier (ltr, rtl)
    /// - :lang() - takes language code (en, en-US, fr-CA, etc.)
    /// - ::highlight() - takes custom highlight name
    ///
    /// Note: Svelte's parser treats these as selectors in public AST (quirk applied at conversion)
    Identifier {
        value: String, // Identifier value (without quotes or parentheses)
        span: Span,
    },
}

impl PseudoClassArgs {
    /// Get the span of the pseudo-class/pseudo-element arguments
    pub fn span(&self) -> Span {
        match self {
            PseudoClassArgs::Nth { span, .. } => *span,
            PseudoClassArgs::SelectorList { span, .. } => *span,
            PseudoClassArgs::Slotted { span, .. } => *span,
            PseudoClassArgs::Part { span, .. } => *span,
            PseudoClassArgs::Identifier { span, .. } => *span,
        }
    }
}

/// Attribute selector matcher type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AttributeMatcher {
    Exact = 0,     // [attr="value"] - exact match
    Contains = 1,  // [attr~="value"] - whitespace-separated list contains value
    DashMatch = 2, // [attr|="value"] - exact or starts with value followed by -
    Prefix = 3,    // [attr^="value"] - starts with
    Suffix = 4,    // [attr$="value"] - ends with
    Substring = 5, // [attr*="value"] - contains substring
}

impl AttributeMatcher {
    /// Returns the matcher operator symbol
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            AttributeMatcher::Exact => "=",
            AttributeMatcher::Contains => "~=",
            AttributeMatcher::DashMatch => "|=",
            AttributeMatcher::Prefix => "^=",
            AttributeMatcher::Suffix => "$=",
            AttributeMatcher::Substring => "*=",
        }
    }
}

/// CSS Declaration - property: value pair
///
/// Maintains semantic representation:
/// - `value`: Rich semantic AST for manipulation, formatting, linting
/// - Source text extracted via span when needed (e.g., for JSON output)
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    pub property: String,
    pub value: CssValue, // Semantic representation (normalized)
    /// End position including !important (span.end excludes it for the formatter).
    /// `None` means no !important. Use `is_important()` for the bool check.
    pub important_end: Option<u32>,
    pub span: Span,
}

impl CssDeclaration {
    pub fn is_important(&self) -> bool {
        self.important_end.is_some()
    }
}

//
// CSS Value AST
//
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
///
/// Internal representation optimized for traversal and manipulation.
/// Converted to public JSON AST via the convert layer (see ast/convert.rs).
/// Never serialized directly - serde not needed!
#[derive(Debug, Clone)]
pub enum CssValue {
    /// Identifier: auto, bold, inherit, currentColor, etc.
    Identifier { name: String, span: Span },

    /// String literal: "Arial", 'font.woff'
    /// Content includes decoded escape sequences (internal representation)
    String {
        content: String, // string content without quotes (decoded)
        quote: char,     // original quote character (' or ")
        span: Span,
    },

    /// Number with optional unit: 10, 10px, 1.5em, 50%, etc.
    Dimension {
        value: f64,
        unit: String, // empty string for unitless numbers, "px", "%", etc.
        span: Span,
    },

    /// Color - various formats (rgb, hsl, hex, named)
    Color { color: Color, span: Span },

    /// Function call: calc(), var(), rgb(), url(), etc.
    Function {
        name: String,
        args: Vec<CssValue>,
        span: Span,
    },

    /// Space-separated list of values
    List { values: Vec<CssValue>, span: Span },

    /// Comma-separated list of values
    CommaSeparated { values: Vec<CssValue>, span: Span },
}

impl CssValue {
    /// Get the span of this value
    pub fn span(&self) -> Span {
        match self {
            CssValue::Identifier { span, .. } => *span,
            CssValue::String { span, .. } => *span,
            CssValue::Dimension { span, .. } => *span,
            CssValue::Color { span, .. } => *span,
            CssValue::Function { span, .. } => *span,
            CssValue::List { span, .. } => *span,
            CssValue::CommaSeparated { span, .. } => *span,
        }
    }
}

/// CSS color value
///
/// Color channel value - supports numbers, percentages, and CSS Color 4 `none`
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChannel {
    /// Numeric value: 255, 0.5, etc.
    Number(f64),
    /// Percentage value: 50%, 100%, etc.
    Percentage(f64),
    /// CSS Color 4 `none` keyword
    None,
}

/// Angle unit for hue values in HSL
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AngleUnit {
    /// Degrees (default, can be omitted)
    Deg = 0,
    /// Radians
    Rad = 1,
    /// Turns (1turn = 360deg)
    Turn = 2,
    /// Gradians (400grad = 360deg)
    Grad = 3,
}

impl AngleUnit {
    /// Returns the unit suffix string
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            AngleUnit::Deg => "deg",
            AngleUnit::Rad => "rad",
            AngleUnit::Turn => "turn",
            AngleUnit::Grad => "grad",
        }
    }
}

/// Internal representation - converted to JSON via convert layer.
#[derive(Debug, Clone)]
pub enum Color {
    /// Named color: red, blue, currentColor, etc.
    Named(String),

    /// Hex color: #ff0000, #f00, etc.
    Hex(String),

    /// RGB color: rgb(255, 0, 0) or rgb(255 0 0 / 1) or rgb(100% 0% 0%)
    /// Supports CSS Color 4: numbers, percentages, none, alpha as percentage
    Rgb {
        r: ColorChannel,
        g: ColorChannel,
        b: ColorChannel,
        alpha: Option<ColorChannel>,
    },

    /// HSL color: hsl(0, 100%, 50%) or hsl(120deg 75% 25%)
    /// Supports CSS Color 4: angle units, none keyword, alpha as percentage
    Hsl {
        hue: ColorChannel,
        hue_unit: Option<AngleUnit>, // None = unitless number (treated as degrees)
        saturation: ColorChannel,
        lightness: ColorChannel,
        alpha: Option<ColorChannel>,
    },
}

//
// At-Rule AST
//
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

/// At-rule prelude value
///
/// At-rules have different prelude structures:
/// - @import: structured values (url, layer, supports, media)
/// - @supports: structured conditions for line-width wrapping
/// - @media, @container: raw condition strings
/// - @keyframes: raw animation name
#[derive(Debug, Clone)]
pub enum PreludeValue {
    /// Structured values (for @import)
    /// Example: `url('styles.css') layer(base)` → [Function(url), Function(layer)]
    Values { values: Vec<CssValue>, span: Span },

    /// Raw prelude (for `@keyframes`, `@layer`, `@namespace`, `@page`, … — at-rules with
    /// no `property: value` / media-query grammar). `content` is the **printer-facing**
    /// string: verbatim source with internal whitespace + comments preserved and only
    /// `url()` inner whitespace trimmed (`@namespace` is the exception — its prelude is
    /// whitespace-normalized to match postcss). The public AST is reproduced separately
    /// from `span` (source-verbatim), so `content` never feeds the AST.
    /// Example: `@layer` → `a , b`; `@keyframes` → `my-anim`.
    Raw { content: String, span: Span },

    /// Selector lists (for @scope)
    /// Example: `@scope (.card) to (.footer)` → root: [.card], limit: Some([.footer])
    Selectors {
        root: SelectorList,
        limit: Option<SelectorList>,
        span: Span,
    },

    /// @supports condition (structured for line-width wrapping)
    /// Example: `(display: grid) and (flex: 1)` → parts connected by `and`/`or`
    Supports {
        condition: SupportsCondition,
        span: Span,
    },

    /// @container query (structured for line-width wrapping)
    /// Example: `sidebar (min-width: 100px) and (max-width: 200px)`
    Container {
        /// Optional container name (e.g., "sidebar")
        name: Option<String>,
        /// The condition parts connected by `and`/`or`
        condition: SupportsCondition,
        span: Span,
    },

    /// @media query - uses raw string with printer-side wrapping
    ///
    /// Unlike @supports/@container which use structured parsing, @media uses
    /// raw string parsing to preserve comments. Wrapping is handled in the
    /// printer by finding `and`/`or` boundaries in the raw string.
    ///
    /// Fully structuring preludes (vs. this raw form) is a deferred design option
    /// — see docs/architecture.md § "Red-Green Trees (Deferred)".
    Media { content: String, span: Span },
}

/// @supports condition structure for formatting
///
/// Allows wrapping at `and`/`or` boundaries while keeping the keyword
/// on the current line and the condition on the next.
#[derive(Debug, Clone)]
pub struct SupportsCondition {
    /// The condition parts connected by `and`/`or`
    pub parts: Vec<SupportsPart>,
}

/// A single part of a @supports condition
#[derive(Debug, Clone)]
pub struct SupportsPart {
    /// The connector before this part (None for first part)
    pub connector: Option<SupportsConnector>,
    /// The condition content (e.g., "(display: grid)" or "not (color: red)")
    pub content: String,
    pub span: Span,
}

/// Connector between @supports condition parts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportsConnector {
    And,
    Or,
}

impl PreludeValue {
    pub fn span(&self) -> Span {
        match self {
            PreludeValue::Values { span, .. } => *span,
            PreludeValue::Raw { span, .. } => *span,
            PreludeValue::Selectors { span, .. } => *span,
            PreludeValue::Supports { span, .. } => *span,
            PreludeValue::Container { span, .. } => *span,
            PreludeValue::Media { span, .. } => *span,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            PreludeValue::Values { values, .. } => values.is_empty(),
            PreludeValue::Raw { content, .. } => content.is_empty(),
            PreludeValue::Selectors { root, .. } => root.selectors.is_empty(),
            PreludeValue::Supports { condition, .. } => condition.parts.is_empty(),
            PreludeValue::Container {
                name, condition, ..
            } => name.is_none() && condition.parts.is_empty(),
            PreludeValue::Media { content, .. } => content.is_empty(),
        }
    }
}

/// At-rule (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
#[derive(Debug, Clone)]
pub struct CssAtrule {
    /// At-rule name without @ (e.g., "media", "keyframes")
    pub name: String,

    /// Prelude value (structured for @import, raw string for others)
    pub prelude: PreludeValue,

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
    Comment(Comment),
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
