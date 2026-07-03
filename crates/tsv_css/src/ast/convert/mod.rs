// Conversion from internal AST to public AST
//
// ARCHITECTURE: clean model inside, Svelte's scan semantics at the boundary.
//
// The internal AST is the spec-faithful semantic representation (decoded
// strings/escapes, structured values, normalized once during parsing) and is
// what the FORMATTER derives from. The public JSON strings, by contrast, are
// deliberately reconstructed from RAW SOURCE here, because Svelte's parseCss
// builds them by raw text scanning and tsv's public AST is a drop-in for it:
// - Declaration `property`/`value` — raw split at the colon, block comments
//   stripped, ends trimmed (`read_declaration`/`read_value` semantics; the
//   structured internal value is never re-serialized into the JSON)
// - Declaration `end` — the `;`/`}` terminator scan position
// - Selector names — half-decoded like `read_identifier` (hex escapes decode,
//   identity escapes keep the backslash)
// Spans always index the real file; Svelte's `remove_bom` shift is a
// documented divergence (docs/conformance_svelte.md), not replicated.

use super::internal;
use super::public;
use std::borrow::Cow;
use tsv_lang::Span;
use tsv_lang::source_scan::{TriviaProfile, find_char};

mod translate_typed;
pub use translate_typed::{
    translate_byte_to_char_offsets_typed, translate_style_sheet_byte_to_char_offsets_typed,
};

/// Whether the public AST is being built for a standalone `.css` file or an
/// embedded `<style>` block. `parseCss()` attaches constant `metadata` to
/// `Rule`/`ComplexSelector`/`RelativeSelector` for standalone CSS but never for
/// embedded `<style>`; the converters thread this so one typed-node pass
/// produces both shapes — no separate metadata walk.
#[derive(Debug, Clone, Copy)]
enum AstScope {
    /// Standalone `.css` file (`parseCss()` shape, `metadata` attached).
    Standalone,
    /// Embedded `<style>` block in a `.svelte` file (no `metadata`).
    Embedded,
}

impl AstScope {
    /// Standalone CSS carries `parseCss()` metadata; embedded `<style>` doesn't.
    fn has_metadata(self) -> bool {
        matches!(self, AstScope::Standalone)
    }
}

fn rule_meta(scope: AstScope) -> Option<public::RuleMetadata> {
    scope.has_metadata().then(public::RuleMetadata::default)
}

fn complex_meta(scope: AstScope) -> Option<public::ComplexSelectorMetadata> {
    scope
        .has_metadata()
        .then(public::ComplexSelectorMetadata::default)
}

fn relative_meta(scope: AstScope) -> Option<public::RelativeSelectorMetadata> {
    scope
        .has_metadata()
        .then(public::RelativeSelectorMetadata::default)
}

/// Split a declaration source into property and value, matching Svelte's quirky behavior.
///
/// SVELTE QUIRK: When there's a CSS comment between the property name and the colon,
/// Svelte puts the comment AND the colon into the value instead of the property.
///
/// Example: `color /* comment */ : red`
/// - Normal split: property=`color /* comment */ `, value=`red`
/// - Svelte quirk: property=`color`, value=`/* comment */ : red`
///
/// This is a tokenization bug in Svelte's CSS parser, but we replicate it for compatibility.
/// Our internal AST remains semantically correct; this quirk is only applied in conversion.
///
/// Note: `convert_declaration` runs `strip_css_comments` on the returned value, so the
/// public AST for `color /* c */ : red` ends up as property=`color`, value=`": red"`
/// (Svelte 5.55+ strips block comments from value strings post-split).
fn split_declaration_svelte_compat(decl_source: &str) -> (&str, &str) {
    // The real `property : value` colon is the first one outside any comment or
    // string — a property comment may itself contain a `:` (`color /* x:y */: red`).
    let Some(colon_pos) = find_char(
        decl_source.as_bytes(),
        0,
        decl_source.len(),
        b':',
        TriviaProfile::CSS,
    ) else {
        return (decl_source, "");
    };

    let before_colon = &decl_source[..colon_pos];

    // Look for /* that appears after some property text
    if let Some(comment_idx) = before_colon.find("/*") {
        // Only apply quirk if there's actual property content before the comment
        let before_comment = &before_colon[..comment_idx];
        if !before_comment.trim().is_empty() {
            // SVELTE QUIRK: Comment between property and colon
            // Property = just the text before the comment (trimmed)
            // Value = comment + colon + actual value (everything from comment onward)
            let property = before_comment.trim();
            let value = &decl_source[comment_idx..];
            return (property, value);
        }
    }

    // Normal case: split at colon
    let property = &decl_source[..colon_pos];
    let value = decl_source[colon_pos + 1..].trim_start();
    (property, value)
}

