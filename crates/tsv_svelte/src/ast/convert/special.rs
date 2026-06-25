// Svelte special node conversions
//
// Converts internal special nodes to public format:
// - Script: <script> tags with TypeScript content
// - Style: <style> tags with CSS content
// - SvelteOptions: <svelte:options> configuration
// - SpecialElement: <svelte:*> elements (head, body, window, etc.)

use crate::ast::{internal, public};
use string_interner::DefaultStringInterner;
use tsv_lang::{InfallibleResolve, LocationTracker};

use super::comment_attachment::{
    CommentAttachmentContext, attach_comments_recursively, comment_to_json,
};
use super::{convert_attribute_node, convert_fragment, span_to_name_loc, to_json_value};

/// Detect if a script tag has `lang="ts"` attribute.
///
/// When `lang="ts"` is present, the script is parsed by acorn-typescript (TypeScript context).
/// Otherwise (plain `<script>`), it's parsed by Svelte's parser (Svelte context), which
/// omits `importKind`/`exportKind` for "value" and always includes `attributes` on
/// import/export declarations.
fn script_has_lang_ts(
    script: &internal::Script,
    source: &str,
    interner: &DefaultStringInterner,
) -> bool {
    for attr_node in &script.attributes {
        let internal::AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let name = interner.resolve_infallible(attr.name);
        if name == "lang"
            && let Some(values) = &attr.value
            && let Some(internal::AttributeValue::Text(text)) = values.first()
            && text.data(source) == "ts"
        {
            return true;
        }
    }
    false
}

pub(super) fn convert_script(
    script: &internal::Script,
    source: &str,
    interner: &DefaultStringInterner,
    html_leading_comment: Option<&internal::HtmlComment>,
) -> public::Script {
    let context = script.context.as_str();

    // Use full source LocationTracker for absolute line/column numbers everywhere
    let loc = LocationTracker::new(source);

    // Detect whether this is a TypeScript script (lang="ts") or plain script.
    // Plain scripts use Svelte's parser conventions (no importKind/exportKind for "value",
    // always include `attributes` on import/export declarations).
    let is_lang_ts = script_has_lang_ts(script, source, interner);

    // Delegate to tsv_ts for program conversion, using the appropriate schema
    let schema = if is_lang_ts {
        tsv_ts::ast::convert::Schema::Acorn
    } else {
        tsv_ts::ast::convert::Schema::SvelteScript
    };
    let mut program = tsv_ts::ast::convert::convert_program(&script.content, source, &loc, schema);

    // Svelte uses the line of the <script> tag itself, not the content start
    // (matters when the opening tag spans multiple lines, e.g., multiline attr values)
    let start_pos = loc.offset_to_position(script.span.start as usize);
    program.loc.start = tsv_ts::ast::public::Position {
        line: start_pos.line,
        column: 0,
        character: None,
    };

    // Svelte's quirk: loc.end extends to the end of the </script> closing tag
    let end_pos = loc.offset_to_position(script.span.end as usize);
    program.loc.end = tsv_ts::ast::public::Position {
        line: end_pos.line,
        column: end_pos.column,
        character: None,
    };

    // Attach comments to all nodes (recursively)
    // Convert program to JSON so we can inject leadingComments/trailingComments
    //
    // Architecture note: We use JSON roundtrip instead of adding fields to AST structs
    // because it keeps the internal TypeScript AST clean (zero Svelte-specific pollution).
    // The trade-off is we lose type safety on Script.content (now serde_json::Value),
    // but this is acceptable for the public API layer.
    let mut program_json = to_json_value(&program);

    // Convert comments to JSON and build the queue (sorted by position, already in order).
    // Each comment is emitted once: tsv corrects acorn-typescript's backtrack-reparse comment
    // duplication rather than replicating it (see docs/conformance_svelte.md §Comment Attachment
    // Differences).
    let comment_queue: std::collections::VecDeque<serde_json::Value> = script
        .content
        .comments
        .iter()
        .map(|c| comment_to_json(c, source))
        .collect();

    // Create context for comment attachment (acorn-style queue algorithm)
    let mut ctx = CommentAttachmentContext {
        comments: comment_queue,
        source,
    };

    // Recursively attach comments to all nodes using acorn's DFS queue algorithm
    attach_comments_recursively(&mut program_json, &mut ctx);

    // Vanilla acorn (non-TS) emits `"options": null` on ImportExpression nodes;
    // acorn-typescript omits the field entirely. We inject it via JSON post-processing
    // rather than adding an Option<Value> field to the typed public AST, because:
    // 1. This is a Svelte-specific acorn quirk (tsv_ts shouldn't know about it)
    // 2. serde's skip_serializing_if can't conditionally include a null field
    // 3. The JSON roundtrip already exists for comment attachment (same pattern)
    if !is_lang_ts {
        inject_import_options_null(&mut program_json);
    }

    // Inject HTML comment immediately preceding <script> as leadingComments on Program.
    // Svelte treats these as type "Line" (its convention for HTML comments) with just
    // the comment content (no start/end/loc fields).
    if let Some(comment) = html_leading_comment
        && let serde_json::Value::Object(ref mut map) = program_json
    {
        let html_comment = serde_json::json!({
            "type": "Line",
            "value": comment.content(source),
        });
        match map.get_mut("leadingComments") {
            Some(serde_json::Value::Array(arr)) => {
                // Prepend HTML comment before any JS comments
                arr.insert(0, html_comment);
            }
            _ => {
                map.insert(
                    "leadingComments".to_string(),
                    serde_json::Value::Array(vec![html_comment]),
                );
            }
        }
    }

    // Return program_json directly (preserves leadingComments/trailingComments)
    public::Script {
        node_type: "Script".to_string(),
        start: script.span.start,
        end: script.span.end,
        context: context.to_string(),
        content: program_json,
        attributes: script
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, &loc, interner))
            .collect(),
    }
}

