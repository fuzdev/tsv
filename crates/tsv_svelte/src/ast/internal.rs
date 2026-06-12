// Svelte internal AST types
//
// Internal representation optimized for manipulation and formatting.
// Uses string interning for efficient storage and comparison of identifiers.

use std::borrow::Cow;

use string_interner::DefaultSymbol;
use tsv_css::ast::internal::CssStyleSheet;
pub use tsv_lang::{Comment, SharedInterner, Span};
use tsv_ts::ast::internal::{Expression, Program};

/// Svelte Root - top-level AST node
///
/// Represents a complete Svelte component with template, scripts, and styles.
/// Contains optional instance script, module script, and style sections.
#[derive(Debug, Clone)]
pub struct Root {
    pub fragment: Fragment,
    pub instance: Option<Box<Script>>,
    pub module: Option<Box<Script>>,
    pub css: Option<Box<Style>>,
    /// `<svelte:options>` configuration (not part of fragment)
    pub options: Option<SvelteOptions>,
    /// All comments from scripts and template expressions.
    /// Use `comments_in_range(span)` to find comments for a specific node.
    pub comments: Vec<Comment>,
    pub span: Span,
    pub interner: SharedInterner,
}

/// Svelte Fragment - container for template nodes
///
/// A fragment contains a sequence of template nodes (elements, text, expressions).
/// Used both at the root level and as children of elements.
#[derive(Debug, Clone)]
pub struct Fragment {
    pub nodes: Vec<FragmentNode>,
}

/// Svelte template node types
///
/// Represents the different kinds of nodes that can appear in a Svelte template.
#[derive(Debug, Clone)]
pub enum FragmentNode {
    Element(Element),
    SpecialElement(SpecialElement),
    ExpressionTag(ExpressionTag),
    Text(Text),
    Comment(HtmlComment),
    IfBlock(IfBlock),
    EachBlock(EachBlock),
    AwaitBlock(AwaitBlock),
    KeyBlock(KeyBlock),
    SnippetBlock(SnippetBlock),
    HtmlTag(HtmlTag),
    ConstTag(ConstTag),
    DebugTag(DebugTag),
    RenderTag(RenderTag),
}

/// HTML comment node: <!-- content -->
///
/// Represents an HTML comment in the template. The `content` field contains
/// the raw content between `<!--` and `-->`, with whitespace preserved exactly.
///
/// Note: This uses `content` internally for consistency with `tsv_lang::Comment`
/// and `CssComment`. The public AST uses `data` (Svelte's naming) via conversion.
#[derive(Debug, Clone)]
pub struct HtmlComment {
    pub content: String, // Content between <!-- and -->
    pub span: Span,
}

