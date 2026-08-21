// Svelte internal AST types
//
// Internal representation optimized for manipulation and formatting.
// Element/attribute names are span-identity — recovered from `source[name_span]`
// on demand, never stored (see `Element::name` / `Attribute::name`).
//
// ## Arena allocation
//
// Like `tsv_ts`, the Svelte AST is allocated in a per-parse [`bumpalo::Bump`]
// supplied by the caller. Recursive children are `&'arena T<'arena>` (not
// `Box`), child collections are `&'arena [T<'arena>]` (not `Vec`), and decoded /
// raw strings are `&'arena str` (not `String`) — so a whole parse (template plus
// the embedded TS `<script>`/`{expr}` and CSS `<style>` ASTs, which share the
// same `Bump`) is one bump-allocated graph, freed wholesale when the `Bump`
// drops. `Style` holds a `CssStyleSheet<'arena>` borrowing that shared arena.
// Leaf nodes that hold only `Span`/primitives carry no lifetime
// (`HtmlComment`, `Text`, `SvelteOptions`, the `*Tag`-with-no-expression nodes).

use std::borrow::Cow;

use tsv_css::ast::internal::CssStyleSheet;
pub use tsv_lang::{Comment, Span};
use tsv_ts::ast::internal::{Expression, Program, TSTypeParameterDeclaration, VariableDeclaration};

/// Svelte Root - top-level AST node
///
/// Represents a complete Svelte component with template, scripts, and styles.
/// Contains optional instance script, module script, and style sections.
#[derive(Debug, Clone)]
pub struct Root<'arena> {
    pub fragment: Fragment<'arena>,
    pub instance: Option<&'arena Script<'arena>>,
    pub module: Option<&'arena Script<'arena>>,
    pub css: Option<&'arena Style<'arena>>,
    /// `<svelte:options>` configuration (not part of fragment)
    pub options: Option<SvelteOptions<'arena>>,
    /// All comments from scripts and template expressions.
    /// Use `comments_to_emit_in_range(span)` to find comments for a specific node.
    pub comments: Vec<Comment>,
}

/// Svelte Fragment - container for template nodes
///
/// A fragment contains a sequence of template nodes (elements, text, expressions).
/// Used both at the root level and as children of elements.
#[derive(Debug, Clone)]
pub struct Fragment<'arena> {
    pub nodes: &'arena [FragmentNode<'arena>],
}

/// Svelte template node types
///
/// Represents the different kinds of nodes that can appear in a Svelte template.
///
/// All variants are inline by value: the layout favors traversal locality over
/// node size (boxing the fat variants added a pointer-chase on hot format-read
/// paths that cost more than the slice-density win).
#[derive(Debug, Clone)]
pub enum FragmentNode<'arena> {
    Element(Element<'arena>),
    SpecialElement(SpecialElement<'arena>),
    ExpressionTag(ExpressionTag<'arena>),
    Text(Text),
    Comment(HtmlComment),
    IfBlock(IfBlock<'arena>),
    EachBlock(EachBlock<'arena>),
    AwaitBlock(AwaitBlock<'arena>),
    KeyBlock(KeyBlock<'arena>),
    SnippetBlock(SnippetBlock<'arena>),
    HtmlTag(HtmlTag<'arena>),
    ConstTag(ConstTag<'arena>),
    DeclarationTag(DeclarationTag<'arena>),
    DebugTag(DebugTag<'arena>),
    RenderTag(RenderTag<'arena>),
}

/// HTML comment node: <!-- content -->
///
/// Represents an HTML comment in the template. `content_span` is the span of the
/// raw content between `<!--` and `-->` (whitespace preserved exactly) in the host
/// source; recover the text via `HtmlComment::content`. A pure sub-slice — no
/// decode — so it is a `Span`, not an owned `String` (mirrors
/// `tsv_lang::Comment::content_span`).
///
/// Note: `content` mirrors `tsv_lang::Comment` and `CssComment` for naming
/// consistency. The public AST uses `data` (Svelte's naming) via conversion.
#[derive(Debug, Clone)]
pub struct HtmlComment {
    /// Span of the content between `<!--` and `-->` in the host source; text via `content`.
    pub content_span: Span,
    pub span: Span,
}

impl HtmlComment {
    /// Content between `<!--` and `-->` — a sub-slice of `source`, no allocation.
    /// `source` must be the host document the spans were recorded against.
    pub fn content<'s>(&self, source: &'s str) -> &'s str {
        self.content_span.extract(source)
    }
}

/// Svelte IfBlock - conditional rendering
///
/// Represents {#if test}...{:else if test}...{:else}...{/if} blocks.
/// The `elseif` field is true for {:else if} branches (nested in alternate).
#[derive(Debug, Clone)]
pub struct IfBlock<'arena> {
    pub elseif: bool,
    pub test: Expression<'arena>,
    pub consequent: Fragment<'arena>,
    pub alternate: Option<Fragment<'arena>>,
    pub span: Span,
    /// Span of the opening tag `{#if ... }` or `{:else if ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte EachBlock - list iteration
///
/// Represents {#each expression as context, index (key)}...{:else}...{/each} blocks.
/// Also supports {#each expression} and {#each expression, index} without `as`.
#[derive(Debug, Clone)]
pub struct EachBlock<'arena> {
    pub expression: Expression<'arena>,
    pub context: Option<Expression<'arena>>, // Pattern (identifier or destructuring), None if no `as`
    pub index: Option<&'arena str>,
    pub key: Option<Expression<'arena>>,
    /// Span of the key including parentheses `(key)` for comment lookup
    pub key_span: Option<Span>,
    pub body: Fragment<'arena>,
    pub fallback: Option<Fragment<'arena>>,
    pub span: Span,
    /// Span of the opening tag `{#each ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte AwaitBlock - promise handling
///
/// Represents {#await expression}...{:then value}...{:catch error}...{/await} blocks.
/// Also supports shorthand: {#await expression then value}...{/await}
#[derive(Debug, Clone)]
pub struct AwaitBlock<'arena> {
    pub expression: Expression<'arena>,
    pub value: Option<Expression<'arena>>, // Pattern for :then binding
    pub error: Option<Expression<'arena>>, // Pattern for :catch binding
    /// The pending-phase **content** (`{#await x}<here>{:then}…`), or `None` when
    /// empty. Distinct from `pending_block`: an empty block-form pending is `None`
    /// here but `pending_block == true`. The printer reads this (an empty pending
    /// full form collapses to the `then`/`catch` shorthand, matching prettier).
    pub pending: Option<Fragment<'arena>>,
    /// Whether the block form was used (`{#await x}…{/await}`) vs the inline
    /// `then`/`catch` shorthand (`{#await x then v}` / `{#await x catch e}`). The
    /// block form always has a pending Fragment — empty or not — matching Svelte's
    /// `block.pending = create_fragment()`; the shorthand has `pending: null`. The
    /// writer emits `{Fragment, nodes: []}` vs `null` from this flag (the wire
    /// distinction the formatter's shorthand-collapse erases). See
    /// `ast/convert/write.rs::write_await_block`.
    pub pending_block: bool,
    pub then: Option<Fragment<'arena>>,
    pub catch: Option<Fragment<'arena>>,
    pub span: Span,
    /// Span of the opening tag `{#await ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte KeyBlock - keyed updates
///
/// Represents {#key expression}...{/key} blocks.
/// Forces re-creation of contents when expression changes.
#[derive(Debug, Clone)]
pub struct KeyBlock<'arena> {
    pub expression: Expression<'arena>,
    pub fragment: Fragment<'arena>,
    pub span: Span,
    /// Span of the opening tag `{#key ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte SnippetBlock - reusable template snippets
///
/// Represents {#snippet name(params)}...{/snippet} blocks.
/// Defines a reusable chunk of markup that can be rendered with {@render}.
#[derive(Debug, Clone)]
pub struct SnippetBlock<'arena> {
    pub expression: Expression<'arena>, // Snippet name (Identifier)
    /// Parsed generic type parameters (`<T extends X = Y>`), routed through
    /// `tsv_ts`'s type-parameter printer for constraint/default/modifier
    /// handling and width-based wrapping. `Some` whenever `type_params_raw` is
    /// — a signature the parser cannot read is a parse error, not a node.
    pub type_parameters: Option<TSTypeParameterDeclaration<'arena>>,
    /// Raw inner text of the generics (`T extends X` for `<T extends X>`),
    /// always set when generics are present. Feeds the public AST's `typeParams`
    /// string, matching Svelte's parser (which stores it raw too).
    pub type_params_raw: Option<&'arena str>,
    pub parameters: &'arena [Expression<'arena>], // Function parameters (patterns)
    /// Source span of the parameter parens: `start` is the `(`, `end` is the `)`
    /// (for leading / dangling / trailing comment lookup when printing parameters).
    /// `None` only if no `(` was found (malformed).
    pub params_paren: Option<Span>,
    pub body: Fragment<'arena>,
    pub span: Span,
    /// Span of the opening tag `{#snippet ... }` for comment lookup
    pub opening_tag_span: Span,
}

/// Svelte HtmlTag - raw HTML injection
///
/// Represents {@html expression} tags.
/// Injects raw HTML content without escaping.
#[derive(Debug, Clone)]
pub struct HtmlTag<'arena> {
    pub expression: Expression<'arena>,
    pub span: Span,
}

/// Svelte ConstTag - local constant declaration
///
/// Represents {@const name = expression} tags.
/// Declares a local constant within a block scope.
/// The `id` is the pattern (identifier or destructuring) and `init` is the value.
#[derive(Debug, Clone)]
pub struct ConstTag<'arena> {
    pub id: Expression<'arena>,   // Pattern (identifier or destructuring)
    pub init: Expression<'arena>, // Initializer expression
    pub span: Span,
}

/// Svelte DeclarationTag - local `{const …}` / `{let …}` declaration
///
/// The bare `{const name = expr}` / `{let name = expr}` tags (no `@`). The body is
/// a TS `VariableDeclaration` parsed, printed, and converted by `tsv_ts`, so
/// multiple declarators, comments, and every bracket/string case are handled
/// natively. (`{@const}` is a separate `ConstTag` on its own path.)
#[derive(Debug, Clone)]
pub struct DeclarationTag<'arena> {
    pub declaration: VariableDeclaration<'arena>,
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
pub struct DebugTag<'arena> {
    pub identifiers: &'arena [Expression<'arena>], // List of identifiers to debug
    pub span: Span,
}

/// Svelte RenderTag - snippet rendering
///
/// Represents {@render fn()} or {@render fn?.()} tags.
/// Renders a snippet, optionally with arguments.
#[derive(Debug, Clone)]
pub struct RenderTag<'arena> {
    pub expression: Expression<'arena>, // CallExpression or ChainExpression
    pub span: Span,
}

/// Svelte AttachTag - element attachment
///
/// Represents {@attach expr} inside element opening tags.
/// Attaches reactive functions to elements (Svelte 5.29+).
#[derive(Debug, Clone)]
pub struct AttachTag<'arena> {
    pub expression: Expression<'arena>,
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
pub struct OnDirective<'arena> {
    /// Span of the directive name only (e.g. "click"). A **verbatim** source slice —
    /// HTML/Svelte attribute names are never entity-decoded — recovered via
    /// `name_span.extract(source)`. Distinct from `head_span` below, which is the whole
    /// directive head token ("on:click|preventDefault", prefix + name + modifiers).
    pub name_span: Span,
    pub expression: Option<Expression<'arena>>, // Handler function
    pub modifiers: &'arena [&'arena str],       // "preventDefault", "stopPropagation", etc.
    pub span: Span,
    /// Span of the whole directive head (`on:click|preventDefault`); used as `name_loc`.
    pub head_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// BindDirective - two-way binding (`bind:value={name}`)
///
/// Bindings connect a property to a variable. When shorthand (`bind:value`),
/// an identifier with the same name is auto-generated as the expression.
#[derive(Debug, Clone)]
pub struct BindDirective<'arena> {
    /// Span of the property name only (e.g. "value") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Expression<'arena>, // Binding target (always present - auto-generated for shorthand)
    pub modifiers: &'arena [&'arena str], // Unofficial — no official modifier support; preserved verbatim
    pub span: Span,
    pub head_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None for shorthand bindings)
    pub expression_tag_span: Option<Span>,
}

