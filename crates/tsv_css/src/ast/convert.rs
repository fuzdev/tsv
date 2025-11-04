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
        internal::PseudoClassArgs::Nth { value, span } => {
            // Generate Svelte's triple-wrapper structure
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
                            "type": "Nth",
                            "value": value,
                            "start": span.start,
                            "end": span.end
                        }]
                    }]
                }]
            })
        }
    }
}

/// Format a CssValue as a string for semantic output
/// NOTE: Currently unused - we use source_value for JSON compatibility with Svelte
/// This function is kept for future use by the printer and other semantic tools
#[allow(dead_code)]
fn format_css_value_for_json(value: &internal::CssValue) -> String {
    match value {
        internal::CssValue::Identifier(id) => id.clone(),
        internal::CssValue::String { content, quote } => {
            // Format with original quotes, escape backslashes for JSON
            let escaped_content = content.replace('\\', "\\\\");
            format!("{}{}{}", quote, escaped_content, quote)
        }
        internal::CssValue::Dimension { source, .. } => source.clone(),
        internal::CssValue::Color(color) => match color {
            internal::Color::Named(name) => name.clone(),
            internal::Color::Hex(hex) => hex.clone(),
            internal::Color::Rgb {
                r,
                g,
                b,
                alpha: None,
            } => format!("rgb({}, {}, {})", r, g, b),
            internal::Color::Rgb {
                r,
                g,
                b,
                alpha: Some(a),
            } => format!("rgba({}, {}, {}, {})", r, g, b, a),
            internal::Color::Hsl {
                hue,
                saturation,
                lightness,
                alpha: None,
            } => {
                format!("hsl({}, {}%, {}%)", hue, saturation, lightness)
            }
            internal::Color::Hsl {
                hue,
                saturation,
                lightness,
                alpha: Some(a),
            } => {
                format!("hsla({}, {}%, {}%, {})", hue, saturation, lightness, a)
            }
        },
        internal::CssValue::Function { name, args } => {
            let args_str = args
                .iter()
                .map(format_css_value_for_json)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", name, args_str)
        }
        internal::CssValue::List { values } => values
            .iter()
            .map(format_css_value_for_json)
            .collect::<Vec<_>>()
            .join(" "),
        internal::CssValue::CommaSeparated { values } => values
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
    let declarations: Vec<serde_json::Value> = rule
        .declarations
        .iter()
        .map(|decl| {
            // SVELTE QUIRK: Extract property and value from source to preserve raw escapes
            // Svelte does NOT decode escape sequences in property names (only in selectors)
            // Example: `\00e9motion` stays as `\00e9motion`, not `émotion`
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
        let children: Vec<serde_json::Value> = b
            .children
            .iter()
            .map(|child| convert_atrule_block_child(child, source))
            .collect();

        serde_json::json!({
            "type": "Block",
            "start": b.span.start,
            "end": b.span.end,
            "children": children,
        })
    });

    serde_json::json!({
        "type": "Atrule",
        "name": atrule.name,
        "prelude": atrule.prelude,
        "block": block.unwrap_or(serde_json::Value::Null),
        "start": atrule.span.start,
        "end": atrule.span.end,
    })
}

/// Convert an at-rule block child to JSON representation
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
        internal::CssBlockChild::Comment(comment) => convert_css_comment(comment),
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
            namespace,
            name,
            span,
        } => {
            let mut obj = serde_json::json!({
                "type": "TypeSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            });
            if let Some(ns) = namespace {
                obj["namespace"] = serde_json::Value::String(ns.clone());
            }
            obj
        }
        internal::SimpleSelector::Universal { namespace, span } => {
            // Svelte represents universal selector as TypeSelector with name "*"
            let mut obj = serde_json::json!({
                "type": "TypeSelector",
                "name": "*",
                "start": span.start,
                "end": span.end,
            });
            if let Some(ns) = namespace {
                obj["namespace"] = serde_json::Value::String(ns.clone());
            }
            obj
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
        internal::SimpleSelector::PseudoElement { name, args, span } => {
            let mut obj = serde_json::json!({
                "type": "PseudoElementSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            });
            // PseudoElement args are rare (::slotted(), ::part())
            // For now, we don't have any that use Nth, but keep consistent structure
            if let Some(args) = args {
                obj["args"] = convert_pseudo_class_args(args);
            }
            obj
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
    }
}

/// Convert a list of CSS nodes to a StyleSheet JSON structure
pub fn convert_css_nodes(nodes: &[internal::CssNode], source: &str) -> serde_json::Value {
    // Filter out comments (Svelte's CSS parser doesn't include them in the AST)
    // but convert rules and at-rules
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
