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
use crate::escapes;

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
                        "start": span.start,
                        "end": span.end,
                        "selectors": [nth_node]
                    }]
                }]
            })
        }
        internal::PseudoClassArgs::SelectorList { selectors, .. } => {
            // For :is(), :not(), :where(), :has(), :global() - convert the nested selector list
            // Filter out Invalid selectors (from forgiving parsing) and pseudo-elements
            // (contextually invalid in :is/:where per CSS Selectors Level 4)
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
                        "start": span.start,
                        "end": span.end,
                        "selectors": [{
                            "type": "TypeSelector",
                            "name": value,
                            "start": span.start,
                            "end": span.end
                        }]
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
    use crate::printer::source_fidelity;

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
            format!("{}({})", name, args_str)
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
        internal::CssNode::Comment(comment) => convert_css_comment(comment),
        internal::CssNode::Atrule(atrule) => convert_css_atrule(atrule, source),
    }
}

/// Convert a CSS comment to JSON representation
fn convert_css_comment(comment: &internal::CssComment) -> serde_json::Value {
    serde_json::json!({
        "type": "Comment",
        "start": comment.span.start,
        "end": comment.span.end,
        "data": comment.content,
    })
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
                    // SVELTE QUIRK: Extract property and value from source to preserve raw escapes
                    // Svelte does NOT decode escape sequences in property names (only in selectors)
                    // Example: `\00e9motion` stays as `\00e9motion`, not `émotion`
                    let decl_source = &source[decl.span.start as usize..decl.span.end as usize];

                    // Find the colon separator between property and value
                    let (property_source, value_source) =
                        if let Some(colon_pos) = decl_source.find(':') {
                            let prop = &decl_source[..colon_pos];
                            let val = decl_source[colon_pos + 1..].trim_start();
                            (prop, val)
                        } else {
                            // Shouldn't happen, but fallback
                            (decl_source, "")
                        };

                    // Apply Svelte quirks to value (backslash doubling, unicode duplication)
                    let value_with_quirks = escapes::apply_svelte_quirks(value_source);

                    Some(serde_json::json!({
                        "type": "Declaration",
                        "start": decl.span.start,
                        "end": decl.span.end,
                        "property": property_source,  // Raw from source (Svelte quirk)
                        "value": value_with_quirks,
                    }))
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
        internal::PreludeValue::Selectors { root: _, limit: _, span } => {
            // Format selector lists for @scope: (root) [to (limit)]
            // Extract from source for maximum fidelity
            source[span.start as usize..span.end as usize].to_string()
        }
    }
}

/// Convert a CssValue to its string representation (for prelude conversion)
fn value_to_string(value: &internal::CssValue, source: &str) -> String {
    match value {
        internal::CssValue::String { span, .. } => {
            // Extract from source to preserve quotes
            source[span.start as usize..span.end as usize].to_string()
        }
        internal::CssValue::Identifier { name, .. } => name.clone(),
        internal::CssValue::Function { name, args, span } => {
            // For functions with args, reconstruct from args
            // For functions without args (like supports with complex conditions), extract from source
            if args.is_empty() {
                // Extract from source (includes the function name and parentheses)
                source[span.start as usize..span.end as usize].to_string()
            } else {
                // Reconstruct function call from args
                let args_str = args
                    .iter()
                    .map(|arg| value_to_string(arg, source))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, args_str)
            }
        }
        _ => {
            // For other types, extract from source
            let span = value.span();
            source[span.start as usize..span.end as usize].to_string()
        }
    }
}

