// Svelte internal AST types
//
// Internal representation optimized for manipulation and formatting.
// Uses string interning for efficient storage and comparison of identifiers.

use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};
use tsv_css::ast::internal::CssStyleSheet;
pub use tsv_lang::{Comment, Span};
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
    pub comments: Vec<Comment>,
    pub span: Span,
    pub interner: Rc<RefCell<DefaultStringInterner>>,
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

/// Svelte Element kind - distinguishes HTML elements from components
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ElementKind {
    /// HTML element: `<div>`, `<span>`, `<input>`, etc. (lowercase first character)
    #[serde(rename = "Html")]
    Html,
    /// Svelte component: `<MyComponent>`, `<Button>`, etc. (uppercase first character)
    #[serde(rename = "Component")]
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
    ExpressionTag(ExpressionTag),
}

/// Svelte Text node - raw text content
///
/// Represents static text in the template or attribute values.
/// In attribute values, this represents the unquoted string content.
///
/// The `raw` field contains the original text with HTML entities (`&lt;`, `&#65;`),
/// while `data` contains the decoded text (`<`, `A`). Both fields are necessary:
/// - `raw` preserves the original source for accurate formatting/roundtrips
/// - `data` provides the decoded text for rendering and semantic analysis
///
/// TODO(performance): Text nodes store duplicate data (raw + data fields).
/// When raw has no entities, both fields are identical (~50% memory waste).
/// Possible optimization: store only raw, compute data on-demand when entities present.
///
/// TODO(performance): Printer repeatedly calls is_whitespace_only() on text nodes in
/// hot loops (multiline children, inline run detection). Could cache this as a bool field
/// computed during parsing: `pub is_whitespace_only: bool`. Trade-off: 1 byte per Text
/// node vs repeated string scans. Profile before optimizing.
#[derive(Debug, Clone)]
pub struct Text {
    pub raw: String,  // Raw text with HTML entities: "&lt;", "&#65;"
    pub data: String, // Decoded text: "<", "A"
    pub span: Span,
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
    pub attributes: Vec<Attribute>,
    pub context: ScriptContext,
    pub span: Span,
}

/// Script context type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScriptContext {
    Default, // <script>
    Module,  // <script context="module">
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
    pub css_stylesheet: CssStyleSheet, // Parsed CSS stylesheet (nodes + value comments)
}
