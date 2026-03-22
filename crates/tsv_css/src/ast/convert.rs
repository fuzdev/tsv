// Conversion from internal AST to public AST
//
// ARCHITECTURE: Single Source of Truth Pattern
//
// CSS values are formatted from the AST, NOT extracted from source text.
// This enables:
// - Clean semantic representation (AST stores structured data, not tagged strings)
// - Normalization applied once during parsing (e.g., Svelte's unicode escape quirk)
// - Source text reserved for spans (positions) and error reporting only
// - Formatter and JSON conversion both derive from the same AST structure
//
// Example: CssValue::String { content: "Hello", quote: '\'' }
// - Parser extracts content and quote separately
// - Lexer normalizes unicode escapes (Svelte quirk applied)
// - Formatter chooses optimal quote style based on content
// - JSON conversion preserves original quote style
// - Both derive from the same semantic AST, not from source slices

use super::internal;
use crate::printer::source_fidelity;

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

/// Convert a CSS declaration to JSON, including !important in value and end position
fn convert_declaration(decl: &internal::CssDeclaration, source: &str) -> serde_json::Value {
    let decl_source = decl.span.extract(source);
    let (property_source, value_source) = split_declaration_svelte_compat(decl_source);

    let (end, value) = if let Some(important_end) = decl.important_end {
        (important_end, format!("{value_source} !important"))
    } else {
        (decl.span.end, value_source.to_string())
    };

    serde_json::json!({
        "type": "Declaration",
        "start": decl.span.start,
        "end": end,
        "property": property_source,
        "value": value,
    })
}

