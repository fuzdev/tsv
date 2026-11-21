// CSS AST modules

#[cfg(feature = "json")]
pub(crate) mod convert;

// Internal AST - CSS-specific types optimized for traversal and manipulation
//
// ARCHITECTURE: This is our internal AST representation, optimized for traversal,
// manipulation, and formatting. The wire-JSON writer (`convert/write.rs`) emits
// the public JSON directly from it.

mod stores;
mod view;

pub(crate) use stores::with_one_shot_stores;
pub use stores::{
    CssStores, CssText, ParsedCss, WarmStores, park_stores, park_stores_capped,
    parked_stores_capacity, take_stores, with_stores,
};
pub use tsv_lang::Comment;
use tsv_lang::Span;
pub use tsv_lang::packed::PackedF64;
use tsv_lang::store::{PoolText, Run};
pub use view::{CssParsedRef, CssView};

/// CSS Stylesheet - top-level container for CSS nodes and comments
///
/// Comments are stored in a separate Vec, sorted by span.start.
/// This matches Svelte's `Root` (a TypeScript `Program`'s comments ride its stores as a
/// `Run<Comment>`) and enables efficient range queries using `comments_to_emit_in_range()`
/// from tsv_lang.
#[derive(Debug, Clone)]
pub struct CssStyleSheet {
    /// CSS nodes (rules, at-rules); comments are not among them
    pub nodes: Run<CssNode>,

    /// Top-level detached comments (between rules), sorted by span.start.
    ///
    /// Does **not** include value comments (inside property values like
    /// `font-size: /* comment */ 12px;`): those are never lexed as `Comment`s — they're
    /// re-derived from source at print time — so they're out of scope by construction.
    /// Nor the in-block ones, which ride `CssBlockChild::Comment`. **Three disjoint
    /// places, one per printer need** — the wire's flat `CSSComment[]` is a fourth
    /// reading that spans all of them, rebuilt by `ast/convert/write.rs`'s
    /// `write_children` as it emits.
    ///
    /// Use `tsv_lang::comments_to_emit_in_range()` for efficient range lookups.
    pub comments: Vec<Comment>,
}

