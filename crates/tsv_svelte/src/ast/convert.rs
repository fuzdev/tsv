// Svelte AST conversion
//
// Converts internal AST to public JSON-compatible representation.
// Matches Svelte's official parser output format.

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::LocationTracker;

/// Convert Svelte Root AST to public format
pub fn convert_root(root: &internal::Root, source: &str) -> public::Root {
    let loc = LocationTracker::new(source);
    let interner = root.interner.borrow();

    // Svelte Root start/end behavior (discovered through empirical testing)
    //
    // The Root node's start/end positions follow conditional rules based on fragment content:
    //
    // Rule 1: NULL when fragment is EMPTY (no nodes at all)
    //   - Script-only files: `<script>...</script>` → start=null, end=null (0 fragment nodes)
    //   - CSS-only files: `<style>...</style>` → start=null, end=null (0 fragment nodes)
    //   - Empty files or whitespace-only → start=null, end=null (0 fragment nodes)
    //
    // Rule 2: SET when fragment has ANY nodes (including Text nodes)
    //   - Text-only: `plain text` → start=0, end=10 (1 Text node)
    //   - Template-only: `<div></div>` → start=0, end=11 (1 Element node)
    //   - Script+template: `<script>...</script>\n<div></div>` → start=div_start, end=div_end
    //   - Any combination with fragment content (Text, Element, ExpressionTag, etc.)
    //
    // Rule 3: What the positions span
    //   - start: Position of the FIRST fragment node (Text, Element, or ExpressionTag)
    //   - end: Position AFTER the LAST fragment node (Text, Element, or ExpressionTag)
    //
    // Critical insight: Text nodes ARE fragment content
    //   - Text-only files get start/end set (e.g., "hello" → start=0, end=5)
    //   - Script/style-only files have empty fragments, so start=null, end=null
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
    //
    // NOTE: The parser now calculates start/end correctly in the internal AST (root.span),
    // so we use those values directly. The parser handles all the edge cases including:
    // - Script/style tags in any order
    // - Proper root.start positioning (first non-instance-script item)
    // - Maximum end across all top-level nodes
    let (start, end) = {
        // Check if fragment has ANY nodes (Text, Element, or ExpressionTag)
        // Text nodes ARE content - text-only files should get start/end set
        let has_fragment_content = !root.fragment.nodes.is_empty();

        if has_fragment_content {
            // Fragment has content (Text, Element, or ExpressionTag) - use parser's calculated span
            (Some(root.span.start), Some(root.span.end))
        } else {
            // Fragment is empty (script-only, CSS-only, or empty file)

            // ⚠️ BUG REPLICATION: Detect script+CSS with no template
            // Svelte produces inverted positions (start > end) in this case
            if let (Some(css), Some(script)) = (&root.css, &root.instance) {
                // BUG: start=css.start, end=script.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if let (Some(css), Some(script)) = (&root.css, &root.module) {
                // BUG: start=css.start, end=module.end (inverted!)
                (Some(css.span.start), Some(script.span.end))
            } else if root.instance.is_some() || root.module.is_some() || root.css.is_some() {
                // Script-only or CSS-only: null/null
                (None, None)
            } else {
                // Empty file (no script, no CSS, no fragment content)
                (Some(0), Some(0))
            }
        }
    };

    public::Root {
        css: root
            .css
            .as_ref()
            .map(|style| convert_style(style, source, &interner)),
        js: vec![],
        start,
        end,
        node_type: "Root".to_string(),
        fragment: convert_fragment(&root.fragment, source, &loc, &interner),
        options: None,
        comments: vec![],
        instance: root
            .instance
            .as_ref()
            .map(|script| convert_script(script, source, &interner)),
        module: root
            .module
            .as_ref()
            .map(|script| convert_script(script, source, &interner)),
    }
}

