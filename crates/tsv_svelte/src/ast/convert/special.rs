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
use tsv_lang::{
    Comment, JsonWriter, LocationMapper, LocationTracker, Span, estimated_json_capacity,
};
use tsv_ts::AcornSeed;
use tsv_ts::ast::convert::{
    CommentMode, EmbedWriter, ProgramLoc, ProgramWriter, Schema, SkeletonRecorder, SkeletonTree,
    WriterComments, write_expression_embedded, write_program_embedded,
    write_variable_declaration_embedded,
};

use super::comment_attachment::{
    AttachInputs, CommentAttachmentContext, attach_comments_recursively, attach_expression_list,
    try_attach_comments_to_node, try_attach_comments_to_node_ending_at,
};

/// A throwaway skeleton-emit buffer, sized for the island's own span (the
/// skeleton bytes are discarded — only the recorded tree is used — so the
/// buffer never needs the whole document's capacity).
///
/// ⚠️ **The discarded bytes are not where a comment-bearing island's cost is.**
/// Measured on real Svelte app code (413 components from four repos, ~50–80% of
/// which carry a comment-bearing island), the whole Record pass — this emit plus
/// the attach walk — is **32% of the wire-write phase**, and it decomposes:
/// `loc` emission ≈**24%** of that, the emit's traversal + remaining bytes +
/// recorder ≈**52%**, the attach walk + map build ≈**25%**. Only the *byte*
/// writing inside that middle share is what a null sink or a recording-only
/// walk could remove, and it is **~14%** of the pass. So a null-sink `JsonWriter` mode loses outright: the branch it puts in
/// the hot write path costs **+2.4%** of the write phase on TypeScript, where
/// there is no skeleton at all, against a **−2.1%** net on comment-dense Svelte.
/// A monomorphized recording-only walk avoids that branch but duplicates the
/// `tsv_ts` writer into the parse WASM bundle for the same ~4.5% ceiling.
/// The `loc` share is gone (see [`skeleton_env`]); the rest is reachable only by
/// fusing the attach into the emit — one pass, no recorded tree, no map.
fn skeleton_writer(island_span: Span) -> JsonWriter {
    JsonWriter::with_capacity(estimated_json_capacity(
        (island_span.end - island_span.start) as usize,
    ))
}

/// The `EmbedWriter` a byte-space skeleton pass hands to a `tsv_ts` embedded
/// writer: identity map (comment-attach spans line up in byte space), the
/// `Record` role, and `emit_loc: false`.
///
/// ⚠️ **`emit_loc` is off because the recorder never sees a `loc`.** The tree is
/// built from `(type, span)` open/close events alone, and every `record_open`
/// site in `tsv_ts` fires *before* its header branches on `emit_loc` — so the
/// recorded tree is identical either way, while the line/column resolution the
/// `loc` costs is pure discarded work. It is the single largest share of the
/// skeleton emit: **−7.6% of the whole Svelte wire-write phase** on real app
/// code (−6.9% wall, gated and rotated; exactly 0 on a TypeScript null control),
/// for a byte-identical wire over 13,401 files. It is **−9.5% on the
/// `no-locations` wire**, which is the tell: that product resolves no `loc` at
/// all, so the skeleton was the *only* thing computing one. Don't turn it back on to "match
/// the fused emit" — matching it is precisely what the recorded tree does not
/// need.
///
/// ⭐ And turning it off is what makes that independence **gated**: with the
/// skeleton running `loc`-free, a `record_open` that ever became conditional on
/// `emit_loc` stops firing, the recorded tree loses those nodes, and comments
/// attach to the wrong ones — the fixture corpus goes red immediately.
/// Mutation-tested both ways (`node_header_impl`'s `record_open` wrapped in
/// `if ctx.emit_loc`): red here, **green** with the skeleton emitting `loc`,
/// where the hazard is invisible because the branch is always taken. The parser variant is **not** in that class, and comes from the same
/// component-global fact the fused emit uses: the tree recorded here keys the
/// map that emit consults, so a variant that disagreed would key it off a shape
/// nothing emits.
fn skeleton_env<'a>(attach: AttachInputs<'a>, recorder: &'a SkeletonRecorder) -> EmbedWriter<'a> {
    EmbedWriter {
        source: attach.source,
        loc: LocationMapper::identity(attach.tracker),
        comments: CommentMode::Record(recorder),
        emit_loc: false,
        vanilla_acorn: attach.vanilla_acorn,
        // Same reason as `loc`: the emitted positions are discarded.
        acorn: AcornSeed::NONE,
        acorn_annotation: AcornSeed::NONE,
    }
}

