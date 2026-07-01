// Svelte public AST types
//
// JSON-compatible representation matching Svelte's official parser output.
// Used for serialization and external tool compatibility.

use crate::ast::internal::ElementKind;
use serde::Serialize;
use tsv_css::ast::public::StyleSheet;
use tsv_ts::ast::public::{Expression, Program};

/// A template expression position: typed, or a `serde_json::Value` island
/// carrying attached comments.
///
/// Conversion always produces `Typed`. The island-scoped comment-attachment
/// pass (`ast/convert/attach_typed.rs`) swaps an expression to `Attached`
/// only when a template comment falls in its container's window — the
/// expression subtree is serialized to a `Value` so `leadingComments` /
/// `trailingComments` can be injected without adding comment fields to the
/// typed `tsv_ts` public AST (a deliberate design decision — see
/// `ast/convert/special.rs` on `Script.content`). `#[serde(untagged)]` keeps
/// both arms wire-identical to a plain expression.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ExpressionIsland<'src> {
    Typed(Expression<'src>),
    Attached(serde_json::Value),
}

impl<'src> From<Expression<'src>> for ExpressionIsland<'src> {
    fn from(expression: Expression<'src>) -> Self {
        ExpressionIsland::Typed(expression)
    }
}

/// A `<script>` `Program`: typed, or a `serde_json::Value` island carrying
/// injected comments / non-TS quirks.
///
/// Conversion produces `Typed` when nothing will be injected — no script
/// comments, no preceding HTML comment, and `lang="ts"` (a plain script may
/// need `"options": null` injected on ImportExpressions) — skipping the
/// per-script JSON roundtrip on the direct-serialization path. Otherwise the
/// `Attached` arm holds the JSON-roundtripped `Program` with
/// `leadingComments`/`trailingComments` injected, keeping the typed `tsv_ts`
/// public AST free of comment fields (see the architecture note in
/// `ast/convert/special.rs`). `#[serde(untagged)]` keeps both arms
/// wire-identical.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ProgramIsland<'src> {
    Typed(Program<'src>),
    Attached(serde_json::Value),
}

/// Svelte name location - tracks the precise span of a node's name
///
/// Used on elements, attributes, and directives. The `character` field
/// is the byte offset (Svelte-specific; TS `loc` doesn't have it).
#[derive(Debug, Clone, Serialize)]
pub struct NameLocation {
    pub start: NamePosition,
    pub end: NamePosition,
}

/// Position within a name location (line, column, byte offset)
#[derive(Debug, Clone, Serialize)]
pub struct NamePosition {
    pub line: usize,
    pub column: usize,
    pub character: u32,
}

/// Svelte Root node - top level of a .svelte file
///
/// Serializes to match Svelte's parser output exactly.
///
/// The public AST is Serialize-only — it is an output format (matching Svelte's
/// JSON), never deserialized back into these types. (`css` also embeds
/// `tsv_css`'s `StyleSheet`, whose `&'static str` type tags couldn't round-trip
/// regardless.) The `'src` lifetime ties embedded `tsv_ts` expressions and
/// `tsv_css` style text back to the source they were converted against.
#[derive(Debug, Clone, Serialize)]
pub struct Root<'src> {
    pub css: Option<StyleSheet<'src>>,
    pub js: Vec<serde_json::Value>, // empty array for now
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub fragment: Fragment<'src>,
    pub options: Option<SvelteOptions<'src>>,
    pub comments: Vec<serde_json::Value>, // root comments as JSON values (populated in ast/convert)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<Script<'src>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<Script<'src>>,
}

/// Svelte Fragment - container for template nodes
#[derive(Debug, Clone, Serialize)]
pub struct Fragment<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub nodes: Vec<FragmentNode<'src>>,
}

