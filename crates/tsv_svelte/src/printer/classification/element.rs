// Element type classification adapters for Svelte printer
//
// These methods extend Printer to provide convenient element classification
// by wrapping the pure language-level functions from the tsv_html crate.
//
// The printer-specific part is resolving symbols (interned strings) to
// tag names. The actual classification logic lives in tsv_html
// and can be reused by other tools (linter, type-checker, language server).

use crate::ast::internal;
use crate::ast::internal::ElementKind;
use crate::printer::Printer;
use string_interner::DefaultSymbol;
use tsv_html as html;
use tsv_lang::{SymbolResolver, SymbolToU32};

/// Every classification fact derivable from a tag *name* alone, packed for the per-document
/// memo on [`Printer::tag_facts`].
///
/// Nothing element-instance-specific lives here — a `<script>`'s has-content overlay stays in
/// [`Printer::is_block_element`], and the Component early-return reads the parse-time
/// `ElementKind`, not the name. The printer's element/fragment/sibling paths re-ask these
/// questions many times per element (the sibling/child probes alone outnumber elements ~5×),
/// and a document holds only a handful of distinct tag names, so each fact is computed once
/// per (document, symbol) and read back as one vector index + bit test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TagFacts(u16);

impl TagFacts {
    /// Marks a computed entry: the memo vector holds `0` for never-computed slots, and
    /// every computed entry carries this bit so a legitimately all-false fact set is still
    /// distinguishable from an empty slot.
    const FILLED: u16 = 1 << 0;
    /// `tsv_html::is_block_element` (flow content).
    const BLOCK: u16 = 1 << 1;
    /// `tsv_html::is_void_element` (`<br>`, `<img>`, `!doctype`).
    const VOID: u16 = 1 << 2;
    /// `tsv_html::is_foreign_element` (SVG or MathML).
    const FOREIGN: u16 = 1 << 3;
    /// Component-shaped name — the parser's [`internal::is_component_name`] (Unicode-uppercase
    /// initial or a dotted member name — `Button`, `Δcomp`, `foo.bar`). Drives the `Component`
    /// element kind. A `:`-namespaced name (`foo:bar`) is a `RegularElement`, not a component,
    /// so it is *not* here — it carries [`NAMESPACED`](Self::NAMESPACED) instead.
    const COMPONENT_NAME: u16 = 1 << 4;
    const STYLE: u16 = 1 << 5;
    const SCRIPT: u16 = 1 << 6;
    const TEMPLATE: u16 = 1 << 7;
    /// `tsv_html::preserves_whitespace` (`<pre>`, `<textarea>`).
    const WS_SENSITIVE: u16 = 1 << 8;
    /// `<!DOCTYPE>`-style declaration (leading `!`), which closes with `>`, not `/>`.
    const DECLARATION: u16 = 1 << 9;
    /// A `:` in the name (`<foo:bar>`) — a namespaced `RegularElement`. Independent of
    /// [`COMPONENT_NAME`](Self::COMPONENT_NAME): it takes the inline element kind like any other
    /// non-block regular element, but may still print self-closing (prettier's `didSelfClose`),
    /// so it is the third contributor to `can_self_close` alongside component and foreign.
    const NAMESPACED: u16 = 1 << 10;

    /// Derive the facts from the tag name. The single source: the memo stores exactly this,
    /// and the equivalence test below grades every accessor against the predicates named here.
    fn compute(tag_name: &str) -> Self {
        let mut bits = Self::FILLED;
        if html::is_block_element(tag_name) {
            bits |= Self::BLOCK;
        }
        if html::is_void_element(tag_name) {
            bits |= Self::VOID;
        }
        if html::is_foreign_element(tag_name) {
            bits |= Self::FOREIGN;
        }
        if internal::is_component_name(tag_name) {
            bits |= Self::COMPONENT_NAME;
        }
        if tag_name.contains(':') {
            bits |= Self::NAMESPACED;
        }
        if tag_name == "style" {
            bits |= Self::STYLE;
        }
        if tag_name == "script" {
            bits |= Self::SCRIPT;
        }
        if tag_name == "template" {
            bits |= Self::TEMPLATE;
        }
        if html::preserves_whitespace(tag_name) {
            bits |= Self::WS_SENSITIVE;
        }
        if tag_name.starts_with('!') {
            bits |= Self::DECLARATION;
        }
        Self(bits)
    }

    pub(crate) fn is_block(self) -> bool {
        self.0 & Self::BLOCK != 0
    }
    pub(crate) fn is_void(self) -> bool {
        self.0 & Self::VOID != 0
    }
    pub(crate) fn is_foreign(self) -> bool {
        self.0 & Self::FOREIGN != 0
    }
    pub(crate) fn is_component_name(self) -> bool {
        self.0 & Self::COMPONENT_NAME != 0
    }
    pub(crate) fn is_namespaced(self) -> bool {
        self.0 & Self::NAMESPACED != 0
    }
    pub(crate) fn is_style(self) -> bool {
        self.0 & Self::STYLE != 0
    }
    pub(crate) fn is_script(self) -> bool {
        self.0 & Self::SCRIPT != 0
    }
    pub(crate) fn is_template(self) -> bool {
        self.0 & Self::TEMPLATE != 0
    }
    pub(crate) fn is_ws_sensitive(self) -> bool {
        self.0 & Self::WS_SENSITIVE != 0
    }
    pub(crate) fn is_declaration(self) -> bool {
        self.0 & Self::DECLARATION != 0
    }
}

