// Internal AST - CSS-specific types optimized for traversal and manipulation
//
// ARCHITECTURE: This is our internal AST representation, optimized for traversal,
// manipulation, and formatting. The wire-JSON writer (`convert/write.rs`) emits
// the public JSON directly from it.

pub use tsv_lang::Comment;
use tsv_lang::Span;

/// CSS Stylesheet - top-level container for CSS nodes and comments
///
/// Comments are stored in a separate Vec, sorted by span.start.
/// This matches the TS/Svelte pattern and enables efficient range queries
/// using `comments_in_range()` from tsv_lang.
#[derive(Debug, Clone)]
pub struct CssStyleSheet<'arena> {
    /// CSS nodes (rules, at-rules) - no longer includes Comment variant
    pub nodes: &'arena [CssNode<'arena>],

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

impl<'arena> CssStyleSheet<'arena> {
    /// Create a new empty stylesheet
    pub fn new() -> Self {
        Self {
            nodes: &[],
            comments: Vec::new(),
            line_breaks: Vec::new(),
        }
    }
}

impl Default for CssStyleSheet<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// CSS AST node types
///
/// Comments are stored separately in `CssStyleSheet.comments` and looked up by position.
#[derive(Debug, Clone)]
pub enum CssNode<'arena> {
    Rule(CssRule<'arena>),
    Atrule(CssAtrule<'arena>), // @media, @keyframes, @supports, etc.
}

impl CssNode<'_> {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
            CssNode::Atrule(atrule) => atrule.span,
        }
    }
}

/// CSS Rule - selector with declaration block
#[derive(Debug, Clone)]
pub struct CssRule<'arena> {
    pub selector: SelectorList<'arena>,
    pub block_span: Span, // Span of the block including braces
    pub declarations: &'arena [CssBlockChild<'arena>], // Declarations and comments
    pub span: Span,       // Full rule span
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
pub struct SelectorList<'arena> {
    pub selectors: &'arena [ComplexSelector<'arena>],
    pub span: Span,
}

/// Complex selector - one or more relative selectors connected by combinators
#[derive(Debug, Clone)]
pub struct ComplexSelector<'arena> {
    pub children: &'arena [RelativeSelector<'arena>],
    pub span: Span,
}

/// Relative selector - combinator + simple selectors
#[derive(Debug, Clone)]
pub struct RelativeSelector<'arena> {
    pub combinator: Option<Combinator>,
    pub combinator_span: Option<Span>, // Position of the combinator symbol
    pub selectors: &'arena [SimpleSelector<'arena>],
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
pub enum SimpleSelector<'arena> {
    Type {
        /// Presence of a namespace prefix (`svg|rect`). The prefix text is kept only
        /// for the rare namespaced form; the element name is recovered from `span` at
        /// print time, so no separate `name` copy is stored (span-for-verbatim).
        namespace: Option<&'arena str>,
        span: Span,
    },
    Universal {
        namespace: Option<&'arena str>,
        span: Span,
    },
    /// Class selector. Name (including the `.`) recovered verbatim from `span`.
    Class { span: Span },
    /// Id selector. Name (including the `#`) recovered verbatim from `span`.
    Id { span: Span },
    Attribute {
        namespace: Option<&'arena str>,
        /// Span of the attribute name only (the `attr` in `[ns|attr op 'value' flags]`),
        /// verbatim. The name carries no decoded copy: the printer emits it raw from source
        /// (escapes preserved — `[f\oo]` stays `[f\oo]`, never `[foo]`) and convert
        /// half-decodes it via `raw_selector_name`, matching Svelte's `read_identifier`
        /// (see convert/mod.rs). `value`/`flags` stay owned strings — they are
        /// processed/re-quoted, not verbatim source slices.
        name_span: Span,
        matcher: Option<AttributeMatcher>,
        value: Option<&'arena str>,
        flags: Option<&'arena str>, // i (case-insensitive), s (case-sensitive)
        span: Span,
    },
    // Pseudo selectors carry no `name` field: the name is recovered verbatim from `span`
    // (the printer reads `source` directly; convert half-decodes via `raw_selector_name`,
    // matching Svelte's `read_identifier` — see convert/mod.rs). Storing a decoded name would
    // be a redundant copy that, for identity escapes (`:f\oo`), disagrees with the public
    // (Svelte) form anyway.
    PseudoClass {
        args: Option<PseudoClassArgs<'arena>>,
        span: Span,
    },
    PseudoElement {
        args: Option<PseudoClassArgs<'arena>>,
        span: Span,
    },
    Nesting {
        span: Span, // & selector (CSS Nesting spec)
    },
    Percentage {
        value: f64,
        span: Span, // @keyframes percentage selectors (0%, 50%, 100%)
    },
    /// `An+B` term (`2n`, `2n + 1`, `odd`, `123`) appearing as a simple selector
    /// inside functional pseudo-class arguments (`:is(.a, 123)`, `:foo(2n + 1)`).
    /// Matches Svelte's `read_selector` Nth production, which is active only inside
    /// pseudo-class args (top-level `123 {}` still rejects). `span` covers the An+B
    /// value text verbatim: the public `Nth` node's `value` is that source slice
    /// (parseCss stores it raw) and the printer normalizes the spacing
    /// (`2n+1` → `2n + 1`), like the dedicated `:nth-child` path. For an `An+B of S`
    /// term the span folds in the ` of ` (`2n of `), matching Svelte's `\s+of\s+`
    /// terminator; `S` follows as ordinary sibling selectors (NOT nested), unlike the
    /// dedicated `:nth-*()` path which nests `S` under `Nth.selector`.
    Nth { span: Span },
    /// Invalid selector - unparseable syntax preserved for forgiving parsing
    ///
    /// Used in :is() and :where() pseudo-classes which use forgiving selector lists.
    /// Per CSS Selectors Level 4, invalid selectors are ignored for matching purposes
    /// but preserved in the source for formatter output.
    ///
    /// Examples: `.` (incomplete class), `[` (incomplete attribute), etc.
    Invalid {
        // Raw selector text as written — recovered verbatim from `span` (trimmed) at
        // print time (span-for-verbatim); convert filters Invalid selectors out.
        span: Span,
    },
}

