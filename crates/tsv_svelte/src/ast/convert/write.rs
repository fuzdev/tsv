//! Writer-mode conversion: emit compact wire JSON directly from the internal
//! Svelte AST.
//!
//! The Svelte sibling of `tsv_ts`'s and `tsv_css`'s `ast/convert/write*` — the
//! hot path behind `convert_ast_json_bytes` (FFI/CLI compact output, WASM
//! `JSON.parse`). It walks the *internal* Svelte AST once and writes the final
//! JSON bytes as it goes, never materializing the typed public `Root`.
//!
//! **Fused emission.** The Svelte spine (elements, blocks, tags, directives,
//! attributes, `name_loc`, positions) is emitted *fused* — final char-space
//! `start`/`end`/`loc`/`character` written directly via a `LocationMapper`
//! (tracker + `ByteToCharMap`), exactly as the `tsv_ts`/`tsv_css` writers do.
//! Almost everything else fuses too:
//!
//! - **Root comments, `<svelte:options>`** (scalar props + `customElement`),
//!   **`<style>`** (CSS `children` via `tsv_css`'s `write_css_node`, plus the
//!   `StyleSheet`/`StyleContent` envelope and preceding comment),
//!   **`<script>`/`<style>`/`<svelte:options>` tag attributes**, the
//!   **`<svelte:element>` string `tag`**, and **bind/class shorthand** identifiers
//!   all emit fused, directly from the internal AST.
//! - **Block patterns** (`{#each … as ctx}`, `{:then value}`/`{:catch error}`,
//!   `{@const}` ids) fuse via `tsv_ts`'s `write_pattern_embedded` (the
//!   `+1`-column / `character` / stripped-type-annotation-`loc` quirks).
//! - **Shorthand attributes / snippet names** fuse via
//!   `write_identifier_expression_with_character`.
//! - **Generic template expressions** (`{expr}`, block tests, directive
//!   expressions, …) emit fused via `tsv_ts`'s `write_expression_embedded`. When
//!   a template comment lands inside the expression's window (the
//!   `any_comment_in` pre-check), the comment
//!   assignments are precomputed into a per-node `WriterComments` map and the
//!   expression fuses with `CommentMode::Emit`, emitting each
//!   node's `leadingComments`/`trailingComments` at its close.
//! - **Snippet names / parameters** fuse the same way (with `character`
//!   injection / the shared one-queue list attach, matching canonical's single
//!   acorn parse of the list — multi-identifier `{@debug}` rides the same path).
//! - **`{@const}` / `{const}` / `{let}` declarations** fuse their
//!   `VariableDeclaration` structure (the `{@const}` declaration `end` is always
//!   `tag.span.end - 1`, Svelte's `parser.index - 1`); when the document has a
//!   template comment the init/declaration subtree carries a `WriterComments` map.
//! - **`<script>` content** always fuses via `write_program_embedded`: an
//!   eligible script (`lang="ts"` ∧ no script comments ∧ no preceding HTML
//!   comment) with no map; an ineligible one (a plain non-`lang="ts"` script, one
//!   with comments, or one with a preceding HTML comment) with the schema-driven
//!   `options: null` quirk and, when it has comments, a `WriterComments` map (the
//!   acorn attach precomputed over a byte-space skeleton, the preceding HTML
//!   comment prepended to the `Program`'s `leadingComments`).
//!
//! **The comment map** (`ast/convert/special.rs`'s `build_*_writer_comments`,
//! `tsv_ts`'s `WriterComments`): the comment-attach paths never build a
//! `serde_json::Value` at all. Each island records its wire tree during its own
//! byte-space skeleton emit (`SkeletonRecorder`), runs the shared acorn attach
//! DFS over the recorded tree, and folds the assignments into a span-keyed map
//! the fused writer consults at each node's close — so attached comments
//! serialize *last* within a node exactly as acorn's appended keys place them,
//! regardless of child-visit order.
//!
//! **Byte-identity**: the wire JSON is a faithful emission of the Svelte
//! parser's JSON (its acorn `<script>` shape plus `parseCss` `<style>` shape) —
//! the shape the canonical Svelte parser's `expected.json` records.

use crate::ast::internal;
use tsv_css::ast::convert::write_css_node;
use tsv_lang::{
    Comment, JsonWriter, LocationMapper, LocationTracker, Position, Span, estimated_json_capacity,
    write_array, write_or_null,
};
use tsv_ts::ast::convert::{
    CommentMode, Schema, translate_column, write_expression_embedded,
    write_identifier_expression_with_character, write_pattern_embedded, write_program_embedded,
    write_variable_declaration_embedded,
};

use super::comment_attachment::{get_comment_value, is_template_comment};
use super::special::{
    bool_option, build_const_tag_writer_comments, build_declaration_tag_writer_comments,
    build_expression_list_writer_comments, build_expression_writer_comments,
    build_script_writer_comments, component_is_typescript, find_option_values, text_value,
};

/// Convert an internal Svelte `Root` straight to its compact wire-JSON bytes.
///
/// One AST walk, no intermediate `serde_json::Value` for the spine — the fused
/// char-space wire the FFI/CLI/WASM parse bindings ship.
pub(crate) fn write_root_bytes(root: &internal::Root<'_>, source: &str) -> Vec<u8> {
    write_root_bytes_variant(root, source, true)
}

/// The `no-locations` variant of `write_root_bytes`: drops every line/column
/// object from the Svelte wire — the acorn `loc` on `<script>`/`{expr}` nodes,
/// the `name_loc` on elements/attributes/directives, and the root-comment
/// `loc`. Only `start`/`end` offsets remain; nothing else changes. Because that
/// removes *all* line/column emission, the LF line table is never queried.
pub(crate) fn write_root_bytes_no_locations(root: &internal::Root<'_>, source: &str) -> Vec<u8> {
    write_root_bytes_variant(root, source, false)
}

fn write_root_bytes_variant(root: &internal::Root<'_>, source: &str, emit_loc: bool) -> Vec<u8> {
    // LF-only tracker (Svelte's `locate-character` convention) + byte→UTF-16 map
    // in one source scan; the identity map short-circuits on ASCII. The
    // `no-locations` path emits no line/column at all (loc, name_loc, and
    // root-comment loc are all gated off), so it skips the LF line scan entirely
    // (`new_map_only` builds just the byte→char map) — once per file, no per-node cost.
    let (tracker, map) = if emit_loc {
        LocationTracker::new_with_map(source)
    } else {
        LocationTracker::new_map_only(source)
    };

    // Template comments (outside `<script>` content spans) are the only comments
    // the template attach passes move; everything else stays where it is.
    let script_spans = crate::script_content_spans(root);
    let template_comments: Vec<&Comment> = root
        .comments
        .iter()
        .filter(|c| is_template_comment(c, &script_spans))
        .collect();

    let ctx = Ctx {
        source,
        loc: LocationMapper {
            tracker: &tracker,
            map: &map,
        },
        comments: &template_comments,
        // Component-global: `lang="ts"` on any script makes *every* script emit the
        // acorn-typescript wire shape (Svelte's single `this.ts` flag).
        component_is_ts: component_is_typescript(root, source),
        emit_loc,
    };

    let mut w = JsonWriter::with_capacity(estimated_json_capacity(source.len()));
    write_root(&mut w, root, &ctx);
    w.into_bytes()
}

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    source: &'a str,
    /// Real-map mapper for fused char-space spine emission and the embedded TS
    /// expression writer. Its `tracker` also serves byte-space uses alone (the
    /// comment-island skeleton builders, paired with `LocationMapper::identity`,
    /// and the `<script>` tag-line lookups); its `map`, the `<style>` CSS
    /// children.
    loc: LocationMapper<'a>,
    /// Template comments, sorted by position (empty on the common no-comment
    /// template — the whole spine then fuses).
    comments: &'a [&'a Comment],
    /// Whether the component parses as TypeScript (component-global, from the first
    /// lang-bearing `<script>` — see `component_is_typescript`). Selects every `<script>`'s
    /// wire schema (`Schema::Acorn` vs `Schema::SvelteScript`), so a plain `<script>` beside a
    /// `lang="ts"` sibling still emits the acorn-typescript import/export shape.
    component_is_ts: bool,
    /// Whether to emit line/column data (the acorn `loc`, `name_loc`, and
    /// root-comment `loc`). `true` for the default drop-in wire; `false` for the
    /// opt-in `no-locations` variant (offsets only). Threaded into the embedded
    /// `tsv_ts` writers so `<script>`/`{expr}` islands drop `loc` in lockstep.
    emit_loc: bool,
}

impl<'a> Ctx<'a> {
    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    fn pos(&self, byte: u32) -> u32 {
        self.loc.pos(byte)
    }