impl<'a> Printer<'a> {
    /// The name-derived facts for `name`, memoized per document.
    ///
    /// A hit is one vector index; a miss resolves the symbol once and computes. Sound because
    /// a symbol resolves to the same string for the printer's whole lifetime (the interner is
    /// per-document and append-only), so the facts can never go stale. In debug builds every
    /// hit is re-derived from the resolved name and asserted equal — the fixture and test
    /// suites grade the memo on every element they format.
    pub(crate) fn tag_facts(&self, name: DefaultSymbol) -> TagFacts {
        let idx = name.to_u32() as usize;
        if let Some(&bits) = self.tag_facts_cache.borrow().get(idx)
            && bits != 0
        {
            debug_assert!(
                self.with_resolved_symbol(name, TagFacts::compute) == TagFacts(bits),
                "memoized TagFacts disagree with a fresh compute for symbol {idx}"
            );
            return TagFacts(bits);
        }
        let facts = self.with_resolved_symbol(name, TagFacts::compute);
        let mut cache = self.tag_facts_cache.borrow_mut();
        if cache.len() <= idx {
            cache.resize(idx + 1, 0);
        }
        cache[idx] = facts.0;
        facts
    }

    /// Check if element is block (flow content)
    ///
    /// Adapter over the memoized name facts plus the two element-instance overlays.
    ///
    /// Components are treated as inline, not block elements.
    ///
    /// Note: `<script>` and `<style>` elements with content are treated as block
    /// elements for formatting purposes, since their content will be formatted
    /// on separate lines. Empty `<script>`/`<style>` remain inline.
    pub(crate) fn is_block_element(&self, element: &internal::Element<'_>) -> bool {
        // Components are treated as inline, not block
        if element.kind == ElementKind::Component {
            return false;
        }

        let facts = self.tag_facts(element.name);

        // <script>/<style> are block only when they carry real content, which
        // formats on its own lines. An empty <script></script> / <style></style>
        // stays inline (prettier parity). The raw-text parser always emits one
        // (possibly empty) Text node, so node-presence alone is not "has content".
        if (facts.is_script() || facts.is_style()) && has_raw_content(element) {
            return true;
        }

        facts.is_block()
    }
}