fn convert_fragment(
    fragment: &internal::Fragment,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Fragment {
    public::Fragment {
        node_type: "Fragment".to_string(),
        nodes: fragment
            .nodes
            .iter()
            .map(|node| convert_fragment_node(node, source, loc, interner))
            .collect(),
    }
}

fn convert_fragment_node(
    node: &internal::FragmentNode,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::FragmentNode {
    match node {
        internal::FragmentNode::Element(elem) => {
            let converted = convert_element(elem, source, loc, interner);
            // Return appropriate variant based on element kind
            match elem.kind {
                internal::ElementKind::Component => public::FragmentNode::Component(converted),
                internal::ElementKind::Html => public::FragmentNode::RegularElement(converted),
            }
        }
        internal::FragmentNode::ExpressionTag(tag) => {
            public::FragmentNode::ExpressionTag(convert_expression_tag(tag, source, loc, interner))
        }
        internal::FragmentNode::Text(text) => public::FragmentNode::Text(convert_text(text)),
    }
}

fn convert_element(
    elem: &internal::Element,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Element {
    // Set node_type based on element kind
    let node_type = match elem.kind {
        internal::ElementKind::Component => "Component",
        internal::ElementKind::Html => "RegularElement",
    };

    public::Element {
        node_type: node_type.to_string(),
        start: elem.span.start,
        end: elem.span.end,
        name: interner.resolve(elem.name).unwrap().to_string(),
        kind: elem.kind,
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, source, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, source, loc, interner),
    }
}

fn convert_expression_tag(
    tag: &internal::ExpressionTag,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::ExpressionTag {
    // Delegate to tsv_ts for expression conversion
    let ts_expr =
        tsv_ts::ast::convert::convert_expression(&tag.expression, source, loc, interner, 0);

    public::ExpressionTag {
        node_type: "ExpressionTag".to_string(),
        start: tag.span.start,
        end: tag.span.end,
        expression: ts_expr,
    }
}

fn convert_attribute(
    attr: &internal::Attribute,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::Attribute {
    // Extract attribute name from interner
    let name = interner.resolve(attr.name).unwrap().to_string();

    // Convert attribute value following Svelte's JSON format:
    // - Boolean attributes (no value): serialize as `true`
    // - Text attributes (contain Text nodes): serialize as array
    // - Pure expression (single ExpressionTag): serialize as object
    // - Multiple expressions: serialize as array
    let value = match &attr.value {
        None => Some(serde_json::Value::Bool(true)), // Boolean attribute
        Some(values) => {
            // Check if any value is Text (string content)
            let has_text = values
                .iter()
                .any(|v| matches!(v, internal::AttributeValue::Text(_)));

            if has_text {
                // Has text content: always serialize as array (even if single Text value)
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(serde_json::to_value(converted).unwrap())
            } else if values.len() == 1 {
                // Single expression only: serialize as object
                Some(
                    serde_json::to_value(convert_attribute_value(
                        &values[0], source, loc, interner,
                    ))
                    .unwrap(),
                )
            } else {
                // Multiple expressions: serialize as array
                let converted: Vec<_> = values
                    .iter()
                    .map(|v| convert_attribute_value(v, source, loc, interner))
                    .collect();
                Some(serde_json::to_value(converted).unwrap())
            }
        }
    };

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
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::AttributeValue {
    match value {
        internal::AttributeValue::Text(text) => public::AttributeValue::Text(convert_text(text)),
        internal::AttributeValue::ExpressionTag(tag) => public::AttributeValue::ExpressionTag(
            convert_expression_tag(tag, source, loc, interner),
        ),
    }
}

fn convert_text(text: &internal::Text) -> public::Text {
    // raw contains original source with entities (&lt;, &#65;, etc.)
    // data contains decoded text (<, A, etc.)
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

    // Use full source LocationTracker for absolute line/column numbers everywhere
    let loc = LocationTracker::new(source);

    // Delegate to tsv_ts for program conversion
    let mut program = tsv_ts::ast::convert::convert_program(&script.content, source, &loc);

    // Svelte's quirk: loc.start is hardcoded to {line: 1, column: 0}
    program.loc.start = tsv_ts::ast::public::Position { line: 1, column: 0 };

    public::Script {
        node_type: "Script".to_string(),
        start: script.span.start,
        end: script.span.end,
        context: context.to_string(),
        content: program,
        attributes: script
            .attributes
            .iter()
            .map(|attr| convert_attribute(attr, source, &loc, interner))
            .collect(),
    }
}

fn convert_style(
    style: &internal::Style,
    source: &str,
    interner: &DefaultStringInterner,
) -> tsv_css::StyleSheet {
    // Create LocationTracker for the full source (for attributes)
    let full_loc = LocationTracker::new(source);

    // Extract the raw CSS content
    let styles =
        source[style.content_span.start as usize..style.content_span.end as usize].to_string();

    // Delegate to tsv_css for CSS node conversion
    // Filter out comments to match Svelte's CSS parser output
    // (Our internal AST has comments for the formatter, but public JSON AST should match Svelte)
    let children: Vec<serde_json::Value> = style
        .css_stylesheet
        .nodes
        .iter()
        .filter(|node| !matches!(node, tsv_css::ast::internal::CssNode::Comment(_)))
        .map(|node| tsv_css::ast::convert::convert_css_node(node, source))
        .collect();

    tsv_css::StyleSheet {
        node_type: "StyleSheet".to_string(),
        start: style.span.start,
        end: style.span.end,
        attributes: style
            .attributes
            .iter()
            .map(|attr| {
                let public_attr = convert_attribute(attr, source, &full_loc, interner);
                serde_json::to_value(public_attr).expect("Failed to serialize attribute")
            })
            .collect(),
        children,
        content: tsv_css::StyleContent {
            start: style.content_span.start,
            end: style.content_span.end,
            styles,
            comment: None,
        },
    }
}
