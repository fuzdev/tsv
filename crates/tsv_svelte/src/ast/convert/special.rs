// Svelte special node conversions
//
// Converts internal special nodes to public format:
// - Script: <script> tags with TypeScript content
// - Style: <style> tags with CSS content
// - SvelteOptions: <svelte:options> configuration
// - SpecialElement: <svelte:*> elements (head, body, window, etc.)

use crate::ast::{internal, public};
use std::borrow::Cow;
use std::collections::VecDeque;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    Comment, InfallibleResolve, JsonWriter, LocationMapper, LocationTracker, Position,
    estimated_json_capacity,
};
use tsv_ts::ast::convert::{
    Schema, WriterComments, convert_expression, write_expression_embedded, write_program_embedded,
};

use tsv_ts::ast::convert::write_variable_declaration_embedded;

use super::comment_attachment::{
    CommentAttachmentContext, attach_comments_recursively, attach_const_tag_declaration,
    attach_declaration_tag_declaration, attach_snippet_parameters, comment_to_json,
    try_attach_comments_to_node,
};
use super::{convert_attribute_node, convert_fragment, span_to_name_loc, to_json_value};

/// Emit an internal expression's byte-space wire JSON (identity map) and parse
/// it back — the reusable skeleton the island-scoped attach passes mutate.
#[allow(clippy::expect_used)]
fn expression_skeleton(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
) -> serde_json::Value {
    let mut w = JsonWriter::with_capacity(estimated_json_capacity(source.len()));
    write_expression_embedded(
        &mut w,
        expr,
        source,
        LocationMapper::identity(tracker),
        interner,
    );
    serde_json::from_slice(w.as_bytes()).expect("writer emits valid JSON")
}

/// Build the per-node comment map for a comment-bearing template expression
/// island (`{expr}`, block test, directive expression, `{@debug}` id, spread,
/// `<svelte:element>` tag/`<svelte:component>` expression, snippet name).
///
/// The writer emits the expression's byte-space wire JSON, it's run through the
/// island-scoped attach (`try_attach_comments_to_node` — the same window the
/// `Value` dispatcher uses), and the assignments are read back into a
/// `WriterComments` the fused emit consults at each node's close. Byte-identical
/// to the `Value` oracle's convert + attach + splice.
pub(super) fn build_expression_writer_comments(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    container_start: u32,
    range_end: u32,
) -> WriterComments {
    let mut value = expression_skeleton(expr, source, tracker, interner);
    try_attach_comments_to_node(
        &mut value,
        template_comments,
        source,
        container_start,
        range_end,
    );
    WriterComments::from_attached_skeleton(&value)
}

/// Build the per-node comment map for a comment-bearing `{@const id = init}`.
///
/// Svelte runs `add_comments` on the **init expression directly**, so only the
/// init subtree can carry comments — the map is read off an init-only skeleton
/// wrapped in the minimal declaration shape `attach_const_tag_declaration`
/// navigates (`declarations[0].init`). The `VariableDeclaration`/
/// `VariableDeclarator` envelope and the `end = tag.span.end - 1` rewrite carry
/// no comments and are reproduced at emit time.
pub(super) fn build_const_tag_writer_comments(
    tag: &internal::ConstTag<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
) -> WriterComments {
    let init_skeleton = expression_skeleton(&tag.init, source, tracker, interner);
    let mut declaration = serde_json::json!({
        "declarations": [{ "init": init_skeleton }],
        "end": tag.init.span().end,
    });
    attach_const_tag_declaration(
        &mut declaration,
        template_comments,
        source,
        tag.span.start,
        tag.span.end,
    );
    WriterComments::from_attached_skeleton(&declaration)
}

/// Build the per-node comment map for a comment-bearing `{const …}` / `{let …}`
/// declaration tag. The declaration is a real TS `VariableDeclaration`, so
/// comments attach across its whole tree (`attach_declaration_tag_declaration`).
pub(super) fn build_declaration_tag_writer_comments(
    var_decl: &tsv_ts::ast::internal::VariableDeclaration<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    tag_start: u32,
    tag_end: u32,
) -> WriterComments {
    let mut w = JsonWriter::with_capacity(estimated_json_capacity(source.len()));
    write_variable_declaration_embedded(
        &mut w,
        var_decl,
        source,
        LocationMapper::identity(tracker),
        interner,
    );
    #[allow(clippy::expect_used)]
    let mut value: serde_json::Value =
        serde_json::from_slice(w.as_bytes()).expect("writer emits valid JSON");
    attach_declaration_tag_declaration(&mut value, template_comments, source, tag_start, tag_end);
    WriterComments::from_attached_skeleton(&value)
}