impl SimpleSelector<'_> {
    /// Get the span of this simple selector.
    pub fn span(&self) -> Span {
        match self {
            SimpleSelector::Type { span, .. }
            | SimpleSelector::Universal { span, .. }
            | SimpleSelector::Class { span }
            | SimpleSelector::Id { span }
            | SimpleSelector::Attribute { span, .. }
            | SimpleSelector::PseudoClass { span, .. }
            | SimpleSelector::PseudoElement { span, .. }
            | SimpleSelector::Nesting { span }
            | SimpleSelector::Percentage { span, .. }
            | SimpleSelector::Nth { span }
            | SimpleSelector::Invalid { span } => *span,
        }
    }
}

/// Pseudo-class/pseudo-element argument types (semantic representation)
///
/// Stores semantic data (what the args mean), not output structure.
/// Conversion layer generates Svelte's wrapper format.
#[derive(Debug, Clone)]
pub enum PseudoClassArgs<'arena> {
    /// Nth expression for :nth-child(), :nth-of-type(), :nth-last-child(), :nth-last-of-type()
    ///
    /// Values: "2n + 1", "odd", "even", "3", "-n+6", etc.
    /// Optional selector list for `:nth-child(An+B of S)` syntax (CSS Selectors Level 4)
    /// Span covers the argument content (inside the parentheses); `value_span`
    /// covers just the trimmed An+B text, so the printer can find the comments
    /// in the gaps around it (before the An+B, around `of`, after the list)
    Nth {
        value: &'arena str,
        of_selector: Option<SelectorList<'arena>>,
        span: Span,
        value_span: Span,
    },

    /// Selector list for `:is()`, `:not()`, `:where()`, `:has()`, and `::slotted()`
    ///
    /// Contains a full SelectorList that can include multiple complex selectors.
    /// Used for logical combinations, relational selectors, and `::slotted()` — whose
    /// spec grammar is `<compound-selector>` but which parseCss parses (and tsv
    /// reproduces) as a lenient `<complex-selector-list>`, dropping it from the wire
    /// AST at the pseudo-element convert boundary.
    SelectorList {
        selectors: SelectorList<'arena>,
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
    ///
    /// `span` covers the argument content (inside the parentheses); `value_span`
    /// covers just the identifier run, so the printer can find the comments in the
    /// gaps around it (before the first ident, after the last), mirroring `Nth`.
    Part {
        idents: &'arena [&'arena str], // Space-separated part names
        span: Span,
        value_span: Span,
    },
}