/// Convert an at-rule block child to JSON representation
///
/// Note: Comments are filtered out before calling this function (see convert_css_atrule)
fn convert_atrule_block_child(child: &internal::CssBlockChild, source: &str) -> serde_json::Value {
    match child {
        internal::CssBlockChild::Rule(rule) => convert_css_rule(rule, source),
        internal::CssBlockChild::Declaration(decl) => {
            // SVELTE QUIRK: Extract property and value from source to preserve raw escapes
            let decl_source = &source[decl.span.start as usize..decl.span.end as usize];

            // Find the colon separator between property and value
            let (property_source, value_source) = if let Some(colon_pos) = decl_source.find(':') {
                let prop = &decl_source[..colon_pos];
                let val = decl_source[colon_pos + 1..].trim_start();
                (prop, val)
            } else {
                // Shouldn't happen, but fallback
                (decl_source, "")
            };

            // Apply Svelte quirks to value (backslash doubling, unicode duplication)
            let value_with_quirks = escapes::apply_svelte_quirks(value_source);

            serde_json::json!({
                "type": "Declaration",
                "start": decl.span.start,
                "end": decl.span.end,
                "property": property_source,  // Raw from source (Svelte quirk)
                "value": value_with_quirks,
            })
        }
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

/// Convert a SelectorList to JSON, filtering out Invalid and PseudoElement selectors
///
/// Used for pseudo-class arguments (:is, :where, :not, :has) to ensure Svelte compatibility.
///
/// Per CSS Selectors Level 4:
/// - Invalid selectors (from forgiving parsing) are ignored for matching
/// - Pseudo-elements are contextually invalid in :is() and :where()
///
/// This filtering happens at conversion time, not in the internal AST, to preserve
/// full semantic information for the formatter (which outputs all selectors).
fn convert_selector_list_filtered(selector_list: &internal::SelectorList) -> serde_json::Value {
    let children: Vec<serde_json::Value> = selector_list
        .selectors
        .iter()
        .filter(|selector| !selector_contains_invalid_or_pseudo_element(selector))
        .map(convert_complex_selector)
        .collect();

    serde_json::json!({
        "type": "SelectorList",
        "start": selector_list.span.start,
        "end": selector_list.span.end,
        "children": children,
    })
}

/// Check if a complex selector contains Invalid or PseudoElement simple selectors
fn selector_contains_invalid_or_pseudo_element(complex: &internal::ComplexSelector) -> bool {
    for relative in &complex.children {
        for simple in &relative.selectors {
            match simple {
                internal::SimpleSelector::Invalid { .. } => return true,
                internal::SimpleSelector::PseudoElement { .. } => return true,
                _ => {}
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
            let name = match comb {
                internal::Combinator::Descendant => " ",
                internal::Combinator::Child => ">",
                internal::Combinator::NextSibling => "+",
                internal::Combinator::SubsequentSibling => "~",
                internal::Combinator::Column => "||",
            };
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
        "start": relative.span.start,
        "end": relative.span.end,
        "selectors": selectors,
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
                serde_json::Value::String(
                    match m {
                        internal::AttributeMatcher::Exact => "=",
                        internal::AttributeMatcher::Contains => "~=",
                        internal::AttributeMatcher::DashMatch => "|=",
                        internal::AttributeMatcher::Prefix => "^=",
                        internal::AttributeMatcher::Suffix => "$=",
                        internal::AttributeMatcher::Substring => "*=",
                    }
                    .to_string(),
                )
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
                .map(convert_pseudo_class_args)
                .unwrap_or(serde_json::Value::Null);

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
                format!("{}%", value)
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

/// Convert a list of CSS nodes to a StyleSheet JSON structure
pub fn convert_css_nodes(nodes: &[internal::CssNode], source: &str) -> serde_json::Value {
    // Filter out comments to match Svelte's CSS parser output
    // (Our internal AST has comments for the formatter, but public JSON AST should match Svelte)
    let children: Vec<serde_json::Value> = nodes
        .iter()
        .filter(|node| !matches!(node, internal::CssNode::Comment(_)))
        .map(|node| convert_css_node(node, source))
        .collect();

    // Calculate content span from all nodes (including comments for accurate bounds)
    let (content_start, content_end) = if let Some(first) = nodes.first() {
        let start = match first {
            internal::CssNode::Rule(rule) => rule.span.start,
            internal::CssNode::Comment(comment) => comment.span.start,
            internal::CssNode::Atrule(atrule) => atrule.span.start,
        };
        let end = match nodes.last().unwrap() {
            internal::CssNode::Rule(rule) => rule.span.end,
            internal::CssNode::Comment(comment) => comment.span.end,
            internal::CssNode::Atrule(atrule) => atrule.span.end,
        };
        (start, end)
    } else {
        (0, 0)
    };

    serde_json::json!({
        "type": "StyleSheet",
        "start": content_start,
        "end": content_end,
        "attributes": [],
        "children": children,
        "content": {
            "start": content_start,
            "end": content_end,
            "styles": source[content_start as usize..content_end as usize].to_string(),
            "comment": serde_json::Value::Null,
        }
    })
}
