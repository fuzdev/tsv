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
    let Some(colon_pos) = decl_source.find(':') else {
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
fn strip_css_comments(input: &str) -> String {
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
    out.trim().to_string()
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
fn convert_declaration(decl: &internal::CssDeclaration, source: &str) -> serde_json::Value {
    let content_end = decl
        .important_end
        .map_or(decl.span.end, |e| e.max(decl.span.end));
    let end = scan_to_terminator(source, content_end as usize);
    let decl_source = &source[decl.span.start as usize..end];
    let (property_source, value_source) = split_declaration_svelte_compat(decl_source);

    // Svelte 5.55.x+ strips block comments from declaration values.
    let value = strip_css_comments(value_source);

    serde_json::json!({
        "type": "Declaration",
        "start": decl.span.start,
        "end": end,
        "property": property_source.trim_end(),
        "value": value,
    })
}

/// Convert PseudoClassArgs to Svelte's expected JSON structure
///
/// Generates the wrapper structure: SelectorList → ComplexSelector → RelativeSelector → Nth
fn convert_pseudo_class_args(args: &internal::PseudoClassArgs, source: &str) -> serde_json::Value {
    match args {
        internal::PseudoClassArgs::Nth {
            value,
            of_selector,
            span,
        } => {
            // Generate Svelte's triple-wrapper structure
            // If there's an "of <selector-list>", include it in the Nth node
            let mut nth_node = serde_json::json!({
                "type": "Nth",
                "value": value,
                "start": span.start,
                "end": span.end
            });

            // Add selector list if present (CSS Selectors Level 4: :nth-child(An+B of S))
            if let Some(selectors) = of_selector {
                nth_node["selector"] = convert_selector_list_filtered(selectors, source);
            }

            serde_json::json!({
                "type": "SelectorList",
                "start": span.start,
                "end": span.end,
                "children": [{
                    "type": "ComplexSelector",
                    "start": span.start,
                    "end": span.end,
                    "children": [{
                        "type": "RelativeSelector",
                        "combinator": null,
                        "selectors": [nth_node],
                        "start": span.start,
                        "end": span.end,
                    }]
                }]
            })
        }
        internal::PseudoClassArgs::SelectorList { selectors, .. } => {
            // For :is(), :not(), :where(), :has(), :global() - convert the nested selector list
            // Filter out Invalid selectors (from forgiving parsing)
            convert_selector_list_filtered(selectors, source)
        }
        internal::PseudoClassArgs::Identifier { value, span } => {
            // SVELTE QUIRK: Identifier arguments (e.g., :dir(ltr), :lang(en-US)) are wrapped
            // in a SelectorList → ComplexSelector → RelativeSelector → TypeSelector structure
            // even though the spec says they should be identifiers, not selectors.
            //
            // This matches Svelte's parser behavior for compatibility.
            //
            // Spec-compliant internal: Identifier { value: "ltr" }
            // Svelte's public quirk: TypeSelector wrapping
            serde_json::json!({
                "type": "SelectorList",
                "start": span.start,
                "end": span.end,
                "children": [{
                    "type": "ComplexSelector",
                    "start": span.start,
                    "end": span.end,
                    "children": [{
                        "type": "RelativeSelector",
                        "combinator": null,
                        "selectors": [{
                            "type": "TypeSelector",
                            "name": value,
                            "start": span.start,
                            "end": span.end
                        }],
                        "start": span.start,
                        "end": span.end,
                    }]
                }]
            })
        }
        // Note: Slotted and Part args are parsed internally but NOT exposed in public AST
        // This matches Svelte's behavior (they omit pseudo-element args from JSON output)
        // Internal AST retains these for formatter/tooling, but convert_pseudo_class_args is
        // only called for PseudoClass, not PseudoElement, so these cases are unreachable
        internal::PseudoClassArgs::Slotted { .. } | internal::PseudoClassArgs::Part { .. } => {
            unreachable!("Pseudo-element args not exposed in public AST")
        }
    }
}

/// Convert a CSS node to JSON representation
pub fn convert_css_node(node: &internal::CssNode, source: &str) -> serde_json::Value {
    match node {
        internal::CssNode::Rule(rule) => convert_css_rule(rule, source),
        internal::CssNode::Atrule(atrule) => convert_css_atrule(atrule, source),
    }
}

/// Convert a CSS rule to JSON representation
fn convert_css_rule(rule: &internal::CssRule, source: &str) -> serde_json::Value {
    // Filter out comments to match Svelte's CSS parser output
    // (Our internal AST has comments for the formatter, but public JSON AST should match Svelte)
    // Support nested rules (CSS Nesting Module) and at-rules within rule blocks
    let declarations: Vec<serde_json::Value> = rule
        .declarations
        .iter()
        .filter_map(|child| {
            match child {
                internal::CssBlockChild::Declaration(decl) => {
                    Some(convert_declaration(decl, source))
                }
                internal::CssBlockChild::Rule(nested_rule) => {
                    // CSS Nesting Module - recursively convert nested rules
                    Some(convert_css_rule(nested_rule, source))
                }
                internal::CssBlockChild::Atrule(nested_atrule) => {
                    // At-rules can also be nested (e.g., @media inside a rule)
                    Some(convert_css_atrule(nested_atrule, source))
                }
                internal::CssBlockChild::Comment(_) => {
                    // Filter out comments to match Svelte's CSS parser output
                    None
                }
            }
        })
        .collect();

    let prelude = convert_selector_list(&rule.selector, source);

    serde_json::json!({
        "type": "Rule",
        "prelude": prelude,
        "block": {
            "type": "Block",
            "start": rule.block_span.start,
            "end": rule.block_span.end,
            "children": declarations,
        },
        "start": rule.span.start,
        "end": rule.span.end,
    })
}

/// Convert a CSS at-rule to JSON representation
fn convert_css_atrule(atrule: &internal::CssAtrule, source: &str) -> serde_json::Value {
    let block = atrule.block.as_ref().map(|b| {
        // Filter out comments to match Svelte's CSS parser output
        // (Our internal AST has comments for the formatter, but public JSON AST should match Svelte)
        let children: Vec<serde_json::Value> = b
            .children
            .iter()
            .filter(|child| !matches!(child, internal::CssBlockChild::Comment(_)))
            .map(|child| convert_atrule_block_child(child, source))
            .collect();

        serde_json::json!({
            "type": "Block",
            "start": b.span.start,
            "end": b.span.end,
            "children": children,
        })
    });

    // Convert prelude to string format for Svelte compatibility
    let prelude_string = convert_prelude_to_string(&atrule.prelude, source);

    serde_json::json!({
        "type": "Atrule",
        "name": atrule.name,
        "prelude": prelude_string,
        "block": block.unwrap_or(serde_json::Value::Null),
        "start": atrule.span.start,
        "end": atrule.span.end,
    })
}

/// Convert PreludeValue to string representation for public AST
///
/// Svelte 5.55.x strips `/* ... */` block comments from at-rule preludes (surrounding
/// whitespace preserved, then trimmed). Applied to all source-extracted variants;
/// `Values` is built from parsed tokens that never contained comments.
fn convert_prelude_to_string(prelude: &internal::PreludeValue, source: &str) -> String {
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
        internal::PreludeValue::Selectors {
            root: _,
            limit: _,
            span,
        } => {
            // Format selector lists for @scope: (root) [to (limit)]
            // Extract from source for maximum fidelity
            strip_css_comments(span.extract(source))
        }
        internal::PreludeValue::Supports { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Container { span, .. } => strip_css_comments(span.extract(source)),
        internal::PreludeValue::Media { span, .. } => strip_css_comments(span.extract(source)),
    }
}

/// Convert an at-rule block child to JSON representation
///
/// Note: Comments are filtered out before calling this function (see convert_css_atrule)
fn convert_atrule_block_child(child: &internal::CssBlockChild, source: &str) -> serde_json::Value {
    match child {
        internal::CssBlockChild::Rule(rule) => convert_css_rule(rule, source),
        internal::CssBlockChild::Declaration(decl) => convert_declaration(decl, source),
        internal::CssBlockChild::Atrule(atrule) => convert_css_atrule(atrule, source),
        internal::CssBlockChild::Comment(_) => {
            // Comments are filtered out before calling this function
            unreachable!("Comments should be filtered in convert_css_atrule")
        }
    }
}

/// Convert a SelectorList to JSON
fn convert_selector_list(
    selector_list: &internal::SelectorList,
    source: &str,
) -> serde_json::Value {
    let children: Vec<serde_json::Value> = selector_list
        .selectors
        .iter()
        .map(|c| convert_complex_selector(c, source))
        .collect();

    serde_json::json!({
        "type": "SelectorList",
        "start": selector_list.span.start,
        "end": selector_list.span.end,
        "children": children,
    })
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
fn convert_selector_list_filtered(
    selector_list: &internal::SelectorList,
    source: &str,
) -> serde_json::Value {
    let children: Vec<serde_json::Value> = selector_list
        .selectors
        .iter()
        .filter(|selector| !selector_contains_invalid(selector))
        .map(|c| convert_complex_selector(c, source))
        .collect();

    serde_json::json!({
        "type": "SelectorList",
        "start": selector_list.span.start,
        "end": selector_list.span.end,
        "children": children,
    })
}

/// Check if a complex selector contains Invalid simple selectors (from forgiving parsing)
fn selector_contains_invalid(complex: &internal::ComplexSelector) -> bool {
    for relative in &complex.children {
        for simple in &relative.selectors {
            if matches!(simple, internal::SimpleSelector::Invalid { .. }) {
                return true;
            }
        }
    }
    false
}

/// Convert a ComplexSelector to JSON
fn convert_complex_selector(
    complex: &internal::ComplexSelector,
    source: &str,
) -> serde_json::Value {
    let children: Vec<serde_json::Value> = complex
        .children
        .iter()
        .map(|r| convert_relative_selector(r, source))
        .collect();

    serde_json::json!({
        "type": "ComplexSelector",
        "start": complex.span.start,
        "end": complex.span.end,
        "children": children,
    })
}

/// Convert a RelativeSelector to JSON
fn convert_relative_selector(
    relative: &internal::RelativeSelector,
    source: &str,
) -> serde_json::Value {
    let combinator =
        if let (Some(comb), Some(span)) = (&relative.combinator, &relative.combinator_span) {
            let name = comb.as_str();
            serde_json::json!({
                "type": "Combinator",
                "name": name,
                "start": span.start,
                "end": span.end,
            })
        } else {
            serde_json::Value::Null
        };

    let selectors: Vec<serde_json::Value> = relative
        .selectors
        .iter()
        .map(|s| convert_simple_selector(s, source))
        .collect();

    serde_json::json!({
        "type": "RelativeSelector",
        "combinator": combinator,
        "selectors": selectors,
        "start": relative.span.start,
        "end": relative.span.end,
    })
}

/// Extract a selector name from source, skipping `prefix_len` bytes of sigil (`.`/`#`),
/// half-decoded the way Svelte's `read_identifier` does it: hex escapes (`\3A `,
/// `\1F4A9`, optional single whitespace terminator) decode to their codepoint, while
/// identity escapes (`\?`) keep the backslash. The internal AST stores the fully
/// decoded spec form; this reconstructs Svelte's public form at the boundary.
fn raw_selector_name(source: &str, span: tsv_lang::Span, prefix_len: usize) -> String {
    let raw = &source[span.start as usize + prefix_len..span.end as usize];
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
    out
}

/// Convert a SimpleSelector to JSON
fn convert_simple_selector(simple: &internal::SimpleSelector, source: &str) -> serde_json::Value {
    match simple {
        internal::SimpleSelector::Type {
            namespace,
            name,
            span,
        } => {
            // SVELTE QUIRK: Namespace prefixes are parsed but NOT included in the JSON AST
            // Example: svg|rect → {"type": "TypeSelector", "name": "rect"}
            // The namespace is preserved in the source span but not exposed in the JSON.
            // Without a namespace the span is exactly the name, so emit the raw source
            // (Svelte never decodes escapes in selector names); with one, the raw slice
            // would include the prefix, so keep the decoded name (canonical errors on
            // namespaces anyway — see conformance_svelte.md).
            let raw_name = if namespace.is_none() {
                raw_selector_name(source, *span, 0)
            } else {
                name.clone()
            };
            serde_json::json!({
                "type": "TypeSelector",
                "name": raw_name,
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Universal { namespace: _, span } => {
            // Svelte represents universal selector as TypeSelector with name "*"
            // SVELTE QUIRK: Namespace prefixes are parsed but NOT included in the JSON AST
            serde_json::json!({
                "type": "TypeSelector",
                "name": "*",
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Class { name: _, span } => {
            serde_json::json!({
                "type": "ClassSelector",
                "name": raw_selector_name(source, *span, 1),
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Id { name: _, span } => {
            serde_json::json!({
                "type": "IdSelector",
                "name": raw_selector_name(source, *span, 1),
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Attribute {
            namespace,
            name,
            matcher,
            value,
            flags,
            span,
        } => {
            let matcher_val = matcher.as_ref().map_or(serde_json::Value::Null, |m| {
                serde_json::Value::String(m.as_str().to_string())
            });
            let value_val = value.as_ref().map_or(serde_json::Value::Null, |v| {
                serde_json::Value::String(v.clone())
            });
            let flags_val = flags.as_ref().map_or(serde_json::Value::Null, |f| {
                serde_json::Value::String(f.clone())
            });

            let mut obj = serde_json::json!({
                "type": "AttributeSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
                "matcher": matcher_val,
                "value": value_val,
                "flags": flags_val,
            });
            if let Some(ns) = namespace {
                obj["namespace"] = serde_json::Value::String(ns.clone());
            }
            obj
        }
        // TODO: PseudoClass/PseudoElement/Attribute names still emit the fully
        // decoded internal form; Svelte keeps escapes half-decoded there too
        // (`:\68over`). No corpus file hits it — align if one ever does.
        internal::SimpleSelector::PseudoClass { name, args, span } => {
            let args_val = args.as_ref().map_or(serde_json::Value::Null, |a| {
                convert_pseudo_class_args(a, source)
            });

            serde_json::json!({
                "type": "PseudoClassSelector",
                "name": name,
                "args": args_val,
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::PseudoElement {
            name,
            args: _,
            span,
        } => {
            // Truncate span to match Svelte: just the pseudo-element name, excluding args
            // Example: ::slotted(*) has full span 9-21, but Svelte outputs 9-18 (just ::slotted)
            // Rationale: Public AST matches Svelte for drop-in compatibility
            // Internal AST retains full accurate span (including args) for formatter/tooling
            let name_end = span.start + 2 + name.len() as u32; // :: = 2 chars, name = name.len()

            serde_json::json!({
                "type": "PseudoElementSelector",
                "name": name,
                "start": span.start,
                "end": name_end,  // Matches Svelte (name only, not including args)
            })
        }
        internal::SimpleSelector::Nesting { span } => {
            serde_json::json!({
                "type": "NestingSelector",
                "name": "&",
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Percentage { value, span } => {
            // Format value as string with % suffix to match Svelte
            let value_str = if value.fract() == 0.0 {
                format!("{}%", *value as i64)
            } else {
                format!("{value}%")
            };
            serde_json::json!({
                "type": "Percentage",
                "value": value_str,
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Invalid { .. } => {
            // Invalid selectors should be filtered out before reaching this function
            // This case exists for safety, but should never be hit in practice
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
pub fn convert_css_nodes(nodes: &[internal::CssNode], source: &str) -> super::public::StyleSheet {
    // Convert all nodes (comments are stored separately and not included in JSON output)
    let children: Vec<serde_json::Value> = nodes
        .iter()
        .map(|node| convert_css_node(node, source))
        .collect();

    // Calculate content span from nodes
    let (content_start, content_end) = match (nodes.first(), nodes.last()) {
        (Some(first), Some(last)) => (first.span().start, last.span().end),
        _ => (0, 0),
    };

    super::public::StyleSheet {
        node_type: "StyleSheetFile".to_string(),
        start: content_start,
        end: content_end,
        attributes: Vec::new(),
        children,
        content: super::public::StyleContent {
            start: content_start,
            end: content_end,
            styles: source[content_start as usize..content_end as usize].to_string(),
            comment: None,
        },
    }
}

/// Convert a list of CSS nodes to a standalone StyleSheetFile JSON value
///
/// Unlike `convert_css_nodes` (used for Svelte `<style>` embedding which includes
/// `attributes` and `content` fields), this produces the minimal `StyleSheetFile`
/// structure matching Svelte's `parseCss()` output: just `type`, `start`, `end`,
/// and `children`.
///
/// The `end` offset is set to the full source length (not the last node's span end),
/// matching Svelte's behavior of including trailing whitespace in the file span.
///
/// Also adds `metadata` fields to `Rule`, `ComplexSelector`, and `RelativeSelector`
/// nodes, matching Svelte's `parseCss()` output (which includes these for standalone
/// CSS but not for embedded `<style>` in `.svelte` files).
pub fn convert_css_nodes_standalone(
    nodes: &[internal::CssNode],
    source: &str,
) -> serde_json::Value {
    let children: Vec<serde_json::Value> = nodes
        .iter()
        .map(|node| convert_css_node(node, source))
        .collect();

    let mut result = serde_json::json!({
        "type": "StyleSheetFile",
        "start": 0,
        "end": source.len() as u32,
        "children": children,
    });

    // Add metadata fields matching parseCss() output
    add_parsecss_metadata(&mut result);

    result
}

/// Add `metadata` fields to CSS AST nodes for standalone `parseCss()` output
///
/// Svelte's `parseCss()` includes metadata on `Rule`, `ComplexSelector`, and
/// `RelativeSelector` nodes. These metadata fields are NOT present in Svelte's
/// `.svelte` file parser output, so they're added as a post-processing step
/// only for standalone CSS files.
fn add_parsecss_metadata(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(obj) => {
            // Add metadata based on node type
            if let Some(node_type) = obj.get("type").and_then(|v| v.as_str()).map(String::from) {
                match node_type.as_str() {
                    "Rule" => {
                        obj.insert(
                            "metadata".to_string(),
                            serde_json::json!({
                                "parent_rule": null,
                                "has_local_selectors": false,
                                "has_global_selectors": false,
                                "is_global_block": false,
                            }),
                        );
                    }
                    "ComplexSelector" => {
                        obj.insert(
                            "metadata".to_string(),
                            serde_json::json!({
                                "rule": null,
                                "is_global": false,
                                "used": false,
                            }),
                        );
                    }
                    "RelativeSelector" => {
                        obj.insert(
                            "metadata".to_string(),
                            serde_json::json!({
                                "is_global": false,
                                "is_global_like": false,
                                "scoped": false,
                            }),
                        );
                    }
                    _ => {}
                }
            }

            // Recurse into all values
            for val in obj.values_mut() {
                add_parsecss_metadata(val);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                add_parsecss_metadata(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_css_comments_basic_removal_and_trim() {
        assert_eq!(strip_css_comments("/* c */ 12px"), "12px");
        assert_eq!(strip_css_comments("blue /* c */"), "blue");
        assert_eq!(strip_css_comments("/* a */ red"), "red");
    }

    #[test]
    fn strip_css_comments_interior_whitespace_preserved() {
        assert_eq!(
            strip_css_comments("var(--a, /* c */ red)"),
            "var(--a,  red)",
        );
        assert_eq!(
            strip_css_comments("sidebar /* x */ (min-width: 100px)"),
            "sidebar  (min-width: 100px)",
        );
    }

    #[test]
    fn strip_css_comments_inside_strings_are_preserved() {
        assert_eq!(
            strip_css_comments("\"/* not a comment */\""),
            "\"/* not a comment */\"",
        );
        assert_eq!(strip_css_comments("'/* keep */'"), "'/* keep */'");
    }

    #[test]
    fn strip_css_comments_inside_url_are_preserved() {
        assert_eq!(
            strip_css_comments("url(\"data:image/svg+xml,/* x */\")"),
            "url(\"data:image/svg+xml,/* x */\")",
        );
    }

    #[test]
    fn strip_css_comments_inside_other_functions_are_stripped() {
        // Only url() is special — calc/var/etc. follow normal CSS tokenization,
        // so block comments inside them are stripped just like at top level.
        assert_eq!(
            strip_css_comments("calc(/* x */ 1px + 2px)"),
            "calc( 1px + 2px)",
        );
        assert_eq!(strip_css_comments("URL(/* keep */)"), "URL(/* keep */)");
    }

    #[test]
    fn strip_css_comments_unterminated_kept_verbatim() {
        assert_eq!(strip_css_comments("red /* oops"), "red /* oops");
    }

    #[test]
    fn strip_css_comments_escaped_quote_does_not_close_string() {
        assert_eq!(
            strip_css_comments("\"a\\\" /* in str */ b\" /* real */ c"),
            "\"a\\\" /* in str */ b\"  c",
        );
    }
}