/// Record an internal expression's wire tree via a byte-space skeleton emit
/// (identity map) — the structure the island-scoped attach passes walk.
fn expression_skeleton(
    expr: &tsv_ts::ast::internal::Expression<'_>,
    attach: AttachInputs<'_>,
) -> SkeletonTree {
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(expr.span());
    write_expression_embedded(&mut w, expr, skeleton_env(attach, &recorder));
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
    attach: AttachInputs<'_>,
    container_start: u32,
    range_end: u32,
) -> WriterComments {
    let tree = expression_skeleton(expr, attach);
    let mut out = WriterComments::default();
    try_attach_comments_to_node(
        &tree,
        tree.roots()[0],
        attach,
        container_start,
        range_end,
        &mut out,
    );
    out
}

/// Build the per-node comment map for a comment-bearing **binding pattern** —
/// the `{#each … as ctx}` context, the `{:then value}` / `{:catch error}`
/// bindings, and the `{@const}` id, which reach it through
/// [`build_const_tag_writer_comments`].
///
/// The window is the binding's own, and it runs to the end of the ANNOTATION
/// where one follows: canonical parses a destructure as a synthetic
/// `(pattern = 1)` acorn expression and its trailing `: T` as a second parse,
/// and a comment inside either attaches within the pattern subtree. Deriving
/// the end from the root node instead collapses it to the bare binding — an
/// annotated *identifier*'s span stops at the name — and every annotation
/// comment attaches nowhere. The start is the binding's, never the enclosing
/// head's: canonical filters each parse's comments to `start >= index`, where
/// `index` is where *that* parse began, so a `{#each}` key's own parse (which
/// begins at its `(`) must not see a comment written back in the pattern.
pub(super) fn build_pattern_island_writer_comments(
    pattern: &tsv_ts::ast::internal::Expression<'_>,
    attach: AttachInputs<'_>,
) -> WriterComments {
    let mut out = WriterComments::default();
    fold_pattern_window(pattern, attach, &mut out);
    out
}

/// The region a binding pattern's comments come from — its own start through the
/// end of its annotation, if it has one.
///
/// One definition because two callers must agree on it exactly: the writer's cheap
/// "is there anything here at all" pre-check and this module's attach filter. A
/// pre-check narrower than the filter silently DROPS a comment (the map is never
/// built, and nothing else emits it); a wider one only wastes a build. Stating the
/// region twice is how the two drift.
pub(super) fn pattern_comment_window(pattern: &tsv_ts::ast::internal::Expression<'_>) -> Span {
    Span::new(pattern.span().start, tsv_ts::pattern_binding_end(pattern))
}