/// Svelte IfBlock - conditional rendering
///
/// Represents {#if test}...{:else if test}...{:else}...{/if} blocks.
/// The `elseif` field is true for {:else if} branches (nested in alternate).
#[derive(Debug, Clone)]
pub struct IfBlock {
    pub elseif: bool,
    pub test: Expression,
    pub consequent: Fragment,
    pub alternate: Option<Fragment>,
    pub span: Span,
    /// Span of the opening tag `{#if ... }` or `{:else if ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte EachBlock - list iteration
///
/// Represents {#each expression as context, index (key)}...{:else}...{/each} blocks.
/// Also supports {#each expression} and {#each expression, index} without `as`.
#[derive(Debug, Clone)]
pub struct EachBlock {
    pub expression: Expression,
    pub context: Option<Expression>, // Pattern (identifier or destructuring), None if no `as`
    pub index: Option<String>,
    pub key: Option<Expression>,
    /// Span of the key including parentheses `(key)` for comment lookup
    pub key_span: Option<Span>,
    pub body: Fragment,
    pub fallback: Option<Fragment>,
    pub span: Span,
    /// Span of the opening tag `{#each ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte AwaitBlock - promise handling
///
/// Represents {#await expression}...{:then value}...{:catch error}...{/await} blocks.
/// Also supports shorthand: {#await expression then value}...{/await}
#[derive(Debug, Clone)]
pub struct AwaitBlock {
    pub expression: Expression,
    pub value: Option<Expression>, // Pattern for :then binding
    pub error: Option<Expression>, // Pattern for :catch binding
    pub pending: Option<Fragment>,
    pub then: Option<Fragment>,
    pub catch: Option<Fragment>,
    pub span: Span,
    /// Span of the opening tag `{#await ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte KeyBlock - keyed updates
///
/// Represents {#key expression}...{/key} blocks.
/// Forces re-creation of contents when expression changes.
#[derive(Debug, Clone)]
pub struct KeyBlock {
    pub expression: Expression,
    pub fragment: Fragment,
    pub span: Span,
    /// Span of the opening tag `{#key ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte SnippetBlock - reusable template snippets
///
/// Represents {#snippet name(params)}...{/snippet} blocks.
/// Defines a reusable chunk of markup that can be rendered with {@render}.
#[derive(Debug, Clone)]
pub struct SnippetBlock {
    pub expression: Expression,          // Snippet name (Identifier)
    pub type_parameters: Option<String>, // Generic type params, e.g., "T" for <T>
    pub parameters: Vec<Expression>, // Function parameters (patterns) - may be empty if raw_parameters is set
    pub raw_parameters: Option<String>, // Raw parameter string for TypeScript (when type annotations present)
    pub body: Fragment,
    pub span: Span,
    /// Span of the opening tag `{#snippet ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte HtmlTag - raw HTML injection
///
/// Represents {@html expression} tags.
/// Injects raw HTML content without escaping.
#[derive(Debug, Clone)]
pub struct HtmlTag {
    pub expression: Expression,
    pub span: Span,
}

/// Svelte ConstTag - local constant declaration
///
/// Represents {@const name = expression} tags.
/// Declares a local constant within a block scope.
/// The `id` is the pattern (identifier or destructuring) and `init` is the value.
#[derive(Debug, Clone)]
pub struct ConstTag {
    pub id: Expression,   // Pattern (identifier or destructuring)
    pub init: Expression, // Initializer expression
    pub span: Span,
}

/// Svelte DebugTag - debugging helper
///
/// Represents {@debug} or {@debug x, y, z} tags.
/// Triggers debugger when any listed variable changes.
/// Empty identifiers array means "debug all state".
///
/// Note: Unlike Prettier (which strips comments), we preserve TS comments
/// within debug tags. Comments are stored in `Root.comments` and looked
/// up by span during formatting. This is an intentional divergence.
#[derive(Debug, Clone)]
pub struct DebugTag {
    pub identifiers: Vec<Expression>, // List of identifiers to debug
    pub span: Span,
}

/// Svelte RenderTag - snippet rendering
///
/// Represents {@render fn()} or {@render fn?.()} tags.
/// Renders a snippet, optionally with arguments.
#[derive(Debug, Clone)]
pub struct RenderTag {
    pub expression: Expression, // CallExpression or ChainExpression
    pub span: Span,
}

/// Svelte AttachTag - element attachment
///
/// Represents {@attach expr} inside element opening tags.
/// Attaches reactive functions to elements (Svelte 5.29+).
#[derive(Debug, Clone)]
pub struct AttachTag {
    pub expression: Expression,
    pub span: Span,
}

//
// Directives
//

/// OnDirective - event handler (`on:click={handler}`)
///
/// Event handlers can have modifiers like `preventDefault`, `stopPropagation`, etc.
/// When no expression is provided (rare), expression is null.
#[derive(Debug, Clone)]
pub struct OnDirective {
    pub name: String,                   // Event name: "click", "keydown", etc.
    pub expression: Option<Expression>, // Handler function
    pub modifiers: Vec<String>,         // "preventDefault", "stopPropagation", etc.
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// BindDirective - two-way binding (`bind:value={name}`)
///
/// Bindings connect a property to a variable. When shorthand (`bind:value`),
/// an identifier with the same name is auto-generated as the expression.
#[derive(Debug, Clone)]
pub struct BindDirective {
    pub name: String,           // Property name: "value", "checked", "this", etc.
    pub expression: Expression, // Binding target (always present - auto-generated for shorthand)
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None for shorthand bindings)
    pub expression_tag_span: Option<Span>,
}

/// ClassDirective - conditional class (`class:class1={cond}`)
///
/// Applies a class conditionally based on an expression.
/// When shorthand (`class:class1`), an identifier with the same name is auto-generated.
#[derive(Debug, Clone)]
pub struct ClassDirective {
    pub name: String,           // Class name: "class1", "class2", etc.
    pub expression: Expression, // Condition (always present - auto-generated for shorthand)
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None for shorthand)
    pub expression_tag_span: Option<Span>,
}

/// StyleDirective - inline style (`style:color={value}`)
///
/// Sets a CSS property value. Unlike other directives, uses `value` instead of `expression`
/// because it can be a string value, not just an expression.
/// When shorthand (`style:color`), value is `true` (boolean).
#[derive(Debug, Clone)]
pub struct StyleDirective {
    pub name: String,               // CSS property: "color", "--custom", etc.
    pub value: StyleDirectiveValue, // true, ExpressionTag, or mixed text/expressions
    pub modifiers: Vec<String>,     // "important"
    pub span: Span,
    pub name_span: Span,
}

/// Value of a style directive
#[derive(Debug, Clone)]
pub enum StyleDirectiveValue {
    /// Shorthand: `style:color` (uses variable with same name)
    True,
    /// Pure expression: `style:color={value}`
    ExpressionTag(ExpressionTag),
    /// Mixed value (string with possible expressions): `style:color="red"`
    Parts(Vec<AttributeValue>),
}

/// UseDirective - action (`use:action={params}`)
///
/// Actions are functions that run when an element is mounted.
#[derive(Debug, Clone)]
pub struct UseDirective {
    pub name: String,                   // Action name: "action", "tooltip", etc.
    pub expression: Option<Expression>, // Parameters passed to the action
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// Direction of a transition directive
///
/// Encodes the three valid states instead of two booleans:
/// - `Both`: bidirectional transition (`transition:fade`)
/// - `In`: intro only (`in:fly`)
/// - `Out`: outro only (`out:slide`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionDirection {
    /// Bidirectional: `transition:name` - runs on both enter and exit
    Both,
    /// Intro only: `in:name` - runs only on enter
    In,
    /// Outro only: `out:name` - runs only on exit
    Out,
}

impl TransitionDirection {
    /// Returns the directive prefix for this direction
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Both => "transition",
            Self::In => "in",
            Self::Out => "out",
        }
    }