/// Remove all `/* ... */` block comments from a CSS string, then trim outer whitespace.
///
/// Matches Svelte 5.55+ behavior for Declaration `value` and Atrule `prelude` strings:
/// comments are stripped in place (surrounding whitespace preserved), then the result
/// is trimmed.
///
/// String- and url()-aware: `/*` sequences inside `"..."`, `'...'`, or `url(...)` are
/// treated as content, not comments. Unterminated comments are left intact (parse
/// error caught elsewhere).
fn strip_css_comments(input: &str) -> Cow<'_, str> {
    // Fast path: no block-comment delimiter anywhere means nothing is stripped, so
    // the result is just the trimmed input — a borrowed sub-slice, no allocation.
    // (Conservative: a `/*` inside a string/url is preserved either way, so those
    // rare inputs fall to the owned path; correctness is unaffected.)
    if !input.contains("/*") {
        return Cow::Borrowed(input.trim());
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(ch) = rest.chars().next() {
        // Block comment — strip
        if ch == '/' && rest.as_bytes().get(1) == Some(&b'*') {
            if let Some(end_rel) = rest[2..].find("*/") {
                rest = &rest[2 + end_rel + 2..];
                continue;
            }
            // Unterminated — keep verbatim
            out.push_str(rest);
            break;
        }
        // String literal — copy through unchanged (escape-aware)
        if ch == '"' || ch == '\'' {
            emit(&mut out, &mut rest, ch);
            copy_quoted(&mut out, &mut rest, ch);
            continue;
        }
        // url(...) — copy through to matching ')'
        if starts_with_url_open(rest) {
            out.push_str(&rest[..4]);
            rest = &rest[4..];
            copy_balanced_parens(&mut out, &mut rest);
            continue;
        }
        emit(&mut out, &mut rest, ch);
    }
    Cow::Owned(out.trim().to_string())
}

/// Push `ch` to `out` and advance `rest` past it.
fn emit(out: &mut String, rest: &mut &str, ch: char) {
    out.push(ch);
    *rest = &rest[ch.len_utf8()..];
}

/// Copy a CSS string body (opening quote already emitted) through `out`,
/// advancing `rest` past the closing quote. Handles backslash escapes.
fn copy_quoted(out: &mut String, rest: &mut &str, quote: char) {
    while let Some(ch) = rest.chars().next() {
        emit(out, rest, ch);
        if ch == '\\' {
            if let Some(esc) = rest.chars().next() {
                emit(out, rest, esc);
            }
        } else if ch == quote {
            break;
        }
    }
}

/// Copy through `out` until the depth-1 close paren that ends `url(...)` (or eof).
/// Skips over quoted strings so embedded `)` characters are not treated as terminators.
fn copy_balanced_parens(out: &mut String, rest: &mut &str) {
    let mut depth: u32 = 1;
    while depth > 0 {
        let Some(ch) = rest.chars().next() else { break };
        emit(out, rest, ch);
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '"' | '\'' => copy_quoted(out, rest, ch),
            _ => {}
        }
    }
}

/// Whether `s` begins with `url(` (case-insensitive for `url`).
fn starts_with_url_open(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 4
        && bytes[0].eq_ignore_ascii_case(&b'u')
        && bytes[1].eq_ignore_ascii_case(&b'r')
        && bytes[2].eq_ignore_ascii_case(&b'l')
        && bytes[3] == b'('
}

/// Advance past whitespace and block comments to the `;`/`}` terminator, returning its index.
///
/// Mirrors Svelte's `read_declaration`: `read_value` returns with the scan index AT the
/// terminator and the declaration's `end` is taken there — so trailing whitespace and
/// comments after the value (and after `!important`) sit inside the declaration extent.
/// Only whitespace, comments, and the `!important` tail can occur between the parsed
/// value's end and the terminator, so a flat byte walk is safe (no string/url content).
fn scan_to_terminator(source: &str, from: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b';' | b'}' => break,
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i = source[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |rel| i + 2 + rel + 2);
            }
            _ => i += 1,
        }
    }
    i
}

/// Convert a CSS declaration to JSON, matching Svelte's `read_declaration` exactly:
/// `end` is the `;`/`}` terminator scan position, `property` is the text before the
/// first whitespace/colon, and `value` is the raw post-colon source with block
/// comments stripped and the ends trimmed (so `red   !   important` stays raw and
/// `!important` is never re-serialized).
fn convert_declaration<'src>(
    decl: &internal::CssDeclaration<'_>,
    source: &'src str,
) -> public::Declaration<'src> {
    let content_end = decl
        .important_end
        .map_or(decl.span.end, |e| e.max(decl.span.end));
    let end = scan_to_terminator(source, content_end as usize);
    let decl_source = &source[decl.span.start as usize..end];
    let (property_source, value_source) = split_declaration_svelte_compat(decl_source);

    // Svelte 5.55.x+ strips block comments from declaration values.
    let value = strip_css_comments(value_source);

    public::Declaration {
        node_type: "Declaration",
        start: decl.span.start,
        end: end as u32,
        property: Cow::Borrowed(property_source.trim_end()),
        value,
    }
}