/// [`build_pattern_island_writer_comments`]'s body, folding into a caller-owned
/// map so `{@const}` can add its init window to the same one.
fn fold_pattern_window(
    pattern: &tsv_ts::ast::internal::Expression<'_>,
    attach: AttachInputs<'_>,
    out: &mut WriterComments,
) {
    let window = pattern_comment_window(pattern);
    let tree = expression_skeleton(pattern, attach);
    try_attach_comments_to_node_ending_at(
        &tree,
        tree.roots()[0],
        window.end,
        attach,
        window.start,
        window.end,
        out,
    );
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
///
/// The id window is the shared binding-pattern one
/// ([`build_pattern_island_writer_comments`]) — the same window the `{#each}` /
/// `{:then}` / `{:catch}` bindings take, which is what makes the two windows
/// here split at the end of the BINDING rather than of its bare name.
pub(super) fn build_const_tag_writer_comments(
    tag: &internal::ConstTag<'_>,
    attach: AttachInputs<'_>,
) -> WriterComments {
    let binding_end = pattern_comment_window(&tag.id).end;
    let mut out = WriterComments::default();
    fold_pattern_window(&tag.id, attach, &mut out);
    let init_tree = expression_skeleton(&tag.init, attach);
    try_attach_comments_to_node(
        &init_tree,
        init_tree.roots()[0],
        attach,
        binding_end,
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
    attach: AttachInputs<'_>,
    tag_start: u32,
    tag_end: u32,
) -> WriterComments {
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(var_decl.span);
    write_variable_declaration_embedded(&mut w, var_decl, skeleton_env(attach, &recorder));
    let tree = recorder.finish();
    let mut out = WriterComments::default();
    try_attach_comments_to_node(&tree, tree.roots()[0], attach, tag_start, tag_end, &mut out);
    out
}

/// Build the merged per-node comment map for a comment-bearing expression list
/// (`{#snippet}` parameters, multi-identifier `{@debug}`). Canonical Svelte
/// parses the list in one acorn parse, so the whole list is recorded into one
/// skeleton tree (one root per item), attached via one shared queue
/// (`attach_expression_list` — an inter-item comment is claimed exactly once,
/// per acorn's same-line rule), and folded into one map keyed by each item's
/// spans. `wrapper` is the discarded parse wrapper's own span (`{@debug}`'s
/// `SequenceExpression`, which spans first identifier to last) — every comment
/// outside it binds to the wrapper and dies with it; `None` for snippet
/// parameters, whose function wrapper encloses the whole list.
pub(super) fn build_expression_list_writer_comments(
    items: &[tsv_ts::ast::internal::Expression<'_>],
    attach: AttachInputs<'_>,
    container_start: u32,
    range_end: u32,
    wrapper: Option<Span>,
) -> WriterComments {
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(Span::new(container_start, range_end));
    let env = skeleton_env(attach, &recorder);
    for item in items {
        write_expression_embedded(&mut w, item, env);
    }
    let tree = recorder.finish();
    let mut out = WriterComments::default();
    attach_expression_list(
        &tree,
        attach.template_comments,
        attach.source,
        container_start,
        range_end,
        wrapper,
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
    html_leading_comment: Option<&internal::HtmlComment>,
    schema: Schema,
) -> WriterComments {
    // Byte-space skeleton (identity map), `loc`-free — the final fused emit
    // supplies the real tag-line `loc`.
    let recorder = SkeletonRecorder::new();
    let mut w = skeleton_writer(script.content.span);
    write_program_embedded(
        &mut w,
        &script.content,
        ProgramWriter {
            source,
            loc: LocationMapper::identity(tracker),
            schema,
            // Skeleton pass: the recorder reads spans, never `loc` — and `Omit`
            // is what carries that down to every node in the `Program` (it sets
            // `Ctx::emit_loc`), the same reason the seed below is the identity.
            program_loc: ProgramLoc::Omit,
            comments: CommentMode::Record(&recorder),
            acorn: AcornSeed::NONE,
        },
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

/// A script tag's `lang` attribute value, if it carries one (`<script lang="ts">` → `Some("ts")`,
/// plain `<script>` → `None`).
///
/// The value decides the wire schema: acorn-typescript context (`Some("ts")`) emits
/// `importKind`/`exportKind = "value"` and omits `attributes`; the Svelte context (anything else)
/// omits `importKind`/`exportKind` and always includes `attributes`. But the *choice* is
/// component-global, not per-script — this only feeds [`component_is_typescript`].
///
/// ⚠️ Reads the attribute's **raw** bytes, and is the one `lang` reader that does. The oracle
/// here is Svelte's own `this.ts`, which never builds an AST for the question: it runs a regex
/// over the template text (`1-parse/index.js`, `lang=(["'])?([^"' >]+)`) and compares the
/// captured bytes against `ts`. So `<script lang="&#116;s">` is NOT TypeScript to the wire,
/// where the *formatting* question — `internal::lang_attribute`, the reader behind
/// [`internal::EmbeddedLang::is_frozen`], which asks what language the body is in — decodes
/// and calls the same spelling `ts`. Two questions, two readers, on
/// purpose; `tests/fixtures/svelte/attributes/lang_entity` holds both answers.
fn script_lang<'s>(script: &internal::Script<'_>, source: &'s str) -> Option<&'s str> {
    for attr_node in script.attributes {
        let internal::AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let name = attr.name(source);
        if name == "lang"
            && let Some(values) = &attr.value
            && let Some(internal::AttributeValue::Text(text)) = values.first()
        {
            return Some(text.raw(source));
        }
    }
    None
}

/// Whether the component parses as TypeScript, matching Svelte's parser
/// (`1-parse/index.js`): TS is determined **once for the whole component** from the first
/// `<script>` tag (in source order) that carries a `lang` attribute — `lang="ts"` ⇒ every script
/// (module *and* instance) emits the acorn-typescript wire shape. So a plain `<script>` alongside
/// a `lang="ts"` sibling still emits `importKind`/`exportKind = "value"` and omits `attributes`.
/// A `<script>` with no `lang` attribute doesn't decide; nor does `<style lang=…>`.
pub(super) fn component_is_typescript(root: &internal::Root<'_>, source: &str) -> bool {
    // The two top-level scripts in source order — the first one carrying a `lang` decides.
    let mut scripts = [root.module, root.instance];
    scripts.sort_by_key(|s| s.map_or(u32::MAX, |script| script.span.start));
    scripts
        .into_iter()
        .flatten()
        .find_map(|script| script_lang(script, source))
        .is_some_and(|lang| lang == "ts")
}

/// Find a named attribute's value in `<svelte:options>` attributes.
///
/// `pub(super)` so the wire-JSON writer reproduces `<svelte:options>` extraction
/// (scalar props + `customElement`) without materializing a typed options struct.
pub(super) fn find_option_values<'arena>(
    attrs: &[internal::AttributeNode<'arena>],
    name: &str,
    source: &str,
) -> Option<&'arena [internal::AttributeValue<'arena>]> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && attr.name(source) == name
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
    source: &str,
) -> Option<bool> {
    attrs.iter().find_map(|attr| {
        if let internal::AttributeNode::Attribute(attr) = attr
            && attr.name(source) == name
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