    /// Returns the directive prefix with colon (e.g., "transition:")
    pub const fn prefix_with_colon(self) -> &'static str {
        match self {
            Self::Both => "transition:",
            Self::In => "in:",
            Self::Out => "out:",
        }
    }

    /// Returns true if this includes intro (enter) animation
    pub const fn has_intro(self) -> bool {
        matches!(self, Self::Both | Self::In)
    }

    /// Returns true if this includes outro (exit) animation
    pub const fn has_outro(self) -> bool {
        matches!(self, Self::Both | Self::Out)
    }
}

/// TransitionDirective - transition (`transition:fade`, `in:fly`, `out:slide`)
///
/// Controls enter/exit animations. Can be bidirectional (transition:) or unidirectional (in:/out:).
#[derive(Debug, Clone)]
pub struct TransitionDirective {
    pub name: String,                   // Transition name: "fade", "fly", "slide", etc.
    pub expression: Option<Expression>, // Transition parameters
    pub modifiers: Vec<String>,         // "local", "global"
    pub direction: TransitionDirection, // Which animations to run
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// AnimateDirective - animation (`animate:flip={params}`)
///
/// FLIP animations for list items.
#[derive(Debug, Clone)]
pub struct AnimateDirective {
    pub name: String,                   // Animation name: "flip", etc.
    pub expression: Option<Expression>, // Animation parameters
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// LetDirective - slot prop (`let:item={localItem}`)
///
/// Receives values from a slot. The expression is the local binding pattern.
#[derive(Debug, Clone)]
pub struct LetDirective {
    pub name: String,                   // Slot prop name: "item", "index", etc.
    pub expression: Option<Expression>, // Local binding pattern (Identifier, ArrayPattern, ObjectPattern)
    pub span: Span,
    pub name_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

//
// Special Elements
//

/// Tag identifier for special elements (used during parsing before data is available)
///
/// This is a simple Copy enum used to identify the kind of special element
/// before we've parsed the `this` attribute. After parsing, use `SpecialElementKind`
/// which includes the associated data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialElementTag {
    SvelteHead,
    SvelteWindow,
    SvelteBody,
    SvelteDocument,
    SvelteElement,
    SvelteComponent,
    SvelteSelf,
    SlotElement,
    SvelteFragment,
    SvelteBoundary,
    TitleElement,
}

impl SpecialElementTag {
    /// Try to parse a tag name into a special element tag
    ///
    /// Note: `title` is only TitleElement when inside `<svelte:head>`,
    /// which must be checked by the caller.
    pub fn from_tag_name(name: &str, in_svelte_head: bool) -> Option<Self> {
        match name {
            "svelte:head" => Some(Self::SvelteHead),
            "svelte:window" => Some(Self::SvelteWindow),
            "svelte:body" => Some(Self::SvelteBody),
            "svelte:document" => Some(Self::SvelteDocument),
            "svelte:element" => Some(Self::SvelteElement),
            "svelte:component" => Some(Self::SvelteComponent),
            "svelte:self" => Some(Self::SvelteSelf),
            "slot" => Some(Self::SlotElement),
            "svelte:fragment" => Some(Self::SvelteFragment),
            "svelte:boundary" => Some(Self::SvelteBoundary),
            "title" if in_svelte_head => Some(Self::TitleElement),
            _ => None,
        }
    }