impl PseudoClassArgs<'_> {
    /// Get the span of the pseudo-class/pseudo-element arguments
    pub fn span(&self) -> Span {
        match self {
            PseudoClassArgs::Nth { span, .. } => *span,
            PseudoClassArgs::SelectorList { span, .. } => *span,
            PseudoClassArgs::Part { span, .. } => *span,
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
pub struct CssDeclaration<'arena> {
    /// Property name, **escape-decoded** (spec-canonical `<ident-token>` value, e.g. `\63 olor`
    /// → `color`; CSS Syntax §4.3.11). This is NOT a verbatim slice of `span`, so it is kept as
    /// an arena string rather than recovered from a span (a span would silently un-decode it).
    /// Used internally for the `--`-prefix check, keyword matches (`grid*`), and inline-width
    /// math. The printer's *output* and the public AST both emit the **raw** property text from
    /// source instead (`extract_property_name` / convert), preserving escapes — a documented
    /// Svelte quirk. So decoded(internal) / raw(output+public) is intentional; see the `(c)`
    /// follow-up in the bumpalo arena lore for the spec-vs-Svelte encoding map.
    pub property: &'arena str,
    pub value: CssValue<'arena>, // Semantic representation (normalized)
    /// End position including !important (span.end excludes it for the formatter).
    /// `None` means no !important. Use `is_important()` for the bool check.
    pub important_end: Option<u32>,
    pub span: Span,
    /// Absolute (host-coordinate, like `span`) byte offset of the real
    /// `property : value` colon — the one the parser `expect`s, outside any
    /// comment/string. Recorded at parse time so the wire-JSON writer splits
    /// property/value without re-scanning for it. The colon is one ASCII byte, so
    /// the value starts at `colon_offset + 1`.
    pub colon_offset: u32,
    /// Whether a `/* … */` block comment appears anywhere in the declaration
    /// extent (the property→colon gap, or the value / `!important` / trailing
    /// region up to the terminator). Precomputed from the lexer's comment tokens so
    /// the writer takes a zero-scan fast path — split at `colon_offset`, value just
    /// trimmed — in the common no-comment case, and the comment-aware split/strip
    /// path only when true.
    pub has_block_comment: bool,
}

impl CssDeclaration<'_> {
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

/// The decoded value of a CSS string literal, mirroring `tsv_ts`'s `StringCooked`.
///
/// `Verbatim` (the common no-escape case) carries **no allocation** — the decoded
/// value equals the inner source slice (the literal's `span` minus the two quote
/// bytes). Only strings with escape sequences own arena bytes.
#[derive(Debug, Clone)]
pub enum StringCooked<'arena> {
    /// Decoded value == the inner source slice (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw inner text.
    Decoded(&'arena str),
}

impl<'arena> StringCooked<'arena> {
    /// The decoded string value. `span` is the owning [`CssValue::String`]'s span
    /// (the quoted literal); `source` is the host document. `Verbatim` slices the
    /// inner text (zero-copy); `Decoded` returns the arena bytes.
    #[inline]
    pub fn resolve<'s>(&'s self, span: Span, source: &'s str) -> &'s str {
        match self {
            StringCooked::Verbatim => {
                let raw = span.extract(source);
                &raw[1..raw.len() - 1]
            }
            StringCooked::Decoded(decoded) => decoded,
        }
    }
}

/// CSS value - right-hand side of a declaration
///
/// Internal representation optimized for traversal and manipulation.
/// Converted to public JSON AST via the convert layer (see ast/convert/mod.rs).
/// Never serialized directly - serde not needed!
#[derive(Debug, Clone)]
pub enum CssValue<'arena> {
    /// Identifier: auto, bold, inherit, currentColor, etc.
    ///
    /// The text is recovered verbatim from `span` at print time
    /// (`build_identifier_doc`, escapes preserved) — span-for-verbatim, so no copied
    /// string is stored. An empty/whitespace-only `span` marks the empty-identifier
    /// sentinel (`var(--a,)`'s trailing fallback, an empty custom-property value).
    Identifier { span: Span },

    /// String literal: `"Arial"`, `'font.woff'`.
    ///
    /// The printed text is recovered verbatim from `span` (escapes preserved, quote
    /// normalized) — span-for-verbatim, like `Identifier`. The quote char is the
    /// first byte of `span` (`source[span.start]`), not stored. `content` carries the
    /// *decoded* value for the defensive span-unavailable fallback only: `Verbatim`
    /// for no-escape strings (zero alloc), `Decoded` when escapes were applied.
    String {
        content: StringCooked<'arena>,
        span: Span,
    },

    /// Number with optional unit: 10, 10px, 1.5em, 50%, etc.
    ///
    /// The variant tag classifies the token as a dimension (vs an identifier) so the
    /// printer applies number normalization; the numeric value and unit text are
    /// recovered verbatim from `span` at print time (`build_dimension_doc`), so they
    /// are not stored — span-for-verbatim (see the arena string-representation idiom).
    Dimension { value: f64, span: Span },

    /// Color - various formats (rgb, hsl, hex, named)
    Color { color: Color, span: Span },

    /// Function call: calc(), var(), rgb(), url(), etc.
    ///
    /// `name` is a **verbatim** source slice (the text before `(`, never escape-decoded —
    /// the value subtree is source-faithful, never re-serialized; see `convert/mod.rs`). It is a
    /// candidate for the span-for-verbatim idiom (a dedicated `name_span` would drop this
    /// copy), but the exact name span needs trim-aware arithmetic (`s[..paren].trim()`) plus
    /// base-offset alignment across callers — deferred as a perf-neutral additive (see the
    /// `(c)` follow-up in the bumpalo arena lore). Not a decoded identifier, unlike
    /// `CssAtrule.name` / `CssDeclaration.property`.
    Function {
        name: &'arena str,
        args: &'arena [CssValue<'arena>],
        span: Span,
    },

    /// Space-separated list of values
    List {
        values: &'arena [CssValue<'arena>],
        span: Span,
    },

    /// Comma-separated list of values
    CommaSeparated {
        values: &'arena [CssValue<'arena>],
        span: Span,
    },
}

