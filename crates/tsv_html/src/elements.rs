// HTML element type classification (language-level)
//
// Pure functions for classifying HTML elements by their rendering characteristics.
// These are language-level utilities independent of any specific tool (printer,
// linter, type-checker, etc.)
//
// References:
// - HTML spec phrasing content (inline): WHITESPACE_HTML.md line 13299
// - HTML spec flow content (block): WHITESPACE_HTML.md line 145233
// - Svelte void elements: node_modules/svelte/src/utils.js:16-41
//
// Performance: Uses phf::Set for compile-time perfect hash O(1) lookups with no runtime initialization.

use phf::phf_set;

// Perfect hash sets compiled at build time for O(1) element classification
static INLINE_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "a", "abbr", "b", "bdi", "bdo", "br", "button", "canvas", "cite", "code", "data", "dfn",
    "em", "i", "img", "input", "kbd", "label", "mark", "q", "s", "samp", "small", "span",
    "strong", "sub", "sup", "textarea", "time", "u", "var",
};

static BLOCK_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "dialog",
    "div",
    "figure",
    "figcaption",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
};

static VOID_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
};

/// Check if an HTML element is inline (phrasing content)
///
/// Inline elements flow with text and don't cause line breaks.
/// Examples: `<span>`, `<strong>`, `<a>`
#[inline]
pub fn is_inline_element(tag_name: &str) -> bool {
    INLINE_ELEMENTS.contains(tag_name)
}

/// Check if an HTML element is block (flow content)
///
/// Block elements create rectangular blocks and typically start on new lines.
/// Examples: `<div>`, `<p>`, `<section>`
#[inline]
pub fn is_block_element(tag_name: &str) -> bool {
    BLOCK_ELEMENTS.contains(tag_name)
}

/// Check if an HTML element is void (self-closing)
///
/// Void elements cannot have children and don't need closing tags.
/// Examples: `<br>`, `<img>`, `<input>`
#[inline]
pub fn is_void_element(tag_name: &str) -> bool {
    VOID_ELEMENTS.contains(tag_name)
}