    /// Returns the tag name as it appears in source code
    #[inline]
    pub const fn tag_name(self) -> &'static str {
        match self {
            Self::SvelteHead => "svelte:head",
            Self::SvelteWindow => "svelte:window",
            Self::SvelteBody => "svelte:body",
            Self::SvelteDocument => "svelte:document",
            Self::SvelteElement => "svelte:element",
            Self::SvelteComponent => "svelte:component",
            Self::SvelteSelf => "svelte:self",
            Self::SlotElement => "slot",
            Self::SvelteFragment => "svelte:fragment",
            Self::SvelteBoundary => "svelte:boundary",
            Self::TitleElement => "title",
        }
    }
}

/// Kind of Svelte special element
///
/// These are elements with special behavior in Svelte:
/// - Document injection: `<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`
/// - Dynamic elements: `<svelte:element>`, `<svelte:component>`, `<svelte:self>`
/// - Content slots: `<slot>`, `<svelte:fragment>`
/// - Error handling: `<svelte:boundary>`
/// - Semantic HTML: `<title>` (inside svelte:head)
///
/// Variants that require additional data (SvelteElement, SvelteComponent) carry it
/// directly, eliminating the need for Option fields on the parent struct.
#[derive(Debug, Clone)]
pub enum SpecialElementKind {
    /// `<svelte:head>` - inject content into document head
    SvelteHead,
    /// `<svelte:window>` - bind to window events/properties
    SvelteWindow,
    /// `<svelte:body>` - bind to body events
    SvelteBody,
    /// `<svelte:document>` - bind to document events
    SvelteDocument,
    /// `<svelte:element this={tag}>` - dynamic element tag
    SvelteElement { tag: Expression },
    /// `<svelte:component this={Component}>` - dynamic component (legacy)
    SvelteComponent { expression: Expression },
    /// `<svelte:self>` - recursive self-reference
    SvelteSelf,
    /// `<slot>` - content slot
    SlotElement,
    /// `<svelte:fragment>` - wrapper for slot content
    SvelteFragment,
    /// `<svelte:boundary>` - error boundary (Svelte 5)
    SvelteBoundary,
    /// `<title>` - semantic title element (inside svelte:head)
    TitleElement,
}

impl SpecialElementKind {
    /// Whether this special element is a block-level element (forces line breaks).
    ///
    /// Block elements: `svelte:head`, `svelte:window`, `svelte:body`, `svelte:document`
    /// — these bind to global objects and don't participate in inline flow.
    ///
    /// Inline elements: `slot`, `svelte:element`, `svelte:component`, `svelte:self`,
    /// `svelte:fragment`, `svelte:boundary`, `title` — these render content inline.
    #[inline]
    pub const fn is_block(&self) -> bool {
        matches!(
            self,
            Self::SvelteHead | Self::SvelteWindow | Self::SvelteBody | Self::SvelteDocument
        )
    }