/// ClassDirective - conditional class (`class:class1={cond}`)
///
/// Applies a class conditionally based on an expression.
/// When shorthand (`class:class1`), an identifier with the same name is auto-generated.
#[derive(Debug, Clone)]
pub struct ClassDirective<'arena> {
    /// Span of the class name only (e.g. "class1") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Expression<'arena>, // Condition (always present - auto-generated for shorthand)
    pub modifiers: &'arena [&'arena str], // Unofficial — no official modifier support; preserved verbatim
    pub span: Span,
    pub head_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None for shorthand)
    pub expression_tag_span: Option<Span>,
}

/// StyleDirective - inline style (`style:color={value}`)
///
/// Sets a CSS property value. Unlike other directives, uses `value` instead of `expression`
/// because it can be a string value, not just an expression.
/// When shorthand (`style:color`), value is `true` (boolean).
#[derive(Debug, Clone)]
pub struct StyleDirective<'arena> {
    /// Span of the CSS property name only (e.g. "color") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub value: StyleDirectiveValue<'arena>, // true, ExpressionTag, or mixed text/expressions
    pub modifiers: &'arena [&'arena str],   // "important"
    pub span: Span,
    pub head_span: Span,
}

/// Value of a style directive
#[derive(Debug, Clone)]
pub enum StyleDirectiveValue<'arena> {
    /// Shorthand: `style:color` (uses variable with same name)
    True,
    /// Pure expression: `style:color={value}`
    ExpressionTag(ExpressionTag<'arena>),
    /// Mixed value (string with possible expressions): `style:color="red"`
    Parts(&'arena [AttributeValue<'arena>]),
}

/// UseDirective - action (`use:action={params}`)
///
/// Actions are functions that run when an element is mounted.
#[derive(Debug, Clone)]
pub struct UseDirective<'arena> {
    /// Span of the action name only (e.g. "action") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Option<Expression<'arena>>, // Parameters passed to the action
    pub modifiers: &'arena [&'arena str], // Unofficial — no official modifier support; preserved verbatim
    pub span: Span,
    pub head_span: Span,
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
pub struct TransitionDirective<'arena> {
    /// Span of the transition name only (e.g. "fade") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Option<Expression<'arena>>, // Transition parameters
    pub modifiers: &'arena [&'arena str],       // "local", "global"
    pub direction: TransitionDirection,         // Which animations to run
    pub span: Span,
    pub head_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// AnimateDirective - animation (`animate:flip={params}`)
///
/// FLIP animations for list items.
#[derive(Debug, Clone)]
pub struct AnimateDirective<'arena> {
    /// Span of the animation name only (e.g. "flip") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Option<Expression<'arena>>, // Animation parameters
    pub modifiers: &'arena [&'arena str], // Unofficial — no official modifier support; preserved verbatim
    pub span: Span,
    pub head_span: Span,
    /// Span of the expression tag `{...}` for comment lookup (None if no expression)
    pub expression_tag_span: Option<Span>,
}