/// Svelte template node types
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum FragmentNode<'src> {
    Component(Element<'src>),
    RegularElement(Element<'src>),
    SpecialElement(SpecialElement<'src>),
    ExpressionTag(ExpressionTag<'src>),
    Text(Text),
    Comment(Comment),
    IfBlock(IfBlock<'src>),
    EachBlock(EachBlock<'src>),
    AwaitBlock(AwaitBlock<'src>),
    KeyBlock(KeyBlock<'src>),
    SnippetBlock(SnippetBlock<'src>),
    HtmlTag(HtmlTag<'src>),
    ConstTag(ConstTag),
    DeclarationTag(DeclarationTag),
    DebugTag(DebugTag<'src>),
    RenderTag(RenderTag<'src>),
}

/// Svelte HTML Comment node: <!-- content -->
#[derive(Debug, Clone, Serialize)]
pub struct Comment {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub data: String,
}

/// Svelte Element - HTML/component tag
#[derive(Debug, Clone, Serialize)]
pub struct Element<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    #[serde(skip_serializing)]
    pub kind: ElementKind,
    pub attributes: Vec<AttributeNode<'src>>,
    pub fragment: Fragment<'src>,
}

/// Svelte Special Element - special Svelte elements
///
/// Represents: `<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`,
/// `<svelte:element>`, `<svelte:component>`, `<svelte:self>`, `<slot>`,
/// `<svelte:fragment>`, `<svelte:boundary>`, `<title>` (inside svelte:head)
#[derive(Debug, Clone, Serialize)]
pub struct SpecialElement<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    pub attributes: Vec<AttributeNode<'src>>,
    pub fragment: Fragment<'src>,
    /// Dynamic tag for `<svelte:element this={tag}>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<serde_json::Value>,
    /// Component expression for `<svelte:component this={Component}>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<ExpressionIsland<'src>>,
}

/// Svelte Options - component configuration
///
/// Represents `<svelte:options runes={true} />` etc.
/// Not part of the fragment - stored in Root.options
#[derive(Debug, Clone, Serialize)]
pub struct SvelteOptions<'src> {
    pub start: u32,
    pub end: u32,
    pub attributes: Vec<AttributeNode<'src>>,
    /// Parsed from `runes={true/false}` attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runes: Option<bool>,
    /// Parsed from `immutable` / `immutable={true/false}` attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immutable: Option<bool>,
    /// Parsed from `accessors` / `accessors={true/false}` attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessors: Option<bool>,
    /// Parsed from `preserveWhitespace` / `preserveWhitespace={true/false}` attribute
    #[serde(rename = "preserveWhitespace", skip_serializing_if = "Option::is_none")]
    pub preserve_whitespace: Option<bool>,
    /// Parsed from `css="injected"` attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
    /// Parsed from `namespace="svg"` attribute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Parsed from `customElement={{ tag: '...', shadow: '...' }}` attribute
    #[serde(rename = "customElement", skip_serializing_if = "Option::is_none")]
    pub custom_element: Option<serde_json::Value>,
}

/// Svelte Attribute - element attribute
#[derive(Debug, Clone, Serialize)]
pub struct Attribute<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    pub value: AttributeValueField<'src>,
}

/// Svelte AttachTag - element attachment (Svelte 5.29+)
#[derive(Debug, Clone, Serialize)]
pub struct AttachTag<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
}

/// Svelte SpreadAttribute - spread object as attributes (`{...obj}`)
#[derive(Debug, Clone, Serialize)]
pub struct SpreadAttribute<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
}

//
// Directives
//

/// OnDirective - event handler (`on:click={handler}`)
#[derive(Debug, Clone, Serialize)]
pub struct OnDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<ExpressionIsland<'src>>,
    pub modifiers: Vec<String>,
}

/// BindDirective - two-way binding (`bind:value={name}`)
///
/// The `expression` field is `serde_json::Value` because shorthand directives
/// (`bind:value`) produce Svelte-style field ordering without `loc`, while
/// explicit directives (`bind:value={a}`) use acorn-style ordering with `loc`.
#[derive(Debug, Clone, Serialize)]
pub struct BindDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: serde_json::Value,
    pub modifiers: Vec<String>,
}

/// ClassDirective - conditional class (`class:class1={cond}`)
///
/// See `BindDirective` for why `expression` is `serde_json::Value`.
#[derive(Debug, Clone, Serialize)]
pub struct ClassDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: serde_json::Value,
    pub modifiers: Vec<String>,
}

/// StyleDirective - inline style (`style:color={value}`)
#[derive(Debug, Clone, Serialize)]
pub struct StyleDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub modifiers: Vec<String>,
    pub value: AttributeValueField<'src>,
}

/// UseDirective - action (`use:action={params}`)
#[derive(Debug, Clone, Serialize)]
pub struct UseDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<ExpressionIsland<'src>>,
    pub modifiers: Vec<String>,
}

/// TransitionDirective - transition (`transition:fade`, `in:fly`, `out:slide`)
#[derive(Debug, Clone, Serialize)]
pub struct TransitionDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<ExpressionIsland<'src>>,
    pub modifiers: Vec<String>,
    pub intro: bool,
    pub outro: bool,
}

/// AnimateDirective - animation (`animate:flip={params}`)
#[derive(Debug, Clone, Serialize)]
pub struct AnimateDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<ExpressionIsland<'src>>,
    pub modifiers: Vec<String>,
}

/// LetDirective - slot prop (`let:item={localItem}`)
#[derive(Debug, Clone, Serialize)]
pub struct LetDirective<'src> {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<ExpressionIsland<'src>>,
    pub modifiers: Vec<String>,
}