    /// Returns the tag name as it appears in source code
    #[inline]
    pub const fn tag_name(&self) -> &'static str {
        match self {
            Self::SvelteHead => "svelte:head",
            Self::SvelteWindow => "svelte:window",
            Self::SvelteBody => "svelte:body",
            Self::SvelteDocument => "svelte:document",
            Self::SvelteElement { .. } => "svelte:element",
            Self::SvelteComponent { .. } => "svelte:component",
            Self::SvelteSelf => "svelte:self",
            Self::SlotElement => "slot",
            Self::SvelteFragment => "svelte:fragment",
            Self::SvelteBoundary => "svelte:boundary",
            Self::TitleElement => "title",
        }
    }

    /// Returns the AST node type name for JSON output
    #[inline]
    pub const fn node_type(&self) -> &'static str {
        match self {
            Self::SvelteHead => "SvelteHead",
            Self::SvelteWindow => "SvelteWindow",
            Self::SvelteBody => "SvelteBody",
            Self::SvelteDocument => "SvelteDocument",
            Self::SvelteElement { .. } => "SvelteElement",
            Self::SvelteComponent { .. } => "SvelteComponent",
            Self::SvelteSelf => "SvelteSelf",
            Self::SlotElement => "SlotElement",
            Self::SvelteFragment => "SvelteFragment",
            Self::SvelteBoundary => "SvelteBoundary",
            Self::TitleElement => "TitleElement",
        }
    }

    /// Get the tag expression for SvelteElement
    pub fn tag(&self) -> Option<&Expression> {
        match self {
            Self::SvelteElement { tag } => Some(tag),
            _ => None,
        }
    }

    /// Get the component expression for SvelteComponent
    pub fn expression(&self) -> Option<&Expression> {
        match self {
            Self::SvelteComponent { expression } => Some(expression),
            _ => None,
        }
    }
}

/// Svelte Special Element
///
/// Represents special Svelte elements that have unique behavior:
/// - `<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`
/// - `<svelte:element>` (dynamic tag), `<svelte:component>` (dynamic component)
/// - `<svelte:self>`, `<slot>`, `<svelte:fragment>`, `<svelte:boundary>`
/// - `<title>` (when inside `<svelte:head>`)
///
/// Variant-specific data (tag for SvelteElement, expression for SvelteComponent)
/// is stored in the `SpecialElementKind` enum, not as Option fields here.
#[derive(Debug, Clone)]
pub struct SpecialElement {
    pub kind: SpecialElementKind,
    pub attributes: Vec<AttributeNode>,
    pub fragment: Fragment,
    pub span: Span,
    pub name_span: Span,
    /// Position of the `>` that closes the opening tag.
    /// Used by the printer to find trailing comments between the last attribute and `>`.
    pub open_tag_end: u32,
}

/// Svelte Options
///
/// Represents `<svelte:options>` which configures component behavior.
/// Stored separately from the fragment in `Root.options`.
#[derive(Debug, Clone)]
pub struct SvelteOptions {
    pub attributes: Vec<AttributeNode>,
    pub span: Span,
}

impl FragmentNode {
    pub fn span(&self) -> Span {
        match self {
            FragmentNode::Element(elem) => elem.span,
            FragmentNode::SpecialElement(elem) => elem.span,
            FragmentNode::ExpressionTag(tag) => tag.span,
            FragmentNode::Text(text) => text.span,
            FragmentNode::Comment(comment) => comment.span,
            FragmentNode::IfBlock(block) => block.span,
            FragmentNode::EachBlock(block) => block.span,
            FragmentNode::AwaitBlock(block) => block.span,
            FragmentNode::KeyBlock(block) => block.span,
            FragmentNode::SnippetBlock(block) => block.span,
            FragmentNode::HtmlTag(tag) => tag.span,
            FragmentNode::ConstTag(tag) => tag.span,
            FragmentNode::DebugTag(tag) => tag.span,
            FragmentNode::RenderTag(tag) => tag.span,
        }
    }