impl CssValue<'_> {
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
///
/// `Named`/`Hex` carry no text: it is recovered verbatim from the enclosing
/// `CssValue::Color.span` at print time (span-for-verbatim), so `Color` holds no
/// arena reference and needs no lifetime.
#[derive(Debug, Clone)]
pub enum Color {
    /// Named color: red, blue, currentColor, etc.
    Named,

    /// Hex color: #ff0000, #f00, etc.
    Hex,

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
pub enum PreludeValue<'arena> {
    /// Structured values (for @import)
    /// Example: `url('styles.css') layer(base)` → [Function(url), Function(layer)]
    Values {
        values: &'arena [CssValue<'arena>],
        span: Span,
    },

    /// Raw prelude (for `@keyframes`, `@layer`, `@namespace`, `@page`, … — at-rules with
    /// no `property: value` / media-query grammar). `content` is the **printer-facing**
    /// string: verbatim source with internal whitespace + comments preserved and only
    /// `url()` inner whitespace trimmed (`@namespace` is the exception — its prelude is
    /// whitespace-normalized to match postcss). The public AST is reproduced separately
    /// from `span` (source-verbatim), so `content` never feeds the AST.
    /// Example: `@layer` → `a , b`; `@keyframes` → `my-anim`.
    Raw { content: &'arena str, span: Span },

    /// Selector lists (for @scope). Both clauses are independently optional per
    /// css-cascade-6 (`@scope [(<scope-start>)]? [to (<scope-end>)]?`), so a bare
    /// `@scope { … }` has `root: None, limit: None`, and `@scope to (.footer)` has
    /// `root: None, limit: Some(…)`.
    /// Example: `@scope (.card) to (.footer)` → root: Some([.card]), limit: Some([.footer])
    Selectors {
        root: Option<SelectorList<'arena>>,
        limit: Option<SelectorList<'arena>>,
        span: Span,
    },

    /// @supports condition (structured for line-width wrapping)
    /// Example: `(display: grid) and (flex: 1)` → parts connected by `and`/`or`
    Supports {
        condition: ConditionQuery<'arena>,
        span: Span,
    },

    /// @container query (structured for line-width wrapping)
    /// Example: `sidebar (min-width: 100px) and (max-width: 200px)`
    Container {
        /// Optional container name (e.g., "sidebar"), **escape-decoded** (spec-canonical
        /// `<ident-token>`). Printer-only — convert emits the whole prelude from `span`
        /// (raw), not this field. Kept as an arena string (not a span) because the span
        /// holds the raw escaped form, which would diverge from the decoded value; same
        /// category as `CssAtrule.name`.
        name: Option<&'arena str>,
        /// The condition parts connected by `and`/`or`
        condition: ConditionQuery<'arena>,
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
    Media { content: &'arena str, span: Span },
}

/// A boolean condition query — the structured prelude shared by `@supports`
/// (`<supports-condition>`) and `@container` (`<container-query>`), whose
/// grammars are identical. Structured (rather than raw) so the printer can wrap
/// at `and`/`or` boundaries, keeping the keyword on the current line and the
/// condition on the next.
#[derive(Debug, Clone)]
pub struct ConditionQuery<'arena> {
    /// The condition parts connected by `and`/`or`
    pub parts: &'arena [ConditionPart<'arena>],
}