/// Wrap a single simple selector in Svelte's triple-wrapper
/// (SelectorList → ComplexSelector → RelativeSelector → selector), all sharing
/// `span`. Used for the `Nth` pseudo-class arg, whose `Nth` node is the wrapped
/// selector.
fn wrap_single_selector<'src>(
    selector: public::SimpleSelector<'src>,
    span: Span,
    scope: AstScope,
) -> public::SelectorList<'src> {
    let relative = public::RelativeSelector {
        node_type: "RelativeSelector",
        combinator: None,
        selectors: vec![selector],
        start: span.start,
        end: span.end,
        metadata: relative_meta(scope),
    };
    let complex = public::ComplexSelector {
        node_type: "ComplexSelector",
        start: span.start,
        end: span.end,
        children: vec![relative],
        metadata: complex_meta(scope),
    };
    public::SelectorList {
        node_type: "SelectorList",
        start: span.start,
        end: span.end,
        children: vec![complex],
    }
}

/// Convert PseudoClassArgs to Svelte's expected wrapper structure
/// (SelectorList → ComplexSelector → RelativeSelector → Nth/TypeSelector). The
/// caller boxes the result into the recursive `args`/`selector` field.
fn convert_pseudo_class_args<'src>(
    args: &internal::PseudoClassArgs<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::SelectorList<'src> {
    match args {
        internal::PseudoClassArgs::Nth {
            value,
            of_selector,
            span,
            value_span,
        } => {
            // If there's an "of <selector-list>", include it in the Nth node
            // (CSS Selectors Level 4: :nth-child(An+B of S)).
            let selector = of_selector.as_ref().map(|selectors| {
                Box::new(convert_selector_list_filtered(selectors, source, scope))
            });
            // Anchor the public span's START at the An+B token, not at `(`, so a
            // leading comment (`:nth-child(/* c */ 2n)`) isn't absorbed into it —
            // matching parseCss and tsv's own selector-list args (`:is(/* c */ .a)`,
            // which already anchor past the comment). A no-op without a leading
            // comment (`value_span.start == span.start`); the internal `span` is
            // unchanged, so the printer still uses `[span.start, value_span.start)`
            // to interleave the gap comment.
            let public_span = Span {
                start: value_span.start,
                end: span.end,
            };
            let nth = public::SimpleSelector::Nth(public::Nth {
                node_type: "Nth",
                value: Cow::Owned((*value).to_string()),
                start: public_span.start,
                end: public_span.end,
                selector,
            });
            wrap_single_selector(nth, public_span, scope)
        }
        internal::PseudoClassArgs::SelectorList { selectors, .. } => {
            // For :is()/:not()/:where()/:has()/:global() and the identifier-arg
            // pseudos (:dir()/:lang()/::highlight()) - convert the nested selector
            // list. Filter out Invalid selectors (from forgiving parsing).
            //
            // SVELTE QUIRK: `:dir(ltr)`/`:lang(en-US)` args are ordinary selector
            // lists in Svelte's AST, not identifiers — a `TypeSelector` per
            // comma-separated range (`:lang(en, fr)` is two). Escaped names decode
            // via the shared `TypeSelector` path (`raw_selector_name`), so
            // `:dir(\6c\74\72)` exposes `ltr` while the printer keeps the raw form.
            convert_selector_list_filtered(selectors, source, scope)
        }
        // Slotted/Part args are parsed internally (for the formatter/tooling) but NOT
        // exposed in the public AST — Svelte omits pseudo-element args from its JSON.
        // The parser only ever builds these for the `::` pseudo-element form (see
        // `parse_pseudo_args`'s `is_pseudo_element` gate); a single-colon `:slotted`/
        // `:part` is parsed as an ordinary pseudo-class with selector-list args. Since
        // pseudo-element nodes never reach `convert_pseudo_class_args`, these variants
        // cannot appear here.
        #[allow(clippy::unreachable)] // parser only builds Slotted/Part for `::` pseudo-elements
        internal::PseudoClassArgs::Slotted { .. } | internal::PseudoClassArgs::Part { .. } => {
            unreachable!("Pseudo-element args (Slotted/Part) never attach to a pseudo-class")
        }
    }
}

/// Convert a top-level CSS node (rule or at-rule) to the typed public node for
/// the embedded `<style>` path (`tsv_svelte`'s `StyleSheet { children:
/// Vec<CssNodePublic> }`). `AstScope::Embedded` since embedded CSS never carries
/// `parseCss()` metadata; the standalone root uses `convert_stylesheet_file`.
/// Both paths now build the same typed tree — no intermediate `serde_json::Value`
/// for embedded CSS either.
pub fn convert_css_node<'src>(
    node: &internal::CssNode<'_>,
    source: &'src str,
) -> public::CssNodePublic<'src> {
    convert_node(node, source, AstScope::Embedded)
}

