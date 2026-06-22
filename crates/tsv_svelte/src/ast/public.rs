// Svelte public AST types
//
// JSON-compatible representation matching Svelte's official parser output.
// Used for serialization and external tool compatibility.

use crate::ast::internal::ElementKind;
use serde::{Deserialize, Serialize};
use tsv_css::ast::public::StyleSheet;
use tsv_ts::ast::public::Expression;

/// Svelte name location - tracks the precise span of a node's name
///
/// Used on elements, attributes, and directives. The `character` field
/// is the byte offset (Svelte-specific; TS `loc` doesn't have it).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameLocation {
    pub start: NamePosition,
    pub end: NamePosition,
}

/// Position within a name location (line, column, byte offset)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamePosition {
    pub line: usize,
    pub column: usize,
    pub character: u32,
}

/// Svelte Root node - top level of a .svelte file
///
/// Serializes to match Svelte's parser output exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub css: Option<StyleSheet>,
    pub js: Vec<serde_json::Value>, // empty array for now
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub fragment: Fragment,
    pub options: Option<SvelteOptions>,
    pub comments: Vec<serde_json::Value>, // root comments as JSON values (populated in ast/convert)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<Script>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<Script>,
}

/// Svelte Fragment - container for template nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub nodes: Vec<FragmentNode>,
}

/// Svelte template node types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FragmentNode {
    Component(Element),
    RegularElement(Element),
    SpecialElement(SpecialElement),
    ExpressionTag(ExpressionTag),
    Text(Text),
    Comment(Comment),
    IfBlock(IfBlock),
    EachBlock(EachBlock),
    AwaitBlock(AwaitBlock),
    KeyBlock(KeyBlock),
    SnippetBlock(SnippetBlock),
    HtmlTag(HtmlTag),
    ConstTag(ConstTag),
    DeclarationTag(DeclarationTag),
    DebugTag(DebugTag),
    RenderTag(RenderTag),
}

/// Svelte HTML Comment node: <!-- content -->
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub data: String,
}

/// Svelte Element - HTML/component tag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    #[serde(skip_serializing)]
    pub kind: ElementKind,
    pub attributes: Vec<AttributeNode>,
    pub fragment: Fragment,
}

/// Svelte Special Element - special Svelte elements
///
/// Represents: `<svelte:head>`, `<svelte:window>`, `<svelte:body>`, `<svelte:document>`,
/// `<svelte:element>`, `<svelte:component>`, `<svelte:self>`, `<slot>`,
/// `<svelte:fragment>`, `<svelte:boundary>`, `<title>` (inside svelte:head)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialElement {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    pub attributes: Vec<AttributeNode>,
    pub fragment: Fragment,
    /// Dynamic tag for `<svelte:element this={tag}>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<serde_json::Value>,
    /// Component expression for `<svelte:component this={Component}>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expression>,
}

/// Svelte Options - component configuration
///
/// Represents `<svelte:options runes={true} />` etc.
/// Not part of the fragment - stored in Root.options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvelteOptions {
    pub start: u32,
    pub end: u32,
    pub attributes: Vec<AttributeNode>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub name_loc: NameLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

/// Svelte AttachTag - element attachment (Svelte 5.29+)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}

/// Svelte SpreadAttribute - spread object as attributes (`{...obj}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadAttribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}

//
// Directives
//

/// OnDirective - event handler (`on:click={handler}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// BindDirective - two-way binding (`bind:value={name}`)
///
/// The `expression` field is `serde_json::Value` because shorthand directives
/// (`bind:value`) produce Svelte-style field ordering without `loc`, while
/// explicit directives (`bind:value={a}`) use acorn-style ordering with `loc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: serde_json::Value,
    pub modifiers: Vec<String>,
}

/// ClassDirective - conditional class (`class:class1={cond}`)
///
/// See `BindDirective` for why `expression` is `serde_json::Value`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: serde_json::Value,
    pub modifiers: Vec<String>,
}

/// StyleDirective - inline style (`style:color={value}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub modifiers: Vec<String>,
    pub value: serde_json::Value, // true | ExpressionTag | [Text | ExpressionTag]
}