/// Build the merged per-node comment map for a comment-bearing `{#snippet}`
/// parameter list. The parameters share one advancing cursor
/// (`attach_snippet_parameters`) so an inter-parameter comment is claimed once,
/// so the whole list is skeletonized together, attached, and read back into one
/// map keyed by each parameter's spans.
pub(super) fn build_snippet_parameters_writer_comments(
    parameters: &[tsv_ts::ast::internal::Expression<'_>],
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    container_start: u32,
    range_end: u32,
) -> WriterComments {
    let mut values: Vec<serde_json::Value> = parameters
        .iter()
        .map(|p| expression_skeleton(p, source, tracker, interner))
        .collect();
    attach_snippet_parameters(
        &mut values,
        template_comments,
        source,
        container_start,
        range_end,
    );
    WriterComments::from_attached_skeleton(&serde_json::Value::Array(values))
}

/// Build the per-node comment map for a comment-bearing (or preceding-HTML)
/// `<script>` `Program`, for the fused writer to consult at each node's close.
///
/// The writer emits the `Program`'s byte-space wire JSON (the exact structure
/// the final fused emit produces, in byte offsets so the acorn positions line
/// up), it's run through the shared attach DFS with the script's own comments,
/// the preceding HTML comment is prepended to the `Program`'s `leadingComments`
/// (Svelte's `{type: "Line", value}` shape), and the assignments are read back
/// into a span-keyed `WriterComments`. The `options: null` non-TS quirk is
/// reproduced at emit time (schema-driven), not here, so it never perturbs the
/// attach walk.
pub(super) fn build_script_writer_comments(
    script: &internal::Script<'_>,
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    html_leading_comment: Option<&internal::HtmlComment>,
    schema: Schema,
) -> WriterComments {
    // Byte-space skeleton (identity map). `loc` is unused by attach — a dummy
    // override suffices; the final fused emit supplies the real tag-line `loc`.
    let dummy = Position { line: 1, column: 0 };
    let mut w = JsonWriter::with_capacity(estimated_json_capacity(source.len()));
    write_program_embedded(
        &mut w,
        &script.content,
        source,
        LocationMapper::identity(tracker),
        interner,
        schema,
        (dummy, dummy),
        None,
    );
    #[allow(clippy::expect_used)]
    let mut program_json: serde_json::Value =
        serde_json::from_slice(w.as_bytes()).expect("writer emits valid JSON");

    // Attach the script's own comments (byte positions) via acorn's DFS queue.
    let comment_queue: VecDeque<serde_json::Value> = script
        .content
        .comments
        .iter()
        .map(|c| comment_to_json(c, source))
        .collect();
    let mut ctx = CommentAttachmentContext {
        comments: comment_queue,
        source,
    };
    attach_comments_recursively(&mut program_json, &mut ctx);

    // Prepend the preceding HTML comment as the Program's first leadingComment
    // (Svelte reports it as `{type: "Line", value}` with no positions).
    if let Some(comment) = html_leading_comment
        && let serde_json::Value::Object(map) = &mut program_json
    {
        let html_comment = serde_json::json!({
            "type": "Line",
            "value": comment.content(source),
        });
        match map.get_mut("leadingComments") {
            Some(serde_json::Value::Array(arr)) => arr.insert(0, html_comment),
            _ => {
                map.insert(
                    "leadingComments".to_string(),
                    serde_json::Value::Array(vec![html_comment]),
                );
            }
        }
    }

    WriterComments::from_attached_skeleton(&program_json)
}

/// Detect if a script tag has `lang="ts"` attribute.
///
/// When `lang="ts"` is present, the script is parsed by acorn-typescript (TypeScript context).
/// Otherwise (plain `<script>`), it's parsed by Svelte's parser (Svelte context), which
/// omits `importKind`/`exportKind` for "value" and always includes `attributes` on
/// import/export declarations.
///
/// `pub(super)` so the wire-JSON writer reuses the exact `lang="ts"` test behind
/// the fused-Program eligibility gate.
pub(super) fn script_has_lang_ts(
    script: &internal::Script<'_>,
    source: &str,
    interner: &DefaultStringInterner,
) -> bool {
    for attr_node in script.attributes {
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

pub(super) fn convert_script<'src>(
    script: &internal::Script<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    html_leading_comment: Option<&internal::HtmlComment>,
) -> public::Script<'src> {
    // Detect whether this is a TypeScript script (lang="ts") or plain script.
    // Plain scripts use Svelte's parser conventions (no importKind/exportKind for "value",
    // always include `attributes` on import/export declarations).
    let is_lang_ts = script_has_lang_ts(script, source, interner);

    // Delegate to tsv_ts for program conversion, using the appropriate schema
    let schema = if is_lang_ts {
        Schema::Acorn
    } else {
        Schema::SvelteScript
    };
    let mut program = tsv_ts::ast::convert::convert_program(
        &script.content,
        source,
        LocationMapper::identity(loc),
        schema,
    );

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

    // Fast path: nothing to inject — no script comments (nothing to attach),
    // no preceding HTML comment, and `lang="ts"` (a plain script may need
    // `"options": null` injected on its ImportExpressions) — so the JSON
    // roundtrip would reproduce the typed program unchanged. Keep it typed:
    // the direct-serialization path then skips this script's `to_value`
    // entirely (`#[serde(untagged)]` makes the arms wire-identical).
    let content =
        if is_lang_ts && script.content.comments.is_empty() && html_leading_comment.is_none() {
            public::ProgramIsland::Typed(program)
        } else {
            public::ProgramIsland::Attached(attached_program_json(
                &program,
                script,
                source,
                is_lang_ts,
                html_leading_comment,
            ))
        };

    public::Script {
        node_type: "Script",
        start: script.span.start,
        end: script.span.end,
        context: script.context.as_str(),
        content,
        attributes: script
            .attributes
            .iter()
            .map(|attr| convert_attribute_node(attr, source, loc, interner))
            .collect(),
    }
}

/// JSON-roundtrip a converted script `Program` and inject what the typed tree
/// can't carry: attached comments, the non-TS `"options": null` quirk, and a
/// preceding HTML comment.
///
/// Architecture note: We use JSON roundtrip instead of adding fields to AST structs
/// because it keeps the internal TypeScript AST clean (zero Svelte-specific pollution).
/// The trade-off is we lose type safety on Script.content (the ProgramIsland::Attached
/// arm is serde_json::Value), but this is acceptable for the public API layer.
fn attached_program_json(
    program: &tsv_ts::ast::public::Program<'_>,
    script: &internal::Script<'_>,
    source: &str,
    is_lang_ts: bool,
    html_leading_comment: Option<&internal::HtmlComment>,
) -> serde_json::Value {
    let mut program_json = to_json_value(program);

    // Convert comments to JSON and build the queue (sorted by position, already in order).
    // Each comment is emitted once: tsv corrects acorn-typescript's backtrack-reparse comment
    // duplication rather than replicating it (see docs/conformance_svelte.md §Comment Attachment
    // Differences).
    let comment_queue: VecDeque<serde_json::Value> = script
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

    program_json
}

/// Convert internal SvelteOptions to public format
/// Find a named attribute's value in `<svelte:options>` attributes.
///
/// `pub(super)` so the wire-JSON writer reproduces `<svelte:options>` extraction
/// (scalar props + `customElement`) without materializing a `public::SvelteOptions`.
pub(super) fn find_option_values<'arena>(
    attrs: &[internal::AttributeNode<'arena>],
    name: &str,
    interner: &DefaultStringInterner,
) -> Option<&'arena [internal::AttributeValue<'arena>]> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && interner.resolve_infallible(attr.name) == name
        {
            attr.value
        } else {
            None
        }
    })
}