/// LetDirective - slot prop (`let:item={localItem}`)
///
/// Receives values from a slot. The expression is the local binding pattern.
#[derive(Debug, Clone)]
pub struct LetDirective<'arena> {
    /// Span of the slot-prop name only (e.g. "item") — verbatim source slice (see `OnDirective`).
    pub name_span: Span,
    pub expression: Option<Expression<'arena>>, // Local binding pattern (Identifier, ArrayPattern, ObjectPattern)
    pub modifiers: &'arena [&'arena str], // Unofficial — no official modifier support; preserved verbatim
    pub span: Span,
    pub head_span: Span,
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
    /// Try to parse a tag name into a special element tag.
    ///
    /// Two classifications are ancestor-context-dependent, so the caller supplies its parse
    /// state (mirroring Svelte's stack walks in `1-parse/state/element.js`):
    /// - `title` is `TitleElement` only inside `<svelte:head>` (`parent_is_head`).
    /// - `slot` is `SlotElement` only when *not* inside a `<template shadowrootmode>`
    ///   (`parent_is_shadowroot_template`); there it's an ordinary `RegularElement`, so this
    ///   returns `None` and the caller parses it on the regular-element path.
    pub fn from_tag_name(
        name: &str,
        in_svelte_head: bool,
        in_shadowroot_template: bool,
    ) -> Option<Self> {
        match name {
            "svelte:head" => Some(Self::SvelteHead),
            "svelte:window" => Some(Self::SvelteWindow),
            "svelte:body" => Some(Self::SvelteBody),
            "svelte:document" => Some(Self::SvelteDocument),
            "svelte:element" => Some(Self::SvelteElement),
            "svelte:component" => Some(Self::SvelteComponent),
            "svelte:self" => Some(Self::SvelteSelf),
            "slot" if !in_shadowroot_template => Some(Self::SlotElement),
            "svelte:fragment" => Some(Self::SvelteFragment),
            "svelte:boundary" => Some(Self::SvelteBoundary),
            "title" if in_svelte_head => Some(Self::TitleElement),
            _ => None,
        }
    }

    /// Whether `name` is one of the `svelte:*` meta tags — Svelte's `meta_tags.has(name)`
    /// (`1-parse/state/element.js`), the gate that makes the `svelte:` namespace reserved.
    ///
    /// Derived from [`Self::from_tag_name`] rather than a second hand-written list, so the two
    /// cannot drift. The caller gates on the `svelte:` prefix first, which makes the two
    /// context flags irrelevant here: neither `slot` nor `title` is namespaced. The one meta
    /// tag with no `SpecialElementTag` is `svelte:options` — it fills `Root`'s single `Option`
    /// slot instead of becoming a fragment node, so the root dispatch takes it before element
    /// parsing ever sees it.
    pub fn is_meta_tag_name(name: &str) -> bool {
        name == "svelte:options" || Self::from_tag_name(name, false, false).is_some()
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

    /// Whether a `this` attribute on this tag binds the element rather than staying an
    /// ordinary attribute.
    ///
    /// Only these two consume one, and only the first — on every other special element (and
    /// on any later `this` here) the name has no meaning beyond being an attribute.
    #[inline]
    pub const fn takes_this(self) -> bool {
        matches!(self, Self::SvelteElement | Self::SvelteComponent)
    }
}

/// The `this=` binding of `<svelte:element>`, in the two forms Svelte lets it be spelled.
///
/// Svelte's public AST holds the expression bare (`SvelteElement.tag`) with no
/// `ExpressionTag` around it. Keeping the tag here instead of unwrapping to the expression
/// is what preserves the `{…}` span, and with it the `{`→expression and expression→`}`
/// gaps: a comment lives in either (`this={/* c */ tag}`), and an unwrapped expression
/// leaves nowhere to look for one.
///
/// `<svelte:component>` does **not** use this type: it accepts only the braced form (a
/// non-`{expression}` `this` is a parse error), so it carries an `ExpressionTag` directly.
/// The asymmetry is Svelte's — `<svelte:element this="div">` merely *warns*, deliberately
/// preserving Svelte 4 behavior, where the component errors.
#[derive(Debug, Clone)]
pub enum SpecialThis<'arena> {
    /// `this={expr}` — the expression form. The tag's span covers the braces, so the
    /// printer emits it as the `{…}` attribute value it is.
    Braced(ExpressionTag<'arena>),
    /// `this="value"` — the plain HTML-attribute form. No braces (so no gap a comment could
    /// occupy) and **no expression parse at all**: the value is the attribute's decoded
    /// text, held as such. Svelte's wire reports it as a `Literal`, which the writer emits
    /// from these two fields directly — synthesizing an `Expression::Literal` here only to
    /// take it apart again at every consumer bought nothing but an unreachable arm in each
    /// of their matches.
    Plain {
        /// Decoded attribute text (entities resolved, quotes stripped).
        content: &'arena str,
        /// Span of that text in source — no quotes, so `raw` is not recoverable from it.
        span: Span,
    },
}

impl<'arena> SpecialThis<'arena> {
    /// Span of the bound value: the braced form's expression, or the plain form's text.
    /// Both sit hard against whatever delimits them, so this is the region a comment can
    /// be adjacent to.
    pub fn span(&self) -> Span {
        match self {
            Self::Braced(tag) => tag.expression.span(),
            Self::Plain { span, .. } => *span,
        }
    }