/// Convert a top-level CSS node to the typed public node.
fn convert_node<'src>(
    node: &internal::CssNode<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::CssNodePublic<'src> {
    match node {
        internal::CssNode::Rule(rule) => {
            public::CssNodePublic::Rule(convert_rule(rule, source, scope))
        }
        internal::CssNode::Atrule(atrule) => {
            public::CssNodePublic::Atrule(convert_atrule(atrule, source, scope))
        }
    }
}

/// Convert a block child (declaration, nested rule, or nested at-rule) to a
/// typed node. Comments are dropped to match Svelte's CSS parser output (our
/// internal AST keeps them for the formatter).
fn convert_block_child<'src>(
    child: &internal::CssBlockChild<'_>,
    source: &'src str,
    scope: AstScope,
) -> Option<public::CssNodePublic<'src>> {
    match child {
        internal::CssBlockChild::Declaration(decl) => Some(public::CssNodePublic::Declaration(
            convert_declaration(decl, source),
        )),
        // CSS Nesting Module - recursively convert nested rules
        internal::CssBlockChild::Rule(rule) => Some(public::CssNodePublic::Rule(convert_rule(
            rule, source, scope,
        ))),
        // At-rules can also be nested (e.g., @media inside a rule)
        internal::CssBlockChild::Atrule(atrule) => Some(public::CssNodePublic::Atrule(
            convert_atrule(atrule, source, scope),
        )),
        internal::CssBlockChild::Comment(_) => None,
    }
}

/// Convert a CSS rule to the typed public node.
fn convert_rule<'src>(
    rule: &internal::CssRule<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::Rule<'src> {
    let children = rule
        .declarations
        .iter()
        .filter_map(|child| convert_block_child(child, source, scope))
        .collect();

    public::Rule {
        node_type: "Rule",
        prelude: convert_selector_list(&rule.selector, source, scope),
        block: public::Block {
            node_type: "Block",
            start: rule.block_span.start,
            end: rule.block_span.end,
            children,
        },
        start: rule.span.start,
        end: rule.span.end,
        metadata: rule_meta(scope),
    }
}

/// Convert a CSS at-rule to the typed public node.
fn convert_atrule<'src>(
    atrule: &internal::CssAtrule<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::Atrule<'src> {
    let block = atrule.block.as_ref().map(|b| {
        let children = b
            .children
            .iter()
            .filter_map(|child| convert_block_child(child, source, scope))
            .collect();

        public::Block {
            node_type: "Block",
            start: b.span.start,
            end: b.span.end,
            children,
        }
    });

    public::Atrule {
        node_type: "Atrule",
        // `atrule.name` is an `'arena` slice (not `source`), so it can't borrow into
        // the `'src` public tree — owned. At-rules are rare and the name is short.
        name: Cow::Owned(atrule.name.to_string()),
        // Convert prelude to string format for Svelte compatibility
        prelude: convert_prelude_to_string(&atrule.prelude, source),
        block,
        start: atrule.span.start,
        end: atrule.span.end,
    }
}

