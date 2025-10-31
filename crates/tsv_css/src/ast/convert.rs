// Conversion from internal AST to public AST

use super::internal;

/// Convert a CSS node to JSON representation
pub fn convert_css_node(node: &internal::CssNode, source: &str) -> serde_json::Value {
    match node {
        internal::CssNode::Rule(rule) => convert_css_rule(rule, source),
    }
}

/// Convert a CSS rule to JSON representation
fn convert_css_rule(rule: &internal::CssRule, _source: &str) -> serde_json::Value {
    // For minimal implementation, create a simplified Rule structure
    // TODO: Properly parse selectors into SelectorList, ComplexSelector, RelativeSelector, TypeSelector hierarchy

    let declarations: Vec<serde_json::Value> = rule
        .declarations
        .iter()
        .map(|decl| {
            serde_json::json!({
                "type": "Declaration",
                "start": decl.span.start,
                "end": decl.span.end,
                "property": decl.property,
                "value": decl.value,
            })
        })
        .collect();

    // Create a minimal selector structure
    // TODO: Parse selector properly to match Svelte's structure
    let prelude = serde_json::json!({
        "type": "SelectorList",
        "start": rule.selector_span.start,
        "end": rule.selector_span.end,
        "children": [
            {
                "type": "ComplexSelector",
                "start": rule.selector_span.start,
                "end": rule.selector_span.end,
                "children": [
                    {
                        "type": "RelativeSelector",
                        "combinator": serde_json::Value::Null,
                        "selectors": [
                            {
                                "type": "TypeSelector",
                                "name": rule.selector.trim(),
                                "start": rule.selector_span.start,
                                "end": rule.selector_span.end,
                            }
                        ],
                        "start": rule.selector_span.start,
                        "end": rule.selector_span.end,
                    }
                ]
            }
        ]
    });

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

/// Convert a list of CSS nodes to a StyleSheet JSON structure
pub fn convert_css_nodes(nodes: &[internal::CssNode], source: &str) -> serde_json::Value {
    let children: Vec<serde_json::Value> = nodes
        .iter()
        .map(|node| convert_css_node(node, source))
        .collect();

    // Calculate content span from all nodes
    let (content_start, content_end) = if let Some(first) = nodes.first() {
        let start = match first {
            internal::CssNode::Rule(rule) => rule.span.start,
        };
        let end = match nodes.last().unwrap() {
            internal::CssNode::Rule(rule) => rule.span.end,
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