    /// A copy of this context with no template comments — for subtrees the
    /// template attach passes never visit (`<script>`/`<style>`/`<svelte:options>`
    /// tag attributes), so their embedded expressions always fuse comment-free.
    #[inline]
    fn without_comments(&self) -> Ctx<'a> {
        Ctx {
            comments: &[],
            ..*self
        }
    }

    /// Superset pre-check: does any template comment *start* in `[start, end)`?
    /// A miss means the expression stays fused (no skeleton, no attach).
    ///
    /// `self.comments` is sorted ascending by `span.start`, so the first comment
    /// at/after `start` (binary search) starting before `end` settles the query.
    #[inline]
    fn any_comment_in(&self, start: u32, end: u32) -> bool {
        let idx = self.comments.partition_point(|c| c.span.start < start);
        self.comments.get(idx).is_some_and(|c| c.span.start < end)
    }
}

/// Start position of a fragment's first node — the range-end tightener the
/// attach passes use so a sibling expression context (`{:else if}`) doesn't
/// bleed into a block's own expression window.
#[inline]
fn fragment_first_start(fragment: &internal::Fragment<'_>) -> Option<u32> {
    fragment.nodes.first().map(|n| n.span().start)
}

/// Whether the byte immediately before `pos` is a quote — the discriminator
/// between a bare `{expr}` (plain object) and a quoted `"{expr}"` (array).
#[inline]
fn preceded_by_quote(source: &str, pos: u32) -> bool {
    matches!(
        (pos as usize)
            .checked_sub(1)
            .and_then(|i| source.as_bytes().get(i)),
        Some(b'"' | b'\'')
    )
}

/// Emit the `Root` node. Field order:
/// `css, js, start, end, type, fragment, options, comments, [instance], [module]`.
fn write_root(w: &mut JsonWriter, root: &internal::Root<'_>, ctx: &Ctx<'_>) {
    let source = ctx.source;

    // Helper: HTML comment immediately preceding a tag (whitespace-only between).
    let find_preceding_comment = |tag_start: u32| -> Option<&internal::HtmlComment> {
        root.fragment.nodes.iter().find_map(|node| {
            if let internal::FragmentNode::Comment(comment) = node
                && comment.span.end <= tag_start
            {
                let between = &source[comment.span.end as usize..tag_start as usize];
                if between.trim().is_empty() {
                    return Some(comment);
                }
            }
            None
        })
    };

    w.raw("{\"css\":");
    write_or_null(w, root.css.as_ref(), |w, style| {
        let style_comment = find_preceding_comment(style.span.start);
        write_style_sheet(w, style, style_comment, ctx);
    });
    w.raw(",\"js\":[],\"start\":");
    w.u32(ctx.pos(0));
    w.raw(",\"end\":");
    w.u32(ctx.pos(source.len() as u32));
    w.raw(",\"type\":\"Root\",\"fragment\":");
    write_fragment(w, &root.fragment, ctx);
    w.raw(",\"options\":");
    write_or_null(w, root.options.as_ref(), |w, opts| {
        write_svelte_options(w, opts, ctx);
    });
    w.raw(",\"comments\":");
    write_array(w, root.comments.iter(), |w, c| {
        write_root_comment(w, c, ctx);
    });
    // Svelte assigns `module` before `instance` on the root.
    if let Some(script) = root.module {
        let comment = find_preceding_comment(script.span.start);
        w.raw(",\"module\":");
        write_script(w, script, comment, ctx);
    }
    if let Some(script) = root.instance {
        let comment = find_preceding_comment(script.span.start);
        w.raw(",\"instance\":");
        write_script(w, script, comment, ctx);
    }
    w.raw("}");
}

/// A root-level comment, emitted fused in final char space. Svelte's two
/// comment collectors build different literals: a `<script>` comment (acorn's
/// `onComment` wrapper) is `{type, value, start, end, loc}`, a
/// template-expression comment `{type, start, end, value, loc}` with
/// `character` in its `loc` — the `emit_character_field` axis keys both
/// differences.
fn write_root_comment(w: &mut JsonWriter, comment: &Comment, ctx: &Ctx<'_>) {
    let start_char = ctx.loc.pos(comment.span.start);
    let end_char = ctx.loc.pos(comment.span.end);
    w.raw("{\"type\":\"");
    w.raw(if comment.is_block { "Block" } else { "Line" });
    if comment.emit_character_field {
        w.raw("\",\"start\":");
        w.u32(start_char);
        w.raw(",\"end\":");
        w.u32(end_char);
        w.raw(",\"value\":");
        w.string(&get_comment_value(comment, ctx.source));
    } else {
        w.raw("\",\"value\":");
        w.string(&get_comment_value(comment, ctx.source));
        w.raw(",\"start\":");
        w.u32(start_char);
        w.raw(",\"end\":");
        w.u32(end_char);
    }
    if !ctx.emit_loc {
        // `no-locations`: no `loc` on the comment; close the object directly.
        w.raw("}");
        return;
    }
    let start_pos = ctx.loc.pos_and_position(comment.span.start).1;
    let end_pos = ctx.loc.pos_and_position(comment.span.end).1;
    // The block-pattern synthetic-`(` column shift (`bump_pattern_columns`);
    // a multiline block comment's `end` sits on an unshifted later line.
    let bump = usize::from(comment.bump_pattern_columns);
    let bump_end = usize::from(comment.bump_pattern_columns && !comment.multiline);
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start_pos.line);
    w.raw(",\"column\":");
    w.usize(start_pos.column + bump);
    if comment.emit_character_field {
        w.raw(",\"character\":");
        w.u32(start_char);
    }
    w.raw("},\"end\":{\"line\":");
    w.usize(end_pos.line);
    w.raw(",\"column\":");
    w.usize(end_pos.column + bump_end);
    if comment.emit_character_field {
        w.raw(",\"character\":");
        w.u32(end_char);
    }
    // Close end-position, `loc`, and the comment object.
    w.raw("}}}");
}

/// Emits a `Fragment` node.
fn write_fragment(w: &mut JsonWriter, fragment: &internal::Fragment<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Fragment\",\"nodes\":");
    write_array(w, fragment.nodes, |w, n| write_fragment_node(w, n, ctx));
    w.raw("}");
}

/// Emit a fragment node, dispatching on its variant.
fn write_fragment_node(w: &mut JsonWriter, node: &internal::FragmentNode<'_>, ctx: &Ctx<'_>) {
    match node {
        internal::FragmentNode::Element(elem) => write_element(w, elem, ctx),
        internal::FragmentNode::SpecialElement(elem) => write_special_element(w, elem, ctx),
        internal::FragmentNode::ExpressionTag(tag) => write_expression_tag(w, tag, ctx),
        internal::FragmentNode::Text(text) => write_text(w, text, ctx),
        internal::FragmentNode::Comment(comment) => write_html_comment(w, comment, ctx),
        internal::FragmentNode::IfBlock(block) => write_if_block(w, block, ctx),
        internal::FragmentNode::EachBlock(block) => write_each_block(w, block, ctx),
        internal::FragmentNode::AwaitBlock(block) => write_await_block(w, block, ctx),
        internal::FragmentNode::KeyBlock(block) => write_key_block(w, block, ctx),
        internal::FragmentNode::SnippetBlock(block) => write_snippet_block(w, block, ctx),
        internal::FragmentNode::HtmlTag(tag) => write_html_tag(w, tag, ctx),
        internal::FragmentNode::ConstTag(tag) => write_const_tag(w, tag, ctx),
        internal::FragmentNode::DeclarationTag(tag) => write_declaration_tag(w, tag, ctx),
        internal::FragmentNode::DebugTag(tag) => write_debug_tag(w, tag, ctx),
        internal::FragmentNode::RenderTag(tag) => write_render_tag(w, tag, ctx),
    }
}

/// A generic template expression island: fused when comment-free, else the
/// comment-bearing path — precompute a `WriterComments` map off a byte-space
/// skeleton (`build_expression_writer_comments`), then fuse-emit with it.
fn write_generic_island(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if ctx.any_comment_in(container_start, range_end) {
        let wc = build_expression_writer_comments(
            expr,
            ctx.comments,
            ctx.source,
            ctx.loc.tracker,
            container_start,
            range_end,
        );
        write_expression_embedded(
            w,
            expr,
            ctx.source,
            ctx.loc,
            CommentMode::Emit(&wc),
            ctx.emit_loc,
        );
        wc.debug_assert_consumed();
    } else {
        write_expression_embedded(w, expr, ctx.source, ctx.loc, CommentMode::Off, ctx.emit_loc);
    }
}