/// Convert PreludeValue to string representation for public AST
///
/// Svelte 5.55.x strips `/* ... */` block comments from at-rule preludes (surrounding
/// whitespace preserved, then trimmed). Applied to all source-extracted variants;
/// `Values` is built from parsed tokens that never contained comments.
fn convert_prelude_to_string<'src>(
    prelude: &internal::PreludeValue<'_>,
    source: &'src str,
) -> Cow<'src, str> {
    match prelude {
        internal::PreludeValue::Values { span, .. } => {
            // Extract the prelude verbatim from source and strip comments, matching
            // Svelte (which removes `/* ... */` from the `@import` prelude string while
            // preserving the surrounding whitespace, then trims). Extracting from the
            // span (rather than rejoining the structured values) keeps the public AST
            // byte-for-byte with Svelte even when comments sit between the url/string and
            // the media query — the structured values exist for the printer's quote
            // normalization and media-query wrapping.
            strip_css_comments(span.extract(source))
        }
        // Extract verbatim from source (comments stripped, outer-trimmed) so the public
        // AST matches Svelte, which stores the raw prelude — e.g. `@layer a , b` → `a , b`
        // and `@namespace url(  x  )` → `url(  x  )`. The internal `content` string is a
        // normalized (printer-facing) form; the AST must stay source-faithful, like the
        // `Media`/`Supports`/`Container`/`Values` branches.
        internal::PreludeValue::Raw { span, .. } => strip_css_comments(span.extract(source)),
        // @scope selector lists: `[(root)]? [to (limit)]?`. Extracted verbatim from
        // `span` for fidelity (a bare `@scope` has a zero-width span → `""`), like the
        // sibling raw/condition branches.
        internal::PreludeValue::Selectors { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Supports { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Container { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Media { span, .. } => strip_css_comments(span.extract(source)),
    }
}

/// Convert a SelectorList to the typed public node.
fn convert_selector_list<'src>(
    selector_list: &internal::SelectorList<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::SelectorList<'src> {
    let children = selector_list
        .selectors
        .iter()
        .map(|c| convert_complex_selector(c, source, scope))
        .collect();

    public::SelectorList {
        node_type: "SelectorList",
        start: selector_list.span.start,
        end: selector_list.span.end,
        children,
    }
}

/// Convert a SelectorList to JSON, filtering out Invalid selectors (from forgiving parsing).
///
/// Used for pseudo-class arguments (:is, :where, :not, :has) to ensure Svelte compatibility.
///
/// Per CSS Selectors Level 4:
/// - Invalid selectors (from forgiving parsing) are ignored for matching
///
/// Note: Pseudo-elements are technically contextually invalid in :is() and :where()
/// per the spec, but Svelte's parser keeps them in the AST, so we do too.
///
/// This filtering happens at conversion time, not in the internal AST, to preserve
/// full semantic information for the formatter (which outputs all selectors).
fn convert_selector_list_filtered<'src>(
    selector_list: &internal::SelectorList<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::SelectorList<'src> {
    let children = selector_list
        .selectors
        .iter()
        .filter(|selector| !selector_contains_invalid(selector))
        .map(|c| convert_complex_selector(c, source, scope))
        .collect();

    public::SelectorList {
        node_type: "SelectorList",
        start: selector_list.span.start,
        end: selector_list.span.end,
        children,
    }
}

/// Check if a complex selector contains Invalid simple selectors (from forgiving parsing)
fn selector_contains_invalid(complex: &internal::ComplexSelector<'_>) -> bool {
    for relative in complex.children {
        for simple in relative.selectors {
            if matches!(simple, internal::SimpleSelector::Invalid { .. }) {
                return true;
            }
        }
    }
    false
}

/// Convert a ComplexSelector to the typed public node.
fn convert_complex_selector<'src>(
    complex: &internal::ComplexSelector<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::ComplexSelector<'src> {
    let children = complex
        .children
        .iter()
        .map(|r| convert_relative_selector(r, source, scope))
        .collect();

    public::ComplexSelector {
        node_type: "ComplexSelector",
        start: complex.span.start,
        end: complex.span.end,
        children,
        metadata: complex_meta(scope),
    }
}

/// Convert a RelativeSelector to the typed public node.
fn convert_relative_selector<'src>(
    relative: &internal::RelativeSelector<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::RelativeSelector<'src> {
    let combinator =
        if let (Some(comb), Some(span)) = (&relative.combinator, &relative.combinator_span) {
            Some(public::Combinator {
                node_type: "Combinator",
                name: comb.as_str(),
                start: span.start,
                end: span.end,
            })
        } else {
            None
        };

    let selectors = relative
        .selectors
        .iter()
        .map(|s| convert_simple_selector(s, source, scope))
        .collect();

    public::RelativeSelector {
        node_type: "RelativeSelector",
        combinator,
        selectors,
        start: relative.span.start,
        end: relative.span.end,
        metadata: relative_meta(scope),
    }
}

/// Extract a selector name from source, skipping `prefix_len` bytes of sigil (`.`/`#`),
/// half-decoded the way Svelte's `read_identifier` does it: hex escapes (`\3A `,
/// `\1F4A9`, optional single whitespace terminator) decode to their codepoint, while
/// identity escapes (`\?`) keep the backslash. The internal AST stores the fully
/// decoded spec form; this reconstructs Svelte's public form at the boundary.
fn raw_selector_name(source: &str, span: Span, prefix_len: usize) -> Cow<'_, str> {
    let raw = &source[span.start as usize + prefix_len..span.end as usize];
    // Fast path: no backslash means no escapes to decode, so the name is the raw
    // source slice verbatim — borrowed, no allocation. (The vast majority of names.)
    if !raw.contains('\\') {
        return Cow::Borrowed(raw);
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        if chars.peek().is_some_and(char::is_ascii_hexdigit) {
            let mut hex = String::new();
            for _ in 0..6 {
                match chars.peek() {
                    Some(&d) if d.is_ascii_hexdigit() => {
                        hex.push(d);
                        chars.next();
                    }
                    _ => break,
                }
            }
            // Optional single whitespace terminator (Svelte: `(\r\n|\s)?`)
            if chars.peek() == Some(&'\r') {
                chars.next();
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            } else if chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            // Surrogate/overflow codepoints are unrepresentable in Rust strings —
            // dropped, same as `escapes::decode_escape_sequences`
            if let Ok(cp) = u32::from_str_radix(&hex, 16)
                && let Some(c) = char::from_u32(cp)
            {
                out.push(c);
            }
        } else if let Some(next) = chars.next() {
            out.push('\\');
            out.push(next);
        } else {
            out.push('\\');
        }
    }
    Cow::Owned(out)
}

