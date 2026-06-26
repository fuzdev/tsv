// Public AST types - with serde, for JSON output
// Uses u32 for positions (max 4GB file size) for memory efficiency

use serde::Serialize;

/// StyleSheet - CSS content with parsed AST
///
/// Represents a <style> tag's parsed CSS content.
/// Used when CSS is embedded in Svelte components.
///
/// Serialize-only (like the whole public AST): `children` holds the typed
/// `CssNodePublic` tree directly, so the embedded `<style>` path no longer
/// materializes an intermediate `serde_json::Value` per node. The public AST is
/// an output format — nothing deserializes it — and `CssNodePublic`'s
/// `&'static str` type tags couldn't round-trip anyway.
#[derive(Debug, Clone, Serialize)]
pub struct StyleSheet {
    #[serde(rename = "type")]
    pub node_type: String,
    pub start: u32,
    pub end: u32,
    pub attributes: Vec<serde_json::Value>, // Attributes from <style> tag
    pub children: Vec<CssNodePublic>,       // CSS AST nodes (Rules, etc.)
    pub content: StyleContent,
}

/// StyleSheet content - raw CSS text
#[derive(Debug, Clone, Serialize)]
pub struct StyleContent {
    pub start: u32,
    pub end: u32,
    pub styles: String,
    pub comment: Option<serde_json::Value>,
}

//
// Standalone public AST — the typed tree behind `convert_ast_json` /
// `convert_ast_json_string`. Mirrors `tsv_ts`/`tsv_svelte`: a typed serde tree
// serialized directly, never an intermediate `serde_json::Value`.
//
// These types are SERIALIZE-ONLY (output to JSON), so the `type` tag is a
// zero-allocation `&'static str` (it never needs to round-trip through
// `Deserialize`). Field declaration order IS the JSON key order — it must match
// Svelte's `parseCss()` output exactly (the contract `fixtures_update_parsed`
// regenerates against and the P1 fixture gate byte-checks). Dynamic text
// (`name`/`property`/`value`/`prelude`) is owned `String`, matching the strings
// the old `json!` path allocated; the win is dropping the `IndexMap<String,
// Value>` overhead, not the leaf strings.
//
// The embedded `<style>` path (`StyleSheet` above) shares these typed nodes as
// its `children` (built with `AstScope::Embedded`), so it never carries
// `metadata`; only its `attributes`/`content` envelope stays `serde_json::Value`.
//

/// A top-level or block child node: a rule, an at-rule, or (in blocks) a
/// declaration. Serialized untagged — each variant carries its own `type`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CssNodePublic {
    Rule(Rule),
    Atrule(Atrule),
    Declaration(Declaration),
}

/// Standalone stylesheet root (`parseCss()` shape: `type`/`start`/`end`/`children`,
/// no `attributes`/`content`). `end` is the full source length.
#[derive(Debug, Clone, Serialize)]
pub struct StyleSheetFile {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub children: Vec<CssNodePublic>,
}

/// CSS rule: selector list + declaration block.
#[derive(Debug, Clone, Serialize)]
pub struct Rule {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub prelude: SelectorList,
    pub block: Block,
    pub start: u32,
    pub end: u32,
    /// `parseCss()` standalone metadata; omitted for embedded `<style>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RuleMetadata>,
}

/// Declaration block `{ ... }`.
#[derive(Debug, Clone, Serialize)]
pub struct Block {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub children: Vec<CssNodePublic>,
}

/// `property: value` declaration. `property`/`value` are reconstructed from raw
/// source (Svelte's scan semantics), not from the structured internal value.
#[derive(Debug, Clone, Serialize)]
pub struct Declaration {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub property: String,
    pub value: String,
}

/// At-rule (`@media`, `@keyframes`, …). `prelude` is the raw prelude string;
/// `block` is `null` for statement at-rules (`@import`).
#[derive(Debug, Clone, Serialize)]
pub struct Atrule {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub prelude: String,
    pub block: Option<Block>,
    pub start: u32,
    pub end: u32,
}

