// Internal AST - optimized for traversal and manipulation
// Uses string interning for memory efficiency

use crate::span::Span;
use string_interner::{DefaultStringInterner, DefaultSymbol};

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Statement>,
    pub span: Span,
    pub interner: std::rc::Rc<std::cell::RefCell<DefaultStringInterner>>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    ExpressionStatement(ExpressionStatement),
    VariableDeclaration(VariableDeclaration),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::ExpressionStatement(stmt) => stmt.span,
            Statement::VariableDeclaration(decl) => decl.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExpressionStatement {
    pub expression: Expression,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal(Literal),
    Identifier(Identifier),
    // TODO: BinaryExpression, etc.
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Literal(lit) => lit.span,
            Expression::Identifier(id) => id.span,
        }
    }
}

/// Literal value type - supports numbers and strings
#[derive(Debug, Clone)]
pub enum LiteralValue {
    Number(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub struct Literal {
    pub value: LiteralValue,
    // TODO: Consider interning raw string if profiling shows literal memory is significant.
    // Current assessment: Internal AST is ephemeral (~ms), benefit unclear without real-world workloads.
    // Revisit when: parsing large files (>10K lines) or batch processing shows memory pressure.
    pub raw: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: DefaultSymbol,
    pub type_annotation: Option<TSTypeAnnotation>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableDeclarationKind {
    Const,
    Let,
    Var,
}

#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub kind: VariableDeclarationKind,
    pub declarations: Vec<VariableDeclarator>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct VariableDeclarator {
    pub id: Identifier,
    pub init: Option<Expression>,
    pub span: Span,
}

// TypeScript type annotation nodes

/// TypeScript type annotation node (e.g., `: number` in `const a: number = 5`)
///
/// Represents the full type annotation including the colon. The span covers the
/// entire annotation (`: number`), while the inner type covers just the type (`number`).
///
/// # Memory Layout
/// Uses `Box<TSType>` to avoid bloating `Identifier` size. The indirection is acceptable
/// since type annotations are relatively rare and accessed infrequently during traversal.
#[derive(Debug, Clone)]
pub struct TSTypeAnnotation {
    pub type_annotation: Box<TSType>,
    pub span: Span,
}

/// TypeScript type expression
///
/// Represents the various types in TypeScript's type system. Currently only
/// primitive keyword types are implemented. Complex types (unions, intersections,
/// generics, etc.) will be added incrementally.
#[derive(Debug, Clone)]
pub enum TSType {
    /// The `number` type keyword
    TSNumberKeyword(TSNumberKeyword),
    // TODO: TSStringKeyword, TSBooleanKeyword, etc.
}

impl TSType {
    pub fn span(&self) -> Span {
        match self {
            TSType::TSNumberKeyword(node) => node.span,
        }
    }
}

/// TypeScript `number` type keyword
///
/// Represents the primitive `number` type in TypeScript.
/// The span covers just the keyword itself (not including surrounding whitespace).
#[derive(Debug, Clone)]
pub struct TSNumberKeyword {
    pub span: Span,
}

// ============= Svelte Nodes =============

/// Svelte Root node - top level of a .svelte file
///
/// Contains the template fragment, optional script/style blocks, and metadata.
/// This is the entry point for Svelte AST traversal.
#[derive(Debug, Clone)]
pub struct Root {
    pub fragment: Fragment,
    pub instance: Option<Box<Script>>,
    pub module: Option<Box<Script>>,
    pub css: Option<Box<Style>>,
    pub span: Span,
    pub interner: std::rc::Rc<std::cell::RefCell<DefaultStringInterner>>,
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
    ExpressionTag(ExpressionTag),
    Text(Text),
}

impl FragmentNode {
    pub fn span(&self) -> Span {
        match self {
            FragmentNode::Element(elem) => elem.span,
            FragmentNode::ExpressionTag(tag) => tag.span,
            FragmentNode::Text(text) => text.span,
        }
    }
}

/// Svelte Element - HTML/component tag
///
/// Represents an HTML element or Svelte component in the template.
/// Elements have a name, attributes, and child nodes in a fragment.
#[derive(Debug, Clone)]
pub struct Element {
    pub name: DefaultSymbol,
    pub attributes: Vec<Attribute>,
    pub fragment: Fragment,
    pub span: Span,
}

/// Svelte Attribute - element attribute
///
/// Represents an attribute on an element, e.g., `class="foo"` or `disabled`.
/// The value is optional (for boolean attributes) and can contain text or expressions.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: DefaultSymbol,
    pub value: Option<Vec<AttributeValue>>,
    pub span: Span,
}

/// Svelte Attribute value part
///
/// Attribute values can contain static text or dynamic expressions.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    Text(Text),
    // TODO(Future sprint): ExpressionTag(ExpressionTag)
    // For dynamic attribute values like: <div class={expr} title={"text"}>
    // Note: Sprint 6 implemented template-level expression tags (<div>{expr}</div>)
    // but not attribute-level expression tags yet.
}

/// Svelte Text node - raw text content
///
/// Represents static text in the template or attribute values.
/// For Sprint 7, we store the content directly to simplify conversion.
/// In attribute values, this represents the unquoted string content.
///
/// TODO(performance): Text nodes store duplicate data (raw + data fields).
/// For now, raw and data are identical since HTML entity decoding isn't implemented.
/// This wastes ~50% memory for text nodes. See TODO_PERF.md "P1: Text Node Dual Storage"
/// for optimization strategies (store only raw, compute data on-demand).
#[derive(Debug, Clone)]
pub struct Text {
    pub raw: String,  // Raw text content (for attributes: "ts" has raw="ts")
    pub data: String, // Processed text (for Sprint 7, same as raw; future: decode entities)
    pub span: Span,
}

/// Svelte ExpressionTag - {expression} in template
///
/// Represents a JavaScript/TypeScript expression embedded in the template.
/// The expression is evaluated and its result is rendered.
#[derive(Debug, Clone)]
pub struct ExpressionTag {
    pub expression: Expression,
    pub span: Span,
}

/// Svelte Script block - <script> tag contents
///
/// Contains a TypeScript/JavaScript program and metadata about the script tag.
/// The `context` field distinguishes between instance and module scripts.
#[derive(Debug, Clone)]
pub struct Script {
    pub content: Program,
    pub attributes: Vec<Attribute>,
    pub context: ScriptContext,
    pub span: Span,
}

/// Script context type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptContext {
    Default,  // <script>
    Module,   // <script context="module">
}

/// Svelte Style block - <style> tag contents
///
/// Stores the span of the entire <style> tag and the content span.
/// Style tag with parsed CSS content
#[derive(Debug, Clone)]
pub struct Style {
    pub span: Span,         // Full <style>...</style> span
    pub content_span: Span, // Just the CSS text inside the tags
    pub attributes: Vec<Attribute>,
    pub css_nodes: Vec<CssNode>,  // Parsed CSS AST
}

// CSS AST nodes

#[derive(Debug, Clone)]
pub enum CssNode {
    Rule(CssRule),
    // TODO: Add more node types as needed (AtRule, Comment, etc.)
}

impl CssNode {
    pub fn span(&self) -> Span {
        match self {
            CssNode::Rule(rule) => rule.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: String,  // TODO: Parse selector structure (SelectorList, ComplexSelector, etc.)
    pub selector_span: Span,  // Span of just the selector
    pub block_span: Span,     // Span of the block including braces
    pub declarations: Vec<CssDeclaration>,
    pub span: Span,           // Full rule span
}

#[derive(Debug, Clone)]
pub struct CssDeclaration {
    pub property: String,
    pub value: String,
    pub span: Span,
}