/// The shared `NameLocation` shape: `start`/`end` each `{line, column, character}`
/// (all three, always). Char-space via one fused translation per endpoint.
fn write_name_loc(w: &mut JsonWriter, span: Span, ctx: &Ctx<'_>) {
    let (start_char, start_pos) = ctx.loc.pos_and_position(span.start);
    let (end_char, end_pos) = ctx.loc.pos_and_position(span.end);
    w.raw("{\"start\":{\"line\":");
    w.usize(start_pos.line);
    w.raw(",\"column\":");
    w.usize(start_pos.column);
    w.raw(",\"character\":");
    w.u32(start_char);
    w.raw("},\"end\":{\"line\":");
    w.usize(end_pos.line);
    w.raw(",\"column\":");
    w.usize(end_pos.column);
    w.raw(",\"character\":");
    w.u32(end_char);
    w.raw("}}");
}

/// Emits a `RegularElement` (HTML) or `Component` node.
fn write_element(w: &mut JsonWriter, elem: &internal::Element<'_>, ctx: &Ctx<'_>) {
    let node_type = match elem.kind {
        internal::ElementKind::Component => "Component",
        internal::ElementKind::Html => "RegularElement",
    };
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\",\"start\":");
    w.u32(ctx.pos(elem.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(elem.span.end));
    w.raw(",\"name\":");
    w.string(elem.name(ctx.source));
    if ctx.emit_loc {
        w.raw(",\"name_loc\":");
        write_name_loc(w, elem.name_span, ctx);
    }
    w.raw(",\"attributes\":");
    write_array(w, elem.attributes, |w, a| write_attribute_node(w, a, ctx));
    w.raw(",\"fragment\":");
    // A `<textarea>`'s content is read with the attribute-value sequence
    // machinery in the canonical parser, whose `Text` literal leads with the
    // positions (`{start, end, type, raw, data}`).
    if elem.name(ctx.source) == "textarea" {
        w.raw("{\"type\":\"Fragment\",\"nodes\":");
        write_array(w, elem.fragment.nodes, |w, n| match n {
            internal::FragmentNode::Text(text) => write_text_sequence(w, text, ctx),
            _ => write_fragment_node(w, n, ctx),
        });
        w.raw("}");
    } else {
        write_fragment(w, &elem.fragment, ctx);
    }
    w.raw("}");
}

/// Emits a special-element node (`svelte:element`, `svelte:component`, …).
/// `tag`/`expression` are skip-if-none.
fn write_special_element(w: &mut JsonWriter, elem: &internal::SpecialElement<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"");
    w.raw(elem.kind.node_type());
    w.raw("\",\"start\":");
    w.u32(ctx.pos(elem.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(elem.span.end));
    w.raw(",\"name\":");
    // Escape-free `&'static str` (`svelte:head`, `slot`, `title`, …) → skip the
    // serde string-escape scan.
    w.token(elem.kind.tag_name());
    if ctx.emit_loc {
        w.raw(",\"name_loc\":");
        write_name_loc(w, elem.name_span, ctx);
    }
    w.raw(",\"attributes\":");
    write_array(w, elem.attributes, |w, a| write_attribute_node(w, a, ctx));
    w.raw(",\"fragment\":");
    write_fragment(w, &elem.fragment, ctx);
    // `<svelte:element this={…}>` tag. A plain-string `this="x"` is a
    // Svelte-style `Literal` (no `loc`, single-quoted `raw`) that carries no
    // expression parse, so no template comment can attach — emit it fused.
    // Every other `this={…}` is a generic island keyed on the element's span.
    if let Some(tag) = elem.kind.tag() {
        w.raw(",\"tag\":");
        write_special_tag(w, tag, elem.span, ctx);
    }
    // `<svelte:component this={…}>` expression — a generic island.
    if let Some(expr) = elem.kind.expression() {
        w.raw(",\"expression\":");
        write_generic_island(w, expr, elem.span.start, elem.span.end, ctx);
    }
    w.raw("}");
}

/// A `<svelte:element this={…}>` tag. The plain-string form (`this="x"`) is a Svelte-style
/// `Literal` (`{type, value, raw, start, end}` — no `loc`, single-quoted `raw`) fused
/// directly; the braced form is a generic island keyed on the element's span (the window
/// Svelte's own comment attach uses for `SvelteElement`).
///
/// The form is read off [`internal::SpecialThis`], never sniffed back out of the source: it
/// is the parser's own answer, and a second way to ask the same question is a second way to
/// get it wrong.
fn write_special_tag(
    w: &mut JsonWriter,
    this: &internal::SpecialThis<'_>,
    elem_span: Span,
    ctx: &Ctx<'_>,
) {
    match this {
        internal::SpecialThis::Plain { content, span } => {
            w.raw("{\"type\":\"Literal\",\"value\":");
            w.string(content);
            w.raw(",\"raw\":");
            // Svelte reports the raw as a single-quoted string regardless of source.
            w.string(&format!("'{content}'"));
            w.raw(",\"start\":");
            w.u32(ctx.pos(span.start));
            w.raw(",\"end\":");
            w.u32(ctx.pos(span.end));
            w.raw("}");
        }
        internal::SpecialThis::Braced(tag) => {
            write_generic_island(w, &tag.expression, elem_span.start, elem_span.end, ctx);
        }
    }
}

/// Emits an `ExpressionTag` node (fragment `{expr}`).
fn write_expression_tag(w: &mut JsonWriter, tag: &internal::ExpressionTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ExpressionTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"expression\":");
    write_generic_island(w, &tag.expression, tag.span.start, tag.span.end, ctx);
    w.raw("}");
}

/// A shorthand attribute `{name}`'s `ExpressionTag`: Svelte injects `character`
/// into the identifier's `loc`. The shorthand form requires `tag.span == id.span`
/// (the identifier *is* the tag), so no comment can lie between the braces and the
/// name — attach is always a no-op here — and the whole tag fuses.
fn write_shorthand_expression_tag(
    w: &mut JsonWriter,
    tag: &internal::ExpressionTag<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"ExpressionTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"expression\":");
    write_identifier_expression_with_character(
        w,
        &tag.expression,
        ctx.source,
        ctx.loc,
        CommentMode::Off,
        ctx.emit_loc,
    );
    w.raw("}");
}

