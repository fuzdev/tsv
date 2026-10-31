// HTML element type classification (language-level)
//
// Pure functions for classifying HTML elements by their rendering characteristics.
// These are language-level utilities independent of any specific tool (printer,
// linter, type-checker, etc.)
//
// References:
// - the block set: the UA stylesheet's `display: block` element list — ../html/source,
//   Rendering §"Flow content" (the section's title; HTML's flow-content *category* is wider,
//   holding every inline element too) — as prettier-plugin-svelte lists it
// - Svelte void elements: node_modules/svelte/src/utils.js:16-41
// - prettier-plugin-svelte: isInlineElement = !isBlockElement (negation, no positive list)
//
// Performance: each set is a `match` over its names behind a shape pre-filter — see
// `element_set!`.

/// The shape every name in one element set shares: a length range, and, per first letter, the
/// set of lengths a name starting with it can have. Enough to reject a tag before comparing it.
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
/// is not a member, and the membership `match` is skipped. A `true` proves nothing and the caller
/// still probes.
///
/// This is the whole point: on real markup almost every probe is a **miss** — a `<div>` is asked
/// whether it is void (it is not) and whether it is foreign (twice: SVG, then MathML), and every
/// component (`<Button>`) is asked all three. Those questions are answered here, by a length
/// compare and one table load, instead of by comparing the name against the set's members.
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