/// Whether a raw-text element (`<script>`/`<style>`) carries non-empty content.
/// Raw-text parsing emits exactly one `Text` node whose `raw` is the verbatim
/// body (empty for `<script></script>`), so an empty `raw` means no content.
fn has_raw_content(element: &internal::Element<'_>) -> bool {
    use crate::ast::internal::FragmentNode;
    element
        .fragment
        .nodes
        .iter()
        .any(|node| !matches!(node, FragmentNode::Text(t) if t.raw_span.range().is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::internal::FragmentNode;
    use crate::printer::Printer;
    use std::rc::Rc;

    /// Find the first child element of `parent` whose resolved tag name is `tag`.
    fn child<'p, 'arena>(
        printer: &Printer<'_>,
        parent: &'p internal::Element<'arena>,
        tag: &str,
    ) -> &'p internal::Element<'arena> {
        parent
            .fragment
            .nodes
            .iter()
            .find_map(|n| match n {
                FragmentNode::Element(el)
                    if printer.with_resolved_symbol(el.name, |n| n == tag) =>
                {
                    Some(el)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no <{tag}> child"))
    }

    #[test]
    fn block_adapter_delegates_and_treats_components_as_inline() {
        let src = "<div><span>i</span><Comp>c</Comp></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        // Reuse the parse's interner so the tag-name symbols resolve.
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, Rc::clone(&root.interner), &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        // Plain HTML tags delegate straight to tsv_html: <div> block, <span> inline.
        assert!(printer.is_block_element(div));
        assert!(!printer.is_block_element(child(&printer, div, "span")));
        // A component is always inline, regardless of its (uppercase) name.
        assert!(!printer.is_block_element(child(&printer, div, "Comp")));
    }

    #[test]
    fn block_adapter_promotes_nonempty_script_style_to_block() {
        // The overlay is the printer-specific part: a <script>/<style> with content
        // is block (its body formats on its own lines), even though tsv_html
        // classifies the bare tag as inline.
        assert!(!html::is_block_element("script"));
        assert!(!html::is_block_element("style"));

        let src = "<div><script>let x = 1;</script><style>a { color: red }</style></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, Rc::clone(&root.interner), &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        assert!(printer.is_block_element(child(&printer, div, "script")));
        assert!(printer.is_block_element(child(&printer, div, "style")));
    }

    #[test]
    fn block_adapter_treats_empty_script_style_as_inline() {
        // An empty <script></script> / <style></style> has no content to format
        // on its own lines, so it stays inline (prettier keeps the parent on one
        // line). The raw-text parser still emits a single empty Text node here, so
        // `has_raw_content` — not node-presence — is what makes this inline.
        let src = "<div><script></script><style></style></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, Rc::clone(&root.interner), &[]);
        let div = match &root.fragment.nodes[0] {
            FragmentNode::Element(el) => el,
            other => panic!("expected a <div>, got: {other:?}"),
        };

        assert!(!printer.is_block_element(child(&printer, div, "script")));
        assert!(!printer.is_block_element(child(&printer, div, "style")));
    }

    /// Grade every packed [`TagFacts`] accessor against the pure predicate it encodes, over an
    /// alphabet covering each bit's positive and negative cases. This is the gate with power
    /// over the bit packing: a swapped constant or an accessor reading its neighbour's bit
    /// changes layout only on rare tags at rare widths, which no fixture or corpus diff can be
    /// relied on to see.
    #[test]
    fn tag_facts_bits_agree_with_the_pure_predicates() {
        let probes = [
            // block members (hr is also void; pre is also ws-sensitive)
            "div",
            "p",
            "h1",
            "menu",
            "table",
            "ul",
            "li",
            "pre",
            "hr",
            "blockquote",
            // void members (incl. the case-insensitive !doctype family)
            "br",
            "img",
            "input",
            "command",
            "keygen",
            "!doctype",
            "!DOCTYPE",
            "!DocType",
            // foreign members (SVG incl. camelCase + hyphenated; MathML)
            "svg",
            "circle",
            "foreignObject",
            "color-profile",
            "math",
            "annotation-xml",
            "mi",
            // the name-compare bits
            "script",
            "style",
            "template",
            "textarea",
            // component-shaped names (incl. non-ASCII uppercase initials — Greek, Latin, Cyrillic)
            "Button",
            "MyComponent",
            "svelte:head",
            "svelte:component",
            "foo:bar",
            "foo.bar",
            "Div",
            "DIV",
            "Δcomp",
            "Écomp",
            "Яcomp",
            "étoile",
            // near-misses and odd inputs
            "span",
            "td",
            "divx",
            "di",
            "xdiv",
            "doctype",
            "é",
            "ünknown",
            "",
        ];
        for tag in probes {
            let facts = TagFacts::compute(tag);
            assert_eq!(
                facts.is_block(),
                html::is_block_element(tag),
                "block: {tag:?}"
            );
            assert_eq!(facts.is_void(), html::is_void_element(tag), "void: {tag:?}");
            assert_eq!(
                facts.is_foreign(),
                html::is_foreign_element(tag),
                "foreign: {tag:?}"
            );
            assert_eq!(
                facts.is_component_name(),
                internal::is_component_name(tag),
                "component name: {tag:?}"
            );
            assert_eq!(
                facts.is_namespaced(),
                tag.contains(':'),
                "namespaced: {tag:?}"
            );
            assert_eq!(facts.is_style(), tag == "style", "style: {tag:?}");
            assert_eq!(facts.is_script(), tag == "script", "script: {tag:?}");
            assert_eq!(facts.is_template(), tag == "template", "template: {tag:?}");
            assert_eq!(
                facts.is_ws_sensitive(),
                html::preserves_whitespace(tag),
                "ws-sensitive: {tag:?}"
            );
            assert_eq!(
                facts.is_declaration(),
                tag.starts_with('!'),
                "declaration: {tag:?}"
            );
        }
    }

    /// The memo must be invisible: every read-back — first ask, repeat ask, interleaved with
    /// other symbols — must equal a fresh compute of the resolved name. Catches a keying slip
    /// (an off-by-one index returns the neighbouring tag's facts), which formats real markup
    /// with the wrong element classes while every individual predicate stays correct.
    #[test]
    fn tag_facts_memo_agrees_with_fresh_compute() {
        let src = "<div><span>i</span><Comp>c</Comp><svg><circle /></svg><pre>p</pre>\
                   <script>let x = 1;</script></div>";
        let arena = bumpalo::Bump::new();
        let root = crate::parse(src, &arena).expect("template should parse");
        let doc_arena = tsv_lang::doc::arena::DocArena::for_source(src);
        let printer = Printer::new(&doc_arena, src, Rc::clone(&root.interner), &[]);

        fn collect<'p, 'arena>(
            nodes: &'p [FragmentNode<'arena>],
            out: &mut Vec<&'p internal::Element<'arena>>,
        ) {
            for node in nodes {
                if let FragmentNode::Element(el) = node {
                    out.push(el);
                    collect(el.fragment.nodes, out);
                }
            }
        }
        let mut elements = Vec::new();
        collect(root.fragment.nodes, &mut elements);
        assert!(
            elements.len() >= 6,
            "probe template should hold every element family"
        );

        // Two interleaved rounds: round 1 fills the memo, round 2 reads it back.
        for _ in 0..2 {
            for el in &elements {
                let fresh = printer.with_resolved_symbol(el.name, TagFacts::compute);
                assert_eq!(
                    printer.tag_facts(el.name),
                    fresh,
                    "memoized facts diverge for {:?}",
                    printer.with_resolved_symbol(el.name, str::to_owned)
                );
            }
        }
    }
}
