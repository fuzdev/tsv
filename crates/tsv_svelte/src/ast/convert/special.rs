// Svelte special-node writer support: byte-space skeletons + comment maps.
//
// The wire-JSON writer (`ast/convert/write.rs`) composes these helpers to emit
// `<script>` / `<svelte:options>` and the comment-bearing template islands
// without materializing a typed public tree. Each `build_*_writer_comments`
// emits an island's byte-space skeleton with the wire tree recorded as it goes
// (`SkeletonRecorder` — no re-parse of the emitted bytes), runs the shared
// acorn attach DFS over the recorded tree, and folds the assignments into a
// span-keyed `WriterComments` the fused writer consults at each node's close.

use crate::ast::internal;
use std::borrow::Cow;
use std::collections::VecDeque;
use string_interner::DefaultStringInterner;
use tsv_lang::{
    Comment, InfallibleResolve, JsonWriter, LocationMapper, LocationTracker, Position, Span,
    estimated_json_capacity,
};
use tsv_ts::ast::convert::{
    CommentMode, Schema, SkeletonRecorder, SkeletonTree, WriterComments, write_expression_embedded,
    write_program_embedded, write_variable_declaration_embedded,
};

use super::comment_attachment::{
    CommentAttachmentContext, attach_comments_recursively, attach_expression_list,
    try_attach_comments_to_node,
};

/// A throwaway skeleton-emit buffer, sized for the island's own span (the
/// skeleton bytes are discarded — only the recorded tree is used — so the
/// buffer never needs the whole document's capacity).
///
/// TODO: the Record pass still writes the full skeleton bytes into this
/// discarded buffer (the residual floor of a comment-bearing island's build
/// cost). Eliminating it needs either a null-sink `JsonWriter` mode (a branch
/// in the hot write path) or a monomorphized recording-only walk (duplicates
/// the writer — wasm bloat); neither is a clear win at the current cost.
fn skeleton_writer(island_span: Span) -> JsonWriter {
    JsonWriter::with_capacity(estimated_json_capacity(
        (island_span.end - island_span.start) as usize,
    ))
}

/// Record an internal expression's wire tree via a byte-space skeleton emit
/// (identity map) — the structure the island-scoped attach passes walk.
fn expression_skeleton(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
) -> SkeletonTree {
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(expr.span());
    write_expression_embedded(
        &mut w,
        expr,
        source,
        LocationMapper::identity(tracker),
        interner,
        CommentMode::Record(&recorder),
    );
    recorder.finish()
}

/// Build the per-node comment map for a comment-bearing template expression
/// island (`{expr}`, block test, directive expression, `{@debug}` id, spread,
/// `<svelte:element>` tag/`<svelte:component>` expression, snippet name).
///
/// The writer records the expression's wire tree during a byte-space skeleton
/// emit, it's run through the island-scoped attach
/// (`try_attach_comments_to_node` — the same window the fused writer uses),
/// and the assignments fold into a `WriterComments` the fused emit consults at
/// each node's close.
pub(super) fn build_expression_writer_comments(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    container_start: u32,
    range_end: u32,
) -> WriterComments {
    let tree = expression_skeleton(expr, source, tracker, interner);
    let mut out = WriterComments::default();
    try_attach_comments_to_node(
        &tree,
        tree.roots()[0],
        template_comments,
        source,
        container_start,
        range_end,
        &mut out,
    );
    out
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
    let mut out = WriterComments::default();
    let id_tree = expression_skeleton(&tag.id, source, tracker, interner);
    try_attach_comments_to_node(
        &id_tree,
        id_tree.roots()[0],
        template_comments,
        source,
        id_span.start,
        id_span.end,
        &mut out,
    );
    let init_tree = expression_skeleton(&tag.init, source, tracker, interner);
    try_attach_comments_to_node(
        &init_tree,
        init_tree.roots()[0],
        template_comments,
        source,
        id_span.end,
        tag.span.end,
        &mut out,
    );
    out
}

