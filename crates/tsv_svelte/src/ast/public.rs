// Svelte public AST types
//
// JSON-compatible representation matching Svelte's official parser output.
// Used for serialization and external tool compatibility.

use crate::ast::internal::ElementKind;
use serde::{Deserialize, Serialize};
use tsv_css::ast::public::StyleSheet;
use tsv_ts::ast::public::Expression;

/// Svelte Root node - top level of a .svelte file
///
/// Serializes to match Svelte's parser output exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub css: Option<StyleSheet>,
    pub js: Vec<serde_json::Value>, // empty array for now
    pub start: Option<u32>,
    pub end: Option<u32>,
    #[serde(rename = "type")]
    pub node_type: String,
    pub fragment: Fragment,
    pub options: Option<SvelteOptions>,
    pub comments: Vec<serde_json::Value>, // empty array for now
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
    pub attributes: Vec<AttributeNode>,
    pub fragment: Fragment,
    /// Dynamic tag for `<svelte:element this={tag}>`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<Expression>,
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
}

/// Svelte Attribute - element attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
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

// =============================================================================
// Directives
// =============================================================================

/// OnDirective - event handler (`on:click={handler}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// BindDirective - two-way binding (`bind:value={name}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub expression: Expression,
    pub modifiers: Vec<String>,
}

/// ClassDirective - conditional class (`class:active={isActive}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub expression: Expression,
    pub modifiers: Vec<String>,
}

/// StyleDirective - inline style (`style:color={value}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub modifiers: Vec<String>,
    pub value: serde_json::Value, // true | ExpressionTag | [Text | ExpressionTag]
}

/// UseDirective - action (`use:action={params}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// TransitionDirective - transition (`transition:fade`, `in:fly`, `out:slide`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
    pub intro: bool,
    pub outro: bool,
}

/// AnimateDirective - animation (`animate:flip={params}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expression: Option<Expression>,
    pub modifiers: Vec<String>,
}

/// LetDirective - slot prop (`let:item={localItem}`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LetDirective {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    Text(Text),
    ExpressionTag(ExpressionTag),
}

/// Svelte Text node
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
    pub start: u32,
    pub end: u32,
    pub elseif: bool,
    pub test: Expression,
    pub consequent: Fragment,
    #[serde(skip_serializing_if = "Option::is_none")]
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
    /// None when no `as` clause: {#each expr} or {#each expr, index}
    pub context: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Expression>,
    pub body: Fragment,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Expression>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<Fragment>,
    #[serde(rename = "then", skip_serializing_if = "Option::is_none")]
    pub then_block: Option<Fragment>,
    #[serde(rename = "catch", skip_serializing_if = "Option::is_none")]
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