/// Extract a plain text value from attribute values.
///
/// `pub(super)` — shared with the wire-JSON writer's fused `<svelte:options>`.
pub(super) fn text_value<'src>(
    values: &[internal::AttributeValue<'_>],
    source: &'src str,
) -> Option<Cow<'src, str>> {
    values.iter().find_map(|v| {
        if let internal::AttributeValue::Text(text) = v {
            Some(text.data(source))
        } else {
            None
        }
    })
}

/// Find a boolean option — shorthand (`name`) or explicit (`name={true/false}`).
///
/// `pub(super)` — shared with the wire-JSON writer's fused `<svelte:options>`.
pub(super) fn bool_option(
    attrs: &[internal::AttributeNode<'_>],
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

pub(super) fn convert_svelte_options<'src>(
    options: &internal::SvelteOptions<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SvelteOptions<'src> {
    let runes = bool_option(options.attributes, "runes", interner);
    let immutable = bool_option(options.attributes, "immutable", interner);
    let accessors = bool_option(options.attributes, "accessors", interner);
    let preserve_whitespace = bool_option(options.attributes, "preserveWhitespace", interner);

    // `css` — plain text value (`css="injected"`)
    let css =
        find_option_values(options.attributes, "css", interner).and_then(|v| text_value(v, source));

    // `namespace` — plain text value
    let namespace = find_option_values(options.attributes, "namespace", interner)
        .and_then(|v| text_value(v, source));

    // `customElement` — object expression, string expression, or plain text
    let custom_element = find_option_values(options.attributes, "customElement", interner)
        .and_then(|values| {
            values.iter().find_map(|v| {
                // Expression tag: customElement={{ tag: '...', shadow: '...' }}
                if let internal::AttributeValue::ExpressionTag(expr) = v
                    && let tsv_ts::ast::internal::Expression::ObjectExpression(obj) =
                        &expr.expression
                {
                    let mut map = serde_json::Map::new();
                    for prop in obj.properties {
                        if let tsv_ts::ast::internal::ObjectProperty::Property(p) = prop
                            && let tsv_ts::ast::internal::Expression::Identifier(key) = &p.key
                            && let tsv_ts::ast::internal::Expression::Literal(val) = &p.value
                        {
                            let key_name = interner.resolve_infallible(key.name).to_string();
                            let json_val = match &val.value {
                                tsv_ts::ast::internal::LiteralValue::String(cooked) => {
                                    serde_json::Value::String(
                                        cooked.resolve(val.span, source).to_string(),
                                    )
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
                            && let tsv_ts::ast::internal::LiteralValue::String(cooked) = &lit.value
                        {
                            Some(cooked.resolve(lit.span, source).to_string())
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

pub(super) fn convert_style<'src>(
    style: &internal::Style<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
    preceding_comment: Option<&internal::HtmlComment>,
) -> tsv_css::StyleSheet<'src> {
    // Extract the raw CSS content (borrowed from source — no allocation)
    let styles = style.content_span.extract(source);

    // Delegate to tsv_css for CSS node conversion
    // Comments are stored separately in stylesheet.comments and not included in JSON output
    let children: Vec<tsv_css::ast::public::CssNodePublic<'src>> = style
        .css_stylesheet
        .nodes
        .iter()
        .map(|node| tsv_css::ast::convert::convert_css_node(node, source))
        .collect();

    tsv_css::StyleSheet {
        node_type: "StyleSheet",
        start: style.span.start,
        end: style.span.end,
        attributes: style
            .attributes
            .iter()
            .map(|attr| {
                let public_attr = convert_attribute_node(attr, source, loc, interner);
                to_json_value(&public_attr)
            })
            .collect(),
        children,
        content: tsv_css::StyleContent {
            start: style.content_span.start,
            end: style.content_span.end,
            styles: Cow::Borrowed(styles),
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

pub(super) fn convert_special_element<'src>(
    elem: &internal::SpecialElement<'_>,
    source: &'src str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> public::SpecialElement<'src> {
    // Extract tag and expression from the kind enum
    let tag = elem
        .kind
        .tag()
        .map(|e| convert_special_tag_value(e, source, loc, interner));

    let expression = elem
        .kind
        .expression()
        .map(|e| convert_expression(e, source, LocationMapper::identity(loc), interner).into());

    public::SpecialElement {
        node_type: elem.kind.node_type(),
        start: elem.span.start,
        end: elem.span.end,
        name: elem.kind.tag_name(),
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

/// Convert a `<svelte:element this={…}>` tag expression to its public JSON `Value`.
///
/// Plain string attributes (`this="hello"`) become a Svelte-style `Literal`
/// without `loc` and with single-quoted `raw`, matching Svelte's parser; every
/// other expression goes through the normal acorn conversion. Positions are
/// byte-based (the caller translates). Shared with the wire-JSON writer
/// (`ast/convert/write.rs`) so the quirk lives in one place.
pub(super) fn convert_special_tag_value(
    e: &tsv_ts::ast::internal::Expression<'_>,
    source: &str,
    loc: &LocationTracker,
    interner: &DefaultStringInterner,
) -> serde_json::Value {
    if let tsv_ts::ast::internal::Expression::Literal(lit) = e
        && let tsv_ts::ast::internal::LiteralValue::String(cooked) = &lit.value
    {
        let raw_source = lit.span.extract(source);
        if !raw_source.starts_with('\'') && !raw_source.starts_with('"') {
            let content = cooked.resolve(lit.span, source);
            return serde_json::json!({
                "type": "Literal",
                "value": content,
                "raw": format!("'{content}'"),
                "start": lit.span.start,
                "end": lit.span.end,
            });
        }
    }
    to_json_value(&convert_expression(
        e,
        source,
        LocationMapper::identity(loc),
        interner,
    ))
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
