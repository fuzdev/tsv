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
// Performance: each set is a `phf::Set` (compile-time perfect hash, no runtime init) behind a
// shape pre-filter — see `element_set!`.

use phf::phf_map;

/// The shape every name in one element set shares: a length range, and, per first letter, the
/// set of lengths a name starting with it can have. Enough to reject a tag before hashing it.
struct Shape {
    /// `len_mask[c - b'a']` has bit `n` set iff some name starts with `c` and is `n` bytes long.
    len_mask: [u32; 26],
    min_len: usize,
    max_len: usize,
}

/// Derive the shape from the name list itself, so the filter and the set can never drift apart —
/// and assert, at compile time, the two invariants the filter rests on. A name that broke either
/// (an uppercase or punctuation first byte, a 32-byte name) would make the filter silently blind
/// to it, so it fails the build instead.
const fn shape_of(names: &[&'static str]) -> Shape {
    let mut len_mask = [0u32; 26];
    let mut min_len = usize::MAX;
    let mut max_len = 0;
    let mut i = 0;
    while i < names.len() {
        let bytes = names[i].as_bytes();
        let first = bytes[0];
        assert!(
            first.is_ascii_lowercase(),
            "element name must start with an ASCII lowercase letter"
        );
        assert!(bytes.len() < 32, "element name must be under 32 bytes");
        len_mask[(first - b'a') as usize] |= 1 << bytes.len();
        if bytes.len() < min_len {
            min_len = bytes.len();
        }
        if bytes.len() > max_len {
            max_len = bytes.len();
        }
        i += 1;
    }
    Shape {
        len_mask,
        min_len,
        max_len,
    }
}

/// Whether `tag_name` could be in the set this shape came from. A `false` is decisive — the name
/// is not a member, and the hash is skipped. A `true` proves nothing and the caller still probes.
///
/// This is the whole point: on real markup almost every probe is a **miss** — a `<div>` is asked
/// whether it is void (it is not) and whether it is foreign (twice: SVG, then MathML), and every
/// component (`<Button>`) is asked all three. Those questions are answered here, by a length
/// compare and one table load, instead of by a perfect hash over the name's bytes.
#[inline]
fn shape_admits(shape: &Shape, tag_name: &str) -> bool {
    let bytes = tag_name.as_bytes();
    let len = bytes.len();
    // Also what makes `1 << len` well-defined below: `max_len` is under 32 by the compile-time
    // assertion above, so a name that reaches the shift is shorter than the mask is wide.
    if len < shape.min_len || len > shape.max_len {
        return false;
    }
    let first = bytes[0];
    if !first.is_ascii_lowercase() {
        return false;
    }
    shape.len_mask[(first - b'a') as usize] & (1 << len) != 0
}

/// Declare an element set once: the `phf::Set` that answers membership, the `Shape` that fronts
/// it, and the name slice both are built from — plus an exhaustive test grading the filtered
/// lookup against a plain scan of that same list.
///
/// The test is not a formality. A pre-filter is invisible when it is right and invisible when it
/// is *wrong in the rejecting direction*: it would simply start answering "not an element" for a
/// real one, and the formatter would go on emitting perfectly valid — differently laid out —
/// markup. Keep it green; it is the only thing that can fail.
macro_rules! element_set {
    ($set:ident, $shape:ident, $names:ident, $doc:literal, [$($name:literal),* $(,)?]) => {
        #[doc = $doc]
        static $set: phf::Set<&'static str> = phf::phf_set! { $($name),* };
        /// The same list, as a slice — the source both `$shape` and the equivalence test read.
        const $names: &[&'static str] = &[$($name),*];
        const $shape: Shape = shape_of($names);
    };
}

element_set! {
    BLOCK_ELEMENTS,
    BLOCK_SHAPE,
    BLOCK_NAMES,
    "Block elements for formatting purposes.\n\n\
     Elements NOT in this list are treated as inline for formatting (including table cells), \
     matching prettier's logic: `isInlineElement = !isBlockElement`. Intentionally absent: \
     `<center>`, `<select>`, `<svg>`, `<math>` — prettier-plugin-svelte omits these, and \
     `<svg>`/`<math>` are handled separately as foreign elements. `<menu>` IS included (spec \
     compliance) where prettier-plugin-svelte omits it: the HTML spec treats it identically to \
     `<ul>`. See `tests/fixtures/svelte/elements/menu_block_prettier_divergence/README.md`.",
    [
        "address", "article", "aside", "blockquote", "details", "dialog", "dd", "div", "dl", "dt",
        "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6",
        "header", "hgroup", "hr", "li", "main", "menu", "nav", "ol", "p", "pre", "section",
        "table", "ul",
    ]
}

element_set! {
    VOID_ELEMENTS,
    VOID_SHAPE,
    VOID_NAMES,
    "Void (self-closing) elements — Svelte's `VOID_ELEMENT_NAMES`.\n\n\
     `command` and `keygen` are obsolete (removed from the HTML spec) but included for Svelte \
     parity — the parser (tsv_svelte) also includes them. `!doctype` is NOT here: it is the one \
     case-insensitive member, so `is_void_element` matches it separately.",
    [
        "area", "base", "br", "col", "command", "embed", "hr", "img", "input", "keygen", "link",
        "meta", "param", "source", "track", "wbr",
    ]
}

element_set! {
    SVG_ELEMENTS,
    SVG_SHAPE,
    SVG_NAMES,
    "SVG elements — synced with Svelte's `utils.js` `SVG_ELEMENTS`.",
    [
        "altGlyph", "altGlyphDef", "altGlyphItem", "animate", "animateColor", "animateMotion",
        "animateTransform", "circle", "clipPath", "color-profile", "cursor", "defs", "desc",
        "discard", "ellipse", "feBlend", "feColorMatrix", "feComponentTransfer", "feComposite",
        "feConvolveMatrix", "feDiffuseLighting", "feDisplacementMap", "feDistantLight",
        "feDropShadow", "feFlood", "feFuncA", "feFuncB", "feFuncG", "feFuncR", "feGaussianBlur",
        "feImage", "feMerge", "feMergeNode", "feMorphology", "feOffset", "fePointLight",
        "feSpecularLighting", "feSpotLight", "feTile", "feTurbulence", "filter", "font",
        "font-face", "font-face-format", "font-face-name", "font-face-src", "font-face-uri",
        "foreignObject", "g", "glyph", "glyphRef", "hatch", "hatchpath", "hkern", "image", "line",
        "linearGradient", "marker", "mask", "mesh", "meshgradient", "meshpatch", "meshrow",
        "metadata", "missing-glyph", "mpath", "path", "pattern", "polygon", "polyline",
        "radialGradient", "rect", "set", "solidcolor", "stop", "svg", "switch", "symbol", "text",
        "textPath", "title", "tref", "tspan", "unknown", "use", "view", "vkern",
    ]
}

element_set! {
    MATHML_ELEMENTS,
    MATHML_SHAPE,
    MATHML_NAMES,
    "MathML elements — synced with Svelte's `utils.js` (MathML Core).",
    [
        "annotation", "annotation-xml", "maction", "math", "merror", "mfrac", "mi",
        "mmultiscripts", "mn", "mo", "mover", "mpadded", "mphantom", "mprescripts", "mroot",
        "mrow", "ms", "mspace", "msqrt", "mstyle", "msub", "msubsup", "msup", "mtable", "mtd",
        "mtext", "mtr", "munder", "munderover", "semantics",
    ]
}

/// Check if an HTML element is block (flow content)
///
/// Block elements create rectangular blocks and typically start on new lines.
/// Examples: `<div>`, `<p>`, `<section>`
#[inline]
pub fn is_block_element(tag_name: &str) -> bool {
    shape_admits(&BLOCK_SHAPE, tag_name) && BLOCK_ELEMENTS.contains(tag_name)
}

/// Check if an HTML element is void (self-closing)
///
/// Void elements cannot have children and don't need closing tags.
/// Examples: `<br>`, `<img>`, `<input>`
#[inline]
pub fn is_void_element(tag_name: &str) -> bool {
    // `!doctype` is the one case-insensitive member and the one that does not fit the shape (it
    // opens on `!`), so it sits outside the set and is matched on its own.
    (shape_admits(&VOID_SHAPE, tag_name) && VOID_ELEMENTS.contains(tag_name))
        || tag_name.eq_ignore_ascii_case("!doctype")
}

/// Whether inter-sibling whitespace inside this element is removed **entirely** by Svelte's
/// compiler (`clean_nodes` `can_remove_entirely`) rather than collapsed to a rendered space.
///
/// These containers never render whitespace between their children, so a formatter may lay them
/// out block-style with no inter-sibling space and stay render-equivalent. This is the **exact**
/// Svelte set — a deliberate *subset* of what HTML collapses (Svelte's source carries a
/// `// TODO others?`), verified element-by-element against the compiler: `optgroup` / `ul` / `ol`
/// / `menu` / `dl` / `fieldset` are **not** members (Svelte keeps their inter-sibling space, so a
/// drop-in must too). The other arm of `can_remove_entirely` — any SVG element outside a `<text>`
/// element — is a namespace-level rule the caller applies, not a tag-name question, so it is not
/// covered here.
#[inline]
pub fn collapses_child_whitespace(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "select" | "table" | "tbody" | "thead" | "tfoot" | "tr" | "colgroup" | "datalist"
    )
}

/// Check if an element is an SVG element
#[inline]
pub fn is_svg_element(tag_name: &str) -> bool {
    shape_admits(&SVG_SHAPE, tag_name) && SVG_ELEMENTS.contains(tag_name)
}

/// Check if an element is a MathML element
#[inline]
pub fn is_mathml_element(tag_name: &str) -> bool {
    shape_admits(&MATHML_SHAPE, tag_name) && MATHML_ELEMENTS.contains(tag_name)
}

/// Check if an element is foreign content (SVG or MathML)
#[inline]
pub fn is_foreign_element(tag_name: &str) -> bool {
    is_svg_element(tag_name) || is_mathml_element(tag_name)
}

/// A `PCENChar` — a character the [HTML "valid custom element name"][spec] grammar
/// admits in a custom element's name after its ASCII start, i.e. in the run after the
/// first hyphen (`<my-café>`, `<emotion-😍>`, `<a-·>`). The ranges are the grammar's
/// (`PotentialCustomElementName`) verbatim; surrogate code points are unreachable,
/// since a Rust `char` is never a surrogate.
///
/// Used both by the tokenizer (to keep a whole custom-element name in one token,
/// including the non-alphanumeric members `·`/ZWNJ/ZWJ/astral) and by the name
/// validator (the hyphen-tail run) — a single source of truth for the ranges.
///
/// [spec]: https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
pub fn is_pcen_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '-' | '.' | '_' | '\u{00B7}')
        || matches!(c,
            '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00F6}' | '\u{00F8}'..='\u{037D}'
            | '\u{037F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' | '\u{203F}'..='\u{2040}'
            | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
            | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
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

    /// Grade every shape-filtered lookup against a plain linear scan of the **same** name list the
    /// filter was derived from. A pre-filter fails silently and in the safe-looking direction — it
    /// starts answering "not an element" for a real one, and the formatter goes on emitting valid
    /// markup, just laid out differently — so nothing downstream can be relied on to notice.
    ///
    /// The alphabet covers every arm: real members, near-misses one byte off, the first-letter and
    /// length gates, the case gate (a Svelte **component** is exactly an uppercase-initial tag, and
    /// it must be rejected by shape, never by hash), and the non-ASCII and empty inputs.
    #[test]
    fn shape_filter_agrees_with_a_plain_scan() {
        let probes: Vec<String> = BLOCK_NAMES
            .iter()
            .chain(VOID_NAMES)
            .chain(SVG_NAMES)
            .chain(MATHML_NAMES)
            .flat_map(|name| {
                [
                    (*name).to_string(),
                    name.to_uppercase(), // a component-cased tag
                    format!("{}{}", &name[..1].to_uppercase(), &name[1..]),
                    format!("{name}x"),                 // one byte longer
                    name[..name.len() - 1].to_string(), // one byte shorter
                    format!("x{name}"),                 // different first byte
                ]
            })
            .chain(
                [
                    "",
                    "div",
                    "Div",
                    "DIV",
                    "span",
                    "Button",
                    "MyComponent",
                    "svg",
                    "Svg",
                    "!doctype",
                    "!DOCTYPE",
                    "h1",
                    "h7",
                    "p",
                    "é",
                    "ünknown",
                    "annotation-xml",
                    "foreignObject",
                    "a",
                    "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
                ]
                .iter()
                .map(|s| (*s).to_string()),
            )
            .collect();

        for tag in &probes {
            assert_eq!(
                is_block_element(tag),
                BLOCK_NAMES.contains(&tag.as_str()),
                "is_block_element disagrees with a plain scan on {tag:?}"
            );
            assert_eq!(
                is_void_element(tag),
                VOID_NAMES.contains(&tag.as_str()) || tag.eq_ignore_ascii_case("!doctype"),
                "is_void_element disagrees with a plain scan on {tag:?}"
            );
            assert_eq!(
                is_svg_element(tag),
                SVG_NAMES.contains(&tag.as_str()),
                "is_svg_element disagrees with a plain scan on {tag:?}"
            );
            assert_eq!(
                is_mathml_element(tag),
                MATHML_NAMES.contains(&tag.as_str()),
                "is_mathml_element disagrees with a plain scan on {tag:?}"
            );
        }
    }

    /// The filter may only ever *reject*: every real member must survive it, or the hash behind it
    /// is unreachable and the element silently stops being classified.
    #[test]
    fn shape_admits_every_member_of_its_own_set() {
        for (shape, names) in [
            (&BLOCK_SHAPE, BLOCK_NAMES),
            (&VOID_SHAPE, VOID_NAMES),
            (&SVG_SHAPE, SVG_NAMES),
            (&MATHML_SHAPE, MATHML_NAMES),
        ] {
            for name in names {
                assert!(
                    shape_admits(shape, name),
                    "shape rejects its own member {name:?}"
                );
            }
        }
    }

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
    fn test_collapses_child_whitespace() {
        // The exact Svelte `can_remove_entirely` name set.
        for tag in [
            "select", "table", "tbody", "thead", "tfoot", "tr", "colgroup", "datalist",
        ] {
            assert!(collapses_child_whitespace(tag), "member: {tag:?}");
        }
        // Deliberately NOT members — HTML collapses these but Svelte's set omits them
        // (`// TODO others?`), and tsv must match Svelte, not raw HTML.
        for tag in [
            "optgroup", "ul", "ol", "menu", "dl", "fieldset", "td", "th", "option", "div", "span",
            "svg",
        ] {
            assert!(!collapses_child_whitespace(tag), "non-member: {tag:?}");
        }
        // Case-sensitive: callers pass already-lowercased tag names.
        assert!(!collapses_child_whitespace("TABLE"));
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