/// The end position of a pseudo selector's name, excluding any `(args)`.
///
/// A pseudo's `span` covers the whole `:name(args)` / `::name(args)`, so when it has
/// arguments the name runs only up to the first `(`; without arguments the whole span is
/// the name. Used to bound the `raw_selector_name` slice (and, for pseudo-elements, the
/// public `end`) to just the name — the decoded internal name is never re-serialized.
fn pseudo_name_end(source: &str, span: Span, has_args: bool) -> u32 {
    if has_args {
        let raw = &source[span.start as usize..span.end as usize];
        raw.find('(').map_or(span.end, |i| span.start + i as u32)
    } else {
        span.end
    }
}

/// Build a public `NamedSelector` node (`TypeSelector` / `ClassSelector` /
/// `IdSelector` / `NestingSelector`) — the shared shape behind type, universal,
/// class, id, and nesting selectors. (The `:dir()`/`:lang()` identifier args also
/// land here, as ordinary `TypeSelector`s via the selector-list convert path.)
/// Taking a single `span` for both `start` and `end` keeps the two in lockstep
/// (they always come from the same node); the caller computes `name` (a raw source
/// slice, a half-decoded `raw_selector_name`, or a constant like `"*"`/`"&"`).
fn named_selector<'src>(
    node_type: &'static str,
    name: Cow<'src, str>,
    span: Span,
) -> public::SimpleSelector<'src> {
    public::SimpleSelector::Named(public::NamedSelector {
        node_type,
        name,
        start: span.start,
        end: span.end,
    })
}

