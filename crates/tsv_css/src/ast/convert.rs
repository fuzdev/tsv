// Conversion from internal AST to public AST

use super::internal;

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
            // Extract value as string from source for Svelte compatibility
            let value_str = &source[decl.span.start as usize..decl.span.end as usize];
            // Extract just the value part after "property: "
            let value_only = if let Some(colon_pos) = value_str.find(':') {
                value_str[colon_pos + 1..].trim().trim_end_matches(';').trim()
            } else {
                value_str
            };

            serde_json::json!({
                "type": "Declaration",
                "start": decl.span.start,
                "end": decl.span.end,
                "property": decl.property,
                "value": value_only,
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

    let mut obj = serde_json::json!({
        "type": "Atrule",
        "name": atrule.name,
        "prelude": atrule.prelude,
        "start": atrule.span.start,
        "end": atrule.span.end,
    });

    if let Some(b) = block {
        obj["block"] = b;
    }

    obj
}

/// Convert an at-rule block child to JSON representation
fn convert_atrule_block_child(child: &internal::CssBlockChild, source: &str) -> serde_json::Value {
    match child {
        internal::CssBlockChild::Rule(rule) => convert_css_rule(rule, source),
        internal::CssBlockChild::Declaration(decl) => {
            // Extract value as string from source for Svelte compatibility
            let value_str = &source[decl.span.start as usize..decl.span.end as usize];
            // Extract just the value part after "property: "
            let value_only = if let Some(colon_pos) = value_str.find(':') {
                value_str[colon_pos + 1..].trim().trim_end_matches(';').trim()
            } else {
                value_str
            };

            serde_json::json!({
                "type": "Declaration",
                "start": decl.span.start,
                "end": decl.span.end,
                "property": decl.property,
                "value": value_only,
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
    let combinator = relative.combinator.as_ref().map(|c| match c {
        internal::Combinator::Descendant => " ",
        internal::Combinator::Child => ">",
        internal::Combinator::NextSibling => "+",
        internal::Combinator::SubsequentSibling => "~",
        internal::Combinator::Column => "||",
    });

    let selectors: Vec<serde_json::Value> = relative
        .selectors
        .iter()
        .map(convert_simple_selector)
        .collect();

    serde_json::json!({
        "type": "RelativeSelector",
        "combinator": combinator.map(|s| serde_json::Value::String(s.to_string())).unwrap_or(serde_json::Value::Null),
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
            let mut obj = serde_json::json!({
                "type": "UniversalSelector",
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
            let mut obj = serde_json::json!({
                "type": "AttributeSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            });
            if let Some(ns) = namespace {
                obj["namespace"] = serde_json::Value::String(ns.clone());
            }
            if let Some(m) = matcher {
                obj["matcher"] = serde_json::Value::String(
                    match m {
                        internal::AttributeMatcher::Exact => "=",
                        internal::AttributeMatcher::Contains => "~=",
                        internal::AttributeMatcher::DashMatch => "|=",
                        internal::AttributeMatcher::Prefix => "^=",
                        internal::AttributeMatcher::Suffix => "$=",
                        internal::AttributeMatcher::Substring => "*=",
                    }
                    .to_string(),
                );
            }
            if let Some(v) = value {
                obj["value"] = serde_json::Value::String(v.clone());
            }
            if let Some(f) = flags {
                obj["flags"] = serde_json::Value::String(f.clone());
            }
            obj
        }
        internal::SimpleSelector::PseudoClass {
            name,
            raw_args,
            span,
        } => {
            let mut obj = serde_json::json!({
                "type": "PseudoClassSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            });
            if let Some(args) = raw_args {
                obj["raw_args"] = serde_json::Value::String(args.clone());
            }
            obj
        }
        internal::SimpleSelector::PseudoElement {
            name,
            raw_args,
            span,
        } => {
            let mut obj = serde_json::json!({
                "type": "PseudoElementSelector",
                "name": name,
                "start": span.start,
                "end": span.end,
            });
            if let Some(args) = raw_args {
                obj["raw_args"] = serde_json::Value::String(args.clone());
            }
            obj
        }
        internal::SimpleSelector::Nesting { span } => {
            serde_json::json!({
                "type": "NestingSelector",
                "start": span.start,
                "end": span.end,
            })
        }
        internal::SimpleSelector::Percentage { value, span } => {
            serde_json::json!({
                "type": "PercentageSelector",
                "value": value,
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