/// Convert internal SvelteOptions to public format
/// Find a named attribute's value in `<svelte:options>` attributes.
fn find_option_values<'a>(
    attrs: &'a [internal::AttributeNode],
    name: &str,
    interner: &DefaultStringInterner,
) -> Option<&'a Vec<internal::AttributeValue>> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && interner.resolve_infallible(attr.name) == name
        {
            attr.value.as_ref()
        } else {
            None
        }
    })
}

/// Extract a plain text value from attribute values.
fn text_value(values: &[internal::AttributeValue], source: &str) -> Option<String> {
    values.iter().find_map(|v| {
        if let internal::AttributeValue::Text(text) = v {
            Some(text.data(source).into_owned())
        } else {
            None
        }
    })
}

/// Find a boolean option — shorthand (`name`) or explicit (`name={true/false}`).
fn bool_option(
    attrs: &[internal::AttributeNode],
    name: &str,
    interner: &DefaultStringInterner,
) -> Option<bool> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && interner.resolve_infallible(attr.name) == name
        {
            match &attr.value {
                None => Some(true),
                Some(values) => values.iter().find_map(|v| {
                    if let internal::AttributeValue::ExpressionTag(expr) = v
                        && let tsv_ts::ast::internal::Expression::Literal(lit) = &expr.expression
                        && let tsv_ts::ast::internal::LiteralValue::Boolean(b) = lit.value
                    {
                        Some(b)
                    } else {
                        None
                    }
                }),
            }
        } else {
            None
        }
    })
}