impl CssStyleSheet {
    /// Create a new empty stylesheet
    pub fn new() -> Self {
        Self {
            nodes: Run::EMPTY,
            comments: Vec::new(),
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
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
pub struct CssRule {
    pub selector: SelectorList,
    pub block_span: Span,                 // Span of the block including braces
    pub declarations: Run<CssBlockChild>, // Declarations and comments
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
#[derive(Debug, Clone, Copy)]
pub struct SelectorList {
    pub selectors: Run<ComplexSelector>,
    pub span: Span,
}

/// Complex selector - one or more relative selectors connected by combinators
#[derive(Debug, Clone, Copy)]
pub struct ComplexSelector {
    pub children: Run<RelativeSelector>,
    pub span: Span,
}

/// Relative selector - combinator + simple selectors
#[derive(Debug, Clone, Copy)]
pub struct RelativeSelector {
    pub combinator: Option<Combinator>,
    pub combinator_span: Option<Span>, // Position of the combinator symbol
    /// The compound's simple selectors. Empty for a combinator with no compound after it.
    pub selectors: Run<SimpleSelector>,
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
#[derive(Debug, Clone, Copy)]
pub enum SimpleSelector {
    Type {
        /// Span of the `<ns-prefix>`'s leading token — the `svg` of `svg|rect`, the `*`
        /// of `*|div`, or an EMPTY span at the `|` of `|div` — and `None` for a bare type
        /// selector. Verbatim, never a decoded copy: an escape can put a `|` *inside* the
        /// prefix (`a\|b|rect`), so a reader that scans for the first `|` reads the
        /// escape as the separator and the rest of the prefix as the element name. Every
        /// reader instead steps forward from this span's end, where the next non-trivia
        /// byte is the separator by construction. The element name is likewise recovered
        /// from `span` at print time, so no `name` copy is stored (span-for-verbatim).
        namespace_span: Option<Span>,
        span: Span,
    },
    Universal {
        /// The `<ns-prefix>` span, as [`Self::Type`]'s — the `svg` of `svg|*`, the `*` of
        /// `*|*`, an EMPTY span at the `|` of `|*`. `None` for a bare `*`.
        namespace_span: Option<Span>,
        span: Span,
    },
    /// Class selector. Name (including the `.`) recovered verbatim from `span`.
    Class { span: Span },
    /// Id selector. Name (including the `#`) recovered verbatim from `span`.
    Id { span: Span },
    Attribute {
        /// Span of the `<ns-prefix>`'s leading token, as [`Self::Type`]'s — the `svg` of
        /// `[svg|attr]`, the `*` of `[*|attr]`, an EMPTY span at the `|` of `[|attr]`.
        /// The printer emits it verbatim and the wire half-decodes it like every other
        /// selector name; neither re-derives it from a scan for `|`.
        namespace_span: Option<Span>,
        /// Span of the attribute name only (the `attr` in `[ns|attr op 'value' flags]`),
        /// verbatim. The name carries no decoded copy: the printer emits it raw from source
        /// (escapes preserved — `[f\oo]` stays `[f\oo]`, never `[foo]`) and convert
        /// half-decodes it via `raw_selector_name`, matching Svelte's `read_identifier`
        /// (see convert/mod.rs). The value is likewise a raw span (`value_span`); only
        /// `flags` stays an owned string.
        name_span: Span,
        matcher: Option<AttributeMatcher>,
        /// Raw span of the value token (`None` for bare `[attr]`): a string keeps its
        /// delimiting quotes, an identifier its escapes (`[a=x\27]` stays `x\27`). The
        /// wire and the scoping compiler derive their text via [`attribute_value_text`];
        /// the printer re-quotes from the raw form.
        value_span: Option<Span>,
        flags: Option<PoolText<CssText>>, // i (case-insensitive), s (case-sensitive)
        span: Span,
    },
    // Pseudo selectors carry no `name` field: the name is recovered verbatim from `span`
    // (the printer reads `source` directly; convert half-decodes via `raw_selector_name`,
    // matching Svelte's `read_identifier` — see convert/mod.rs). Storing a decoded name would
    // be a redundant copy that, for identity escapes (`:f\oo`), disagrees with the public
    // (Svelte) form anyway.
    PseudoClass {
        args: Option<PseudoClassArgs>,
        /// End of the name token, from the parser rather than from a scan of `span`.
        /// A functional pseudo's name runs to the `<function-token>`'s `(` — which is
        /// **not** the first `(` in the span, since an identity escape can put one
        /// inside the name (`:foo\(bar(.x)`, name `foo\(bar`). Equals `span.end` when
        /// there are no args.
        name_end: u32,
        span: Span,
    },
    PseudoElement {
        args: Option<PseudoClassArgs>,
        /// End of the name token, as [`Self::PseudoClass`]'s.
        name_end: u32,
        span: Span,
    },
    Nesting {
        span: Span, // & selector (CSS Nesting spec)
    },
    Percentage {
        value: PackedF64,
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

impl SimpleSelector {
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

/// The quote-aware split of an attribute selector's raw value token: the delimiting
/// quote (`None` for a bare identifier value) and the text inside it (the whole raw
/// token when bare). The single statement of "a raw value whose first byte is a quote
/// is a string; its interior is `raw[1..len-1]`" — the wire, the scoping compiler, and
/// the printer's re-quoting all read this split rather than re-deriving it. The slice
/// is safe: a `TokenKind::String` span always carries both delimiters (the lexer
/// errors on an unterminated string).
pub(crate) fn attribute_value_quote_and_text(
    source: &str,
    value_span: Span,
) -> (Option<char>, &str) {
    let raw = value_span.extract(source);
    match raw.as_bytes().first() {
        Some(&q @ (b'\'' | b'"')) => (Some(q as char), &raw[1..raw.len() - 1]),
        _ => (None, raw),
    }
}

/// The public-AST text of an attribute selector's value (`AttributeSelector.value` on
/// the wire, and what the scoping compiler matches against): the raw token with a
/// string's delimiting quotes stripped, then trimmed. Escapes stay encoded in both forms —
/// Svelte's `parseCss` emits `[a=x\27]` as `x\27` and `[a='it\'s']` as `it\'s`, never
/// decoding.
///
/// The trim is `read_attribute_value`'s own `value.trim()` — JS `\s`, over a quoted
/// interior as much as a bare value, and blind to escapes — so `[a=' b ']` is the value `b`
/// and `[a=b\ ]` the value `b\` on the wire, as they are in Svelte. The printer reads the
/// untrimmed [`attribute_value_quote_and_text`] instead: the padding is the author's content
/// and is kept.
pub fn attribute_value_text(source: &str, value_span: Span) -> &str {
    tsv_lang::trim_js_whitespace(attribute_value_quote_and_text(source, value_span).1)
}

/// Pseudo-class/pseudo-element argument types (semantic representation)
///
/// Stores semantic data (what the args mean), not output structure.
/// Conversion layer generates Svelte's wrapper format.
#[derive(Debug, Clone, Copy)]
pub enum PseudoClassArgs {
    /// Nth expression for :nth-child(), :nth-of-type(), :nth-last-child(), :nth-last-of-type()
    ///
    /// Values: "2n + 1", "odd", "even", "3", "-n+6", etc.
    /// Optional selector list for `:nth-child(An+B of S)` syntax (CSS Selectors Level 4)
    /// Span covers the argument content (inside the parentheses); `value_span`
    /// covers just the trimmed An+B text, so the printer can find the comments
    /// in the gaps around it (before the An+B, around `of`, after the list)
    Nth {
        value: PoolText<CssText>,
        of_selector: Option<SelectorList>,
        span: Span,
        value_span: Span,
    },

    /// Selector list for `:is()`, `:not()`, `:where()`, `:has()`, and `::slotted()`
    ///
    /// Contains a full SelectorList that can include multiple complex selectors.
    /// Used for logical combinations, relational selectors, and `::slotted()` — whose
    /// spec grammar is `<compound-selector>` but which parseCss parses (and tsv
    /// reproduces) as a lenient `<complex-selector-list>`.
    SelectorList { selectors: SelectorList, span: Span },

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
    /// `span` covers the argument content (inside the parentheses). `ident_spans`
    /// holds one span per part name, and the names are read from them: the printer
    /// emits each verbatim (an escaped name included), so no decoded copy is stored.
    /// The spans also let the printer find the comments in every gap — around the
    /// run (before the first name, after the last) and *between* names — and derive
    /// the whole-run span (first name's start .. last name's end). Always non-empty
    /// (the parser rejects an empty `::part()`).
    ///
    /// ⚠️ The **wire** reads `ident_spans` too. `parseCss` has no `::part` grammar
    /// — it reads every pseudo-element argument as a selector list — so the writer
    /// projects this run onto that shape (`ast/convert/write.rs`'s
    /// `write_part_args`: one `ComplexSelector` of `TypeSelector`s joined by
    /// descendant combinators, the gaps taken from these spans). A parser change
    /// that admits a shape the projection can't express — a comma list, anything
    /// but a plain ident run — must move the projection in the same breath, or the
    /// wire goes wrong rather than merely different.
    Part {
        ident_spans: Run<Span>, // One span per space-separated part name
        span: Span,
    },

    /// An unknown functional pseudo's argument that is not a selector list — the
    /// `<any-value>` of Selectors 4's `: <function-token> <any-value> )` (`:foo(1 + 2)`,
    /// `::foo('x', 1)`). `span` covers the argument content (inside the parentheses), and
    /// the text is read from it: the printer re-emits it with its whitespace and quotes
    /// normalized and every other token as written (`ValueReader::AnyValue`), comments
    /// included, so nothing else is stored. Its comments are registered like any other.
    ///
    /// ⚠️ The **wire** has no node for it: parseCss rejects a non-selector argument, and
    /// tsv keeps Svelte's shape for the accepted one — `args: null` on a pseudo-class, no
    /// `args` key on a pseudo-element — the node's own span already covering the parens.
    AnyValue { span: Span },
}

impl PseudoClassArgs {
    /// Get the span of the pseudo-class/pseudo-element arguments
    pub fn span(&self) -> Span {
        match self {
            PseudoClassArgs::Nth { span, .. } => *span,
            PseudoClassArgs::SelectorList { span, .. } => *span,
            PseudoClassArgs::Part { span, .. } => *span,
            PseudoClassArgs::AnyValue { span } => *span,
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
#[derive(Debug, Clone, Copy)]
pub struct CssDeclaration {
    /// Property name, **escape-decoded** (spec-canonical `<ident-token>` value, e.g. `\63 olor`
    /// → `color`; CSS Syntax §4.3.11), read through [`CssDeclaration::property_text`]. Used
    /// internally for the `--`-prefix check, keyword matches (`grid*`), and inline-width math.
    /// The printer's *output* and the public AST both emit the **raw** property text from
    /// source instead (`extract_property_name` / convert), preserving escapes — a documented
    /// Svelte quirk. So decoded(internal) / raw(output+public) is intentional (the
    /// spec-vs-Svelte escape-encoding split).
    pub property: PropertyName,
    pub value: CssValue, // Semantic representation (normalized)
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

/// A [`CssDeclaration`]'s escape-decoded property name.
///
/// A name with nothing to decode is the source's own bytes at the start of the declaration,
/// so it is kept as its length and read from there — no copy is made. An escaped name (its
/// decoded text differs from its source bytes) or a wildcard custom property (`--a-*`, which
/// spans past one token) is pool text.
#[derive(Debug, Clone, Copy)]
pub enum PropertyName {
    /// The `len` source bytes at the declaration's `span.start`.
    Verbatim { len: u32 },
    /// Text of its own, in the stores' pool.
    Pooled(PoolText<CssText>),
}

impl CssDeclaration {
    /// The escape-decoded property name ([`Self::property`]).
    ///
    /// # Panics
    ///
    /// When `source` is not the whole document the declaration's `span` indexes — the host
    /// document for a sheet embedded in one (a Svelte `<style>`), not the sheet's own text —
    /// a verbatim name's slice can fall out of bounds or off a char boundary.
    #[inline]
    pub fn property_text<'a>(&self, view: CssView<'a>, source: &'a str) -> &'a str {
        match self.property {
            PropertyName::Verbatim { len } => {
                let start = self.span.start as usize;
                &source[start..start + len as usize]
            }
            PropertyName::Pooled(text) => view.text(text),
        }
    }

    pub fn is_important(&self) -> bool {
        self.important_end.is_some()
    }

    /// Byte offset of the `property : value` colon **within `span.extract(source)`**
    /// (equivalently `ctx.source[span.start..]`) — the absolute [`colon_offset`] rebased
    /// to the declaration slice. Every source-extracting path indexes its slice by this
    /// and finds the value at `colon_pos() + 1`; recorded at parse time, so no path
    /// re-scans for the colon.
    ///
    /// [`colon_offset`]: Self::colon_offset
    pub fn colon_pos(&self) -> usize {
        (self.colon_offset - self.span.start) as usize
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
/// bytes). Only strings with escape sequences own text in the stores' pool.
#[derive(Debug, Clone, Copy)]
pub enum StringCooked {
    /// Decoded value == the inner source slice (no escapes to decode).
    Verbatim,
    /// Escapes were decoded into a value distinct from the raw inner text.
    Decoded(PoolText<CssText>),
}

/// CSS value - right-hand side of a declaration
///
/// Internal representation optimized for traversal and manipulation.
/// Converted to public JSON AST via the convert layer (see ast/convert/mod.rs).
/// Never serialized directly - serde not needed!
///
/// `#[repr(u8)]`, with `span` the first field of every variant: the tag is a plain byte at
/// the front, and every variant's span sits at the same offset behind it, so a `match`
/// reads the tag with no decode and [`CssValue::span`] is one load. Left to the compiler,
/// the 4-aligned layout puts the tag in a niche of `Color`'s channels and scatters the span
/// across five offsets, and every `match` on a value decodes the tag first.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CssValue {
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
    String { span: Span, content: StringCooked },

    /// Number with optional unit: 10, 10px, 1.5em, 50%, etc.
    ///
    /// The variant tag classifies the token as a dimension (vs an identifier) so the
    /// printer applies number normalization; the numeric value and unit text are
    /// recovered verbatim from `span` at print time (`build_dimension_doc`), so they
    /// are not stored — span-for-verbatim (see the string-representation idiom).
    Dimension { span: Span },

    /// Color - various formats (rgb, hsl, hex, named)
    Color { span: Span, color: Color },

    /// Function call: calc(), var(), rgb(), url(), etc.
    ///
    /// `name_span` is the **verbatim** span of the text before `(` — span-for-verbatim,
    /// like `Identifier` and `Dimension`, so the author's bytes reach the output
    /// unchanged and no name is copied into the pool. The value subtree is
    /// source-faithful, never re-serialized (see `convert/mod.rs`), which is what lets a
    /// span stand in for the text: an escape-spelled name (`\6c ayer(`) prints as
    /// written rather than decoded, matching prettier. Unlike `CssAtrule.name` /
    /// `CssDeclaration.property`, which are decoded identifiers.
    ///
    /// Recognition (`url`, `var`) reads the span and decodes only when it holds a `\`
    /// — see `printer::values::function_name_is`.
    Function {
        span: Span,
        name_span: Span,
        args: Run<CssValue>,
    },

    /// An `@import` prelude's `supports()` call, whose argument is parsed as the
    /// condition it is rather than kept as opaque text.
    ///
    /// `supports( <supports-condition> | <declaration> )` (css-cascade-5
    /// §"Conditional import rules") is the grammar `@supports` takes, so the argument
    /// carries the same `ConditionQuery` and prints through the same printer — one
    /// condition, one form, in both positions. The function's own parentheses **are**
    /// the condition part's, so the query holds exactly one part and `span` (the whole
    /// call, `supports` through its `)`) needs no separate paren bookkeeping.
    ///
    /// `name` is the **verbatim** token slice — the author's own bytes, not the decoded
    /// identifier the recognition test above it runs on, so an escape-spelled
    /// `\73 upports(` prints as written (prettier preserves it too). `Function` answers
    /// the same question with a `name_span` and no copy at all; this variant keeps the
    /// text because its printer spells the condition rather than the name, and the copy
    /// is an `@import` prelude's alone.
    SupportsCondition {
        span: Span,
        name: PoolText<CssText>,
        condition: ConditionQuery,
    },

    /// A value **operator** token — `/`, `*`, `+`, `-` or `:` — split out of a run by
    /// `parser::value::operators::split_value_run`.
    ///
    /// The byte is recovered from `span` at print time (span-for-verbatim, like
    /// `Identifier`), so the variant stores only where it is. Its *kind* is read back off
    /// that byte by the printer's separator rule
    /// (`printer::values::ValueOperator::at`), which is the only consumer that needs it.
    ///
    /// An operator is never a value in its own right: it exists only as a member of the
    /// `List` its run was flattened into, beside the operands it joins.
    Operator { span: Span },

    /// Space-separated list of values
    List { span: Span, values: Run<CssValue> },

    /// Comma-separated list of values
    CommaSeparated { span: Span, values: Run<CssValue> },
}

impl CssValue {
    /// Get the span of this value
    pub fn span(&self) -> Span {
        match self {
            CssValue::Identifier { span, .. } => *span,
            CssValue::Operator { span, .. } => *span,
            CssValue::String { span, .. } => *span,
            CssValue::Dimension { span } => *span,
            CssValue::Color { span, .. } => *span,
            CssValue::Function { span, .. } => *span,
            CssValue::SupportsCondition { span, .. } => *span,
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
    Number(PackedF64),
    /// Percentage value: 50%, 100%, etc.
    Percentage(PackedF64),
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
/// text of its own.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
pub enum PreludeValue {
    /// Structured values (for @import): a `url()`/string head and `layer`/`supports()`
    /// calls, then the rest of the prelude to its end as one verbatim `Identifier` tail —
    /// the media-query list when the author wrote one, any other value otherwise
    /// (`parse_import_prelude`).
    /// Example: `url('styles.css') layer(base) screen` → [Function(url), Function(layer),
    /// Identifier(screen)]
    Values { values: Run<CssValue>, span: Span },

    /// Raw prelude (for `@keyframes`, `@layer`, `@namespace`, `@page`, … — at-rules with
    /// no `property: value` / media-query grammar). `content` is the **printer-facing**
    /// string: verbatim source with internal whitespace + comments preserved and only
    /// `url()` inner whitespace trimmed (`@namespace` is the exception — its prelude is
    /// whitespace-normalized to match postcss). The public AST is reproduced separately
    /// from `span` (source-verbatim), so `content` never feeds the AST.
    /// Example: `@layer` → `a , b`; `@keyframes` → `my-anim`.
    Raw {
        content: PoolText<CssText>,
        span: Span,
    },

    /// Selector lists (for @scope). Both clauses are independently optional per
    /// css-cascade-6 (`@scope [(<scope-start>)]? [to (<scope-end>)]?`), so a bare
    /// `@scope { … }` has `root: None, limit: None`, and `@scope to (.footer)` has
    /// `root: None, limit: Some(…)`. Each clause's selector list is a
    /// `<forgiving-selector-list>` (the same production `:is()`/`:where()` use), so it
    /// may be empty or hold `Invalid` items — the printer re-emits them verbatim.
    /// Example: `@scope (.card) to (.footer)` → root: Some([.card]), limit: Some([.footer])
    Selectors {
        root: Option<ScopeClause>,
        limit: Option<ScopeLimit>,
        span: Span,
    },

    /// `@custom-selector` (css-extensions-1: `@custom-selector <custom-selector>
    /// <selector-list> ;`): the `:--name` and the strict complex-selector list it
    /// aliases. The name is a span, printed from source — an `<extension-name>` is
    /// author-defined and keeps its case, like `@custom-media`'s. A prelude that is not
    /// this shape (no name, nothing after the name, a list the selector parser rejects)
    /// takes the `Raw` arm instead (`parse_custom_selector_prelude`).
    /// Example: `:--heading h1, h2` → name `:--heading`, list [h1, h2]
    CustomSelector {
        /// The `:--name` span, from the `:` through the ident.
        name: Span,
        list: SelectorList,
        /// The prelude's raw-reader span: first token after the at-rule name to the
        /// `;` / `{` terminator (trailing whitespace included), the same bounds the `Raw`
        /// arm captures, so routing between the two moves no wire byte.
        span: Span,
    },

    /// @supports condition (structured for line-width wrapping)
    /// Example: `(display: grid) and (flex: 1)` → parts connected by `and`/`or`
    Supports {
        condition: ConditionQuery,
        span: Span,
    },

    /// @container query (structured for line-width wrapping)
    /// Example: `sidebar (min-width: 100px) and (max-width: 200px)`
    Container {
        /// Optional container name (e.g., "sidebar"), the name token's **source** spelling
        /// (`\73 idebar` stays escaped): the printer writes it as is and measures the
        /// prelude's head by its length. Printer-only — convert emits the whole prelude from
        /// `span` (raw), not this field.
        name: Option<PoolText<CssText>>,
        /// The condition parts connected by `and`/`or`
        condition: ConditionQuery,
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
    Media {
        content: PoolText<CssText>,
        span: Span,
    },
}

/// One `@scope` clause: a `<forgiving-selector-list>` and its enclosing `(…)` span.
/// Used for the root (`@scope (<scope-start>)`) and, via `ScopeLimit`, the limit.
/// Bundling the list with its paren span makes the printer-only span **structurally**
/// present whenever the list is (no separate `Option<Span>` to keep in lockstep): the
/// paren bounds the gaps where a comment inside the parens but outside the list sits
/// (`@scope (/* c */ .a)`), which the printer re-emits via `comments_to_emit_in_range` — the
/// same wrapping `:is()` args use. Wire-ignored (the prelude is extracted from the
/// `Selectors` `span`, comments stripped).
#[derive(Debug, Clone, Copy)]
pub struct ScopeClause {
    pub list: SelectorList,
    /// The clause's full `(…)` span (end one past `)`, the pseudo-arg convention).
    pub paren: Span,
}

/// The `@scope` limit clause: `to (<scope-end>)`. Carries the `to` keyword span (so the
/// printer can tell a between-clause comment before `to` from an after-`to` one) plus
/// the limit `ScopeClause`. Present iff the source has a `to` clause.
#[derive(Debug, Clone, Copy)]
pub struct ScopeLimit {
    /// The `to` keyword span (a case-insensitive grammar keyword, lowercased at print).
    pub to_span: Span,
    pub clause: ScopeClause,
}

/// A boolean condition query — the structured prelude shared by `@supports`
/// (`<supports-condition>`) and `@container` (`<container-query>`), whose
/// grammars are identical. Structured (rather than raw) so the printer can wrap
/// at `and`/`or` boundaries, keeping the keyword on the current line and the
/// condition on the next.
#[derive(Debug, Clone, Copy)]
pub struct ConditionQuery {
    /// The condition parts connected by `and`/`or`
    pub parts: Run<ConditionPart>,
    /// The run of boolean operators the reader consumed but could not bind, with the
    /// comments between them, in source order and source case (`(a: b) /* c */ and`).
    ///
    /// Every operator in both grammars binds its operand to the **right** — `<media-and>
    /// = and <media-in-parens>`, `<media-not> = not <media-in-parens>` (mediaqueries-4
    /// §"Syntax"), and `<supports-condition> = not <supports-in-parens> |
    /// <supports-in-parens> [ and <supports-in-parens> ]*` (css-conditional-3
    /// §"@supports") — so an operator with nothing after it is the query's own last word
    /// rather than a term of it, and has no part to ride on. It is malformed input, but
    /// the parser is deliberately permissive: dropping the token loses content the
    /// canonical parser keeps in its `prelude`, so the run is carried here instead.
    /// `None` whenever the query bound everything it consumed, which is every well-formed
    /// condition.
    ///
    /// Held as one normalized text rather than a keyword list because the comments among
    /// the operators keep their authored positions (`/* c */ and` and `and /* c */` are
    /// distinct), and it is the query's **span** that makes it whole: the span reaches
    /// past the run, so the wire prelude (`span.extract`) carries it too.
    pub trailing_operators: Option<PoolText<CssText>>,
}

/// One piece of a `ConditionPart`'s content.
///
/// A condition is normalized text — the parser re-emits its tokens with the
/// spacing the grammar calls for — with one exception: the argument of
/// `selector( <complex-selector> )` (css-conditional-4) is a **selector**, so it
/// is parsed as one and printed by the selector printer, exactly as a rule's own
/// selector is. An argument that doesn't parse as a selector never becomes a
/// `Selectors` segment: it is a `<general-enclosed>`, whose contents the grammar
/// leaves undefined, and it stays `Text`.
///
/// The grammar's argument is a single complex selector, but a comma-separated
/// list is read (and printed) as the list it lexically is — the singular
/// production decides whether the *condition* is true, which is not a formatting
/// question.
#[derive(Debug, Clone, Copy)]
pub enum ConditionSegment {
    /// Normalized condition text (`(display: `, `selector(`, `)`, …).
    Text(PoolText<CssText>),
    /// A `selector()` argument, parsed as the selectors it is, with the function's `(…)` span
    /// (end one past the `)`, the pseudo-argument convention) — the bounds of the gaps between
    /// the parens and the list, where the printer claims a boundary run the parser stepped
    /// (`selector(a <NBSP>)`) as it does in a pseudo-class's argument list.
    Selectors { list: SelectorList, paren: Span },
}

/// A single part of a `ConditionQuery` (one `(prop: val)` term, optionally
/// `not`-prefixed or function-style like `selector(...)`).
#[derive(Debug, Clone, Copy)]
pub struct ConditionPart {
    /// The run of connectors this part binds to (None for a first part with no leading
    /// connector).
    pub connector: Option<ConnectorRun>,
    /// The condition content (e.g., `(display: grid)` or `not (color: red)`) as a
    /// run of segments — one `Text` segment unless the part holds a `selector()`
    /// argument. A leading `not` keeps its source case (preserved like the
    /// connectors).
    pub segments: Run<ConditionSegment>,
    /// From the part's first token to its last — never over the gap ahead of it, which the
    /// query reader steps (whitespace, a boundary run, comments) before the part begins.
    pub span: Span,
}

/// The run of connectors (`and`/`or`) a `ConditionPart` binds to: its last connector's kind,
/// its text, and where it sits.
///
/// One value because the three are meaningless apart — a kind with no text, or a span for a
/// run that never landed, are states a part must not be able to hold.
#[derive(Debug, Clone, Copy)]
pub struct ConnectorRun {
    /// The **last** connector of the run, normalized to the `And`/`Or` enum for logic (which
    /// side of it the gap's comments split to).
    pub kind: ConditionConnector,
    /// The run verbatim from source — the words from the first through the last,
    /// single-space joined, with whatever stood *between* them (comments, a boundary run)
    /// carried along in source order.
    ///
    /// Usually one word (`and`/`AND`/`Or`/…), emitted by the printer so the author's case
    /// is preserved (matching prettier). A grammar-legal query has exactly one, but the
    /// reader is permissive and `parseCss` keeps every word of a run
    /// (`(a: b) and or (c: d)`), so dropping all but the last would be content loss. A
    /// comment inside the run is **un-registered** when the run claims it (the `rewind_to`
    /// pairing) — the run's own text prints it, and a registration would have the printer's
    /// gap sweep print it a second time. Comments *outside* the run (before its first word,
    /// after its last) stay registered and are claimed by that sweep, which is what keeps
    /// them on their authored side of the run.
    pub text: PoolText<CssText>,
    /// From the run's first word to its last: the bound that splits the gap between two
    /// parts into the side before the run and the side after it, each of which the printer
    /// claims on its own.
    pub span: Span,
}

/// Connector (`and`/`or`) between `ConditionQuery` parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionConnector {
    And,
    Or,
}

impl PreludeValue {
    pub fn span(&self) -> Span {
        match self {
            PreludeValue::Values { span, .. } => *span,
            PreludeValue::Raw { span, .. } => *span,
            PreludeValue::Selectors { span, .. } => *span,
            PreludeValue::CustomSelector { span, .. } => *span,
            PreludeValue::Supports { span, .. } => *span,
            PreludeValue::Container { span, .. } => *span,
            PreludeValue::Media { span, .. } => *span,
        }
    }
}

/// At-rule (@media, @keyframes, @supports, @import, @layer, @font-face, etc.)
#[derive(Debug, Clone, Copy)]
pub struct CssAtrule {
    /// At-rule name without `@` (e.g., "media", "keyframes"), **escape-decoded**
    /// (spec-canonical `<at-keyword-token>`, CSS Syntax §4.3.3 — `@\6d edia` → `"media"`),
    /// the form every at-rule dispatch compares against. The printer emits `name_span`
    /// instead, and the wire writer half-decodes `name_span` the way parseCss does. Kept
    /// as pool text, NOT recovered from `span`: the span holds the raw escaped
    /// bytes, so a span read would silently un-decode the name for every dispatch. Same
    /// category as `CssDeclaration.property` / `Container.name`.
    pub name: PoolText<CssText>,

    /// Source span of the name token (the `<ident>` after `@`, delimiter-excluded).
    /// The **printer** emits the name from this span, not from decoded `name`: an
    /// escaped name (`@m\A x`) decodes to a raw control char, and emitting that
    /// verbatim injects it into the output (content loss on reparse). Emitting the
    /// source slice preserves the escape, matching every other decoded-name site in
    /// this crate (`CssDeclaration.property`, preludes). The wire writer reads it too,
    /// half-decoding it like a selector name, since parseCss reads an at-rule name with
    /// the same `read_identifier` (an identity escape keeps its backslash there).
    pub name_span: Span,

    /// Prelude value (structured for @import, raw string for others)
    pub prelude: PreludeValue,

    /// Block contents (Some for conditional/descriptor, None for statement at-rules)
    pub block: Option<CssAtruleBlock>,

    pub span: Span,
}

impl CssAtrule {
    /// The at-rule's **public-AST prelude** — the exact string Svelte's parser exposes as
    /// `node.prelude` (its `read_value(...).trim()`, block comments elided). Routes through
    /// the wire-JSON prelude reconstruction ([`convert_prelude_to_string`]), the single
    /// fixture-gated definition of that shape, so a consumer that must agree with the
    /// oracle's `node.prelude` byte-for-byte — the Svelte compiler's `@keyframes` name
    /// collection and its `-global-` prefix test — reads the identical bytes rather than the
    /// printer-facing [`PreludeValue::Raw`] `content`, which is CSS-whitespace-trimmed and
    /// comment-preserving and so diverges on a prelude carrying a comment or JS-only
    /// whitespace (`@keyframes spin /* c */`, a trailing vertical tab / NBSP).
    ///
    /// A pure visibility bridge over the writer's own reconstruction — no parse, printer, or
    /// wire behavior of its own.
    ///
    /// [`convert_prelude_to_string`]: crate::ast::convert::convert_prelude_to_string
    #[cfg(feature = "json")]
    pub fn public_prelude<'src>(&self, source: &'src str) -> std::borrow::Cow<'src, str> {
        convert::convert_prelude_to_string(&self.prelude, source)
    }
}

/// At-rule block - can contain rules, declarations, or nested at-rules
#[derive(Debug, Clone, Copy)]
pub struct CssAtruleBlock {
    /// Block children - mixture depends on at-rule type
    ///
    /// @media, @supports, @layer: rules + nested at-rules
    /// @font-face, @page: declarations only
    ///
    /// One `Run<CssBlockChild>` holds either.
    pub children: Run<CssBlockChild>,
    pub span: Span,
}

/// At-rule block child - can be rule, declaration, or nested at-rule
///
/// The tag is explicit: left to the compiler it folds into a niche of a variant's payload, and
/// every match on a child — the printer walks each block's run of them — then decodes the
/// variant from that payload instead of reading one word.
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
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

// The numeric carriers' 64-bit widths are pinned here under `cfg(test)`, where a toolchain that
// lays a type out differently fails `cargo test`; the wasm32 widths are pinned in `tsv_wasm`.
// `CssValue` is also a dispatch shape: its widest variant, `Color`, fits behind its explicit tag
// byte only while `ColorChannel`'s number is 4-aligned, and a wider `Color` widens every value.
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(size_of::<ColorChannel>() == 12);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(size_of::<Color>() == 52);
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<CssValue>() == 64);
// `CssBlockChild`'s explicit tag widens its row past its widest payload; a niche-tagged child
// would fit in less, and the pin is what catches the tag being dropped.
#[cfg(all(test, target_pointer_width = "64"))]
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<CssBlockChild>() == 112);
// `Verbatim`'s length rides beside `Pooled`'s non-zero start, which tags the two.
const _: () = assert!(tsv_lang::store::STORE_CHECK || size_of::<PropertyName>() == 8);

// An optional list-bearing node folds its absence into its run's niche: an absent `of S`, an
// absent block, an absent connector run cost nothing beside a present one.
#[cfg(test)]
const _: () = {
    assert!(size_of::<Option<SelectorList>>() == size_of::<SelectorList>());
    assert!(size_of::<Option<CssAtruleBlock>>() == size_of::<CssAtruleBlock>());
    assert!(size_of::<Option<ConnectorRun>>() == size_of::<ConnectorRun>());
};