    /// The `{…}` span of the braced form, braces included; `None` for the plain-string
    /// form, which has no braces and therefore no gap a comment could occupy.
    pub fn braces(&self) -> Option<Span> {
        match self {
            Self::Braced(tag) => Some(tag.span),
            Self::Plain { .. } => None,
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
pub enum SpecialElementKind<'arena> {
    /// `<svelte:head>` - inject content into document head
    SvelteHead,
    /// `<svelte:window>` - bind to window events/properties
    SvelteWindow,
    /// `<svelte:body>` - bind to body events
    SvelteBody,
    /// `<svelte:document>` - bind to document events
    SvelteDocument,
    /// `<svelte:element this={tag}>` - dynamic element tag
    SvelteElement { tag: SpecialThis<'arena> },
    /// `<svelte:component this={Component}>` - dynamic component (legacy). Always the
    /// braced form: a missing or non-`{expression}` `this` is rejected at parse, as Svelte
    /// does — so unlike `<svelte:element>` there is no plain-string variant to model.
    SvelteComponent { expression: ExpressionTag<'arena> },
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

impl<'arena> SpecialElementKind<'arena> {
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

    /// Whether this kind's **content** prints verbatim — the whitespace-sensitive class, which
    /// for regular elements is `tsv_html::preserves_whitespace` (`<pre>` / `<textarea>`).
    ///
    /// Exactly one special kind joins it. A `TitleElement` — `<title>` as a (transparent) child
    /// of `<svelte:head>` — is compiled by visitors that walk `fragment.nodes` **directly**
    /// (`$$renderer.title(…)` on the server, a `document.title` assignment on the client), so
    /// `clean_nodes` never runs over its children and every byte between the tags reaches the
    /// page. The two predicates together are the whole verbatim set, and every reader of "does
    /// the printer own this whitespace?" must consult both — the printer's dispatch and the
    /// audits that mirror it alike, since an audit excluding a different set than the printer
    /// emits either accuses the author's bytes or blinds itself to the printer's.
    ///
    /// ⚠️ This is about the element's **interior**. The `clean_nodes` **hoist** is a separate
    /// fact that still applies to a `TitleElement`: it is lifted out of its parent fragment, so
    /// the run between it and a *sibling* is a fragment edge and is deleted.
    ///
    /// Readers walking the public wire AST ask the same question of a node's `"TitleElement"`
    /// type string, there being no kind enum on that side.
    #[inline]
    pub const fn preserves_content_whitespace(&self) -> bool {
        matches!(self, Self::TitleElement)
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

    /// The `<svelte:element>` `this` binding — the wire's `tag` field.
    ///
    /// Yields the whole [`SpecialThis`], not just its expression, because the two forms
    /// serialize differently: the plain-string `this="x"` is a fused Svelte-style `Literal`,
    /// the braced `this={x}` an island. Which form it is, is the binding's own structure —
    /// the writer must not re-derive it from the source bytes.
    pub fn tag(&self) -> Option<&SpecialThis<'arena>> {
        match self {
            Self::SvelteElement { tag } => Some(tag),
            _ => None,
        }
    }

    /// The `<svelte:component>` `this` binding — the wire's `expression` field.
    ///
    /// The whole tag, not the bare expression: its `{…}` span is what bounds the expression's
    /// comment-attach window, and handing out the expression without it is how comments
    /// outside the braces came to attach to it.
    pub fn expression(&self) -> Option<&ExpressionTag<'arena>> {
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
pub struct SpecialElement<'arena> {
    pub kind: SpecialElementKind<'arena>,
    pub attributes: &'arena [AttributeNode<'arena>],
    pub fragment: Fragment<'arena>,
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
pub struct SvelteOptions<'arena> {
    pub attributes: &'arena [AttributeNode<'arena>],
    pub span: Span,
    /// End of the `svelte:options` tag name — where the attribute list's first gap begins.
    /// Kept from the parse rather than derived from `span.start` + the tag's length, so no
    /// reader has to assume how the name was spelled.
    pub name_end: u32,
    /// The `>` closing the opening tag; with `name_end` it bounds the region the attribute
    /// list's comments live in. `span.end` is not it: the paired form
    /// (`<svelte:options></svelte:options>`) runs on to the closing tag.
    pub open_tag_end: u32,
}

impl<'arena> FragmentNode<'arena> {
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
            FragmentNode::DeclarationTag(tag) => tag.span,
            FragmentNode::DebugTag(tag) => tag.span,
            FragmentNode::RenderTag(tag) => tag.span,
        }
    }

    /// Check if this node is whitespace-only text.
    ///
    /// Returns true only for Text nodes whose content is entirely *collapsible*
    /// whitespace `[ \t\n\r]` ([`is_collapsible_ws`]). A non-breaking space
    /// (U+00A0 / U+202F), a form feed, or any other separator CSS does not collapse
    /// is template *content*, not collapsible whitespace, so a node made only of
    /// those returns false. All non-Text nodes return false.
    ///
    /// Reads the precomputed `Text::is_collapsible_ws_only` flag — O(1), source-free.
    #[inline]
    pub fn is_whitespace_only_text(&self) -> bool {
        matches!(self, FragmentNode::Text(t) if t.is_collapsible_ws_only)
    }

    /// Check if this node is a whitespace-only text containing at least one newline.
    ///
    /// Used to detect source line breaks at element boundaries (hug mode pattern).
    /// "Whitespace-only" is the collapsible (ASCII) class — see `is_whitespace_only_text`;
    /// a node with a non-breaking space is content, not a boundary break. Returns false
    /// for non-Text nodes or Text without newlines. Reads precomputed flags — source-free.
    #[inline]
    pub fn is_boundary_break(&self) -> bool {
        matches!(self, FragmentNode::Text(t) if t.is_collapsible_ws_only && t.has_newline())
    }

    /// Whether this node is a **declaration** — `{@const}`, `{const …}`, `{let …}`, or a
    /// `{#snippet}` block.
    ///
    /// The nodes that declare a binding and render nothing. Because the compiler
    /// [hoists](FragmentNode::is_hoisted_from_fragment) them out of the fragment before it applies
    /// the whitespace rules, the break beside one is render-free, and the printer spends that
    /// licence on giving each its own line — see `Printer::is_own_line_declaration`.
    ///
    /// A `{#snippet}` belongs here on both counts: it declares a binding (the snippet name) and
    /// renders nothing where it is written — `<C>docs{#snippet icon()}…{/snippet}</C>` compiles
    /// byte-identically with the snippet broken onto its own line
    /// (`../test-svelte-prettier-whitespace/hoisted-tags.md` carries the full oracle matrix).
    ///
    /// ⚠️ `{@debug}` is hoisted alike but is **not** a declaration: it is a transient debugging
    /// aid, so it keeps the edge *trim* the hoist also licenses rather than a line of its own.
    #[inline]
    pub fn is_declaration(&self) -> bool {
        matches!(
            self,
            FragmentNode::ConstTag(_)
                | FragmentNode::DeclarationTag(_)
                | FragmentNode::SnippetBlock(_)
        )
    }

    /// Whether the compiler **hoists** this node out of its fragment before it applies the
    /// whitespace rules — `clean_nodes`' `hoisted` list.
    ///
    /// Such a node is invisible to those rules, so the text beside it is the fragment's real
    /// first/last node and the run between them is a render-free *edge* run rather than an
    /// inter-sibling one: `{#if c}text {@const x = 1}{/if}` compiles to `text`, exactly like the
    /// glued authoring, where `{#if c}text <b>y</b>{/if}` keeps its space. The printer's boundary
    /// analysis therefore has to skip these nodes when it asks "am I at the content boundary?"
    /// ([`FragmentNode::content_bounds`]).
    ///
    /// ⚠️ The hoist is **not** one of Svelte 5's three published whitespace rules (collapse
    /// between nodes / trim at the edges / `<pre>` exempt) — those say nothing about which nodes
    /// the edge is measured against. It lives only in `clean_nodes`, so it is verified against
    /// the compiler rather than the summary; see
    /// [hoisted_boundary_convergence](../../../../tests/fixtures/svelte/blocks/hoisted_boundary_convergence_prettier_divergence/).
    ///
    /// ⚠️ Scoped to the **edges**. With content on both sides the hoist makes neither run an
    /// edge: the two runs merge into a single rendered space (`a {@const} b` → `a b`), so a space
    /// must survive there and gluing would be a different document.
    ///
    /// ⚠️ **Deliberately NARROWER than the oracle's list**, which also holds `SvelteHead` /
    /// `SvelteWindow` / `SvelteBody` / `SvelteDocument`. Those four are **block-classified**
    /// here, so `handle_block_child` gives each its own line — and a line break at a fragment
    /// edge is itself render-free, so trimming it and breaking it are both correct for the
    /// render but cannot both happen: `<svelte:body … />b` trims to the glued form, whose next
    /// pass re-breaks it, forever (a real F1 2-cycle the fuzz gate caught). Their own line is
    /// the better form — a `<svelte:head>` welded to its neighbour would be the alternative —
    /// so the printer keeps the break and declines the trim. The four are excluded HERE rather
    /// than at each reader, so the two rules cannot re-collide at a new call site.
    ///
    /// ⚠️ **The hoist licenses two different things, and only one of them is this trim.** The break
    /// beside a hoisted node is render-free for the same reason its edge run is, so trimming the
    /// run and breaking it are both correct and only one can happen. A **declaration**
    /// ([`FragmentNode::is_declaration`] — the tags and the `{#snippet}` block) spends the licence
    /// on the break — it takes its own line (`Printer::is_own_line_declaration`), which is where
    /// authors already write declarations — and reaches the trim only through that break.
    /// `DebugTag` and `TitleElement` are what still trim: no layout rule gives either a line of
    /// its own.
    ///
    /// So the five kinds stay in this ONE set even though they split on layout: every reader here
    /// is asking the compiler's question ("does this node stand between the content and the
    /// fragment edge?"), and the answer is the same for all five — including for the glue scan
    /// behind `is_own_line_declaration`, where a hoisted neighbour is not content.
    ///
    /// The layout split does make [`FragmentNode::content_bounds`] *redundant* for a declaration —
    /// the whole fixture suite stays green with those kinds treated as content there, because the
    /// run beside an own-line declaration is trimmed by `handle_text_child`'s own arms before the
    /// bounds are consulted. Narrowing it would state something false about the compiler to
    /// record which reader happens to fire first, so the set answers the compiler's question and
    /// the readers keep their own.
    #[inline]
    pub fn is_hoisted_from_fragment(&self) -> bool {
        match self {
            FragmentNode::ConstTag(_)
            | FragmentNode::DeclarationTag(_)
            | FragmentNode::DebugTag(_)
            | FragmentNode::SnippetBlock(_) => true,
            FragmentNode::SpecialElement(se) => {
                matches!(se.kind, SpecialElementKind::TitleElement)
            }
            _ => false,
        }
    }

    /// The index range of `nodes` that the whitespace rules see — the first and last node that
    /// is **not** [hoisted](FragmentNode::is_hoisted_from_fragment), or `None` when every node is.
    ///
    /// Computed once per fragment and compared against, rather than asked per node: a
    /// `nodes[..i].all(is_hoisted)` test at each child would be O(n²) over a fragment, on a
    /// question whose answer two bounds settle.
    pub fn content_bounds(nodes: &[FragmentNode<'_>]) -> Option<(usize, usize)> {
        let first = nodes.iter().position(|n| !n.is_hoisted_from_fragment())?;
        let last = nodes.iter().rposition(|n| !n.is_hoisted_from_fragment())?;
        Some((first, last))
    }
}

/// Svelte Element kind - distinguishes HTML elements from components.
///
/// Classification-only (the printer's block/inline decision + the writer's
/// `RegularElement`/`Component` `type` tag, both by `match`); never serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    /// HTML element: `<div>`, `<span>`, `<input>`, etc. (lowercase first character)
    Html,
    /// Svelte component: `<MyComponent>`, `<Button>`, etc. (uppercase first character)
    Component,
}

/// Whether a tag `name` is a Svelte **component** rather than an HTML element.
///
/// A `:`-namespaced tag (`foo:bar`, `Foo:bar`) is never a component — Svelte's
/// `regex_valid_component_name` excludes `:`, so a namespaced name is a `RegularElement` even
/// with an uppercase prefix. Otherwise: a dotted tag (member access, e.g. `ns.Comp`,
/// `Object.component`) is a component, as is any name whose first character is uppercase
/// (Unicode, so `\p{Lu}` such as `Δ` / `Я` counts, not just ASCII). Mirrors Svelte's
/// `regex_valid_component_name` (`1-parse/state/element.js`): uppercase-first with optional dots,
/// or any `ID_Start`-first name with one or more dotted segments.
///
/// The single source for component-ness: the parser reads it to set [`ElementKind::Component`],
/// and the printer's tag classification reads it too (the printer's separate `NAMESPACED` bit
/// carries the `foo:bar` self-close term). One predicate keeps the two from drifting — the
/// printer must not classify a Unicode-uppercase component as a plain inline element and strip
/// its self-close.
///
/// Examples: `Comp` → true, `ns.Comp` → true, `Object.component` → true, `div` → false,
/// `foo:bar` → false, `Foo:bar` → false.
pub(crate) fn is_component_name(name: &str) -> bool {
    !name.contains(':')
        && (name.contains('.') || name.chars().next().is_some_and(char::is_uppercase))
}

/// Every classification fact derivable from a tag *name* alone, packed into a `u16` and computed
/// once at parse (stored on [`Element::facts`], read back by the printer's element/fragment/
/// sibling paths).
///
/// Nothing element-instance-specific lives here — a `<script>`'s has-content overlay and the
/// `Component`/`Block`/`Inline` element-kind split both stay in the printer. Those paths re-ask
/// the same tag-name questions many times per element, and every answer is a pure function of the
/// name, so computing them once (where the raw `&str` is already in hand) turns each print-time
/// read into a single field load. The exhaustive equivalence test below grades each accessor
/// against the pure predicate it encodes — a mispacked bit changes layout only on rare tags at
/// rare widths, which no fixture or corpus diff can be relied on to see.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TagFacts(u16);

impl TagFacts {
    /// `tsv_html::is_block_element` (flow content).
    const BLOCK: u16 = 1 << 0;
    /// `tsv_html::is_void_element` (`<br>`, `<img>`, `!doctype`).
    const VOID: u16 = 1 << 1;
    /// `tsv_html::is_foreign_element` (SVG or MathML).
    const FOREIGN: u16 = 1 << 2;
    /// Component-shaped name — [`is_component_name`] (Unicode-uppercase initial or a dotted member
    /// name — `Button`, `Δcomp`, `foo.bar`). Drives the `Component` element kind. A `:`-namespaced
    /// name (`foo:bar`) is a `RegularElement`, not a component, so it is *not* here — it carries
    /// [`NAMESPACED`](Self::NAMESPACED) instead.
    const COMPONENT_NAME: u16 = 1 << 3;
    const STYLE: u16 = 1 << 4;
    const SCRIPT: u16 = 1 << 5;
    const TEMPLATE: u16 = 1 << 6;
    /// `tsv_html::preserves_whitespace` (`<pre>`, `<textarea>`).
    const WS_SENSITIVE: u16 = 1 << 7;
    /// `<!DOCTYPE>`-style declaration (leading `!`), which closes with `>`, not `/>`.
    const DECLARATION: u16 = 1 << 8;
    /// A `:` in the name (`<foo:bar>`) — a namespaced `RegularElement`. Independent of
    /// [`COMPONENT_NAME`](Self::COMPONENT_NAME): it takes the inline element kind like any other
    /// non-block regular element, but may still print self-closing (prettier's `didSelfClose`), so
    /// it is the third contributor to `can_self_close` alongside component and foreign.
    const NAMESPACED: u16 = 1 << 9;
    /// `tsv_html::collapses_child_whitespace` — a whitespace-collapsing container (`<table>`,
    /// `<select>`, …) whose inter-sibling whitespace the compiler removes entirely.
    const WS_COLLAPSING: u16 = 1 << 10;