/// Declare an element set once: the membership function (a `match` over the names), the `Shape`
/// that fronts it, and the name slice the shape is built from — plus an exhaustive test grading
/// the filtered lookup against a plain scan of that same list.
///
/// A `match` rather than a perfect hash: after the shape filter, the names a probe can still be
/// are a few short strings, which a length switch and a few word compares answer outright, where
/// a hash reads every byte of the name through SipHash before it can compare anything.
///
/// The test is not a formality. A pre-filter is invisible when it is right and invisible when it
/// is *wrong in the rejecting direction*: it would simply start answering "not an element" for a
/// real one, and the formatter would go on emitting perfectly valid — differently laid out —
/// markup. Keep it green; it is the only thing that can fail.
macro_rules! element_set {
    ($contains:ident, $shape:ident, $names:ident, $doc:literal, [$($name:literal),* $(,)?]) => {
        #[doc = $doc]
        #[inline]
        fn $contains(tag_name: &str) -> bool {
            matches!(tag_name, $($name)|*)
        }
        /// The same list, as a slice — the source both `$shape` and the equivalence test read.
        const $names: &[&'static str] = &[$($name),*];
        const $shape: Shape = shape_of($names);
    };
}

element_set! {
    block_set_contains,
    BLOCK_SHAPE,
    BLOCK_NAMES,
    "Block elements for formatting purposes.\n\n\
     Elements NOT in this list are treated as inline for formatting (including table cells), \
     matching prettier's logic: `isInlineElement = !isBlockElement`. Intentionally absent: \
     `<center>`, `<select>`, `<svg>`, `<math>` — prettier-plugin-svelte omits these, and \
     `<svg>`/`<math>` are handled separately as foreign elements. Two members are here for spec \
     compliance where prettier-plugin-svelte omits them: `<menu>`, which the HTML spec treats \
     identically to `<ul>`, and `<summary>`, whose every UA rule is block-level — plain \
     `display: block`, and `display: list-item` (`li`'s own display, and `li` is a member) when \
     it is a `details`' first summary. Note this list tracks UA *display*, not content model: \
     `<p>` and `<summary>` are both phrasing-content-only and both block. See \
     `tests/fixtures/svelte/elements/menu_block_prettier_divergence/README.md` and \
     `tests/fixtures/svelte/elements/summary_block_prettier_divergence/README.md`.",
    [
        "address", "article", "aside", "blockquote", "details", "dialog", "dd", "div", "dl", "dt",
        "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6",
        "header", "hgroup", "hr", "li", "main", "menu", "nav", "ol", "p", "pre", "section",
        "summary", "table", "ul",
    ]
}

element_set! {
    void_set_contains,
    VOID_SHAPE,
    VOID_NAMES,
    "Void (self-closing) elements — Svelte's `VOID_ELEMENT_NAMES`.\n\n\
     `command` and `keygen` are obsolete (removed from the HTML spec) but included for Svelte \
     parity — the parser (tsv_svelte) also includes them. Svelte's `is_void` also answers \
     `true` for a `!doctype` declaration; that is no element, so it is not here — a caller \
     that reads Svelte's answer adds it (`tsv_svelte`'s `is_svelte_void`).",
    [
        "area", "base", "br", "col", "command", "embed", "hr", "img", "input", "keygen", "link",
        "meta", "param", "source", "track", "wbr",
    ]
}

element_set! {
    svg_set_contains,
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
    mathml_set_contains,
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

/// Whether an HTML element is **block-level for formatting**: the elements whose user-agent
/// style renders a box of its own — `display: block` (`<div>`, `<p>`, `<section>`, …), plus
/// `<li>` (`list-item`) and `<table>` (`table`) — as prettier-plugin-svelte lists them, with
/// `<menu>` and `<summary>` added where the plugin omits them.
///
/// Not "flow content": in HTML's content model flow content includes every inline
/// (phrasing) element too, while this set holds only elements that render as blocks — and
/// not all of them: `<center>`, `<legend>`, `<search>`, `<listing>`, `<plaintext>` and
/// `<xmp>` render as blocks but format inline, as in the plugin. Everything outside the set
/// formats as inline, table cells included (prettier's `isInlineElement = !isBlockElement`).
#[inline]
pub fn is_block_element(tag_name: &str) -> bool {
    shape_admits(&BLOCK_SHAPE, tag_name) && block_set_contains(tag_name)
}

/// Whether an HTML element is **void**: it cannot have children and takes no closing tag —
/// `<br>`, `<img>`, `<input>`, … — Svelte's `VOID_ELEMENT_NAMES`, the obsolete `<command>` and
/// `<keygen>` included.
///
/// Elements only: a `<!doctype>` declaration is not one, though Svelte's own `is_void` answers
/// `true` for it.
#[inline]
pub fn is_void_element(tag_name: &str) -> bool {
    shape_admits(&VOID_SHAPE, tag_name) && void_set_contains(tag_name)
}

/// Whether this element IS a rendered line break — `<br>` alone. The [spec][br] says it
/// outright: the element "represents a line break", to "be used only for line breaks that are
/// actually part of the content, as in poems or addresses" — so the break is authored CONTENT,
/// not layout.
///
/// Deliberately narrower than [`is_void_element`]: the other void elements (`<img>`,
/// `<input>`, …) render inline, and prose reflows around them as around any inline element —
/// while a `<br>` renders the very break a source newline beside it mirrors, so a
/// prose-reflowing formatter holds the lines a `<br>` bounds. `<wbr>` is NOT a member: it
/// ["represents a line break opportunity"][wbr] — a wrap affordance inside flowing prose (the
/// spec's own example places it mid-word), not a break — so it reflows with the run like
/// `<img>` and `<input>` do, and measuring confirms the line: prettier formats a `<wbr>`
/// exactly as it formats a `<span>` in the same shapes, and a `<br>` unlike either.
///
/// [br]: https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-br-element
/// [wbr]: https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-wbr-element
#[inline]
pub fn is_line_break_element(tag_name: &str) -> bool {
    tag_name == "br"
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

/// Whether an element is an SVG element — Svelte's `SVG_ELEMENTS`, in SVG's own spelling
/// (`clipPath`, `foreignObject`: the camelCase names are members as written, and only so).
/// A name list, not namespace inference: it answers the name alone, with no ancestor. It
/// holds `title`, a name SVG shares with HTML, so `<title>` answers `true` anywhere (Svelte's
/// list omits it and asks the ancestor instead); `a`, `script` and `style`, also shared,
/// answer `false`.
#[inline]
pub fn is_svg_element(tag_name: &str) -> bool {
    shape_admits(&SVG_SHAPE, tag_name) && svg_set_contains(tag_name)
}

/// Whether an element is a MathML element — Svelte's MathML Core list (`MATHML_ELEMENTS`),
/// by name alone, with no ancestor.
#[inline]
pub fn is_mathml_element(tag_name: &str) -> bool {
    shape_admits(&MATHML_SHAPE, tag_name) && mathml_set_contains(tag_name)
}

/// Whether an element is foreign content: an SVG ([`is_svg_element`]) or MathML
/// ([`is_mathml_element`]) element — by name alone, so `title` answers `true` and `a`,
/// `script` and `style` answer `false`, as those two do.
#[inline]
pub fn is_foreign_element(tag_name: &str) -> bool {
    is_svg_element(tag_name) || is_mathml_element(tag_name)
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
    /// it must be rejected by shape, never by the membership `match`), and the non-ASCII and empty
    /// inputs.
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
                    format!("{}{}", name[..1].to_uppercase(), &name[1..]),
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
                    "!",
                    "!doctypex",
                    "!DOCTYPEX",
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
                VOID_NAMES.contains(&tag.as_str()),
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

    /// The filter may only ever *reject*: every real member must survive it, or the `match` behind
    /// it is unreachable and the element silently stops being classified.
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
        // Membership is exact: tag names arrive lowercased.
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
        // A doctype declaration is no element, in any spelling.
        assert!(!is_void_element("!doctype"));
        assert!(!is_void_element("!DOCTYPE"));
        assert!(!is_void_element("doctype"));
    }

    #[test]
    fn test_is_line_break_element() {
        assert!(is_line_break_element("br"));
        // Narrower than void: the other void elements render inline.
        assert!(!is_line_break_element("img"));
        assert!(!is_line_break_element("input"));
        assert!(!is_line_break_element("hr"));
        // A line break OPPORTUNITY, not a break — it reflows with the prose around it.
        assert!(!is_line_break_element("wbr"));
        // Case-sensitive like the other regular-element predicates.
        assert!(!is_line_break_element("BR"));
        assert!(!is_line_break_element("div"));
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
        // Names SVG shares with HTML, answered by name alone (the predicate docs say so).
        assert!(is_svg_element("title"));
        for shared in ["a", "script", "style"] {
            assert!(!is_svg_element(shared), "{shared}");
            assert!(!is_foreign_element(shared), "{shared}");
        }
    }
}