/// A single part of a `ConditionQuery` (one `(prop: val)` term, optionally
/// `not`-prefixed or function-style like `selector(...)`).
#[derive(Debug, Clone)]
pub struct ConditionPart<'arena> {
    /// The connector before this part (None for first part). Normalized to the
    /// `And`/`Or` enum for logic (comment-split, presence); the original source
    /// **case** is carried separately in `connector_raw` for output.
    pub connector: Option<ConditionConnector>,
    /// The connector's verbatim source text (`and`/`AND`/`Or`/…), emitted by the
    /// printer so the author's case is preserved (matching prettier). `Some` iff
    /// `connector` is `Some`.
    pub connector_raw: Option<&'arena str>,
    /// The condition content (e.g., "(display: grid)" or "not (color: red)"). A
    /// leading `not` keeps its source case (preserved like the connectors).
    pub content: &'arena str,
    pub span: Span,
}

/// Connector (`and`/`or`) between `ConditionQuery` parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionConnector {
    And,
    Or,
}

impl PreludeValue<'_> {
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
            PreludeValue::Selectors { root, limit, .. } => {
                root.as_ref().is_none_or(|r| r.selectors.is_empty()) && limit.is_none()
            }
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
pub struct CssAtrule<'arena> {
    /// At-rule name without `@` (e.g., "media", "keyframes"), **escape-decoded**
    /// (spec-canonical `<at-keyword-token>`, CSS Syntax §4.3.3; Svelte's parser also decodes
    /// it — `@\6d edia` → `"media"`). Both the printer (`@` + this) and convert emit the
    /// decoded form, matching Svelte + spec. Kept as an arena string, NOT recovered from
    /// `span`: the span covers `@name …` and holds the raw escaped bytes, so a span-drop would
    /// silently un-decode the name and diverge from Svelte/spec for escaped names. Same
    /// category as `CssDeclaration.property` / `Container.name`; see the `(c)` follow-up in
    /// the bumpalo arena lore.
    pub name: &'arena str,

    /// Prelude value (structured for @import, raw string for others)
    pub prelude: PreludeValue<'arena>,

    /// Block contents (Some for conditional/descriptor, None for statement at-rules)
    pub block: Option<CssAtruleBlock<'arena>>,

    pub span: Span,
}

/// At-rule block - can contain rules, declarations, or nested at-rules
#[derive(Debug, Clone)]
pub struct CssAtruleBlock<'arena> {
    /// Block children - mixture depends on at-rule type
    ///
    /// @media, @supports, @layer: Vec<CssNode> (rules + nested at-rules)
    /// @font-face, @page: Vec<CssDeclaration> (declarations only)
    ///
    /// We store as Vec<CssBlockChild> to support both:
    pub children: &'arena [CssBlockChild<'arena>],
    pub span: Span,
}

/// At-rule block child - can be rule, declaration, or nested at-rule
#[derive(Debug, Clone)]
pub enum CssBlockChild<'arena> {
    Rule(CssRule<'arena>),
    Declaration(CssDeclaration<'arena>),
    Atrule(CssAtrule<'arena>),
    Comment(Comment),
}

impl CssBlockChild<'_> {
    pub fn span(&self) -> Span {
        match self {
            CssBlockChild::Rule(rule) => rule.span,
            CssBlockChild::Declaration(decl) => decl.span,
            CssBlockChild::Atrule(atrule) => atrule.span,
            CssBlockChild::Comment(comment) => comment.span,
        }
    }
}