    /// Derive the facts from the tag name. The single source: [`Element::facts`] stores exactly
    /// this, and the equivalence test grades every accessor against the predicates named here.
    pub(crate) fn compute(tag_name: &str) -> Self {
        let mut bits: u16 = 0;
        if tsv_html::is_block_element(tag_name) {
            bits |= Self::BLOCK;
        }
        if tsv_html::is_void_element(tag_name) {
            bits |= Self::VOID;
        }
        if tsv_html::is_foreign_element(tag_name) {
            bits |= Self::FOREIGN;
        }
        if is_component_name(tag_name) {
            bits |= Self::COMPONENT_NAME;
        }
        if tag_name.contains(':') {
            bits |= Self::NAMESPACED;
        }
        if tag_name == "style" {
            bits |= Self::STYLE;
        }
        if tag_name == "script" {
            bits |= Self::SCRIPT;
        }
        if tag_name == "template" {
            bits |= Self::TEMPLATE;
        }
        if tsv_html::preserves_whitespace(tag_name) {
            bits |= Self::WS_SENSITIVE;
        }
        if tsv_html::collapses_child_whitespace(tag_name) {
            bits |= Self::WS_COLLAPSING;
        }
        if tag_name.starts_with('!') {
            bits |= Self::DECLARATION;
        }
        Self(bits)
    }