/// Build the per-node comment map for a comment-bearing `{const …}` / `{let …}`
/// declaration tag. The declaration is acorn-parsed, so comments attach across
/// the **whole `VariableDeclaration` tree** (every declarator and its id/init)
/// per acorn's recursive attachment — attaching only to the first init left a
/// comment leading a later declarator (`{let a = 1, /* c */ b}`) unattached.
pub(super) fn build_declaration_tag_writer_comments(
    var_decl: &tsv_ts::ast::internal::VariableDeclaration<'_>,
    template_comments: &[&Comment],
    source: &str,
    tracker: &LocationTracker,
    interner: &DefaultStringInterner,
    tag_start: u32,
    tag_end: u32,
) -> WriterComments {
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(var_decl.span);
    write_variable_declaration_embedded(
        &mut w,
        var_decl,
        source,
        LocationMapper::identity(tracker),
        interner,
        CommentMode::Record(&recorder),
    );
    let tree = recorder.finish();
    let mut out = WriterComments::default();
    try_attach_comments_to_node(
        &tree,
        tree.roots()[0],
        template_comments,
        source,
        tag_start,
        tag_end,
        &mut out,
    );
    out
}

/// Build the merged per-node comment map for a comment-bearing expression list
/// (`{#snippet}` parameters, multi-identifier `{@debug}`). Canonical Svelte
/// parses the list in one acorn parse, so the whole list is recorded into one
/// skeleton tree (one root per item), attached via one shared queue
/// (`attach_expression_list` — an inter-item comment is claimed exactly once,
/// per acorn's same-line rule), and folded into one map keyed by each item's
/// spans. `wrapper_end` is the discarded parse wrapper's end (`{@debug}`'s
/// `SequenceExpression` — its last item never claims a trailing comment);
/// `None` for snippet parameters.
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
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(Span::new(container_start, range_end));
    for item in items {
        write_expression_embedded(
            &mut w,
            item,
            source,
            LocationMapper::identity(tracker),
            interner,
            CommentMode::Record(&recorder),
        );
    }
    let tree = recorder.finish();
    let mut out = WriterComments::default();
    attach_expression_list(
        &tree,
        template_comments,
        source,
        container_start,
        range_end,
        wrapper_end,
        &mut out,
    );
    out
}

/// Build the per-node comment map for a comment-bearing (or preceding-HTML)
/// `<script>` `Program`, for the fused writer to consult at each node's close.
///
/// The writer records the `Program`'s wire tree during a byte-space skeleton
/// emit (the exact structure the final fused emit produces, in byte offsets so
/// the acorn positions line up), the shared attach DFS runs over it with the
/// script's own comments, the preceding HTML comment is prepended to the
/// `Program`'s `leadingComments` (Svelte's `{type: "Line", value}` shape), and
/// the assignments fold into a span-keyed `WriterComments`. The
/// `options: null` non-TS quirk is reproduced at emit time (schema-driven),
/// not here, so it never perturbs the attach walk.
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
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(script.content.span);
    write_program_embedded(
        &mut w,
        &script.content,
        source,
        LocationMapper::identity(tracker),
        interner,
        schema,
        (dummy, dummy),
        CommentMode::Record(&recorder),
    );
    let tree = recorder.finish();
    let root = tree.roots()[0];

    // Attach the script's own comments (byte positions) via acorn's DFS queue.
    let comment_queue: VecDeque<&Comment> = script.content.comments.iter().collect();
    let mut ctx = CommentAttachmentContext::new(comment_queue, source);
    attach_comments_recursively(&tree, root, &mut ctx);

    // The preceding HTML comment becomes the Program's first leadingComment
    // (Svelte reports it as `{type: "Line", value}` with no positions).
    let html_leading = html_leading_comment.map(|c| (root, c.content(source)));

    let mut out = WriterComments::default();
    ctx.into_writer_comments(&tree, html_leading, &mut out);
    out
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
