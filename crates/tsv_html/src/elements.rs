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

// Block elements for formatting purposes (matches prettier-plugin-svelte)
// Elements NOT in this list are treated as inline for formatting (including table cells).
// This matches prettier's logic: isInlineElement = !isBlockElement.
static BLOCK_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "div",
    "dl",
    "dd",
    "dt",
    "fieldset",
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
    "hgroup",
    "hr",
    "li",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "select",
    "table",
    "ul",
    "svg",
    "math",
};

static VOID_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
    "source", "track", "wbr",
};

// SVG elements - synced with Svelte's utils.js SVG_ELEMENTS
static SVG_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "altGlyph", "altGlyphDef", "altGlyphItem", "animate", "animateColor", "animateMotion",
    "animateTransform", "circle", "clipPath", "color-profile", "cursor", "defs", "desc", "discard",
    "ellipse", "feBlend", "feColorMatrix", "feComponentTransfer", "feComposite", "feConvolveMatrix",
    "feDiffuseLighting", "feDisplacementMap", "feDistantLight", "feDropShadow", "feFlood",
    "feFuncA", "feFuncB", "feFuncG", "feFuncR", "feGaussianBlur", "feImage", "feMerge",
    "feMergeNode", "feMorphology", "feOffset", "fePointLight", "feSpecularLighting", "feSpotLight",
    "feTile", "feTurbulence", "filter", "font", "font-face", "font-face-format", "font-face-name",
    "font-face-src", "font-face-uri", "foreignObject", "g", "glyph", "glyphRef", "hatch",
    "hatchpath", "hkern", "image", "line", "linearGradient", "marker", "mask", "mesh",
    "meshgradient", "meshpatch", "meshrow", "metadata", "missing-glyph", "mpath", "path", "pattern",
    "polygon", "polyline", "radialGradient", "rect", "set", "solidcolor", "stop", "svg", "switch",
    "symbol", "text", "textPath", "title", "tref", "tspan", "unknown", "use", "view", "vkern",
};

// MathML elements - synced with Svelte's utils.js (MathML Core)
static MATHML_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "annotation", "annotation-xml", "maction", "math", "merror", "mfrac", "mi", "mmultiscripts",
    "mn", "mo", "mover", "mpadded", "mphantom", "mprescripts", "mroot", "mrow", "ms", "mspace",
    "msqrt", "mstyle", "msub", "msubsup", "msup", "mtable", "mtd", "mtext", "mtr", "munder",
    "munderover", "semantics",
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

/// Check if an element is an SVG element
#[inline]
pub fn is_svg_element(tag_name: &str) -> bool {
    SVG_ELEMENTS.contains(tag_name)
}

/// Check if an element is a MathML element
#[inline]
pub fn is_mathml_element(tag_name: &str) -> bool {
    MATHML_ELEMENTS.contains(tag_name)
}

/// Check if an element is foreign content (SVG or MathML)
#[inline]
pub fn is_foreign_element(tag_name: &str) -> bool {
    SVG_ELEMENTS.contains(tag_name) || MATHML_ELEMENTS.contains(tag_name)
}