    pub(crate) fn is_block(self) -> bool {
        self.0 & Self::BLOCK != 0
    }
    pub(crate) fn is_void(self) -> bool {
        self.0 & Self::VOID != 0
    }
    pub(crate) fn is_foreign(self) -> bool {
        self.0 & Self::FOREIGN != 0
    }
    pub(crate) fn is_component_name(self) -> bool {
        self.0 & Self::COMPONENT_NAME != 0
    }
    pub(crate) fn is_namespaced(self) -> bool {
        self.0 & Self::NAMESPACED != 0
    }
    pub(crate) fn is_style(self) -> bool {
        self.0 & Self::STYLE != 0
    }
    pub(crate) fn is_script(self) -> bool {
        self.0 & Self::SCRIPT != 0
    }
    pub(crate) fn is_template(self) -> bool {
        self.0 & Self::TEMPLATE != 0
    }
    pub(crate) fn is_ws_sensitive(self) -> bool {
        self.0 & Self::WS_SENSITIVE != 0
    }
    pub(crate) fn collapses_child_whitespace(self) -> bool {
        self.0 & Self::WS_COLLAPSING != 0
    }
    pub(crate) fn is_declaration(self) -> bool {
        self.0 & Self::DECLARATION != 0
    }
}

/// Svelte Element - HTML/component tag
///
/// Represents an HTML element or Svelte component in the template.
/// Elements have a name, attributes, and child nodes in a fragment.
#[derive(Debug, Clone)]
pub struct Element<'arena> {
    pub kind: ElementKind,
    /// Name-derived classification, computed once at parse (see [`TagFacts`]). Occupies padding
    /// beside `kind`, so it costs no extra size; the printer reads it instead of re-deriving.
    /// Crate-internal like [`TagFacts`] — derived from `name`, not part of the public wire AST.
    pub(crate) facts: TagFacts,
    pub attributes: &'arena [AttributeNode<'arena>],
    pub fragment: Fragment<'arena>,
    pub span: Span,
    pub name_span: Span,
    /// Position of the `>` that closes the opening tag.
    /// Used by the printer to find trailing comments between the last attribute and `>`.
    pub open_tag_end: u32,
}

impl Element<'_> {
    /// The tag name — span-identity, the verbatim `source[name_span]` slice
    /// (tag names never carry surrounding whitespace).
    #[inline]
    pub fn name<'s>(&self, source: &'s str) -> &'s str {
        &source[self.name_span.range()]
    }

    /// Whether this is a `<template>` whose `lang` / `type` names a language other than HTML
    /// — the elements whose CONTENT the formatter copies out verbatim rather than formatting.
    ///
    /// The one definition of that composite. It has three callers across two crates (the
    /// element builder, the sibling-`>` dangle's eligibility test, and `tsv_debug`'s razor
    /// audit, which must agree with the printer about which bodies are the author's bytes),
    /// and each of them used to spell it out itself — three copies of a predicate whose every
    /// clause is a drift risk, down to which bytes the `lang` value is read off (see
    /// [`lang_attribute`]: the decoded text, not the raw bytes). The template question stays on
    /// the parse-time [`TagFacts`] bitfield, so the printer's hot paths keep the field load
    /// rather than re-comparing the tag name.
    pub fn is_foreign_template(&self, source: &str) -> bool {
        self.facts.is_template()
            && lang_attribute(self.attributes, source).is_some_and(|lang| lang != "html")
    }
}

/// The `lang` or `type` attribute value from an attribute list, `text/` prefix stripped
/// (`type="text/less"` → `"less"`). `None` when neither attribute is present.
///
/// Reads the **decoded** value ([`Text::data`]), never the raw bytes: the language a
/// `lang`/`type` names is what the attribute *means*, and the parser already computed that —
/// `lang="&#99;ss"` is `css`, and a raw read routes it to the preserve-verbatim path instead
/// of the CSS printer. Prettier's `getLangAttribute` reads its `data` for the same reason.
/// The decode borrows the source slice whenever the value carries no `&` (the overwhelming
/// case), so the `Cow` costs nothing there; only an entity-bearing value allocates.
///
/// ⚠️ This is the **formatting** question, and it is the only one that decodes. The wire
/// schema asks a different one — "is this component TypeScript?" — whose oracle is Svelte's
/// own `this.ts`, a regex over the RAW template bytes (`1-parse/index.js`:
/// `lang=(["'])?([^"' >]+)` compared against `ts`). So `<script lang="&#116;s">` is a
/// TypeScript body to *this* reader and NOT TypeScript to the wire, and `convert`'s
/// `script_lang` deliberately stays a separate raw-keyed reader rather than sharing this one.
/// Pinned by `tests/fixtures/svelte/attributes/lang_entity`, which holds both answers.
pub(crate) fn lang_attribute<'s>(
    attributes: &[AttributeNode<'_>],
    source: &'s str,
) -> Option<Cow<'s, str>> {
    for attr_node in attributes {
        if let AttributeNode::Attribute(attr) = attr_node {
            let name = attr.name(source);
            if (name == "lang" || name == "type")
                && let Some(value_parts) = attr.value
            {
                for part in value_parts {
                    if let AttributeValue::Text(text) = part {
                        return Some(narrow_lang_value(text.data(source)));
                    }
                }
            }
        }
    }
    None
}

/// Trim a decoded `lang` / `type` value and strip its `text/` prefix, keeping the borrow when
/// the decode kept one. Both narrowings are sub-slices, so the owned arm re-owns the remainder
/// rather than the whole value — reached only by an entity-bearing attribute.
fn narrow_lang_value(value: Cow<'_, str>) -> Cow<'_, str> {
    fn narrow(raw: &str) -> &str {
        let lang = raw.trim();
        lang.strip_prefix("text/").unwrap_or(lang)
    }
    match value {
        Cow::Borrowed(raw) => Cow::Borrowed(narrow(raw)),
        Cow::Owned(raw) => Cow::Owned(narrow(&raw).to_owned()),
    }
}

/// `facts` rides in the tail padding beside `kind`, so the parse-time classification costs no
/// extra `Element` size. Guards that property against a future field reorder that would spill it.
/// 64-bit only — the slice fields are half-width on wasm32, a different layout.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<Element<'static>>() == 56);

/// Svelte Attribute - element attribute
///
/// Represents an attribute on an element, e.g., `class="foo"` or `disabled`.
/// The value is optional (for boolean attributes) and can contain text or expressions.
///
/// Shorthand attributes like `{a}` (equivalent to `a={a}`) are represented as
/// Attribute with name="a" and value containing an ExpressionTag with Identifier "a".
/// Detection is implicit: check if name matches expression identifier.
#[derive(Debug, Clone)]
pub struct Attribute<'arena> {
    pub value: Option<&'arena [AttributeValue<'arena>]>,
    pub span: Span,
    pub name_span: Span,
}

impl Attribute<'_> {
    /// The attribute name — span-identity, the verbatim `source[name_span]` slice.
    /// Every attribute name is a bare source run, a padded `{ shorthand }`
    /// included: its `name_span` is the identifier alone, not the braces interior
    /// (see the parser's `parse_shorthand_attribute`).
    #[inline]
    pub fn name<'s>(&self, source: &'s str) -> &'s str {
        &source[self.name_span.range()]
    }
}

