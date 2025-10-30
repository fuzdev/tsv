// Conversion from internal AST to public AST

use super::internal;
use super::public;
use crate::location::LocationTracker;
use crate::span::Span;
use string_interner::DefaultStringInterner;

// Helper to create SourceLocation from Span
fn create_source_location(span: Span, tracker: &LocationTracker) -> public::SourceLocation {
    let (start_line, start_col) = tracker.get_line_column(span.start as usize);
    let (end_line, end_col) = tracker.get_line_column(span.end as usize);

    public::SourceLocation {
        start: public::Position {
            line: start_line,
            column: start_col,
        },
        end: public::Position {
            line: end_line,
            column: end_col,
        },
    }
}

// Helper to create SourceLocation with position offset
// Used for embedded content where AST has global positions but LocationTracker is from substring
// TODO(performance): This creates repetitive if/else checks throughout conversion (9 occurrences).
// Consider alternatives:
// 1. Always use this function, make it handle offset=0 efficiently
// 2. Use a trait or wrapper type for offset-aware conversion
// 3. Create separate LocationTracker that has built-in offset handling
// Current approach: Simple and correct, minimal overhead (branch prediction handles it well)
// Refactor when profiling shows this as a bottleneck
fn create_source_location_with_offset(
    span: Span,
    tracker: &LocationTracker,
    offset: usize,
) -> public::SourceLocation {
    // Subtract offset to get positions relative to the tracker's source
    let adjusted_span = Span {
        start: span.start - offset as u32,
        end: span.end - offset as u32,
    };
    create_source_location(adjusted_span, tracker)
}

pub fn convert_program(program: &internal::Program, loc: &LocationTracker) -> public::Program {
    convert_program_with_offset(program, loc, 0)
}

// Convert Program with position offset for embedded content
fn convert_program_with_offset(
    program: &internal::Program,
    loc: &LocationTracker,
    offset: usize,
) -> public::Program {
    let interner = program.interner.borrow();

    public::Program {
        node_type: "Program".to_string(),
        start: program.span.start,
        end: program.span.end,
        loc: if offset == 0 {
            create_source_location(program.span, loc)
        } else {
            create_source_location_with_offset(program.span, loc, offset)
        },
        body: program
            .body
            .iter()
            .map(|s| convert_statement(s, loc, &interner, offset))
            .collect(),
        source_type: "module".to_string(),
    }
}

