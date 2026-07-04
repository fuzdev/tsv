// HTML element type classification (language-level)
//
// Pure functions for classifying HTML elements by their rendering characteristics.
// These are language-level utilities independent of any specific tool (printer,
// linter, type-checker, etc.)
//
// References:
// - HTML spec flow content (block): ../html/source, Rendering §"Flow content"
//   (the UA stylesheet `display: block` element list)
// - Svelte void elements: node_modules/svelte/src/utils.js:16-41
// - prettier-plugin-svelte: isInlineElement = !isBlockElement (negation, no positive list)
//
// Performance: Uses phf::Set for compile-time perfect hash O(1) lookups with no runtime initialization.

use phf::{phf_map, phf_set};

// Block elements for formatting purposes.
// Elements NOT in this list are treated as inline for formatting (including table cells).
// This matches prettier's logic: isInlineElement = !isBlockElement.
// Intentionally absent: <center>, <select>, <svg>, <math> — prettier-plugin-svelte
// omits these. <svg>/<math> are handled separately as foreign elements.
// NOTE: <menu> is included here (spec compliance) but prettier-plugin-svelte omits it.
// The HTML spec treats <menu> identically to <ul> (display: block, same CSS rules).
// See tests/fixtures/svelte/elements/menu_block_prettier_divergence/README.md for details.
static BLOCK_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "address",
    "article",
    "aside",
    "blockquote",
    "details",
    "dialog",
    "dd",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
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
    "table",
    "ul",
};

// Matches Svelte's VOID_ELEMENT_NAMES (node_modules/svelte/src/utils.js:16-41).
// `command` and `keygen` are obsolete (removed from HTML spec) but included
// for Svelte parity — the parser (tsv_svelte) also includes them.
static VOID_ELEMENTS: phf::Set<&'static str> = phf_set! {
    "area", "base", "br", "col", "command", "embed", "hr", "img", "input", "keygen", "link",
    "meta", "param", "source", "track", "wbr",
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
    VOID_ELEMENTS.contains(tag_name) || tag_name.eq_ignore_ascii_case("!doctype")
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

// Elements whose end tag HTML lets you omit when a particular sibling or parent
// boundary follows — the WHATWG optional-end-tag subset Svelte's parser implements
// (../svelte/packages/svelte/src/html-tree-validation.js `autoclosing_children`,
// based on http://developers.whatwg.org/syntax.html#syntax-tag-omission).
//
// Each entry lists the "next" tag names that force the key element to auto-close.
// Svelte's source splits these into `direct` (immediate child) and `descendant`
// (any descendant) variants, but that split only affects the *validation* error
// wording — `closing_tag_omitted` treats both as one membership test — so the two
// are flattened here into a single trigger list per element.
//
// Scope: this is only the *parse-time* auto-close half. Svelte's validation-side
// table (`disallowed_children` + `is_tag_valid_with_parent`/`_ancestor`, which do
// need the direct/descendant/`reset_by`/`only` distinctions) belongs to a future
// diagnostics layer and is a separate port from the same source file.
static AUTOCLOSING_NEXT_TAGS: phf::Map<&'static str, &'static [&'static str]> = phf_map! {
    "li" => &["li"],
    "dt" => &["dt", "dd"],
    "dd" => &["dt", "dd"],
    "p" => &[
        "address", "article", "aside", "blockquote", "div", "dl", "fieldset",
        "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hgroup",
        "hr", "main", "menu", "nav", "ol", "p", "pre", "section", "table", "ul",
    ],
    "rt" => &["rt", "rp"],
    "rp" => &["rt", "rp"],
    "optgroup" => &["optgroup"],
    "option" => &["option", "optgroup"],
    "thead" => &["tbody", "tfoot"],
    "tbody" => &["tbody", "tfoot"],
    "tfoot" => &["tbody"],
    "tr" => &["tr", "tbody"],
    "td" => &["td", "th", "tr"],
    "th" => &["td", "th", "tr"],
};