pub(super) fn convert_svelte_options(
    options: &internal::SvelteOptions,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SvelteOptions {
    let runes = bool_option(&options.attributes, "runes", interner);
    let immutable = bool_option(&options.attributes, "immutable", interner);
    let accessors = bool_option(&options.attributes, "accessors", interner);
    let preserve_whitespace = bool_option(&options.attributes, "preserveWhitespace", interner);

    // `css` — plain text value (`css="injected"`)
    let css = find_option_values(&options.attributes, "css", interner)
        .and_then(|v| text_value(v, source));

    // `namespace` — plain text value
    let namespace = find_option_values(&options.attributes, "namespace", interner)
        .and_then(|v| text_value(v, source));

    // `customElement` — object expression, string expression, or plain text
    let custom_element = find_option_values(&options.attributes, "customElement", interner)
        .and_then(|values| {
            values.iter().find_map(|v| {
                // Expression tag: customElement={{ tag: '...', shadow: '...' }}
                if let internal::AttributeValue::ExpressionTag(expr) = v
                    && let tsv_ts::ast::internal::Expression::ObjectExpression(obj) =
                        &expr.expression
                {
                    let mut map = serde_json::Map::new();
                    for prop in &obj.properties {
                        if let tsv_ts::ast::internal::ObjectProperty::Property(p) = prop
                            && let tsv_ts::ast::internal::Expression::Identifier(key) = &p.key
                            && let tsv_ts::ast::internal::Expression::Literal(val) = &p.value
                        {
                            let key_name = interner.resolve_infallible(key.name).to_string();
                            let json_val = match &val.value {
                                tsv_ts::ast::internal::LiteralValue::String { content, .. } => {
                                    serde_json::Value::String(content.clone())
                                }
                                tsv_ts::ast::internal::LiteralValue::Boolean(b) => {
                                    serde_json::Value::Bool(*b)
                                }
                                _ => continue,
                            };
                            map.insert(key_name, json_val);
                        }
                    }
                    return Some(serde_json::Value::Object(map));
                }
                // Plain text or string literal: customElement="tag-name"
                let tag_str = match v {
                    internal::AttributeValue::Text(text) => Some(text.data(source).into_owned()),
                    internal::AttributeValue::ExpressionTag(expr) => {
                        if let tsv_ts::ast::internal::Expression::Literal(lit) = &expr.expression
                            && let tsv_ts::ast::internal::LiteralValue::String { content, .. } =
                                &lit.value
                        {
                            Some(content.clone())
                        } else {
                            None
                        }
                    }
                };
                tag_str.map(|tag| serde_json::json!({ "tag": tag }))
            })
        });

    public::SvelteOptions {
        start: options.span.start,
        end: options.span.end,
        attributes: options
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
        runes,
        immutable,
        accessors,
        preserve_whitespace,
        css,
        namespace,
        custom_element,
    }
}

pub(super) fn convert_style(
    style: &internal::Style,
    source: &str,
    interner: &DefaultStringInterner,
    preceding_comment: Option<&internal::HtmlComment>,
) -> tsv_css::StyleSheet {
    // Create LocationTracker for the full source (for attributes)
    let full_loc = LocationTracker::new(source);

    // Extract the raw CSS content
    let styles = style.content_span.extract(source).to_string();

    // Delegate to tsv_css for CSS node conversion
    // Comments are stored separately in stylesheet.comments and not included in JSON output
    let children: Vec<serde_json::Value> = style
        .css_stylesheet
        .nodes
        .iter()
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
                let public_attr = convert_attribute_node(attr, source, &full_loc, interner);
                to_json_value(&public_attr)
            })
            .collect(),
        children,
        content: tsv_css::StyleContent {
            start: style.content_span.start,
            end: style.content_span.end,
            styles,
            comment: preceding_comment.map(|c| {
                serde_json::json!({
                    "type": "Comment",
                    "start": c.span.start,
                    "end": c.span.end,
                    "data": c.content(source),
                })
            }),
        },
    }
}

pub(super) fn convert_special_element(
    elem: &internal::SpecialElement,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SpecialElement {
    // Extract tag and expression from the kind enum
    let tag = elem.kind.tag().map(|e| {
        // For plain string attributes (this="hello"), Svelte produces a Literal
        // without loc and with single-quoted raw, rather than normal expression conversion
        if let tsv_ts::ast::internal::Expression::Literal(lit) = e
            && let tsv_ts::ast::internal::LiteralValue::String { content, .. } = &lit.value
        {
            let raw_source = lit.span.extract(source);
            if !raw_source.starts_with('\'') && !raw_source.starts_with('"') {
                return serde_json::json!({
                    "type": "Literal",
                    "value": content,
                    "raw": format!("'{content}'"),
                    "start": lit.span.start,
                    "end": lit.span.end,
                });
            }
        }
        to_json_value(&tsv_ts::ast::convert::convert_expression(
            e, source, loc, interner, 0,
        ))
    });

    let expression = elem
        .kind
        .expression()
        .map(|e| tsv_ts::ast::convert::convert_expression(e, source, loc, interner, 0));

    public::SpecialElement {
        node_type: elem.kind.node_type().to_string(),
        start: elem.span.start,
        end: elem.span.end,
        name: elem.kind.tag_name().to_string(),
        name_loc: span_to_name_loc(elem.name_span, loc),
        attributes: elem
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
        fragment: convert_fragment(&elem.fragment, source, loc, interner),
        tag,
        expression,
    }
}

/// Inject `"options": null` on all ImportExpression nodes in a JSON AST.
/// Vanilla acorn (ecmaVersion 16) always emits this field; acorn-typescript omits it.
fn inject_import_options_null(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("type").and_then(|v| v.as_str()) == Some("ImportExpression") {
                map.entry("options").or_insert(serde_json::Value::Null);
            }
            for v in map.values_mut() {
                inject_import_options_null(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                inject_import_options_null(v);
            }
        }
        _ => {}
    }
}