    /// Check if this node is whitespace-only text.
    ///
    /// Returns true only for Text nodes containing only whitespace characters.
    /// All other node types return false.
    #[inline]
    pub fn is_whitespace_only_text(&self) -> bool {
        matches!(self, FragmentNode::Text(t) if t.raw.trim().is_empty())
    }

    /// Check if this node is a whitespace-only text containing at least one newline.
    ///
    /// Used to detect source line breaks at element boundaries (hug mode pattern).
    /// Returns false for non-Text nodes or Text without newlines.
    #[inline]
    pub fn is_boundary_break(&self) -> bool {
        matches!(self, FragmentNode::Text(t) if t.raw.trim().is_empty() && t.raw.contains('\n'))
    }
}

/// Svelte Element kind - distinguishes HTML elements from components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "convert", derive(serde::Serialize, serde::Deserialize))]
pub enum ElementKind {
    /// HTML element: `<div>`, `<span>`, `<input>`, etc. (lowercase first character)
    #[cfg_attr(feature = "convert", serde(rename = "Html"))]
    Html,
    /// Svelte component: `<MyComponent>`, `<Button>`, etc. (uppercase first character)
    #[cfg_attr(feature = "convert", serde(rename = "Component"))]
    Component,
}

/// Svelte Element - HTML/component tag
///
/// Represents an HTML element or Svelte component in the template.
/// Elements have a name, attributes, and child nodes in a fragment.
#[derive(Debug, Clone)]
pub struct Element {
    pub name: DefaultSymbol,
    pub kind: ElementKind,
    pub attributes: Vec<AttributeNode>,
    pub fragment: Fragment,
    pub span: Span,
    pub name_span: Span,
    /// Position of the `>` that closes the opening tag.
    /// Used by the printer to find trailing comments between the last attribute and `>`.
    pub open_tag_end: u32,
}

/// Svelte Attribute - element attribute
///
/// Represents an attribute on an element, e.g., `class="foo"` or `disabled`.
/// The value is optional (for boolean attributes) and can contain text or expressions.
///
/// Shorthand attributes like `{a}` (equivalent to `a={a}`) are represented as
/// Attribute with name="a" and value containing an ExpressionTag with Identifier "a".
/// Detection is implicit: check if name matches expression identifier.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: DefaultSymbol,
    pub value: Option<Vec<AttributeValue>>,
    pub span: Span,
    pub name_span: Span,
}

/// Svelte SpreadAttribute - spread object as attributes
///
/// Represents `{...obj}` syntax that spreads an object's properties as attributes.
/// The expression can be any valid expression: identifier, call, member access, etc.
#[derive(Debug, Clone)]
pub struct SpreadAttribute {
    pub expression: Expression,
    pub span: Span,
}

/// Svelte attribute-like node
///
/// Elements can have various attribute-like constructs:
/// - `Attribute`: Standard `name="value"` or `name={expr}` attributes (including shorthand `{a}`)
/// - `SpreadAttribute`: `{...obj}` spreads object properties as attributes
/// - `AttachTag`: `{@attach expr}` attachments (Svelte 5.29+)
/// - Directives: `on:`, `bind:`, `class:`, `style:`, `use:`, `transition:`, `in:`, `out:`, `animate:`, `let:`
#[derive(Debug, Clone)]
pub enum AttributeNode {
    Attribute(Attribute),
    SpreadAttribute(SpreadAttribute),
    AttachTag(AttachTag),
    // Directives
    OnDirective(OnDirective),
    BindDirective(BindDirective),
    ClassDirective(ClassDirective),
    StyleDirective(StyleDirective),
    UseDirective(UseDirective),
    TransitionDirective(TransitionDirective),
    AnimateDirective(AnimateDirective),
    LetDirective(LetDirective),
}

impl AttributeNode {
    /// Get the span of this attribute node
    pub fn span(&self) -> Span {
        match self {
            AttributeNode::Attribute(a) => a.span,
            AttributeNode::SpreadAttribute(s) => s.span,
            AttributeNode::AttachTag(t) => t.span,
            AttributeNode::OnDirective(d) => d.span,
            AttributeNode::BindDirective(d) => d.span,
            AttributeNode::ClassDirective(d) => d.span,
            AttributeNode::StyleDirective(d) => d.span,
            AttributeNode::UseDirective(d) => d.span,
            AttributeNode::TransitionDirective(d) => d.span,
            AttributeNode::AnimateDirective(d) => d.span,
            AttributeNode::LetDirective(d) => d.span,
        }
    }
}