/// Svelte attribute-like node
///
/// Elements can have various attribute-like constructs in their attributes array.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeNode<'src> {
    Attribute(Attribute<'src>),
    SpreadAttribute(SpreadAttribute<'src>),
    AttachTag(AttachTag<'src>),
    OnDirective(OnDirective<'src>),
    BindDirective(BindDirective),
    ClassDirective(ClassDirective),
    StyleDirective(StyleDirective<'src>),
    UseDirective(UseDirective<'src>),
    TransitionDirective(TransitionDirective<'src>),
    AnimateDirective(AnimateDirective<'src>),
    LetDirective(LetDirective<'src>),
}

/// An attribute or style-directive value, in Svelte's three wire shapes:
/// `true` (boolean shorthand), a bare `{expr}` (plain object), or a
/// quoted/text sequence (array of parts). `#[serde(untagged)]` serializes
/// each arm as its bare shape.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeValueField<'src> {
    /// Boolean attribute (`disabled`) or valueless style directive —
    /// always `true`.
    True(bool),
    /// Single bare expression (`value={expr}`).
    Single(AttributeValue<'src>),
    /// Text content, quoted expressions, or mixed sequences
    /// (`class="a {b}"`) — always an array, even with one part.
    Sequence(Vec<AttributeValue<'src>>),
}

/// Svelte Attribute value part
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AttributeValue<'src> {
    Text(AttributeText),
    ExpressionTag(ExpressionTag<'src>),
}

/// Svelte Text node in attribute values (field order: start, end, type, raw, data)
///
/// Svelte's parser serializes attribute-value Text nodes with `start, end` before `type`,
/// unlike fragment-level Text nodes which use `type, start, end`.
#[derive(Debug, Clone, Serialize)]
pub struct AttributeText {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub raw: String,
    pub data: String,
}

/// Svelte Text node (fragment context: type, start, end, raw, data)
#[derive(Debug, Clone, Serialize)]
pub struct Text {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub raw: String,
    pub data: String,
}

/// Svelte ExpressionTag - {expression} in template
#[derive(Debug, Clone, Serialize)]
pub struct ExpressionTag<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
}

/// Svelte Script block
#[derive(Debug, Clone, Serialize)]
pub struct Script<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub context: String, // "default" or "module"
    pub content: ProgramIsland<'src>,
    pub attributes: Vec<AttributeNode<'src>>,
}

/// Svelte IfBlock - conditional rendering
#[derive(Debug, Clone, Serialize)]
pub struct IfBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub elseif: bool,
    pub start: u32,
    pub end: u32,
    pub test: ExpressionIsland<'src>,
    pub consequent: Fragment<'src>,
    pub alternate: Option<Fragment<'src>>,
}

/// Svelte EachBlock - list iteration
#[derive(Debug, Clone, Serialize)]
pub struct EachBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
    pub body: Fragment<'src>,
    /// None when no `as` clause: {#each expr} or {#each expr, index}.
    /// Uses `serde_json::Value` because Svelte's `read_pattern()` produces a column +1 quirk
    /// on destructure patterns that we must replicate.
    pub context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<ExpressionIsland<'src>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Fragment<'src>>,
}

/// Svelte AwaitBlock - promise handling
#[derive(Debug, Clone, Serialize)]
pub struct AwaitBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
    /// Uses `serde_json::Value` — see `EachBlock::context` for why.
    pub value: Option<serde_json::Value>,
    /// Uses `serde_json::Value` — see `EachBlock::context` for why.
    pub error: Option<serde_json::Value>,
    pub pending: Option<Fragment<'src>>,
    #[serde(rename = "then")]
    pub then_block: Option<Fragment<'src>>,
    #[serde(rename = "catch")]
    pub catch_block: Option<Fragment<'src>>,
}

/// Svelte KeyBlock - keyed updates
#[derive(Debug, Clone, Serialize)]
pub struct KeyBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
    pub fragment: Fragment<'src>,
}

/// Svelte SnippetBlock - reusable template snippets
#[derive(Debug, Clone, Serialize)]
pub struct SnippetBlock<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
    pub parameters: Vec<ExpressionIsland<'src>>,
    pub body: Fragment<'src>,
    /// Generic type parameters (e.g., `T` in `{#snippet fn<T>(a: T)}`)
    #[serde(rename = "typeParams", skip_serializing_if = "Option::is_none")]
    pub type_params: Option<String>,
}

/// Svelte HtmlTag - raw HTML injection
#[derive(Debug, Clone, Serialize)]
pub struct HtmlTag<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
}

/// Svelte ConstTag - local constant declaration
///
/// The declaration is a VariableDeclaration-like structure with a single declarator.
#[derive(Debug, Clone, Serialize)]
pub struct ConstTag {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub declaration: serde_json::Value, // VariableDeclaration structure
}

/// Svelte DeclarationTag - local `{const …}` / `{let …}` declaration
///
/// The declaration is a VariableDeclaration-like structure (`kind` is `const`
/// or `let`) with one or more comma-separated declarators.
#[derive(Debug, Clone, Serialize)]
pub struct DeclarationTag {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub declaration: serde_json::Value, // VariableDeclaration structure
}

/// Svelte DebugTag - debugging helper
#[derive(Debug, Clone, Serialize)]
pub struct DebugTag<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub identifiers: Vec<ExpressionIsland<'src>>,
}

/// Svelte RenderTag - snippet rendering
#[derive(Debug, Clone, Serialize)]
pub struct RenderTag<'src> {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub expression: ExpressionIsland<'src>,
}