/// Emits a `Text` node (fragment context: `type, start, end, raw, data`).
/// Raw-content element text (`TextDecoding::Raw` — a nested `<script>`/
/// `<style>`) comes from a different canonical construction site whose
/// literal leads with the positions and puts `data` first:
/// `{start, end, type, data, raw}`.
fn write_text(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    if matches!(text.decoding, internal::TextDecoding::Raw) {
        w.raw("{\"start\":");
        w.u32(ctx.pos(text.span.start));
        w.raw(",\"end\":");
        w.u32(ctx.pos(text.span.end));
        w.raw(",\"type\":\"Text\",\"data\":");
        let data = text.data(ctx.source);
        w.string(&data);
        w.raw(",\"raw\":");
        w.string(text.raw(ctx.source));
        w.raw("}");
        return;
    }
    w.raw("{\"type\":\"Text\",\"start\":");
    w.u32(ctx.pos(text.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(text.span.end));
    w.raw(",\"raw\":");
    w.string(text.raw(ctx.source));
    w.raw(",\"data\":");
    let data = text.data(ctx.source);
    w.string(&data);
    w.raw("}");
}

/// A sequence-context `Text` (a `<textarea>`'s content): the canonical
/// attribute-value sequence literal, `{start, end, type, raw, data}`.
fn write_text_sequence(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    w.raw("{\"start\":");
    w.u32(ctx.pos(text.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(text.span.end));
    w.raw(",\"type\":\"Text\",\"raw\":");
    w.string(text.raw(ctx.source));
    w.raw(",\"data\":");
    let data = text.data(ctx.source);
    w.string(&data);
    w.raw("}");
}

/// Emits a `Comment` node (HTML `<!-- … -->`).
fn write_html_comment(w: &mut JsonWriter, comment: &internal::HtmlComment, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Comment\",\"start\":");
    w.u32(ctx.pos(comment.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(comment.span.end));
    w.raw(",\"data\":");
    w.string(comment.content(ctx.source));
    w.raw("}");
}

//
// Blocks
//

/// Emits an `IfBlock` node. Svelte constructs a root `{#if}` as
/// `{type, elseif, start, end, …}` but an `{:else if}` block as
/// `{start, end, type, elseif, …}` — two construction sites with different
/// literal orders, keyed exactly by `elseif`.
fn write_if_block(w: &mut JsonWriter, block: &internal::IfBlock<'_>, ctx: &Ctx<'_>) {
    if block.elseif {
        w.raw("{\"start\":");
        w.u32(ctx.pos(block.span.start));
        w.raw(",\"end\":");
        w.u32(ctx.pos(block.span.end));
        w.raw(",\"type\":\"IfBlock\",\"elseif\":true");
    } else {
        w.raw("{\"type\":\"IfBlock\",\"elseif\":false,\"start\":");
        w.u32(ctx.pos(block.span.start));
        w.raw(",\"end\":");
        w.u32(ctx.pos(block.span.end));
    }
    let range_end = fragment_first_start(&block.consequent).unwrap_or(block.span.end);
    w.raw(",\"test\":");
    write_generic_island(w, &block.test, block.span.start, range_end, ctx);
    w.raw(",\"consequent\":");
    write_fragment(w, &block.consequent, ctx);
    w.raw(",\"alternate\":");
    write_optional_fragment(w, block.alternate.as_ref(), ctx);
    w.raw("}");
}

/// Emits an `EachBlock` node. `context` is a pattern island (patterns never
/// collect comments); `index`/`key`/`fallback` are skip-if-none.
fn write_each_block(w: &mut JsonWriter, block: &internal::EachBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"EachBlock\",\"start\":");
    w.u32(ctx.pos(block.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(block.span.end));
    let range_end = fragment_first_start(&block.body).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_generic_island(w, &block.expression, block.span.start, range_end, ctx);
    w.raw(",\"body\":");
    write_fragment(w, &block.body, ctx);
    w.raw(",\"context\":");
    write_or_null(w, block.context.as_ref(), |w, c| {
        write_pattern_island(w, c, ctx);
    });
    if let Some(index) = block.index {
        w.raw(",\"index\":");
        w.string(index);
    }
    if let Some(key) = &block.key {
        w.raw(",\"key\":");
        write_generic_island(w, key, block.span.start, range_end, ctx);
    }
    if let Some(fallback) = &block.fallback {
        w.raw(",\"fallback\":");
        write_fragment(w, fallback, ctx);
    }
    w.raw("}");
}

/// Emits an `AwaitBlock` node. `value`/`error` are pattern islands; every
/// `Option` → `null` when absent (no skip).
fn write_await_block(w: &mut JsonWriter, block: &internal::AwaitBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"AwaitBlock\",\"start\":");
    w.u32(ctx.pos(block.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(block.span.end));
    let range_end = [
        block.pending.as_ref(),
        block.then.as_ref(),
        block.catch.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(fragment_first_start)
    .min()
    .unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_generic_island(w, &block.expression, block.span.start, range_end, ctx);
    w.raw(",\"value\":");
    write_or_null(w, block.value.as_ref(), |w, v| {
        write_pattern_island(w, v, ctx);
    });
    w.raw(",\"error\":");
    write_or_null(w, block.error.as_ref(), |w, e| {
        write_pattern_island(w, e, ctx);
    });
    w.raw(",\"pending\":");
    // Svelte's block form always has a pending Fragment (empty or not); the inline
    // `then`/`catch` shorthand has `null`. `pending` holds only non-empty content,
    // so an empty block-form pending (`{#await x}{/await}`, `{#await x}{:then v}…`)
    // is `None` here yet must still emit an (empty) Fragment — hence the flag.
    match block.pending.as_ref() {
        Some(fragment) => write_fragment(w, fragment, ctx),
        None if block.pending_block => write_fragment(w, &internal::Fragment { nodes: &[] }, ctx),
        None => w.raw("null"),
    }
    w.raw(",\"then\":");
    write_optional_fragment(w, block.then.as_ref(), ctx);
    w.raw(",\"catch\":");
    write_optional_fragment(w, block.catch.as_ref(), ctx);
    w.raw("}");
}

/// Emits a `KeyBlock` node.
fn write_key_block(w: &mut JsonWriter, block: &internal::KeyBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"KeyBlock\",\"start\":");
    w.u32(ctx.pos(block.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(block.span.end));
    let range_end = fragment_first_start(&block.fragment).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_generic_island(w, &block.expression, block.span.start, range_end, ctx);
    w.raw(",\"fragment\":");
    write_fragment(w, &block.fragment, ctx);
    w.raw("}");
}

/// Emits a `SnippetBlock` node. The snippet name carries `character` (like a
/// shorthand attribute); `typeParams` is skip-if-none, right after
/// `expression` (Svelte assigns it before reading the parameters).
fn write_snippet_block(w: &mut JsonWriter, block: &internal::SnippetBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"SnippetBlock\",\"start\":");
    w.u32(ctx.pos(block.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(block.span.end));
    let range_end = fragment_first_start(&block.body).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_snippet_name(w, &block.expression, block.span.start, range_end, ctx);
    if let Some(type_params) = block.type_params_raw {
        w.raw(",\"typeParams\":");
        w.string(type_params);
    }
    w.raw(",\"parameters\":");
    write_snippet_parameters(w, block.parameters, block.span.start, range_end, ctx);
    w.raw(",\"body\":");
    write_fragment(w, &block.body, ctx);
    w.raw("}");
}

/// The snippet name identifier — Svelte injects `character` into its `loc`. A
/// leading comment (`{#snippet /* c */ name(…)}`) can attach, so the
/// comment-bearing case precomputes a `WriterComments` map (skeleton + attach)
/// and fuse-emits with it; the comment-free common case fuses directly.
fn write_snippet_name(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if ctx.any_comment_in(container_start, range_end) {
        // The injected `character` lives in the identifier's `loc` and doesn't
        // affect the attach walk (span/type keyed), so the skeleton builds
        // without it and the fused emit adds it.
        let wc = build_expression_writer_comments(
            expr,
            ctx.comments,
            ctx.source,
            ctx.loc.tracker,
            container_start,
            range_end,
        );
        write_identifier_expression_with_character(
            w,
            expr,
            ctx.source,
            ctx.loc,
            CommentMode::Emit(&wc),
            ctx.emit_loc,
        );
        wc.debug_assert_consumed();
    } else {
        write_identifier_expression_with_character(
            w,
            expr,
            ctx.source,
            ctx.loc,
            CommentMode::Off,
            ctx.emit_loc,
        );
    }
}

/// Snippet parameters. Comment-free (the common case): each fuses. Otherwise a
/// `WriterComments` map is precomputed off a byte-space skeleton via the shared
/// list attach (`attach_expression_list` — one queue, each inter-parameter
/// comment claimed once per acorn's same-line rule), then each parameter
/// fuse-emits with it. No wrapper-end suppression: canonical parses the list in
/// a function context whose wrapper ends past every param.
fn write_snippet_parameters(
    w: &mut JsonWriter,
    parameters: &[tsv_ts::ast::internal::Expression<'_>],
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if !parameters.is_empty() && ctx.any_comment_in(container_start, range_end) {
        let wc = build_expression_list_writer_comments(
            parameters,
            ctx.comments,
            ctx.source,
            ctx.loc.tracker,
            container_start,
            range_end,
            None,
        );
        write_array(w, parameters, |w, p| {
            write_expression_embedded(
                w,
                p,
                ctx.source,
                ctx.loc,
                CommentMode::Emit(&wc),
                ctx.emit_loc,
            );
        });
        wc.debug_assert_consumed();
    } else {
        write_array(w, parameters, |w, p| {
            write_expression_embedded(w, p, ctx.source, ctx.loc, CommentMode::Off, ctx.emit_loc);
        });
    }
}

//
// Tags
//

/// Emits an `HtmlTag` node (`{@html expr}`).
fn write_html_tag(w: &mut JsonWriter, tag: &internal::HtmlTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"HtmlTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"expression\":");
    write_generic_island(w, &tag.expression, tag.span.start, tag.span.end, ctx);
    w.raw("}");
}

/// Emits a `RenderTag` node (`{@render expr}`).
fn write_render_tag(w: &mut JsonWriter, tag: &internal::RenderTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"RenderTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"expression\":");
    write_generic_island(w, &tag.expression, tag.span.start, tag.span.end, ctx);
    w.raw("}");
}

/// Emits a `DebugTag` node (`{@debug a, b}`).
///
/// A multi-identifier tag is ONE canonical acorn parse (a `SequenceExpression`
/// wrapper, discarded after identifier extraction), so its comment attach runs
/// once across the list with the wrapper-end trailing suppression. A single
/// identifier is itself the parse root and takes the generic-island path
/// (root-fallback trailing).
fn write_debug_tag(w: &mut JsonWriter, tag: &internal::DebugTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"DebugTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"identifiers\":");
    if tag.identifiers.len() > 1 && ctx.any_comment_in(tag.span.start, tag.span.end) {
        let wc = build_expression_list_writer_comments(
            tag.identifiers,
            ctx.comments,
            ctx.source,
            ctx.loc.tracker,
            tag.span.start,
            tag.span.end,
            tag.identifiers.last().map(|id| id.span().end),
        );
        write_array(w, tag.identifiers, |w, id| {
            write_expression_embedded(
                w,
                id,
                ctx.source,
                ctx.loc,
                CommentMode::Emit(&wc),
                ctx.emit_loc,
            );
        });
        wc.debug_assert_consumed();
    } else {
        write_array(w, tag.identifiers, |w, id| {
            write_generic_island(w, id, tag.span.start, tag.span.end, ctx);
        });
    }
    w.raw("}");
}

/// Emits a `ConstTag` node (`{@const id = init}`).
///
/// The `declaration` `VariableDeclaration` is hand-built the way Svelte's
/// parser builds it: single declarator, `start = tag.span.start + 2` (past
/// `{@`), declarator `end = parser.index` after `read_expression` (see
/// `const_declarator_end`), declaration `end = tag.span.end - 1`
/// (`parser.index - 1`, the byte before the closing `}`). The comment-free
/// document fuses directly; a document with template comments precomputes a
/// `WriterComments` map covering both the id pattern and the init (canonical
/// runs a comment attach per acorn parse — `read_pattern`'s synthetic
/// `(pattern = 1)` and `read_expression`'s init) and fuse-emits with it.
fn write_const_tag(w: &mut JsonWriter, tag: &internal::ConstTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ConstTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"declaration\":");
    // The declaration `end` is always `tag.span.end - 1` — canonical Svelte
    // hard-codes `parser.index - 1` (the byte before the closing `}`).
    let decl_end = ctx.pos(tag.span.end - 1);
    let declarator_end = ctx.pos(const_declarator_end(tag, ctx));
    // Scoped comment pre-check: a comment attaching to this tag necessarily starts
    // inside its span, so no comment in `[tag.span.start, tag.span.end)` means the
    // attach map would be empty — fuse directly (Off ≡ Emit(empty)).
    if !ctx.any_comment_in(tag.span.start, tag.span.end) {
        write_const_declaration(w, tag, decl_end, declarator_end, CommentMode::Off, ctx);
    } else {
        // The document has template comments: precompute the init-subtree
        // attach map (comments attach to the init only).
        let wc = build_const_tag_writer_comments(tag, ctx.comments, ctx.source, ctx.loc.tracker);
        write_const_declaration(
            w,
            tag,
            decl_end,
            declarator_end,
            CommentMode::Emit(&wc),
            ctx,
        );
        wc.debug_assert_consumed();
    }
    w.raw("}");
}