/// Comma-separated selector list.
#[derive(Debug, Clone, Serialize)]
pub struct SelectorList {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub children: Vec<ComplexSelector>,
}

/// One complex selector (relative selectors joined by combinators).
#[derive(Debug, Clone, Serialize)]
pub struct ComplexSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub children: Vec<RelativeSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ComplexSelectorMetadata>,
}

/// Combinator + the simple selectors it introduces.
#[derive(Debug, Clone, Serialize)]
pub struct RelativeSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    /// `null` when there's no leading combinator.
    pub combinator: Option<Combinator>,
    pub selectors: Vec<SimpleSelector>,
    pub start: u32,
    pub end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RelativeSelectorMetadata>,
}

/// Combinator node (` `/`>`/`+`/`~`/`||`). `name` is the static symbol.
#[derive(Debug, Clone, Serialize)]
pub struct Combinator {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: &'static str,
    pub start: u32,
    pub end: u32,
}

/// A simple selector (or, inside pseudo-class args, an `Nth` term). Serialized
/// untagged — each variant carries its own `type`.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SimpleSelector {
    /// `TypeSelector`/`ClassSelector`/`IdSelector`/`NestingSelector` — same
    /// `{type, name, start, end}` shape, distinguished by `node_type`.
    Named(NamedSelector),
    Attribute(AttributeSelector),
    PseudoClass(PseudoClassSelector),
    PseudoElement(PseudoElementSelector),
    Percentage(Percentage),
    Nth(Nth),
}

/// `{type, name, start, end}` — Type/Class/Id/Nesting selectors.
#[derive(Debug, Clone, Serialize)]
pub struct NamedSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub start: u32,
    pub end: u32,
}

/// `[name op 'value' flags]`. `matcher`/`value`/`flags` are `null` when absent;
/// `namespace` is omitted when absent.
///
/// Field order is irregular on purpose: `parseCss` emits `start`/`end` *before*
/// `name` here (unlike `NamedSelector`/the pseudo selectors, which are
/// `type, name, start, end`), so this struct matches that quirk rather than the
/// regular pattern. Don't "normalize" `name` back above `start`/`end`.
#[derive(Debug, Clone, Serialize)]
pub struct AttributeSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub start: u32,
    pub end: u32,
    pub name: String,
    pub matcher: Option<String>,
    pub value: Option<String>,
    pub flags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

/// `:name(args)`. `args` is `null` for argument-less pseudo-classes.
#[derive(Debug, Clone, Serialize)]
pub struct PseudoClassSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub args: Option<Box<SelectorList>>,
    pub start: u32,
    pub end: u32,
}

/// `::name` — pseudo-element. `end` excludes any `(args)`, matching Svelte.
#[derive(Debug, Clone, Serialize)]
pub struct PseudoElementSelector {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub name: String,
    pub start: u32,
    pub end: u32,
}

/// `@keyframes` percentage selector (`50%`). `value` is the formatted string.
#[derive(Debug, Clone, Serialize)]
pub struct Percentage {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub value: String,
    pub start: u32,
    pub end: u32,
}

/// `An+B` term inside `:nth-child(...)` etc. `selector` is the optional
/// `of <selector-list>` (omitted when absent).
#[derive(Debug, Clone, Serialize)]
pub struct Nth {
    #[serde(rename = "type")]
    pub node_type: &'static str,
    pub value: String,
    pub start: u32,
    pub end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<Box<SelectorList>>,
}

//
// `parseCss()` metadata — constant payloads, present only on standalone CSS
// (not embedded `<style>`). The `parent_rule`/`rule` fields are always `null`
// (serialized via the unit type).
//

/// `Rule.metadata`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RuleMetadata {
    pub parent_rule: (),
    pub has_local_selectors: bool,
    pub has_global_selectors: bool,
    pub is_global_block: bool,
}

/// `ComplexSelector.metadata`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ComplexSelectorMetadata {
    pub rule: (),
    pub is_global: bool,
    pub used: bool,
}

/// `RelativeSelector.metadata`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RelativeSelectorMetadata {
    pub is_global: bool,
    pub is_global_like: bool,
    pub scoped: bool,
}