/// Convert PseudoClassArgs to Svelte's expected JSON structure
///
/// Generates the wrapper structure: SelectorList → ComplexSelector → RelativeSelector → Nth
fn convert_pseudo_class_args(args: &internal::PseudoClassArgs) -> serde_json::Value {
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
                nth_node["selector"] = convert_selector_list_filtered(selectors);
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
            convert_selector_list_filtered(selectors)
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

/// Format a CssValue as a string for semantic output (no Svelte quirks)
///
/// Uses centralized formatting utilities from `printer::source_fidelity` to ensure
/// consistency between formatter and JSON output.
///
/// This function implements clean semantic formatting without Svelte quirks.
///
/// # Current Status
/// Not currently used - source extraction is required to preserve ALL fidelity
/// (leading zeros, original formatting). Kept as infrastructure for future optimization.
///
/// # Future Use
/// Could be used for values without backslashes IF we add raw value storage to AST.
/// Trade-off: cleaner JSON vs increased AST memory usage (Sprint 1 removed raw values).
#[allow(dead_code)]
fn format_css_value_for_json(value: &internal::CssValue) -> String {
    match value {
        internal::CssValue::Identifier { name, .. } => {
            source_fidelity::format_identifier_value(name)
        }
        internal::CssValue::String { content, quote, .. } => {
            source_fidelity::format_string_value(content, *quote)
        }
        internal::CssValue::Dimension { value, unit, .. } => {
            source_fidelity::format_dimension_value(*value, unit)
        }
        internal::CssValue::Color { color, .. } => source_fidelity::format_color_value(color),
        internal::CssValue::Function { name, args, .. } => {
            let args_str = args
                .iter()
                .map(format_css_value_for_json)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}({args_str})")
        }
        internal::CssValue::List { values, .. } => values
            .iter()
            .map(format_css_value_for_json)
            .collect::<Vec<_>>()
            .join(" "),
        internal::CssValue::CommaSeparated { values, .. } => values
            .iter()
            .map(format_css_value_for_json)
            .collect::<Vec<_>>()
            .join(", "),
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

    let prelude = convert_selector_list(&rule.selector);

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
fn convert_prelude_to_string(prelude: &internal::PreludeValue, source: &str) -> String {
    match prelude {
        internal::PreludeValue::Values { values, .. } => {
            // Convert structured values back to string representation
            values
                .iter()
                .map(|value| value_to_string(value, source))
                .collect::<Vec<_>>()
                .join(" ")
        }
        internal::PreludeValue::Raw { content, .. } => content.clone(),
        internal::PreludeValue::Selectors {
            root: _,
            limit: _,
            span,
        } => {
            // Format selector lists for @scope: (root) [to (limit)]
            // Extract from source for maximum fidelity
            span.extract(source).to_string()
        }
        internal::PreludeValue::Supports { span, .. } => span.extract(source).trim().to_string(),
        internal::PreludeValue::Container { span, .. } => span.extract(source).trim().to_string(),
        internal::PreludeValue::Media { span, .. } => span.extract(source).trim().to_string(),
    }
}

/// Convert a CssValue to its string representation (for prelude conversion)
fn value_to_string(value: &internal::CssValue, source: &str) -> String {
    match value {
        internal::CssValue::String { span, .. } => {
            // Extract from source to preserve quotes
            span.extract(source).to_string()
        }
        internal::CssValue::Identifier { name, .. } => name.clone(),
        internal::CssValue::Function { name, args, span } => {
            // For functions with args, reconstruct from args
            // For functions without args (like supports with complex conditions), extract from source
            if args.is_empty() {
                // Extract from source (includes the function name and parentheses)
                span.extract(source).to_string()
            } else {
                // Reconstruct function call from args
                let args_str = args
                    .iter()
                    .map(|arg| value_to_string(arg, source))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}({args_str})")
            }
        }
        _ => {
            // For other types, extract from source
            value.span().extract(source).to_string()
        }
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
fn convert_selector_list(selector_list: &internal::SelectorList) -> serde_json::Value {
    let children: Vec<serde_json::Value> = selector_list
        .selectors
        .iter()
        .map(convert_complex_selector)
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
fn convert_selector_list_filtered(selector_list: &internal::SelectorList) -> serde_json::Value {
    let children: Vec<serde_json::Value> = selector_list
        .selectors
        .iter()
        .filter(|selector| !selector_contains_invalid(selector))
        .map(convert_complex_selector)
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
fn convert_complex_selector(complex: &internal::ComplexSelector) -> serde_json::Value {
    let children: Vec<serde_json::Value> = complex
        .children
        .iter()
        .map(convert_relative_selector)
        .collect();

    serde_json::json!({
        "type": "ComplexSelector",
        "start": complex.span.start,
        "end": complex.span.end,
        "children": children,
    })
}

/// Convert a RelativeSelector to JSON
fn convert_relative_selector(relative: &internal::RelativeSelector) -> serde_json::Value {
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
        .map(convert_simple_selector)
        .collect();

    serde_json::json!({
        "type": "RelativeSelector",
        "combinator": combinator,
        "selectors": selectors,
        "start": relative.span.start,
        "end": relative.span.end,
    })
}

/// Convert a SimpleSelector to JSON
fn convert_simple_selector(simple: &internal::SimpleSelector) -> serde_json::Value {
    match simple {
        internal::SimpleSelector::Type {
            namespace: _, // Svelte parser ignores namespace prefix in JSON output
            name,
            span,
        } => {
            // SVELTE QUIRK: Namespace prefixes are parsed but NOT included in the JSON AST
            // Example: svg|rect → {"type": "TypeSelector", "name": "rect"}
            // The namespace is preserved in the source span but not exposed in the JSON
            serde_json::json!({
                "type": "TypeSelector",
                "name": name,
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
        internal::SimpleSelector::Class { name, span } => {
            serde_json::json!({
                "type": "ClassSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Id { name, span } => {
            serde_json::json!({
                "type": "IdSelector",
                "name": name,
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
        internal::SimpleSelector::PseudoClass { name, args, span } => {
            let args_val = args
                .as_ref()
                .map_or(serde_json::Value::Null, convert_pseudo_class_args);

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

/// Convert a list of CSS nodes to a typed StyleSheet structure
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