/// The `{@const}` declarator `end`: canonical Svelte sets it to `parser.index`
/// after `read_expression` (svelte#18436) — past the init's wrapping parens
/// (acorn's pre-`remove_parens` node end), and past the last comment the
/// expression parse consumed (`read_expression` extends the index to the last
/// collected comment's end when it lies beyond the expression, which a
/// trailing comment always does). Replicated by walking from the
/// (paren-stripped) internal init end toward the closing `}`, skipping
/// whitespace silently and recording the end of each `)` closer / comment —
/// the last recorded end is the declarator end. Comment-aware by construction
/// (positions come from the parsed comment list, never a substring scan).
fn const_declarator_end(tag: &internal::ConstTag<'_>, ctx: &Ctx<'_>) -> u32 {
    let close = (tag.span.end - 1) as usize; // the closing `}`
    let mut end = tag.init.span().end;
    let mut i = end as usize;
    loop {
        while let Some(ch) = ctx.source[i..close].chars().next() {
            if !ch.is_whitespace() {
                break;
            }
            i += ch.len_utf8();
        }
        if i >= close {
            break;
        }
        if ctx.source.as_bytes()[i] == b')' {
            i += 1;
            end = i as u32;
            continue;
        }
        let idx = ctx
            .comments
            .partition_point(|c| (c.span.start as usize) < i);
        match ctx.comments.get(idx) {
            Some(c) if c.span.start as usize == i => {
                i = c.span.end as usize;
                end = c.span.end;
            }
            _ => break,
        }
    }
    end
}

/// Emit a `{@const}`'s hand-built `VariableDeclaration`. `decl_end` is the
/// already-mapped declaration `end` (`tag.span.end - 1`) and `declarator_end`
/// the already-mapped declarator `end` (`const_declarator_end`); an `Emit`
/// mode feeds the id/init subtrees' fused per-node attach.
fn write_const_declaration(
    w: &mut JsonWriter,
    tag: &internal::ConstTag<'_>,
    decl_end: u32,
    declarator_end: u32,
    mode: CommentMode<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw(
        "{\"type\":\"VariableDeclaration\",\"kind\":\"const\",\"declarations\":[{\"type\":\"VariableDeclarator\",\"id\":",
    );
    write_pattern_embedded(w, &tag.id, ctx.source, ctx.loc, mode, ctx.emit_loc);
    w.raw(",\"init\":");
    write_expression_embedded(w, &tag.init, ctx.source, ctx.loc, mode, ctx.emit_loc);
    w.raw(",\"start\":");
    w.u32(ctx.pos(tag.id.span().start));
    w.raw(",\"end\":");
    w.u32(declarator_end);
    w.raw("}],\"start\":");
    w.u32(ctx.pos(tag.span.start + 2));
    w.raw(",\"end\":");
    w.u32(decl_end);
    w.raw("}");
}

/// Emits a `DeclarationTag` node (`{const …}` / `{let …}`).
///
/// The `declaration` is a real TS `VariableDeclaration`, emitted with its own
/// span `end` in both states (canonical keeps acorn's end for DeclarationTag —
/// unlike `ConstTag`, no `-1` rewrite). The comment-free document fuses via
/// `write_variable_declaration_embedded`; a comment-bearing one precomputes the
/// island's `WriterComments` (`attach_declaration_tag_declaration` attaches
/// across the whole tree).
fn write_declaration_tag(w: &mut JsonWriter, tag: &internal::DeclarationTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"DeclarationTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"declaration\":");
    // Scoped comment pre-check (see `write_const_tag`): no comment inside this
    // tag's span means the attach map is empty, so fuse directly.
    if !ctx.any_comment_in(tag.span.start, tag.span.end) {
        write_variable_declaration_embedded(
            w,
            &tag.declaration,
            ctx.source,
            ctx.loc,
            CommentMode::Off,
            ctx.emit_loc,
        );
    } else {
        let wc = build_declaration_tag_writer_comments(
            &tag.declaration,
            ctx.comments,
            ctx.source,
            ctx.loc.tracker,
            tag.span.start,
            tag.span.end,
        );
        write_variable_declaration_embedded(
            w,
            &tag.declaration,
            ctx.source,
            ctx.loc,
            CommentMode::Emit(&wc),
            ctx.emit_loc,
        );
        wc.debug_assert_consumed();
    }
    w.raw("}");
}

//
// Attributes
//

/// Emit an attribute node, dispatching on its variant (attribute / spread /
/// attach / directive).
fn write_attribute_node(w: &mut JsonWriter, node: &internal::AttributeNode<'_>, ctx: &Ctx<'_>) {
    match node {
        internal::AttributeNode::Attribute(a) => write_attribute(w, a, ctx),
        internal::AttributeNode::SpreadAttribute(s) => write_spread_attribute(w, s, ctx),
        internal::AttributeNode::AttachTag(t) => write_attach_tag(w, t, ctx),
        internal::AttributeNode::OnDirective(d) => write_on_directive(w, d, ctx),
        internal::AttributeNode::BindDirective(d) => write_bind_directive(w, d, ctx),
        internal::AttributeNode::ClassDirective(d) => write_class_directive(w, d, ctx),
        internal::AttributeNode::StyleDirective(d) => write_style_directive(w, d, ctx),
        internal::AttributeNode::UseDirective(d) => write_use_directive(w, d, ctx),
        internal::AttributeNode::TransitionDirective(d) => write_transition_directive(w, d, ctx),
        internal::AttributeNode::AnimateDirective(d) => write_animate_directive(w, d, ctx),
        internal::AttributeNode::LetDirective(d) => write_let_directive(w, d, ctx),
    }
}

/// Emits an `Attribute` node.
fn write_attribute(w: &mut JsonWriter, attr: &internal::Attribute<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Attribute\",\"start\":");
    w.u32(ctx.pos(attr.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(attr.span.end));
    w.raw(",\"name\":");
    w.string(attr.name(ctx.source));
    if ctx.emit_loc {
        w.raw(",\"name_loc\":");
        write_name_loc(w, attr.name_span, ctx);
    }
    w.raw(",\"value\":");
    write_attribute_value_field(w, attr.value, ctx);
    w.raw("}");
}