/// Convert a SimpleSelector to the typed public node.
fn convert_simple_selector<'src>(
    simple: &internal::SimpleSelector<'_>,
    source: &'src str,
    scope: AstScope,
) -> public::SimpleSelector<'src> {
    match simple {
        internal::SimpleSelector::Type { namespace, span } => {
            // SVELTE QUIRK: Namespace prefixes are parsed but NOT included in the JSON AST
            // Example: svg|rect → {"type": "TypeSelector", "name": "rect"}
            // The namespace is preserved in the source span but not exposed in the JSON.
            // Without a namespace the span is exactly the name, so emit the raw source
            // (Svelte never decodes escapes in selector names); with one, skip past the
            // `|` so the prefix isn't included (canonical errors on namespaces anyway —
            // see conformance_svelte.md).
            let raw_name = if namespace.is_none() {
                raw_selector_name(source, *span, 0)
            } else {
                let raw = &source[span.start as usize..span.end as usize];
                let prefix = raw.find('|').map_or(0, |i| i + 1);
                raw_selector_name(source, *span, prefix)
            };
            named_selector("TypeSelector", raw_name, *span)
        }
        internal::SimpleSelector::Universal { namespace: _, span } => {
            // Svelte represents universal selector as TypeSelector with name "*"
            // SVELTE QUIRK: Namespace prefixes are parsed but NOT included in the JSON AST
            named_selector("TypeSelector", Cow::Borrowed("*"), *span)
        }
        internal::SimpleSelector::Class { span } => {
            named_selector("ClassSelector", raw_selector_name(source, *span, 1), *span)
        }
        internal::SimpleSelector::Id { span } => {
            named_selector("IdSelector", raw_selector_name(source, *span, 1), *span)
        }
        internal::SimpleSelector::Attribute {
            namespace,
            name_span,
            matcher,
            value,
            flags,
            span,
        } => {
            // The name is half-decoded from its own span (`name_span` is exactly the name
            // token — no sigil prefix, so `prefix_len` is 0), matching Svelte's `read_identifier`
            // like class/id/type and pseudo names: hex escapes decode, identity escapes keep the
            // backslash (`[f\oo]` → `"f\\oo"`, not `"foo"`). See conformance_svelte.md
            // §Selector-name half-decoding.
            public::SimpleSelector::Attribute(public::AttributeSelector {
                node_type: "AttributeSelector",
                name: raw_selector_name(source, *name_span, 0),
                start: span.start,
                end: span.end,
                // `matcher`/`value`/`flags`/`namespace` are `'arena` slices, not
                // `source`, so they're owned (rare — attribute selectors).
                matcher: matcher.as_ref().map(|m| Cow::Owned(m.as_str().to_string())),
                value: value.as_ref().map(|v| Cow::Owned((*v).to_string())),
                flags: flags.as_ref().map(|f| Cow::Owned((*f).to_string())),
                namespace: namespace.as_ref().map(|ns| Cow::Owned((*ns).to_string())),
            })
        }
        // Pseudo-class/-element names are half-decoded like every other selector name
        // (`raw_selector_name`): hex escapes decode, identity escapes keep the backslash —
        // matching Svelte's `read_identifier`. The name slice runs from the `:`/`::` sigil to
        // the `(` (when there are args) or the span end, so the decoded internal name is never
        // re-serialized; only `span` and `source` feed the public `name`.
        internal::SimpleSelector::PseudoClass { args, span } => {
            let args_val = args
                .as_ref()
                .map(|a| Box::new(convert_pseudo_class_args(a, source, scope)));
            let name_span = Span {
                start: span.start,
                end: pseudo_name_end(source, *span, args.is_some()),
            };
            let name = raw_selector_name(source, name_span, 1);

            public::SimpleSelector::PseudoClass(public::PseudoClassSelector {
                node_type: "PseudoClassSelector",
                name,
                args: args_val,
                start: span.start,
                end: span.end,
            })
        }
        internal::SimpleSelector::PseudoElement { args, span } => {
            // Truncate span to match Svelte: just the pseudo-element name, excluding args.
            // Example: ::slotted(*) has full span 9-21, but Svelte outputs 9-18 (just ::slotted).
            // Public AST matches Svelte for drop-in compatibility; the internal AST keeps the
            // full accurate span (including args) for the formatter/tooling.
            //
            // Derive the name end from the source `(` (when there are args), not a decoded
            // length, which undercounts when the name contains escapes (`::\41 b` is 7 raw
            // bytes but decodes to "Ab"). Without args the span is already exactly `::name`.
            let name_end = pseudo_name_end(source, *span, args.is_some());
            let name = raw_selector_name(
                source,
                Span {
                    start: span.start,
                    end: name_end,
                },
                2,
            );

            public::SimpleSelector::PseudoElement(public::PseudoElementSelector {
                node_type: "PseudoElementSelector",
                name,
                start: span.start,
                end: name_end, // Matches Svelte (name only, not including args)
            })
        }
        internal::SimpleSelector::Nesting { span } => {
            named_selector("NestingSelector", Cow::Borrowed("&"), *span)
        }
        internal::SimpleSelector::Percentage { value, span } => {
            // Format value as string with % suffix to match Svelte
            let value_str = if value.fract() == 0.0 {
                format!("{}%", *value as i64)
            } else {
                format!("{value}%")
            };
            public::SimpleSelector::Percentage(public::Percentage {
                node_type: "Percentage",
                value: Cow::Owned(value_str),
                start: span.start,
                end: span.end,
            })
        }
        internal::SimpleSelector::Nth { span } => {
            // An An+B term inside pseudo-class args. parseCss stores the value verbatim
            // (the raw source slice — never operator-normalized like the printer's
            // output). For an `An+B of S` term the span folds in the ` of ` (`"2n of "`),
            // matching Svelte, which reads `S` as sibling selectors rather than a nested
            // list — so `selector` is always `None` here (only the dedicated `:nth-*()`
            // path nests `S` under `Nth.selector`).
            public::SimpleSelector::Nth(public::Nth {
                node_type: "Nth",
                value: Cow::Borrowed(&source[span.start as usize..span.end as usize]),
                start: span.start,
                end: span.end,
                selector: None,
            })
        }
        // `Invalid` is only ever produced by `parse_forgiving_selector_list` (the
        // `:is()`/`:where()` arguments), and every forgiving list reaches the public
        // AST through `convert_selector_list_filtered`, which drops any complex
        // selector containing an `Invalid`. The sole non-filtering caller,
        // `convert_selector_list`, handles rule preludes — parsed non-forgivingly, so
        // they never contain `Invalid`. Hence no `Invalid` reaches this match.
        #[allow(clippy::unreachable)] // forgiving-list Invalids are filtered before convert
        internal::SimpleSelector::Invalid { .. } => {
            unreachable!("Invalid selectors should be filtered in convert_selector_list_filtered")
        }
    }
}

/// Translate all byte-based positions in a JSON AST to character-based positions
///
/// CSS AST only has `start`/`end` (no `loc`), so this just translates those.
/// For ASCII-only sources, this is a no-op (byte == char offset).
pub fn translate_byte_to_char_offsets(
    value: &mut serde_json::Value,
    map: &tsv_lang::ByteToCharMap,
) {
    if !map.has_multibyte() {
        return;
    }
    translate_positions_recursive(value, map);
}