/// Svelte Attribute value part
///
/// Attribute values can contain static text or dynamic expressions.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    Text(Text),
    ExpressionTag(ExpressionTag),
}

/// Svelte Text node - raw text content
///
/// Represents static text in the template or attribute values.
/// In attribute values, this represents the unquoted string content.
///
/// Stores only `raw` (the original text with HTML entities: `&lt;`, `&#65;`);
/// the decoded form (`<`, `A`) is computed lazily via `Text::data`. The vast
/// majority of real-world text nodes contain no entities, so `data()` borrows
/// `raw` without allocating on that fast path.
///
/// TODO(performance): Printer repeatedly calls is_whitespace_only() on text nodes in
/// hot loops (multiline children, inline run detection). Could cache this as a bool field
/// computed during parsing: `pub is_whitespace_only: bool`. Trade-off: 1 byte per Text
/// node vs repeated string scans. Profile before optimizing.
#[derive(Debug, Clone)]
pub struct Text {
    pub raw: String, // Raw text with HTML entities: "&lt;", "&#65;"
    /// Which entity decode `data()` applies, fixed at parse time by context.
    pub decoding: TextDecoding,
    pub span: Span,
}

/// Entity-decode context for a `Text` node, mirroring the decode the canonical
/// Svelte parser applies when it materializes `data` from `raw`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDecoding {
    /// Fragment/template text — decode with text-content rules.
    Fragment,
    /// Quoted attribute value — decode with attribute-context rules
    /// (stricter semicolon handling for named entities).
    AttributeValue,
    /// No decode — `data` is identical to `raw` (raw-content element text;
    /// also unquoted attribute values, see the TODO at the construction site).
    Raw,
}

impl Text {
    /// Decoded text (`&lt;` → `<`, `&#65;` → `A`), computed lazily from `raw`.
    ///
    /// Borrows `raw` when no `&` is present (no entity possible, decode is
    /// identity) or when the node's context applies no decode.
    pub fn data(&self) -> Cow<'_, str> {
        let is_attribute_value = match self.decoding {
            TextDecoding::Raw => return Cow::Borrowed(&self.raw),
            TextDecoding::Fragment => false,
            TextDecoding::AttributeValue => true,
        };
        if self.raw.contains('&') {
            Cow::Owned(tsv_html::decode_character_references(
                &self.raw,
                is_attribute_value,
            ))
        } else {
            Cow::Borrowed(&self.raw)
        }
    }
}

/// Svelte ExpressionTag - {expression} in template
///
/// Represents a TypeScript/JS expression embedded in the template.
/// The expression is evaluated and its result is rendered.
#[derive(Debug, Clone)]
pub struct ExpressionTag {
    pub expression: Expression,
    pub span: Span,
}

/// Svelte Script block - <script> tag contents
///
/// Contains a TypeScript/JS program and metadata about the script tag.
/// The `context` field distinguishes between instance and module scripts.
#[derive(Debug, Clone)]
pub struct Script {
    pub content: Program,
    pub attributes: Vec<AttributeNode>,
    pub context: ScriptContext,
    pub span: Span,
}

/// Script context type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ScriptContext {
    Default = 0, // <script>
    Module = 1,  // <script context="module">
}

impl ScriptContext {
    /// Returns the context string for JSON output
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            ScriptContext::Default => "default",
            ScriptContext::Module => "module",
        }
    }
}

/// Svelte Style block - <style> tag contents
///
/// Stores the span of the entire <style> tag and the content span.
/// Style tag with parsed CSS content
#[derive(Debug, Clone)]
pub struct Style {
    pub span: Span,         // Full <style>...</style> span
    pub content_span: Span, // Just the CSS text inside the tags
    pub attributes: Vec<AttributeNode>,
    pub css_stylesheet: CssStyleSheet, // Parsed CSS stylesheet (nodes + value comments)
}