/// Svelte SpreadAttribute - spread object as attributes
///
/// Represents `{...obj}` syntax that spreads an object's properties as attributes.
/// The expression can be any valid expression: identifier, call, member access, etc.
#[derive(Debug, Clone)]
pub struct SpreadAttribute<'arena> {
    pub expression: Expression<'arena>,
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
pub enum AttributeNode<'arena> {
    Attribute(Attribute<'arena>),
    SpreadAttribute(SpreadAttribute<'arena>),
    AttachTag(AttachTag<'arena>),
    // Directives
    OnDirective(OnDirective<'arena>),
    BindDirective(BindDirective<'arena>),
    ClassDirective(ClassDirective<'arena>),
    StyleDirective(StyleDirective<'arena>),
    UseDirective(UseDirective<'arena>),
    TransitionDirective(TransitionDirective<'arena>),
    AnimateDirective(AnimateDirective<'arena>),
    LetDirective(LetDirective<'arena>),
}

impl<'arena> AttributeNode<'arena> {
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
pub enum AttributeValue<'arena> {
    Text(Text),
    ExpressionTag(ExpressionTag<'arena>),
}

/// Whether `b` is **collapsible whitespace** — the characters CSS white-space processing
/// actually acts on, and therefore the ones a formatter may add, drop or respell without
/// changing what renders: `[ \t\n\r]`.
///
/// ⚠️ This is **narrower than Rust's `is_ascii_whitespace`**, which includes the form feed
/// `\x0c`, and the difference is not cosmetic. CSS Text 3 §White Space Processing: "white
/// space processing in CSS affects only the document white space characters: spaces
/// (U+0020), tabs (U+0009), and segment breaks" — and in HTML the segment break is U+000A,
/// since the DOM normalizes CR/CRLF away. U+000C is in none of those, so it is *rendered
/// content*: Svelte's `clean_nodes` classifies with `regex_not_whitespace` = `/[^ \t\r\n]/`
/// and keeps a form feed verbatim. Classifying one as whitespace lets the render-free
/// boundary trim delete it and the inter-sibling collapse respell it as a space — both
/// content changes, invisible to a corpus diff against prettier, whose own class
/// (`[\t\n\f\r ]`) has the same defect. See
/// [text_form_feed_prettier_divergence](../../../../tests/fixtures/svelte/elements/text_form_feed_prettier_divergence/).
///
/// ⚠️ It is equally **not** the *tokenizer's* whitespace, which does include the form feed (a
/// form feed separates attributes) and every Unicode space besides. This is a question about
/// what RENDERS, so it is the printer's and `Text`'s class alone — the parser and lexer ask
/// the token-separator question instead, and answer it with
/// [`is_svelte_ws`](crate::whitespace::is_svelte_ws).
#[inline]
pub fn is_collapsible_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// [`is_collapsible_ws`] over a `char`, for the `str` pattern positions
/// (`trim_matches`, `starts_with`, …).
#[inline]
pub fn is_collapsible_ws_char(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// The runs of `s` between [`is_collapsible_ws_char`] characters, empties skipped — the
/// word split of a text node's fill (prettier's `splitTextToDocs`, `/[\t\n\f\r ]+/`).
///
/// A word split *is* a whitespace classification — every character it splits at is deleted along
/// with the run it stood for — so it shares [`is_collapsible_ws_char`] rather than restating the
/// class. `str::split_ascii_whitespace` is the same function over the **wider** set, and using it
/// here dropped a form feed out of the middle of a word; `str::split_whitespace` is wider still
/// and would drop a non-breaking space.
#[inline]
pub fn split_collapsible_ws(s: &str) -> impl Iterator<Item = &str> {
    s.split(is_collapsible_ws_char).filter(|w| !w.is_empty())
}

/// Svelte Text node - raw text content
///
/// Represents static text in the template or attribute values.
/// In attribute values, this represents the unquoted string content.
///
/// Stores `raw_span` — the span of the original text (with HTML entities:
/// `&lt;`, `&#65;`) in the host source; the text is a pure sub-slice (no decode)
/// recovered on demand via `Text::raw`, so it is a `Span` rather than an owned
/// `String` (mirrors `tsv_lang::Comment::content_span`). The decoded form
/// (`<`, `A`) is computed lazily via `Text::data`, borrowing `raw` without
/// allocating when no entity is present.
///
/// The printer's hot template-whitespace predicates (multiline-children analysis,
/// inline-run detection, boundary trimming) read the precomputed `is_collapsible_ws_only`
/// and `newline_count` scalars below instead of re-scanning `raw` each time — the
/// same `multiline`-style trick `comment-as-span` used for `tsv_lang::Comment`. A
/// content `Text` is otherwise re-scanned ~10× per parent-element format, across the
/// analyze and build passes (which share no result). The flags cover the *whole-raw*
/// collapsible-whitespace and newline-count notions only; boundary / leading-trailing
/// / trimmed-substring predicates stay scan-based (they're first/last-char or
/// substring, already O(1) or rare).
#[derive(Debug, Clone)]
pub struct Text {
    /// Span of the raw text (entities preserved) in the host source; text via `raw`.
    pub raw_span: Span,
    /// Which entity decode `data()` applies, fixed at parse time by context.
    pub decoding: TextDecoding,
    pub span: Span,
    /// Precomputed at parse: `raw` is entirely [collapsible whitespace](is_collapsible_ws)
    /// `[ \t\n\r]`, or empty — equivalently
    /// `raw(source).trim_matches(is_collapsible_ws_char).is_empty()`.
    /// A non-breaking space (U+00A0 / U+202F), a form feed, or any other separator CSS does
    /// not collapse is template *content*, so it makes this `false`.
    ///
    /// Keyed on `raw`, not on the decoded `data`: an entity-encoded whitespace character
    /// (`&#9;`) is content here, where Svelte's `clean_nodes` — which tests `data` — calls it
    /// whitespace. That is deliberate and matches prettier (`node.raw || node.data`): rewriting
    /// an entity's bytes is a content edit, so the node is printed verbatim. It is still a
    /// *separator* rather than prose — it carries no word for a `fill` to pack — which is the
    /// question `Printer::is_separator_like_text` answers off `data`.
    pub is_collapsible_ws_only: bool,
    /// Precomputed count of `\n` in `raw`, **saturating at 2** — enough for every test
    /// the printer makes (`has_newline` = `>= 1`, blank line = `>= 2`).
    pub newline_count: u8,
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
    /// Construct a `Text`, precomputing the whitespace scalars from `raw_span`.
    /// `source` must be the host document `raw_span` was recorded against (the same
    /// document every later `raw(source)` reader passes), so the flags stay in sync
    /// with `raw` whether the node is standalone or embedded.
    pub fn new(raw_span: Span, decoding: TextDecoding, span: Span, source: &str) -> Self {
        let raw = raw_span.extract(source);
        // `is_collapsible_ws_only` is true for an empty node too;
        // `newline_count` saturates at 2 (the printer only tests ==0 / <2 / >=1 / >=2).
        let is_collapsible_ws_only = raw.bytes().all(is_collapsible_ws);
        let newline_count = raw.bytes().filter(|&b| b == b'\n').take(2).count() as u8;
        Text {
            raw_span,
            decoding,
            span,
            is_collapsible_ws_only,
            newline_count,
        }
    }

    /// Whether `raw` contains at least one `\n` (precomputed, source-free).
    #[inline]
    pub fn has_newline(&self) -> bool {
        self.newline_count >= 1
    }

    /// Raw text (entities preserved) — a sub-slice of `source`, no allocation.
    /// `source` must be the host document the spans were recorded against.
    pub fn raw<'s>(&self, source: &'s str) -> &'s str {
        self.raw_span.extract(source)
    }

    /// Decoded text (`&lt;` → `<`, `&#65;` → `A`), computed lazily from `raw`.
    ///
    /// Borrows `raw` when no `&` is present (no entity possible, decode is
    /// identity) or when the node's context applies no decode.
    pub fn data<'s>(&self, source: &'s str) -> Cow<'s, str> {
        let raw = self.raw(source);
        let is_attribute_value = match self.decoding {
            TextDecoding::Raw => return Cow::Borrowed(raw),
            TextDecoding::Fragment => false,
            TextDecoding::AttributeValue => true,
        };
        if raw.contains('&') {
            Cow::Owned(tsv_html::decode_character_references(
                raw,
                is_attribute_value,
            ))
        } else {
            Cow::Borrowed(raw)
        }
    }
}

/// Svelte ExpressionTag - {expression} in template
///
/// Represents a TypeScript/JS expression embedded in the template.
/// The expression is evaluated and its result is rendered.
#[derive(Debug, Clone)]
pub struct ExpressionTag<'arena> {
    pub expression: Expression<'arena>,
    pub span: Span,
}

/// Svelte Script block - `<script>` tag contents
///
/// Contains a TypeScript/JS program and metadata about the script tag.
/// The `context` field distinguishes between instance and module scripts.
#[derive(Debug, Clone)]
pub struct Script<'arena> {
    pub content: Program<'arena>,
    pub attributes: &'arena [AttributeNode<'arena>],
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

/// Svelte Style block - `<style>` tag contents
///
/// Stores the span of the entire `<style>` tag and the content span.
/// Style tag with parsed CSS content
#[derive(Debug, Clone)]
pub struct Style<'arena> {
    pub span: Span,         // Full <style>...</style> span
    pub content_span: Span, // Just the CSS text inside the tags
    pub attributes: &'arena [AttributeNode<'arena>],
    /// Parsed CSS stylesheet (nodes + value comments), bump-allocated in the
    /// shared document `Bump` (`tsv_css` is arena-native).
    pub css_stylesheet: CssStyleSheet<'arena>,
}

// No `size_of` guards on the slice-multiplied Svelte AST enums: the arena layout
// deliberately favors traversal locality over node size, keeping every
// `FragmentNode` / `AttributeNode` variant inline by value rather than
// arena-boxing the fat ones for a smaller enum. Boxing them shrank the slice
// element but added a pointer-chase on hot format-read paths that cost more than
// the density win, so the inline form stands.

#[cfg(test)]
mod tests {
    use super::{SpecialElementKind, SpecialElementTag};

    #[test]
    fn special_element_tag_from_name() {
        use SpecialElementTag::*;
        assert_eq!(
            SpecialElementTag::from_tag_name("svelte:head", false, false),
            Some(SvelteHead)
        );
        assert_eq!(
            SpecialElementTag::from_tag_name("svelte:boundary", false, false),
            Some(SvelteBoundary)
        );
        // `title` is special only inside <svelte:head> — the flag gates both arms.
        assert_eq!(
            SpecialElementTag::from_tag_name("title", true, false),
            Some(TitleElement)
        );
        assert_eq!(
            SpecialElementTag::from_tag_name("title", false, false),
            None
        );
        // `slot` is SlotElement normally, but a plain RegularElement (→ None) inside a
        // <template shadowrootmode>.
        assert_eq!(
            SpecialElementTag::from_tag_name("slot", false, false),
            Some(SlotElement)
        );
        assert_eq!(SpecialElementTag::from_tag_name("slot", false, true), None);
        // Unknown / regular tags are not special.
        assert_eq!(SpecialElementTag::from_tag_name("div", true, false), None);
        assert_eq!(
            SpecialElementTag::from_tag_name("svelte:unknown", false, false),
            None
        );
    }

    #[test]
    fn special_element_kind_is_block() {
        // Only the four document-binding elements are block.
        assert!(SpecialElementKind::SvelteHead.is_block());
        assert!(SpecialElementKind::SvelteWindow.is_block());
        assert!(SpecialElementKind::SvelteBody.is_block());
        assert!(SpecialElementKind::SvelteDocument.is_block());
        // The content/dynamic/error elements are inline.
        assert!(!SpecialElementKind::SlotElement.is_block());
        assert!(!SpecialElementKind::SvelteSelf.is_block());
        assert!(!SpecialElementKind::SvelteFragment.is_block());
        assert!(!SpecialElementKind::SvelteBoundary.is_block());
        assert!(!SpecialElementKind::TitleElement.is_block());
    }

    #[test]
    fn text_new_precomputes_whitespace_flags() {
        use super::{Span, Text, TextDecoding};
        // A `Text` whose `raw_span` covers the whole probe string.
        let mk = |raw: &str| {
            let span = Span {
                start: 0,
                end: raw.len() as u32,
            };
            Text::new(span, TextDecoding::Fragment, span, raw)
        };

        // `is_collapsible_ws_only`: collapsible (ASCII) whitespace only; empty counts true.
        assert!(mk("  \t\n ").is_collapsible_ws_only);
        assert!(mk("").is_collapsible_ws_only);
        // A non-breaking space (U+00A0) is content, not collapsible whitespace.
        assert!(!mk("\u{00A0}").is_collapsible_ws_only);
        assert!(!mk("a").is_collapsible_ws_only);

        // `newline_count` saturates at 2 (drives `has_newline`, and the printer's own
        // `newline_count >= 2` reads).
        assert_eq!(mk("a b").newline_count, 0);
        assert!(!mk("a b").has_newline());
        assert_eq!(mk("a\nb").newline_count, 1);
        assert!(mk("a\nb").has_newline());
        assert_eq!(mk("a\n\nb").newline_count, 2);
        // 3+ newlines still report the saturated 2.
        assert_eq!(mk("\n\n\n\n").newline_count, 2);
    }

    /// Grade every packed [`TagFacts`](super::TagFacts) accessor against the pure predicate it
    /// encodes, over an alphabet covering each bit's positive and negative cases. This is the gate
    /// with power over the bit packing: a swapped constant or an accessor reading its neighbour's
    /// bit changes layout only on rare tags at rare widths, which no fixture or corpus diff can be
    /// relied on to see.
    #[test]
    fn tag_facts_bits_agree_with_the_pure_predicates() {
        use super::{TagFacts, is_component_name};
        let probes = [
            // block members (hr is also void; pre is also ws-sensitive)
            "div",
            "p",
            "h1",
            "menu",
            // whitespace-collapsing containers (WS_COLLAPSING, all 8) + near-miss non-members
            "table",
            "select",
            "tr",
            "tbody",
            "thead",
            "tfoot",
            "colgroup",
            "datalist",
            "optgroup",
            "ul",
            "li",
            "pre",
            "hr",
            "blockquote",
            // void members (incl. the case-insensitive !doctype family)
            "br",
            "img",
            "input",
            "command",
            "keygen",
            "!doctype",
            "!DOCTYPE",
            "!DocType",
            // foreign members (SVG incl. camelCase + hyphenated; MathML)
            "svg",
            "circle",
            "foreignObject",
            "color-profile",
            "math",
            "annotation-xml",
            "mi",
            // the name-compare bits
            "script",
            "style",
            "template",
            "textarea",
            // component-shaped names (incl. non-ASCII uppercase initials — Greek, Latin, Cyrillic)
            "Button",
            "MyComponent",
            "svelte:head",
            "svelte:component",
            "foo:bar",
            "foo.bar",
            "Div",
            "DIV",
            "Δcomp",
            "Écomp",
            "Яcomp",
            "étoile",
            // near-misses and odd inputs
            "span",
            "td",
            "divx",
            "di",
            "xdiv",
            "doctype",
            "é",
            "ünknown",
            "",
        ];
        for tag in probes {
            let facts = TagFacts::compute(tag);
            assert_eq!(
                facts.is_block(),
                tsv_html::is_block_element(tag),
                "block: {tag:?}"
            );
            assert_eq!(
                facts.is_void(),
                tsv_html::is_void_element(tag),
                "void: {tag:?}"
            );
            assert_eq!(
                facts.is_foreign(),
                tsv_html::is_foreign_element(tag),
                "foreign: {tag:?}"
            );
            assert_eq!(
                facts.is_component_name(),
                is_component_name(tag),
                "component name: {tag:?}"
            );
            assert_eq!(
                facts.is_namespaced(),
                tag.contains(':'),
                "namespaced: {tag:?}"
            );
            assert_eq!(facts.is_style(), tag == "style", "style: {tag:?}");
            assert_eq!(facts.is_script(), tag == "script", "script: {tag:?}");
            assert_eq!(facts.is_template(), tag == "template", "template: {tag:?}");
            assert_eq!(
                facts.is_ws_sensitive(),
                tsv_html::preserves_whitespace(tag),
                "ws-sensitive: {tag:?}"
            );
            assert_eq!(
                facts.collapses_child_whitespace(),
                tsv_html::collapses_child_whitespace(tag),
                "ws-collapsing: {tag:?}"
            );
            assert_eq!(
                facts.is_declaration(),
                tag.starts_with('!'),
                "declaration: {tag:?}"
            );
        }
    }
}