/// Whether `current`'s end tag is implicitly omitted (auto-closed) when `next`
/// follows as the next tag in the markup.
///
/// Mirrors Svelte's `closing_tag_omitted(current, next)`: `current` auto-closes
/// when it is in the optional-end-tag table and `next` is either absent (the end
/// of the parent's content / EOF, modeled as `None`) or one of its listed triggers.
/// Elements outside the table never auto-close.
#[inline]
pub fn closing_tag_omitted(current: &str, next: Option<&str>) -> bool {
    match AUTOCLOSING_NEXT_TAGS.get(current) {
        Some(triggers) => match next {
            None => true,
            Some(next) => triggers.contains(&next),
        },
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_block_element() {
        assert!(is_block_element("div"));
        assert!(is_block_element("p"));
        // <menu> is block per spec (prettier-plugin-svelte omits it).
        assert!(is_block_element("menu"));
        assert!(!is_block_element("span"));
        // Table cells are inline for formatting purposes.
        assert!(!is_block_element("td"));
        // Foreign roots are intentionally absent (handled separately).
        assert!(!is_block_element("svg"));
        // phf membership is exact: tag names arrive lowercased.
        assert!(!is_block_element("DIV"));
    }

    #[test]
    fn test_is_void_element() {
        assert!(is_void_element("br"));
        assert!(is_void_element("img"));
        assert!(is_void_element("input"));
        // Obsolete but kept for Svelte parity.
        assert!(is_void_element("command"));
        assert!(is_void_element("keygen"));
        // Regular void elements are case-sensitive.
        assert!(!is_void_element("BR"));
        assert!(!is_void_element("div"));
        // `!doctype` is the one case-insensitive special case.
        assert!(is_void_element("!doctype"));
        assert!(is_void_element("!DOCTYPE"));
        assert!(is_void_element("!DocType"));
        // ...but it needs the leading '!'.
        assert!(!is_void_element("doctype"));
    }

    #[test]
    fn test_foreign_element_classification() {
        // SVG members, including camelCase and hyphenated names.
        assert!(is_svg_element("circle"));
        assert!(is_svg_element("foreignObject"));
        assert!(is_svg_element("color-profile"));
        // MathML members.
        assert!(is_mathml_element("math"));
        assert!(is_mathml_element("annotation-xml"));
        assert!(is_mathml_element("mfrac"));
        // Namespace boundaries: each set excludes the other's roots and HTML.
        assert!(!is_svg_element("math"));
        assert!(!is_mathml_element("svg"));
        assert!(!is_svg_element("div"));
        // The union covers both.
        assert!(is_foreign_element("circle"));
        assert!(is_foreign_element("math"));
        assert!(!is_foreign_element("div"));
    }

    #[test]
    fn test_closing_tag_omitted() {
        // `<li>` auto-closes at a sibling `<li>`, and at end-of-parent (`None`).
        assert!(closing_tag_omitted("li", Some("li")));
        assert!(closing_tag_omitted("li", None));
        // ...but not at an unrelated sibling.
        assert!(!closing_tag_omitted("li", Some("span")));
        assert!(!closing_tag_omitted("li", Some("ul")));

        // `<p>` auto-closes at block-level siblings (descendant list), incl. `<div>`.
        assert!(closing_tag_omitted("p", Some("div")));
        assert!(closing_tag_omitted("p", Some("p")));
        assert!(closing_tag_omitted("p", Some("ul")));
        // ...but not at an inline sibling.
        assert!(!closing_tag_omitted("p", Some("span")));

        // Table family: thead/tbody/tfoot and the tr/td/th triggers.
        assert!(closing_tag_omitted("thead", Some("tbody")));
        assert!(closing_tag_omitted("tfoot", Some("tbody")));
        assert!(!closing_tag_omitted("tfoot", Some("thead")));
        assert!(closing_tag_omitted("td", Some("th")));
        assert!(closing_tag_omitted("th", Some("tr")));
        assert!(closing_tag_omitted("tr", Some("tr")));

        // dt/dd and rt/rp reciprocal pairs; option/optgroup.
        assert!(closing_tag_omitted("dt", Some("dd")));
        assert!(closing_tag_omitted("dd", Some("dt")));
        assert!(closing_tag_omitted("rt", Some("rp")));
        assert!(closing_tag_omitted("option", Some("optgroup")));
        assert!(closing_tag_omitted("optgroup", Some("optgroup")));
        assert!(!closing_tag_omitted("optgroup", Some("option")));

        // Elements outside the table never auto-close.
        assert!(!closing_tag_omitted("div", Some("div")));
        assert!(!closing_tag_omitted("div", None));
        assert!(!closing_tag_omitted("span", Some("span")));
    }
}