fn translate_positions_recursive(value: &mut serde_json::Value, map: &tsv_lang::ByteToCharMap) {
    match value {
        serde_json::Value::Object(obj) => {
            let orig_start = obj
                .get("start")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);
            let orig_end = obj
                .get("end")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v as u32);

            if let Some(start_byte) = orig_start {
                obj.insert(
                    "start".to_string(),
                    serde_json::Value::Number(map.byte_to_char(start_byte).into()),
                );
            }
            if let Some(end_byte) = orig_end {
                obj.insert(
                    "end".to_string(),
                    serde_json::Value::Number(map.byte_to_char(end_byte).into()),
                );
            }

            for val in obj.values_mut() {
                translate_positions_recursive(val, map);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                translate_positions_recursive(item, map);
            }
        }
        _ => {}
    }
}

/// Convert a list of CSS nodes to a typed StyleSheet structure (for Svelte embedding)
pub fn convert_css_nodes<'src>(
    nodes: &[internal::CssNode<'_>],
    source: &'src str,
) -> public::StyleSheet<'src> {
    // Convert all nodes (comments are stored separately and not included in JSON output)
    let children: Vec<public::CssNodePublic<'src>> = nodes
        .iter()
        .map(|node| convert_css_node(node, source))
        .collect();

    // Calculate content span from nodes
    let (content_start, content_end) = match (nodes.first(), nodes.last()) {
        (Some(first), Some(last)) => (first.span().start, last.span().end),
        _ => (0, 0),
    };

    public::StyleSheet {
        node_type: "StyleSheetFile",
        start: content_start,
        end: content_end,
        attributes: Vec::new(),
        children,
        content: public::StyleContent {
            start: content_start,
            end: content_end,
            styles: Cow::Borrowed(&source[content_start as usize..content_end as usize]),
            comment: None,
        },
    }
}

/// Convert a list of CSS nodes to the typed standalone `StyleSheetFile` root.
///
/// Unlike `convert_css_nodes` (used for Svelte `<style>` embedding, which includes
/// `attributes` and `content` fields), this produces the minimal `StyleSheetFile`
/// structure matching Svelte's `parseCss()` output: just `type`, `start`, `end`,
/// and `children`, with `metadata` on every `Rule`/`ComplexSelector`/`RelativeSelector`
/// (`AstScope::Standalone`). It's the source for both `convert_ast_json` (via
/// `to_value`) and `convert_ast_json_string` (serialized directly).
///
/// The `end` offset is set to the full source length (not the last node's span end),
/// matching Svelte's behavior of including trailing whitespace in the file span.
pub fn convert_stylesheet_file<'src>(
    nodes: &[internal::CssNode<'_>],
    source: &'src str,
) -> public::StyleSheetFile<'src> {
    let children = nodes
        .iter()
        .map(|node| convert_node(node, source, AstScope::Standalone))
        .collect();

    public::StyleSheetFile {
        node_type: "StyleSheetFile",
        start: 0,
        end: source.len() as u32,
        children,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owns the `Cow` result so assertions can compare against `&str` literals.
    fn strip(s: &str) -> String {
        strip_css_comments(s).into_owned()
    }

    #[test]
    fn strip_css_comments_basic_removal_and_trim() {
        assert_eq!(strip("/* c */ 12px"), "12px");
        assert_eq!(strip("blue /* c */"), "blue");
        assert_eq!(strip("/* a */ red"), "red");
    }

    #[test]
    fn strip_css_comments_interior_whitespace_preserved() {
        assert_eq!(strip("var(--a, /* c */ red)"), "var(--a,  red)",);
        assert_eq!(
            strip("sidebar /* x */ (min-width: 100px)"),
            "sidebar  (min-width: 100px)",
        );
    }

    #[test]
    fn strip_css_comments_inside_strings_are_preserved() {
        assert_eq!(strip("\"/* not a comment */\""), "\"/* not a comment */\"",);
        assert_eq!(strip("'/* keep */'"), "'/* keep */'");
    }

    #[test]
    fn strip_css_comments_inside_url_are_preserved() {
        assert_eq!(
            strip("url(\"data:image/svg+xml,/* x */\")"),
            "url(\"data:image/svg+xml,/* x */\")",
        );
    }

    #[test]
    fn strip_css_comments_inside_other_functions_are_stripped() {
        // Only url() is special — calc/var/etc. follow normal CSS tokenization,
        // so block comments inside them are stripped just like at top level.
        assert_eq!(strip("calc(/* x */ 1px + 2px)"), "calc( 1px + 2px)",);
        assert_eq!(strip("URL(/* keep */)"), "URL(/* keep */)");
    }

    #[test]
    fn strip_css_comments_unterminated_kept_verbatim() {
        assert_eq!(strip("red /* oops"), "red /* oops");
    }

    #[test]
    fn strip_css_comments_escaped_quote_does_not_close_string() {
        assert_eq!(
            strip("\"a\\\" /* in str */ b\" /* real */ c"),
            "\"a\\\" /* in str */ b\"  c",
        );
    }
}