/// UseDirective - action (`use:action={params}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// TransitionDirective - transition (`transition:fade`, `in:fly`, `out:slide`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
    pub intro: bool,
    pub outro: bool,
}

/// AnimateDirective - animation (`animate:flip={params}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// LetDirective - slot prop (`let:item={localItem}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetDirective {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub name: String,
    pub name_loc: NameLocation,
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// Svelte attribute-like node
///
/// Elements can have various attribute-like constructs in their attributes array.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeNode {
    Attribute(Attribute),
    SpreadAttribute(SpreadAttribute),
    AttachTag(AttachTag),
    OnDirective(OnDirective),
    BindDirective(BindDirective),
    ClassDirective(ClassDirective),
    StyleDirective(StyleDirective),
    UseDirective(UseDirective),
    TransitionDirective(TransitionDirective),
    AnimateDirective(AnimateDirective),
    LetDirective(LetDirective),
}

/// Svelte Attribute value part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(AttributeText),
    ExpressionTag(ExpressionTag),
}

/// Svelte Text node in attribute values (field order: start, end, type, raw, data)
///
/// Svelte's parser serializes attribute-value Text nodes with `start, end` before `type`,
/// unlike fragment-level Text nodes which use `type, start, end`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeText {
    pub start: u32,
    pub end: u32,
    #[serde(rename = "type")]
    pub node_type: String,
    pub raw: String,
    pub data: String,
}

/// Svelte Text node (fragment context: type, start, end, raw, data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub raw: String,
    pub data: String,
}

/// Svelte ExpressionTag - {expression} in template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}

/// Svelte Script block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub context: String,            // "default" or "module"
    pub content: serde_json::Value, // Program with leadingComments/trailingComments injected
    pub attributes: Vec<AttributeNode>,
}

/// Svelte IfBlock - conditional rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub elseif: bool,
    pub start: u32,
    pub end: u32,
    pub test: Expression,
    pub consequent: Fragment,
    pub alternate: Option<Fragment>,
}

/// Svelte EachBlock - list iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EachBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub body: Fragment,
    /// None when no `as` clause: {#each expr} or {#each expr, index}.
    /// Uses `serde_json::Value` because Svelte's `read_pattern()` produces a column +1 quirk
    /// on destructure patterns that we must replicate.
    pub context: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Fragment>,
}

/// Svelte AwaitBlock - promise handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    /// Uses `serde_json::Value` — see `EachBlock::context` for why.
    pub value: Option<serde_json::Value>,
    /// Uses `serde_json::Value` — see `EachBlock::context` for why.
    pub error: Option<serde_json::Value>,
    pub pending: Option<Fragment>,
    #[serde(rename = "then")]
    pub then_block: Option<Fragment>,
    #[serde(rename = "catch")]
    pub catch_block: Option<Fragment>,
}

/// Svelte KeyBlock - keyed updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub fragment: Fragment,
}

/// Svelte SnippetBlock - reusable template snippets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetBlock {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
    pub parameters: Vec<Expression>,
    pub body: Fragment,
    /// Generic type parameters (e.g., `T` in `{#snippet fn<T>(a: T)}`)
    #[serde(rename = "typeParams", skip_serializing_if = "Option::is_none")]
    pub type_params: Option<String>,
}

/// Svelte HtmlTag - raw HTML injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}

/// Svelte ConstTag - local constant declaration
///
/// The declaration is a VariableDeclaration-like structure with a single declarator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub declaration: serde_json::Value, // VariableDeclaration structure
}

/// Svelte DeclarationTag - local `{const …}` / `{let …}` declaration
///
/// The declaration is a VariableDeclaration-like structure (`kind` is `const`
/// or `let`) with one or more comma-separated declarators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclarationTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub declaration: serde_json::Value, // VariableDeclaration structure
}

/// Svelte DebugTag - debugging helper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub identifiers: Vec<Expression>,
}

/// Svelte RenderTag - snippet rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderTag {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub expression: Expression,
}