/// Emit an attribute's `value` field: boolean (`true`), a bare `{expr}` (plain
/// object), or a text/quoted sequence (array).
fn write_attribute_value_field(
    w: &mut JsonWriter,
    value: Option<&[internal::AttributeValue<'_>]>,
    ctx: &Ctx<'_>,
) {
    let Some(values) = value else {
        w.raw("true");
        return;
    };
    let has_text = values
        .iter()
        .any(|v| matches!(v, internal::AttributeValue::Text(_)));
    let quoted = values.len() == 1
        && matches!(&values[0], internal::AttributeValue::ExpressionTag(tag)
            if preceded_by_quote(ctx.source, tag.span.start));

    if has_text || quoted {
        write_array(w, values, |w, v| write_attribute_value(w, v, ctx));
    } else if values.len() == 1 {
        // Single bare expression → plain object. A shorthand `{name}` (the tag
        // and its identifier share a span) injects `character`.
        match &values[0] {
            internal::AttributeValue::ExpressionTag(tag)
                if matches!(&tag.expression, tsv_ts::ast::internal::Expression::Identifier(id)
                    if tag.span == id.span) =>
            {
                write_shorthand_expression_tag(w, tag, ctx);
            }
            v => write_attribute_value(w, v, ctx),
        }
    } else {
        write_array(w, values, |w, v| write_attribute_value(w, v, ctx));
    }
}

/// One attribute-value part (array element or bare-object body).
fn write_attribute_value(w: &mut JsonWriter, value: &internal::AttributeValue<'_>, ctx: &Ctx<'_>) {
    match value {
        internal::AttributeValue::Text(text) => write_attribute_text(w, text, ctx),
        internal::AttributeValue::ExpressionTag(tag) => write_expression_tag(w, tag, ctx),
    }
}

/// Emits a `Text` node in attribute context (`start, end, type, raw, data`).
fn write_attribute_text(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    w.raw("{\"start\":");
    w.u32(ctx.pos(text.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(text.span.end));
    w.raw(",\"type\":\"Text\",\"raw\":");
    w.string(text.raw(ctx.source));
    w.raw(",\"data\":");
    let data = text.data(ctx.source);
    w.string(&data);
    w.raw("}");
}

/// Emits a `SpreadAttribute` node (`{...expr}`).
fn write_spread_attribute(
    w: &mut JsonWriter,
    spread: &internal::SpreadAttribute<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"SpreadAttribute\",\"start\":");
    w.u32(ctx.pos(spread.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(spread.span.end));
    w.raw(",\"expression\":");
    write_generic_island(
        w,
        &spread.expression,
        spread.span.start,
        spread.span.end,
        ctx,
    );
    w.raw("}");
}

/// Emits an `AttachTag` node (`{@attach expr}`).
fn write_attach_tag(w: &mut JsonWriter, tag: &internal::AttachTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"AttachTag\",\"start\":");
    w.u32(ctx.pos(tag.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(tag.span.end));
    w.raw(",\"expression\":");
    write_generic_island(w, &tag.expression, tag.span.start, tag.span.end, ctx);
    w.raw("}");
}

//
// Directives
//

/// The head shared by every directive: `start, end, type, name, name_loc`.
fn write_directive_head(
    w: &mut JsonWriter,
    node_type: &str,
    span: Span,
    name_span: Span,
    head_span: Span,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"start\":");
    w.u32(ctx.pos(span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(span.end));
    w.raw(",\"type\":\"");
    w.raw(node_type);
    w.raw("\",\"name\":");
    w.string(name_span.extract(ctx.source));
    if ctx.emit_loc {
        w.raw(",\"name_loc\":");
        write_name_loc(w, head_span, ctx);
    }
}

/// The `modifiers` array (arena `&str`s → JSON strings).
fn write_modifiers(w: &mut JsonWriter, modifiers: &[&str]) {
    write_array(w, modifiers, |w, m| w.string(m));
}

/// An optional directive expression (`on:`/`use:`/`transition:`/`animate:`/`let:`):
/// a generic island when present, else `null`.
fn write_optional_directive_expression(
    w: &mut JsonWriter,
    expression: Option<&tsv_ts::ast::internal::Expression<'_>>,
    span: Span,
    ctx: &Ctx<'_>,
) {
    write_or_null(w, expression, |w, e| {
        write_generic_island(w, e, span.start, span.end, ctx);
    });
}

/// `on:`/`use:`/`animate:`/`let:` share one wire shape — head, optional
/// expression island, `modifiers` — over four field-identical internal types
/// differing only in node type name. One body, stamped per directive.
macro_rules! expression_directive_writer {
    ($fn_name:ident, $ty:ident) => {
        fn $fn_name(w: &mut JsonWriter, d: &internal::$ty<'_>, ctx: &Ctx<'_>) {
            write_directive_head(w, stringify!($ty), d.span, d.name_span, d.head_span, ctx);
            w.raw(",\"expression\":");
            write_optional_directive_expression(w, d.expression.as_ref(), d.span, ctx);
            w.raw(",\"modifiers\":");
            write_modifiers(w, d.modifiers);
            w.raw("}");
        }
    };
}

expression_directive_writer!(write_on_directive, OnDirective);
expression_directive_writer!(write_use_directive, UseDirective);
expression_directive_writer!(write_animate_directive, AnimateDirective);
expression_directive_writer!(write_let_directive, LetDirective);

fn write_transition_directive(
    w: &mut JsonWriter,
    d: &internal::TransitionDirective<'_>,
    ctx: &Ctx<'_>,
) {
    write_directive_head(
        w,
        "TransitionDirective",
        d.span,
        d.name_span,
        d.head_span,
        ctx,
    );
    w.raw(",\"expression\":");
    write_optional_directive_expression(w, d.expression.as_ref(), d.span, ctx);
    w.raw(",\"modifiers\":");
    write_modifiers(w, d.modifiers);
    w.raw(",\"intro\":");
    w.bool(d.direction.has_intro());
    w.raw(",\"outro\":");
    w.bool(d.direction.has_outro());
    w.raw("}");
}

/// `bind:`/`class:` share an expression: the explicit form (`bind:x={e}`) is a
/// generic island keyed on the directive span (a real expression parse, so
/// template comments can attach); the shorthand form (`bind:x`)
/// is a synthetic loc-free `Identifier` with Svelte field order (`start, end,
/// type, name`) that never carries a comment, emitted fused.
fn write_directive_value_expression(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    has_expression_tag: bool,
    span: Span,
    ctx: &Ctx<'_>,
) {
    if has_expression_tag {
        write_generic_island(w, expr, span.start, span.end, ctx);
    } else {
        // Shorthand: the parser builds this as a synthetic `Identifier`.
        #[allow(clippy::unreachable)]
        let tsv_ts::ast::internal::Expression::Identifier(id) = expr else {
            unreachable!("shorthand directive expression is always an Identifier");
        };
        w.raw("{\"start\":");
        w.u32(ctx.pos(id.span.start));
        w.raw(",\"end\":");
        w.u32(ctx.pos(id.span.end));
        w.raw(",\"type\":\"Identifier\",\"name\":");
        w.string(id.name(ctx.source));
        w.raw("}");
    }
}

fn write_bind_directive(w: &mut JsonWriter, d: &internal::BindDirective<'_>, ctx: &Ctx<'_>) {
    write_directive_head(w, "BindDirective", d.span, d.name_span, d.head_span, ctx);
    w.raw(",\"expression\":");
    write_directive_value_expression(
        w,
        &d.expression,
        d.expression_tag_span.is_some(),
        d.span,
        ctx,
    );
    w.raw(",\"modifiers\":");
    write_modifiers(w, d.modifiers);
    w.raw("}");
}

fn write_class_directive(w: &mut JsonWriter, d: &internal::ClassDirective<'_>, ctx: &Ctx<'_>) {
    write_directive_head(w, "ClassDirective", d.span, d.name_span, d.head_span, ctx);
    w.raw(",\"expression\":");
    write_directive_value_expression(
        w,
        &d.expression,
        d.expression_tag_span.is_some(),
        d.span,
        ctx,
    );
    w.raw(",\"modifiers\":");
    write_modifiers(w, d.modifiers);
    w.raw("}");
}

/// Emits a `StyleDirective` node. Field order: `start, end, type, name,
/// name_loc, modifiers, value`.
fn write_style_directive(w: &mut JsonWriter, d: &internal::StyleDirective<'_>, ctx: &Ctx<'_>) {
    write_directive_head(w, "StyleDirective", d.span, d.name_span, d.head_span, ctx);
    w.raw(",\"modifiers\":");
    write_modifiers(w, d.modifiers);
    w.raw(",\"value\":");
    match &d.value {
        internal::StyleDirectiveValue::True => w.raw("true"),
        internal::StyleDirectiveValue::ExpressionTag(tag) => {
            // Quoted (`style:x="{e}"`) → array; bare (`style:x={e}`) → plain object.
            if preceded_by_quote(ctx.source, tag.span.start) {
                w.raw("[");
                write_expression_tag(w, tag, ctx);
                w.raw("]");
            } else {
                write_expression_tag(w, tag, ctx);
            }
        }
        internal::StyleDirectiveValue::Parts(parts) => {
            write_array(w, *parts, |w, p| write_attribute_value(w, p, ctx));
        }
    }
    w.raw("}");
}

//
// Scripts, style, and shared helpers
//

/// A `<style>` `StyleSheet`. `children` fuse via `tsv_css`'s `write_css_node`;
/// `attributes` and the preceding comment fuse too (the `<style>` envelope is
/// never visited by the template attach passes).
fn write_style_sheet(
    w: &mut JsonWriter,
    style: &internal::Style<'_>,
    preceding_comment: Option<&internal::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"StyleSheet\",\"start\":");
    w.u32(ctx.pos(style.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(style.span.end));
    w.raw(",\"attributes\":");
    write_value_attributes(w, style.attributes, ctx);
    w.raw(",\"children\":");
    write_array(w, style.css_stylesheet.nodes, |w, node| {
        write_css_node(w, node, ctx.source, ctx.loc.map);
    });
    w.raw(",\"content\":{\"start\":");
    w.u32(ctx.pos(style.content_span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(style.content_span.end));
    w.raw(",\"styles\":");
    w.string(style.content_span.extract(ctx.source));
    w.raw(",\"comment\":");
    // Same `{type:"Comment", start, end, data}` shape as a fragment HTML comment.
    write_or_null(w, preceding_comment, |w, c| write_html_comment(w, c, ctx));
    w.raw("}}");
}

/// A `<script>` block. `content` always fuses via `write_program_embedded`; the
/// schema and (when needed) a per-node comment map handle the acorn quirks:
///
/// - **Schema**: `lang="ts"` → `Schema::Acorn`; a plain `<script>` →
///   `Schema::SvelteScript` (omit `importKind`/`exportKind="value"`, always emit
///   `attributes`, append `options: null` on `ImportExpression`).
/// - **Comments**: a script whose `Program` carries comments (its own or a
///   preceding HTML comment) precomputes acorn's leading/trailing assignments
///   into a `WriterComments` map (`build_script_writer_comments`); the common
///   comment-free case fuses with no map.
///
/// The `loc` uses the Svelte tag-line override (final char-space positions); the
/// spine and attributes fuse regardless.
fn write_script(
    w: &mut JsonWriter,
    script: &internal::Script<'_>,
    html_leading_comment: Option<&internal::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"Script\",\"start\":");
    w.u32(ctx.pos(script.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(script.span.end));
    w.raw(",\"context\":");
    // Escape-free `&'static str` (`default` / `module`) → skip the serde scan.
    w.token(script.context.as_str());
    w.raw(",\"content\":");
    // The schema is component-global (Svelte's single `this.ts`): a TS component uses acorn's
    // schema for *every* script, a non-TS one uses Svelte's (omit `importKind`/`exportKind="value"`,
    // always emit `attributes`, and — at emit time — append `options: null` on `ImportExpression`).
    let schema = if ctx.component_is_ts {
        Schema::Acorn
    } else {
        Schema::SvelteScript
    };
    // A script whose `Program` carries comments (its own or a preceding HTML
    // comment) needs acorn's leading/trailing attach — precomputed into a
    // per-node map the fused writer consults at each close. The common case (no
    // comments) fuses with no map (`options: null` still comes from the schema).
    let writer_comments = if script.content.comments.is_empty() && html_leading_comment.is_none() {
        None
    } else {
        Some(build_script_writer_comments(
            script,
            ctx.source,
            ctx.loc.tracker,
            html_leading_comment,
            schema,
        ))
    };
    let mode = match &writer_comments {
        Some(wc) => CommentMode::Emit(wc),
        None => CommentMode::Off,
    };
    write_script_program_fused(w, script, ctx, schema, mode);
    if let Some(wc) = &writer_comments {
        wc.debug_assert_consumed();
    }
    w.raw(",\"attributes\":");
    write_value_attributes(w, script.attributes, ctx);
    w.raw("}");
}

/// Fuse a script's `Program`, reproducing Svelte's tag-line `loc` override in
/// final char space (threading the schema and optional per-node comment map).
///
/// Svelte overrides the byte-space `Program.loc` to `locator(<script> tag
/// start)` and `locator(</script> end)` — `{line, column}` of the tag's own
/// `<`/`>` positions, not of the `Program`'s content span. The final char-space
/// columns rewrite those against the `Program`'s own `start`/`end` byte offsets.
/// `translate_column` is exactly that delta-preserving column math, so applying
/// it here yields the final char-space columns directly (on ASCII it collapses to
/// the raw override — the tag's byte column at each end, so an indented
/// `\t<script>` reports the tag's column, not `0`).
#[allow(clippy::cast_possible_truncation)]
fn write_script_program_fused(
    w: &mut JsonWriter,
    script: &internal::Script<'_>,
    ctx: &Ctx<'_>,
    schema: Schema,
    comments: CommentMode<'_>,
) {
    let program = &script.content;
    // The line/column of a tag position (`<script>` `<` / `</script>` end),
    // rewritten to char space against the Program's own content byte offset —
    // `translate_column` is multibyte-correct and the identity (raw byte column) on
    // ASCII. Start and end are the same computation.
    let position_at = |tag_pos: u32, content_pos: u32| {
        let (line, byte_column) = ctx.loc.tracker.get_line_column(tag_pos as usize);
        Position {
            line,
            column: translate_column(
                content_pos,
                byte_column as u64,
                ctx.loc.map,
                ctx.loc.tracker,
            ) as usize,
        }
    };
    // The override is only consumed when `loc` is emitted; on the no-locations
    // path it's discarded, so skip the two `get_line_column` line-table lookups
    // (which would only hit the stub `[0]` table anyway — see `new_map_only`).
    let loc_override = if ctx.emit_loc {
        (
            position_at(script.span.start, program.span.start),
            position_at(script.span.end, program.span.end),
        )
    } else {
        let dummy = Position { line: 1, column: 0 };
        (dummy, dummy)
    };
    write_program_embedded(
        w,
        program,
        ctx.source,
        ctx.loc,
        schema,
        loc_override,
        comments,
        ctx.emit_loc,
    );
}

/// A `<svelte:options>`: everything fuses. Field order: `start, end,
/// attributes` then the skip-if-none `runes, immutable, css, accessors,
/// preserveWhitespace, namespace, customElement` (no `type`).
fn write_svelte_options(w: &mut JsonWriter, options: &internal::SvelteOptions<'_>, ctx: &Ctx<'_>) {
    let attrs = options.attributes;
    w.raw("{\"start\":");
    w.u32(ctx.pos(options.span.start));
    w.raw(",\"end\":");
    w.u32(ctx.pos(options.span.end));
    w.raw(",\"attributes\":");
    write_value_attributes(w, attrs, ctx);
    if let Some(runes) = bool_option(attrs, "runes", ctx.source) {
        w.raw(",\"runes\":");
        w.bool(runes);
    }
    if let Some(immutable) = bool_option(attrs, "immutable", ctx.source) {
        w.raw(",\"immutable\":");
        w.bool(immutable);
    }
    if let Some(css) =
        find_option_values(attrs, "css", ctx.source).and_then(|v| text_value(v, ctx.source))
    {
        w.raw(",\"css\":");
        w.string(&css);
    }
    if let Some(accessors) = bool_option(attrs, "accessors", ctx.source) {
        w.raw(",\"accessors\":");
        w.bool(accessors);
    }
    if let Some(preserve_whitespace) = bool_option(attrs, "preserveWhitespace", ctx.source) {
        w.raw(",\"preserveWhitespace\":");
        w.bool(preserve_whitespace);
    }
    if let Some(namespace) =
        find_option_values(attrs, "namespace", ctx.source).and_then(|v| text_value(v, ctx.source))
    {
        w.raw(",\"namespace\":");
        w.string(&namespace);
    }
    write_custom_element_field(w, attrs, ctx);
    w.raw("}");
}

/// Emit a leading comma before the second and later members of a JSON object/array being
/// hand-assembled: no-op on the first member, `,` thereafter (flips `first` to `false`).
fn json_comma(w: &mut JsonWriter, first: &mut bool) {
    if *first {
        *first = false;
    } else {
        w.raw(",");
    }
}

/// Emit `customElement.props` as Svelte's statically-*evaluated* plain object
/// (`read_options`, `1-parse/read/options.js`): `{ [name]: { reflect?, type?, attribute? } }`,
/// reading each nested string/boolean literal's value in source order. Not an AST node — no
/// positions.
fn write_custom_element_props(
    w: &mut JsonWriter,
    props_obj: &tsv_ts::ast::internal::ObjectExpression<'_>,
    ctx: &Ctx<'_>,
) {
    use tsv_ts::ast::internal::{Expression, LiteralValue, ObjectProperty};
    w.raw("{");
    let mut first_prop = true;
    for prop in props_obj.properties {
        let ObjectProperty::Property(p) = prop else {
            continue;
        };
        let (Expression::Identifier(key), Expression::ObjectExpression(inner)) = (&p.key, &p.value)
        else {
            continue;
        };
        json_comma(w, &mut first_prop);
        w.string(key.name(ctx.source));
        w.raw(":{");
        let mut first_attr = true;
        for inner_prop in inner.properties {
            let ObjectProperty::Property(ip) = inner_prop else {
                continue;
            };
            let (Expression::Identifier(ikey), Expression::Literal(lit)) = (&ip.key, &ip.value)
            else {
                continue;
            };
            let key_name = ikey.name(ctx.source);
            match &lit.value {
                LiteralValue::String(cooked) => {
                    json_comma(w, &mut first_attr);
                    w.string(key_name);
                    w.raw(":");
                    w.string(cooked.resolve(lit.span, ctx.source));
                }
                LiteralValue::Boolean(b) => {
                    json_comma(w, &mut first_attr);
                    w.string(key_name);
                    w.raw(":");
                    w.bool(*b);
                }
                _ => {}
            }
        }
        w.raw("}");
    }
    w.raw("}");
}

/// Emit the skip-if-none `customElement` option, mirroring Svelte's `read_options`
/// (`1-parse/read/options.js`). The first attribute value that is an object expression
/// (`{ tag, props, shadow, extend }`) or a plain string (`"tag-name"` → `{tag}`) produces the
/// field. Only those four recognized keys are extracted (first-wins on a duplicate, like Svelte's
/// `properties.find`), emitted in the fixed order `tag, props, shadow, extend` regardless of
/// source order — Svelte assembles `ce` in that order. `tag` is a string, `props` a
/// statically-evaluated plain object, `shadow` either the string `'open'`/`'none'` or the raw
/// `ObjectExpression` AST, and `extend` the raw expression AST (both via the shared expression
/// writer, so their offsets translate like any template `{expr}`).
fn write_custom_element_field(
    w: &mut JsonWriter,
    attrs: &[internal::AttributeNode<'_>],
    ctx: &Ctx<'_>,
) {
    use tsv_ts::ast::internal::{Expression, LiteralValue, ObjectProperty};
    let Some(values) = find_option_values(attrs, "customElement", ctx.source) else {
        return;
    };
    for v in values {
        // `customElement={{ tag: '…', props: {…}, shadow: …, extend: … }}`
        if let internal::AttributeValue::ExpressionTag(expr) = v
            && let Expression::ObjectExpression(obj) = &expr.expression
        {
            let mut tag: Option<&Expression<'_>> = None;
            let mut props: Option<&Expression<'_>> = None;
            let mut shadow: Option<&Expression<'_>> = None;
            let mut extend: Option<&Expression<'_>> = None;
            for prop in obj.properties {
                if let ObjectProperty::Property(p) = prop
                    && let Expression::Identifier(key) = &p.key
                {
                    let slot = match key.name(ctx.source) {
                        "tag" => &mut tag,
                        "props" => &mut props,
                        "shadow" => &mut shadow,
                        "extend" => &mut extend,
                        _ => continue,
                    };
                    if slot.is_none() {
                        *slot = Some(&p.value);
                    }
                }
            }

            w.raw(",\"customElement\":{");
            let mut first = true;
            // `tag`: the string-literal value.
            if let Some(Expression::Literal(lit)) = tag
                && let LiteralValue::String(cooked) = &lit.value
            {
                json_comma(w, &mut first);
                w.raw("\"tag\":");
                w.string(cooked.resolve(lit.span, ctx.source));
            }
            // `props`: statically-evaluated plain object.
            if let Some(Expression::ObjectExpression(props_obj)) = props {
                json_comma(w, &mut first);
                w.raw("\"props\":");
                write_custom_element_props(w, props_obj, ctx);
            }
            // `shadow`: the string `'open'`/`'none'`, or the raw `ObjectExpression` AST.
            match shadow {
                Some(Expression::Literal(lit)) => {
                    if let LiteralValue::String(cooked) = &lit.value {
                        json_comma(w, &mut first);
                        w.raw("\"shadow\":");
                        w.string(cooked.resolve(lit.span, ctx.source));
                    }
                }
                Some(shadow_expr @ Expression::ObjectExpression(_)) => {
                    json_comma(w, &mut first);
                    w.raw("\"shadow\":");
                    write_expression_embedded(
                        w,
                        shadow_expr,
                        ctx.source,
                        ctx.loc,
                        CommentMode::Off,
                        ctx.emit_loc,
                    );
                }
                _ => {}
            }
            // `extend`: the raw expression AST.
            if let Some(extend_expr) = extend {
                json_comma(w, &mut first);
                w.raw("\"extend\":");
                write_expression_embedded(
                    w,
                    extend_expr,
                    ctx.source,
                    ctx.loc,
                    CommentMode::Off,
                    ctx.emit_loc,
                );
            }
            w.raw("}");
            return;
        }
        // Plain text or string literal: `customElement="tag-name"` → `{tag}`.
        let tag_str = match v {
            internal::AttributeValue::Text(text) => Some(text.data(ctx.source)),
            internal::AttributeValue::ExpressionTag(expr) => {
                if let Expression::Literal(lit) = &expr.expression
                    && let LiteralValue::String(cooked) = &lit.value
                {
                    Some(std::borrow::Cow::Borrowed(
                        cooked.resolve(lit.span, ctx.source),
                    ))
                } else {
                    None
                }
            }
        };
        if let Some(tag) = tag_str {
            w.raw(",\"customElement\":{\"tag\":");
            w.string(&tag);
            w.raw("}");
            return;
        }
    }
}

/// Attributes outside the fragment tree (`<script>`/`<style>`/`<svelte:options>`
/// tags): the template attach passes never visit them, so each fuses through the
/// same attribute writer the fragment path uses but with a comment-free context —
/// no expression-tag value can pick up a template comment.
fn write_value_attributes(
    w: &mut JsonWriter,
    attributes: &[internal::AttributeNode<'_>],
    ctx: &Ctx<'_>,
) {
    let ctx = ctx.without_comments();
    write_array(w, attributes, |w, a| write_attribute_node(w, a, &ctx));
}

/// A block pattern (`{#each … as ctx}`, `{:then value}`/`{:catch error}`):
/// emitted fused via `tsv_ts`'s `write_pattern_embedded` (the `+1`-column /
/// `character` / stripped-type-annotation-`loc` quirks in final char space).
/// Patterns never collect comments, so there is no attach.
fn write_pattern_island(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    ctx: &Ctx<'_>,
) {
    write_pattern_embedded(w, expr, ctx.source, ctx.loc, CommentMode::Off, ctx.emit_loc);
}

/// A fragment or `null` (the `AwaitBlock` branch fields and `IfBlock`'s
/// `alternate`, no skip).
fn write_optional_fragment(
    w: &mut JsonWriter,
    fragment: Option<&internal::Fragment<'_>>,
    ctx: &Ctx<'_>,
) {
    write_or_null(w, fragment, |w, f| write_fragment(w, f, ctx));
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// Parse full Svelte source and return the public JSON AST.
    fn convert_svelte(source: &str) -> Value {
        let arena = bumpalo::Bump::new();
        // Test inputs are hardcoded valid sources; a parse failure should panic
        #[allow(clippy::expect_used)]
        let root = crate::parse(source, &arena).expect("parse");
        crate::convert_ast_json(&root, source)
    }

    // Svelte hard-codes a `{@const}` declaration's `end` to `parser.index - 1`
    // (the byte before the closing `}`) — independent of interior whitespace
    // and of whether the document carries template comments. Not expressible
    // as a fixture: the trigger (whitespace before `}`) is never format-stable.
    #[test]
    fn const_tag_declaration_end_is_byte_before_closing_brace() {
        // `}` at byte 28; the init ends at 27 — the end must be 28.
        let ast = convert_svelte("{#snippet s()}{@const x = 1 }{/snippet}");
        let decl = &ast["fragment"]["nodes"][0]["body"]["nodes"][0]["declaration"];
        assert_eq!(decl["end"], 28);

        // The same tag in a comment-bearing document: identical end.
        let ast = convert_svelte("{#snippet s()}{@const x = 1 }{/snippet}\n{/* c */ y}");
        let decl = &ast["fragment"]["nodes"][0]["body"]["nodes"][0]["declaration"];
        assert_eq!(decl["end"], 28);
    }

    // A `{let}`/`{const}` DeclarationTag keeps acorn's declaration `end`
    // (canonical Svelte applies no `-1` rewrite there, unlike `{@const}`) — in
    // both document states.
    #[test]
    fn declaration_tag_end_is_acorns_declaration_end() {
        let ast = convert_svelte("{let x = 1 }");
        assert_eq!(ast["fragment"]["nodes"][0]["declaration"]["end"], 10);

        let ast = convert_svelte("{let x = 1 }\n{/* c */ y}");
        assert_eq!(ast["fragment"]["nodes"][0]["declaration"]["end"], 10);
    }
}
