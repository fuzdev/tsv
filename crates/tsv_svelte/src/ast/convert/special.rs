// Svelte special-node writer support: byte-space skeletons + comment maps.
//
// The wire-JSON writer (`ast/convert/write.rs`) composes these helpers to emit
// `<script>` / `<svelte:options>` and the comment-bearing template islands
// without materializing a typed public tree. Each `build_*_writer_comments`
// skeletonizes an island's byte-space wire JSON, runs the shared acorn attach
// DFS over it, and reads the assignments back into a span-keyed
// `WriterComments` the fused writer consults at each node's close.

use crate::ast::internal;
use std::borrow::Cow;
use std::collections::VecDeque;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    Comment, InfallibleResolve, JsonWriter, LocationMapper, LocationTracker, Position,
    estimated_json_capacity,
};
use tsv_ts::ast::convert::{
    Schema, WriterComments, write_expression_embedded, write_program_embedded,
    write_variable_declaration_embedded,
};

use super::comment_attachment::{
    CommentAttachmentContext, attach_comments_recursively, attach_declaration_tag_declaration,
    attach_expression_list, comment_to_json, try_attach_comments_to_node,
};

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
/// fused writer uses), and the assignments are read back into a
/// `WriterComments` the fused emit consults at each node's close.
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
/// Canonical Svelte runs **two** acorn parses, each with its own comment
/// attach: `read_pattern` parses a destructure id as a synthetic
/// `(pattern = 1)` expression (so an id-internal comment attaches inside the
/// pattern subtree — e.g. a destructure default's literal), and
/// `read_expression` parses the init (comments from after the id through the
/// tag close attach in the init subtree). Comments *between* the pattern and
/// the `=` are a canonical parse error, so the two windows partition the tag.
/// The `VariableDeclaration`/`VariableDeclarator` envelope carries no comments
/// and is reproduced at emit time.
pub(super) fn build_const_tag_writer_comments(
    tag: &internal::ConstTag<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
) -> WriterComments {
    let id_span = tag.id.span();
    let mut id_skeleton = expression_skeleton(&tag.id, source, tracker, interner);
    try_attach_comments_to_node(
        &mut id_skeleton,
        template_comments,
        source,
        id_span.start,
        id_span.end,
    );
    let mut init_skeleton = expression_skeleton(&tag.init, source, tracker, interner);
    try_attach_comments_to_node(
        &mut init_skeleton,
        template_comments,
        source,
        id_span.end,
        tag.span.end,
    );
    let combined = serde_json::json!({ "id": id_skeleton, "init": init_skeleton });
    WriterComments::from_attached_skeleton(&combined)
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

/// Build the merged per-node comment map for a comment-bearing expression list
/// (`{#snippet}` parameters, multi-identifier `{@debug}`). Canonical Svelte
/// parses the list in one acorn parse, so the whole list is skeletonized
/// together, attached via one shared queue (`attach_expression_list` — an
/// inter-item comment is claimed exactly once, per acorn's same-line rule),
/// and read back into one map keyed by each item's spans. `wrapper_end` is the
/// discarded parse wrapper's end (`{@debug}`'s `SequenceExpression` — its last
/// item never claims a trailing comment); `None` for snippet parameters.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_expression_list_writer_comments(
    items: &[tsv_ts::ast::internal::Expression<'_>],
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    container_start: u32,
    range_end: u32,
    wrapper_end: Option<u32>,
) -> WriterComments {
    let mut values: Vec<serde_json::Value> = items
        .iter()
        .map(|p| expression_skeleton(p, source, tracker, interner))
        .collect();
    attach_expression_list(
        &mut values,
        template_comments,
        source,
        container_start,
        range_end,
        wrapper_end,
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

/// Find a named attribute's value in `<svelte:options>` attributes.
///
/// `pub(super)` so the wire-JSON writer reproduces `<svelte:options>` extraction
/// (scalar props + `customElement`) without materializing a typed options struct.
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