fn convert_statement(
    stmt: &internal::Statement,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Statement {
    match stmt {
        internal::Statement::ExpressionStatement(expr_stmt) => {
            public::Statement::ExpressionStatement(public::ExpressionStatement {
                start: expr_stmt.span.start,
                end: expr_stmt.span.end,
                loc: if offset == 0 {
                    create_source_location(expr_stmt.span, loc)
                } else {
                    create_source_location_with_offset(expr_stmt.span, loc, offset)
                },
                expression: convert_expression(&expr_stmt.expression, loc, interner, offset),
            })
        }
        internal::Statement::VariableDeclaration(var_decl) => {
            public::Statement::VariableDeclaration(public::VariableDeclaration {
                start: var_decl.span.start,
                end: var_decl.span.end,
                loc: if offset == 0 {
                    create_source_location(var_decl.span, loc)
                } else {
                    create_source_location_with_offset(var_decl.span, loc, offset)
                },
                declarations: var_decl
                    .declarations
                    .iter()
                    .map(|d| convert_variable_declarator(d, loc, interner, offset))
                    .collect(),
                kind: match var_decl.kind {
                    internal::VariableDeclarationKind::Const => "const".to_string(),
                    internal::VariableDeclarationKind::Let => "let".to_string(),
                    internal::VariableDeclarationKind::Var => "var".to_string(),
                },
            })
        }
    }
}

fn convert_variable_declarator(
    declarator: &internal::VariableDeclarator,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::VariableDeclarator {
    public::VariableDeclarator {
        node_type: "VariableDeclarator".to_string(),
        start: declarator.span.start,
        end: declarator.span.end,
        loc: if offset == 0 {
            create_source_location(declarator.span, loc)
        } else {
            create_source_location_with_offset(declarator.span, loc, offset)
        },
        id: public::Identifier {
            node_type: "Identifier".to_string(),
            start: declarator.id.span.start,
            end: declarator.id.span.end,
            loc: if offset == 0 {
                create_source_location(declarator.id.span, loc)
            } else {
                create_source_location_with_offset(declarator.id.span, loc, offset)
            },
            name: interner.resolve(declarator.id.name).unwrap().to_string(),
            type_annotation: declarator
                .id
                .type_annotation
                .as_ref()
                .map(|ta| convert_type_annotation(ta, loc, offset)),
        },
        init: declarator
            .init
            .as_ref()
            .map(|expr| convert_expression(expr, loc, interner, offset)),
    }
}

fn convert_expression(
    expr: &internal::Expression,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    offset: usize,
) -> public::Expression {
    match expr {
        internal::Expression::Literal(lit) => {
            let value = match &lit.value {
                internal::LiteralValue::Number(n) => serde_json::Value::Number(
                    serde_json::Number::from_f64(*n)
                        .unwrap_or_else(|| serde_json::Number::from(0))
                ),
                internal::LiteralValue::String(s) => serde_json::Value::String(s.clone()),
            };
            public::Expression::Literal(public::Literal {
                start: lit.span.start,
                end: lit.span.end,
                loc: if offset == 0 {
                    create_source_location(lit.span, loc)
                } else {
                    create_source_location_with_offset(lit.span, loc, offset)
                },
                value,
                raw: lit.raw.clone(),
            })
        }
        internal::Expression::Identifier(id) => {
            public::Expression::Identifier(public::Identifier {
                node_type: "Identifier".to_string(),
                start: id.span.start,
                end: id.span.end,
                loc: if offset == 0 {
                    create_source_location(id.span, loc)
                } else {
                    create_source_location_with_offset(id.span, loc, offset)
                },
                name: interner.resolve(id.name).unwrap().to_string(),
                type_annotation: id
                    .type_annotation
                    .as_ref()
                    .map(|ta| convert_type_annotation(ta, loc, offset)),
            })
        }
    }
}

fn convert_type_annotation(
    type_annotation: &internal::TSTypeAnnotation,
    loc: &LocationTracker,
    offset: usize,
) -> public::TSTypeAnnotation {
    public::TSTypeAnnotation {
        node_type: "TSTypeAnnotation".to_string(),
        start: type_annotation.span.start,
        end: type_annotation.span.end,
        loc: if offset == 0 {
            create_source_location(type_annotation.span, loc)
        } else {
            create_source_location_with_offset(type_annotation.span, loc, offset)
        },
        type_annotation: Box::new(convert_type(&type_annotation.type_annotation, loc, offset)),
    }
}

fn convert_type(ts_type: &internal::TSType, loc: &LocationTracker, offset: usize) -> public::TSType {
    match ts_type {
        internal::TSType::TSNumberKeyword(node) => {
            public::TSType::TSNumberKeyword(public::TSNumberKeyword {
                start: node.span.start,
                end: node.span.end,
                loc: if offset == 0 {
                    create_source_location(node.span, loc)
                } else {
                    create_source_location_with_offset(node.span, loc, offset)
                },
            })
        }
    }
}

// ============= Svelte Conversion =============

pub fn convert_root(root: &internal::Root, source: &str) -> public::Root {
    let loc = LocationTracker::new(source);
    let interner = root.interner.borrow();

    // Svelte Root start/end behavior (discovered through empirical testing)
    //
    // The Root node's start/end positions follow conditional rules based on fragment content:
    //
    // Rule 1: NULL when fragment has no non-Text nodes
    //   - Script-only files: `<script>...</script>` → start=null, end=null
    //   - CSS-only files: `<style>...</style>` → start=null, end=null
    //   - Empty files or whitespace-only → start=null, end=null
    //
    // Rule 2: SET when fragment has at least one non-Text node
    //   - Template-only: `<div></div>` → start=0, end=11
    //   - Script+template: `<script>...</script>\n<div></div>` → start=div_start, end=div_end
    //   - Any combination with actual markup (elements, expression tags, etc.)
    //
    // Rule 3: What the positions span
    //   - start: Position of the FIRST non-Text fragment node (Element, ExpressionTag, etc.)
    //   - end: Position AFTER the LAST fragment node (including trailing Text nodes)
    //
    // Critical insight: Text nodes (whitespace) are asymmetric:
    //   - Leading text does NOT affect start (ignored when finding first non-Text node)
    //   - Trailing text DOES affect end (included in final position)
    //
    // Examples:
    //   `<div></div>` → start=0, end=11
    //   `\n\n<div></div>` → start=2 (skips leading text), end=13
    //   `<script>...</script>\n\n<div>{a}</div>` → start=53 (div), end=67 (after closing tag)
    //
    // Note: The Root span does NOT include script/CSS blocks - those are stored separately
    // in ast.instance, ast.module, and ast.css fields.
    //
    // ⚠️ SVELTE BUG REPLICATION (for exact compatibility):
    //
    // When there's script+CSS with NO template markup, Svelte has a bug where start > end:
    //   `<script>...</script>\n<style>...</style>` → start=34 (CSS start), end=33 (script end)
    //
    // This is clearly incorrect (start should never be greater than end), and the correct
    // behavior would be start=null, end=null (no template content).
    //
    // However, we replicate this bug EXACTLY for compatibility with Svelte's behavior.
    // This quirk is isolated to this conversion layer - our internal AST remains clean.
    //
    // Bug pattern:
    //   - Fragment has NO non-Text nodes (no template markup)
    //   - File has BOTH script (instance OR module) AND CSS
    //   - Result: start = css.span.start, end = script.span.end
    //   - This produces inverted positions since CSS comes after script
    //
    // See: tests/fixtures/3_svelte_parser/bug_script_style_without_markup/SVELTE_BUG.md
    let (start, end) = {
        // Find first and last non-Text nodes (elements/expression tags)
        let first_element = root.fragment.nodes.iter().find(|node| {
            !matches!(node, internal::FragmentNode::Text(_))
        });
        let last_element = root.fragment.nodes.iter().rev().find(|node| {
            !matches!(node, internal::FragmentNode::Text(_))
        });

        if let (Some(first), Some(last)) = (first_element, last_element) {
            // Fragment has template markup - span the non-Text nodes
            (Some(first.span().start), Some(last.span().end))
        } else {
            // No template markup - check for the script+CSS bug case

            // ⚠️ BUG REPLICATION: Detect script+CSS with no template
            // Svelte produces inverted positions (start > end) in this case
            if let (Some(css), Some(script)) = (&root.css, &root.instance) {
                // BUG: start=css.start, end=script.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if let (Some(css), Some(script)) = (&root.css, &root.module) {
                // BUG: start=css.start, end=module.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if root.instance.is_some() || root.module.is_some() || root.css.is_some() {
                // Script-only, CSS-only, or only text nodes: null/null (correct behavior)
                (None, None)
            } else {
                // Empty file
                (Some(0), Some(0))
            }
        }
    };

    public::Root {
        node_type: "Root".to_string(),
        start,
        end,
        fragment: convert_fragment(&root.fragment, &loc, &interner),
        instance: root.instance.as_ref().map(|script| {
            convert_script(script, source, &interner)
        }),
        module: None, // Sprint 5 doesn't use module scripts
        css: root.css.as_ref().map(|style| {
            convert_style(style, source, &interner)
        }),
        js: vec![],
        options: None,
        comments: vec![],
    }
}

fn convert_fragment(
    fragment: &internal::Fragment,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Fragment {
    public::Fragment {
        node_type: "Fragment".to_string(),
        nodes: fragment
            .nodes
            .iter()
            .map(|node| convert_fragment_node(node, loc, interner))
            .collect(),
    }
}

fn convert_fragment_node(
    node: &internal::FragmentNode,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::FragmentNode {
    match node {
        internal::FragmentNode::Element(elem) => {
            public::FragmentNode::RegularElement(convert_element(elem, loc, interner))
        }
        internal::FragmentNode::ExpressionTag(tag) => {
            public::FragmentNode::ExpressionTag(convert_expression_tag(tag, loc, interner))
        }
        internal::FragmentNode::Text(text) => {
            public::FragmentNode::Text(convert_text(text))
        }
    }
}

fn convert_element(
    elem: &internal::Element,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Element {
    public::Element {
        node_type: "RegularElement".to_string(),
        start: elem.span.start,
        end: elem.span.end,
        name: interner.resolve(elem.name).unwrap().to_string(),
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, loc, interner),
    }
}

fn convert_expression_tag(
    tag: &internal::ExpressionTag,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ExpressionTag {
    public::ExpressionTag {
        start: tag.span.start,
        end: tag.span.end,
        expression: convert_expression(&tag.expression, loc, interner, 0),
    }
}

fn convert_attribute(
    attr: &internal::Attribute,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Attribute {
    // Extract attribute name from interner
    let name = interner.resolve(attr.name).unwrap().to_string();

    // Convert attribute value if present
    let value = attr.value.as_ref().map(|values| {
        values
            .iter()
            .map(|v| convert_attribute_value(v, loc, interner))
            .collect()
    });

    public::Attribute {
        node_type: "Attribute".to_string(),
        start: attr.span.start,
        end: attr.span.end,
        name,
        value,
    }
}

fn convert_attribute_value(
    value: &internal::AttributeValue,
    _loc: &LocationTracker,
    _interner: &DefaultStringInterner,
) -> public::AttributeValue {
    match value {
        internal::AttributeValue::Text(text) => {
            public::AttributeValue::Text(convert_text(text))
        }
    }
}

fn convert_text(text: &internal::Text) -> public::Text {
    // For Sprint 7, raw and data are the same (no HTML entity decoding yet)
    // Future sprints: data might decode entities (&lt; -> <, &quot; -> ", etc.)
    public::Text {
        node_type: "Text".to_string(),
        start: text.span.start,
        end: text.span.end,
        raw: text.raw.clone(),
        data: text.data.clone(),
    }
}

fn convert_script(
    script: &internal::Script,
    source: &str,
    interner: &DefaultStringInterner,
) -> public::Script {
    // Convert script context enum to string
    let context = match script.context {
        internal::ScriptContext::Default => "default",
        internal::ScriptContext::Module => "module",
    };

    // TODO(performance): We create two LocationTrackers here (one for script, one for attributes).
    // LocationTracker.new() scans entire source to build line index (O(n) where n=source length).
    // For large files with script tags, this doubles the line-scanning work.
    // Alternatives:
    // 1. Share LocationTracker and adjust positions manually (complex, error-prone)
    // 2. Use lazy line index building (only scan when get_line_column called)
    // 3. Pass pre-built LocationTracker from convert_root (thread through all conversions)
    // Current: Simple and correct, cost is acceptable for typical Svelte file sizes
    // Optimize if profiling shows this as bottleneck

    // Create LocationTracker from script content substring for correct line/column
    // The Program span gives us the global positions of the script content
    let script_content = &source[script.content.span.start as usize..script.content.span.end as usize];
    let script_loc = LocationTracker::new(script_content);

    // Create LocationTracker for attributes (they use global positions in full source)
    let full_loc = LocationTracker::new(source);

    // Convert Program with offset so line/column are relative to script content
    let content_offset = script.content.span.start as usize;

    public::Script {
        node_type: "Script".to_string(),
        start: script.span.start,
        end: script.span.end,
        context: context.to_string(),
        content: convert_program_with_offset(&script.content, &script_loc, content_offset),
        attributes: script
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, &full_loc, interner))
            .collect(),
    }
}

fn convert_style(
    style: &internal::Style,
    source: &str,
    interner: &DefaultStringInterner,
) -> public::StyleSheet {
    // Create LocationTracker for the full source (for attributes)
    let full_loc = LocationTracker::new(source);

    // Extract the raw CSS content
    let styles = source[style.content_span.start as usize..style.content_span.end as usize].to_string();

    // Convert CSS nodes to JSON
    let children = style
        .css_nodes
        .iter()
        .map(|node| convert_css_node(node, source))
        .collect();

    public::StyleSheet {
        node_type: "StyleSheet".to_string(),
        start: style.span.start,
        end: style.span.end,
        attributes: style
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, &full_loc, interner))
            .collect(),
        children,
        content: public::StyleContent {
            start: style.content_span.start,
            end: style.content_span.end,
            styles,
            comment: None,
        },
    }
}

fn convert_css_node(node: &internal::CssNode, source: &str) -> serde_json::Value {
    match node {
        internal::CssNode::Rule(rule) => convert_css_rule(rule, source),
    }
}

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
