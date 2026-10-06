//! Writer-mode conversion: emit compact wire JSON directly from the internal
//! Svelte AST.
//!
//! The Svelte sibling of `tsv_ts`'s and `tsv_css`'s `ast/convert/write*`, writing
//! both wires: the span-only one behind `convert_ast_json_bytes`
//! (every parse binding, the CLI's default output) and the loc-bearing one behind
//! `convert_ast_json_bytes_with_locations` (`tsv parse --locations`). It walks the
//! *internal* Svelte AST once and writes the final JSON bytes as it goes, never
//! materializing the typed public `Root`.
//!
//! **Fused emission.** The Svelte spine (elements, blocks, tags, directives,
//! attributes, `name_loc`, positions) is emitted *fused* — final char-space
//! `start`/`end`/`loc`/`character` written directly via `WirePositions` (the
//! byte→UTF-16 map, plus the document's one line table on the wire that carries
//! `loc`, both in the coordinates `crate::WIRE_COORDINATES` states), exactly as the
//! `tsv_ts`/`tsv_css` writers do.
//!
//! **`loc` everywhere.** Every object with numeric `start`/`end` — template nodes,
//! `Root`, `Script`, `StyleSheet` and its `content`, `<svelte:options>`, attached and
//! root comments, every acorn and `<style>` node — gets `loc` immediately after `end`:
//! the line (1-based, one line rule for the whole document) and column (0-based,
//! UTF-16 code units) of those same offsets. That is a superset of Svelte's own wire, which
//! carries `loc` on acorn-parsed nodes only, and it reproduces none of Svelte's
//! `loc` quirks (the `_ as ` line swallow, acorn's second line class, the
//! tag-position `Program.loc`).
//! Almost everything else fuses too:
//!
//! - **Root comments, `<svelte:options>`** (scalar props + `customElement`),
//!   **`<style>`** (CSS `children` via `tsv_css`'s `write_css_children`, whose
//!   return value is the sibling `comments` array, plus the
//!   `StyleSheet`/`StyleContent` envelope and preceding comment),
//!   **`<script>`/`<style>`/`<svelte:options>` tag attributes**, the
//!   **`<svelte:element>` string `tag`**, and **bind/class shorthand** identifiers
//!   all emit fused, directly from the internal AST.
//! - **Block patterns** (`{#each … as ctx}`, `{:then value}`/`{:catch error}`,
//!   `{@const}` ids) fuse via `tsv_ts`'s `write_pattern_embedded` (the
//!   simple identifier's `character`).
//! - **Shorthand attributes / snippet names** fuse via
//!   `write_identifier_expression_with_character`.
//! - **Generic template expressions** (`{expr}`, block tests, directive
//!   expressions, …) emit fused via `tsv_ts`'s `write_expression_embedded`. When
//!   a template comment lands inside the expression's window (the
//!   `any_comment_in` pre-check), the expression fuses with
//!   `CommentMode::Attach` — acorn's attach runs online off that emit's own node
//!   opens and closes, and each node emits its
//!   `leadingComments`/`trailingComments` at its close.
//! - **Snippet names / parameters** fuse the same way (with `character`
//!   injection / the shared one-queue list attach, matching canonical's single
//!   acorn parse of the list — multi-identifier `{@debug}` rides the same path).
//! - **`{@const}` / `{const}` / `{let}` declarations** fuse their
//!   `VariableDeclaration` structure (the `{@const}` declaration `end` is always
//!   `tag.span.end - 1`, Svelte's `parser.index - 1`); when the document has a
//!   template comment the init/declaration subtree runs its own attach.
//! - **`<script>` content** always fuses via `write_program_embedded`: an
//!   eligible script (`lang="ts"` ∧ no script comments ∧ no preceding HTML
//!   comment) with no attach at all; an ineligible one (a plain non-`lang="ts"` script, one
//!   with comments, or one with a preceding HTML comment) with the schema-driven
//!   `options: null` quirk and, when it has comments, an online attach (the
//!   preceding HTML comment prepended to the `Program`'s `leadingComments`).
//!
//! **The comment attach** (`ast/convert/comment_attachment.rs`'s `attach_*`, `tsv_ts`'s
//! `CommentAttach`) runs **online**, off this one emission: acorn assigns a
//! node's leading comments at node *entry* and its trailing ones after its
//! children, which are exactly the writer's `node_header` and `close_node`, and
//! the wire emits both lists at the close. So there is no second pass, no
//! recorded tree and no per-node map — attached comments serialize *last*
//! within a node exactly as acorn's appended keys place them, and the walk's
//! child-visit order IS the emitted field order.
//!
//! **Staged runs.** Three fixed-shape bursts on the span-only wire — the
//! `Attribute` / element / `Text` node headers — assemble in the writer's scratch
//! and reach the buffer as one append (`JsonWriter::stage_run`), the shape
//! `tsv_ts`'s `node_header_impl` already uses. Written directly, a burst pays
//! `Vec`'s append protocol per fragment *and* re-loads the buffer's pointer, length
//! and capacity after every integer, because `JsonWriter::u32` is deliberately
//! `inline(never)` (a size constraint — see its comment). A run stops at its first
//! dynamic string — a `w.string(…)`, or a name field ([`write_element_name_field`],
//! [`write_scanned_name_field`], which write their own key): its length is dynamic,
//! so the value cannot be staged. The loc-bearing wire stages one more, the
//! `name_loc` field ([`write_name_loc_field`]); that wire is compiled out of every
//! binding, so it costs no shipped artifact anything.
//!
//! **A run is earned by frequency.** Those headers are among the most frequent
//! bursts the writer emits, and each staged emitter is *inlined at its site* — so
//! a rare burst buys nothing and still costs `@fuzdev/tsv-parse-wasm` bytes.
//! `ExpressionTag`, `write_text`'s raw-content arm, `write_sequence_text` and
//! the directive heads are deliberately left unstaged: their `start`/`end`
//! pairs are one out-of-line call each (`JsonWriter::start_end`, or its
//! `start_end_field` / `start_end_object` forms, which also write the
//! `,"start":` / `{"start":` key — reached through `JsonWriter::span_start_end`
//! and its twins, which add the `loc` a loc-bearing wire carries), the pair
//! written into one fixed-width window of the buffer rather than as two `u32`
//! calls around an append.
//! ⚠️ It is the *staged emitters* that pay, not the `StageRun::flush` copy — grade
//! any change here on `cycles:u`, since the run's narrow stores feed a
//! `memmove` that reads them back.
//!
//! **A `Text`'s `raw` and `data` are escaped once** when `data` borrows `raw`
//! — a text with no `&`, which is nearly all template text; a text holding a line
//! break needs escaping, so escaping it once matters. `JsonWriter::string_pair`
//! escapes it once and copies the emitted bytes for the second field. The
//! `raw`-first shapes reach it through [`write_raw_then_data`], which first sends
//! a text made only of collapsible whitespace to `JsonWriter::string_pair_whitespace`
//! (every byte's escape known, no prescan), and escapes two strings only for a
//! text whose `data` is owned (one holding a `&`);
//! `write_text`'s raw-content arm (`data` first) calls it directly, since a
//! `Raw` decoding is no decode at all and its two values are the same bytes
//! even when the text holds a `&`.
//!
//! **Byte-identity**: the wire JSON is a faithful emission of the Svelte
//! parser's JSON (its acorn `<script>` shape plus `parseCss` `<style>` shape) —
//! the shape the canonical Svelte parser's `expected.json` records.

use std::borrow::Cow;

use crate::ast::internal;
use crate::whitespace::is_svelte_ws;
use tsv_css::ast::convert::{write_css_children, write_css_comments};
use tsv_lang::{
    Comment, JsonWriter, LocationMapper, Span, Wire, WirePositions, WireTables,
    estimated_json_capacity, write_array, write_or_null,
};
use tsv_ts::ast::convert::{
    CommentAttach, CommentMode, EmbedWriter, write_expression_embedded,
    write_identifier_expression_with_character, write_pattern_embedded, write_program_embedded,
    write_variable_declaration_embedded,
};

use super::comment_attachment::{
    AttachInputs, attach_binding_pattern, attach_const_tag_init, attach_expression,
    attach_expression_list, attach_script, attach_statement, grouping_parens_around,
    is_template_comment, pattern_comment_window,
};
use super::special::{bool_option, component_is_typescript, find_option_values, text_value};

/// Convert an internal Svelte `Root` straight to its compact wire-JSON bytes.
///
/// One AST walk, no intermediate `serde_json::Value` for the spine. On [`Wire::Span`]
/// it writes the span-only wire every binding ships — only `start`/`end` offsets, no
/// `loc` and no `name_loc`; on `Wire::Loc` it adds every line/column object, the
/// loc-bearing wire `tsv parse --locations` prints.
pub(crate) fn write_root_bytes(root: &internal::Root<'_>, source: &str, wire: Wire) -> Vec<u8> {
    // One set of tables for the whole document — template, every acorn island,
    // `<script>` bodies, root comments and `<style>` — in the coordinates
    // `crate::WIRE_COORDINATES` states.
    let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);

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
        positions: tables.positions(),
        snippet_wire_parameters: root.snippet_wire_parameters,
        comments: &template_comments,
        // Component-global: `lang="ts"` on any script makes *every* script emit the
        // acorn-typescript wire shape (Svelte's single `this.ts` flag).
        component_is_ts: component_is_typescript(root, source),
    };

    let mut w = JsonWriter::with_capacity(estimated_json_capacity(source.len(), wire));
    write_root(&mut w, root, &ctx);
    w.into_bytes()
}

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    source: &'a str,
    /// The document's positions: the byte→UTF-16 map, and — on the wire that carries
    /// `loc` — its one line table, which every node, `name_loc`, root comment,
    /// embedded island and `<style>` node reads.
    positions: WirePositions<'a>,
    /// `Root::snippet_wire_parameters`: the preserved-paren parameter lists the wire emits
    /// in place of a head's paren-free `SnippetBlock::parameters`.
    snippet_wire_parameters: &'a [internal::SnippetWireParameters<'a>],
    /// Template comments, sorted by position (empty on the common no-comment
    /// template — the whole spine then fuses).
    comments: &'a [&'a Comment],
    /// Whether the component parses as TypeScript (component-global, from the first
    /// lang-bearing `<script>` — see `component_is_typescript`). Selects the parser variant
    /// every embedded writer reads (`EmbedWriter::vanilla_acorn`, via `Ctx::embed`), so a
    /// plain `<script>` beside a `lang="ts"` sibling still emits the acorn-typescript
    /// import/export shape.
    component_is_ts: bool,
}

impl<'a> Ctx<'a> {
    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    fn pos(&self, byte: u32) -> u32 {
        self.positions.pos(byte)
    }

    /// The preserved-paren parameter list of the `{#snippet}` block starting at
    /// `snippet_start`, when its head holds a grouping pair (`Root::snippet_wire_parameters`,
    /// ascending by that key).
    fn snippet_wire_parameters(
        &self,
        snippet_start: u32,
    ) -> Option<&'a [tsv_ts::ast::internal::Expression<'a>]> {
        let wire = self.snippet_wire_parameters;
        wire.binary_search_by_key(&snippet_start, |entry| entry.snippet_start)
            .ok()
            .map(|i| wire[i].parameters)
    }

    /// The shared inputs for an embedded `tsv_ts` writer — this document's `source`,
    /// positions and parser variant, plus the per-call comment `mode`.
    ///
    /// Every embedded emission funnels through here — each `<script>`'s `Program`
    /// and every expression island — which is what makes the parser variant reach
    /// them: it is component-global, so a `<script>`, a `{expr}` tag, an attribute
    /// or directive value, a `{@const}` and a `{#snippet}` body all carry vanilla
    /// acorn's wire quirks in a non-TS component.
    #[inline]
    fn embed(&self, mode: CommentMode<'a>) -> EmbedWriter<'a> {
        EmbedWriter {
            source: self.source,
            positions: self.positions,
            comments: mode,
            vanilla_acorn: !self.component_is_ts,
            // Only `embed_pattern` fills this: a block pattern is the one island that
            // can be two parses.
            annotation_comments: CommentMode::Off,
        }
    }

    /// The same, for a **block pattern** island — which is up to *two* parses:
    /// the pattern itself, and its trailing `: T`, which Svelte reads with a
    /// separately padded second one (`read_type_annotation`) that attaches its own
    /// comments under `annotation_mode` (see `attach_binding_pattern`).
    #[inline]
    fn embed_pattern(
        &self,
        mode: CommentMode<'a>,
        annotation_mode: CommentMode<'a>,
    ) -> EmbedWriter<'a> {
        EmbedWriter {
            annotation_comments: annotation_mode,
            ..self.embed(mode)
        }
    }

    /// The shared inputs for a template island's comment attach
    /// (`ast/convert/comment_attachment.rs`'s `attach_*`) — this document's
    /// template comments and source.
    ///
    /// It carries no parser variant and no tracker: the attach runs online off
    /// the one emission, so there is no second pass to configure and no way for
    /// two passes to disagree.
    #[inline]
    fn attach_inputs(&self) -> AttachInputs<'a> {
        AttachInputs {
            template_comments: self.comments,
            source: self.source,
        }
    }

    /// A copy of this context with no template comments — for subtrees no
    /// template island attach ever reaches (`<script>`/`<style>`/`<svelte:options>`
    /// tag attributes), so their embedded expressions always fuse comment-free.
    #[inline]
    fn without_comments(&self) -> Ctx<'a> {
        Ctx {
            comments: &[],
            ..*self
        }
    }

    /// Superset pre-check: does any template comment *start* in `[start, end)`?
    /// A miss means the expression fuses with no attach at all.
    ///
    /// `self.comments` is sorted ascending by `span.start`, so the first comment
    /// at/after `start` (binary search) starting before `end` settles the query.
    #[inline]
    fn any_comment_in(&self, start: u32, end: u32) -> bool {
        let idx = self.comments.partition_point(|c| c.span.start < start);
        self.comments.get(idx).is_some_and(|c| c.span.start < end)
    }
}

/// Start position of a fragment's first node — the range-end tightener an island
/// attach uses so a sibling expression context (`{:else if}`) doesn't bleed into a
/// block's own expression window.
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

/// The leading HTML comment a lifted `<script>` / `<style>` at `tag_start` carries — a
/// script's `content.leadingComments`, a stylesheet's `content.comment`. One reader for all
/// three roots, because Svelte decides all three the same way.
///
/// Svelte's own rule (`1-parse/state/element.js`) walks `current.fragment.nodes` **backwards**
/// from the tag: a `Comment` wins, a `Text` whose `data` is `.trim()`-empty is stepped over,
/// anything else stops the walk. This mirrors it, with the two things that expression really
/// says spelled out — and both were wrong when it was a scan over the raw source between the
/// comment and the tag:
///
/// - ⚠️ **The class is JS `\s`** ([`is_svelte_ws`]), because
///   `data.trim()` is `String.prototype.trim`. `str::trim` is Rust's `White_Space`, which
///   disagrees in both directions and so got both witnesses wrong at once: a `<ZWNBSP>` gap
///   (JS whitespace) blocked an attachment canonical makes, and a `<NEL>` gap (not JS
///   whitespace) allowed one canonical does not.
/// - ⚠️ **It is the Text node's DECODED `data`, not the source bytes.** `<!-- c -->&nbsp;`
///   is one whitespace character to Svelte and six content characters to a raw scan, so an
///   entity-spelled gap lost the attachment. `Text::data` is the same decode the wire emits.
///
/// ⚠️ **The contiguity check is the one deliberate difference from Svelte, and it is load
/// bearing.** A lifted `<script>` / `<style>` is never appended to the fragment, so Svelte's
/// walk crosses one as if it were not there and attaches the same comment to *every* root that
/// follows it. tsv attaches it once, to the nearest — the cataloged anti-duplication stance
/// (docs/conformance_svelte.md §Comment Attachment Differences). A lifted tag is exactly a
/// **hole between two fragment nodes**, so requiring each step to end where the next begins
/// expresses the stance without a second concept: with no lifted tag in the gap the nodes are
/// contiguous and this is Svelte's loop unchanged. Pinned in both directions by
/// tests/svelte_preceding_comment.rs, whose controls fail if a future rewrite reproduces the
/// walk faithfully and reopens the duplication.
fn preceding_comment<'a>(
    root: &'a internal::Root<'_>,
    tag_start: u32,
    source: &str,
) -> Option<&'a internal::HtmlComment> {
    let mut expected_end = tag_start;
    for node in root.fragment.nodes.iter().rev() {
        let span = node.span();
        // Nodes after the tag did not exist when Svelte's walk ran (it runs mid-parse, off
        // the fragment built so far), so they are skipped rather than allowed to stop it.
        if span.start >= tag_start {
            continue;
        }
        if span.end != expected_end {
            break;
        }
        match node {
            internal::FragmentNode::Comment(comment) => return Some(comment),
            internal::FragmentNode::Text(text)
                if text.data(source).trim_matches(is_svelte_ws).is_empty() =>
            {
                expected_end = span.start;
            }
            _ => break,
        }
    }
    None
}

/// Emit the `Root` node. Field order:
/// `css, js, start, end, type, fragment, options, comments, [instance], [module]`.
fn write_root(w: &mut JsonWriter, root: &internal::Root<'_>, ctx: &Ctx<'_>) {
    let source = ctx.source;

    let find_preceding_comment = |tag_start: u32| preceding_comment(root, tag_start, source);

    w.raw("{\"css\":");
    write_or_null(w, root.css.as_ref(), |w, style| {
        let style_comment = find_preceding_comment(style.span.start);
        write_style_sheet(w, style, style_comment, ctx);
    });
    w.raw(",\"js\":[],\"start\":");
    w.span_start_end(ctx.positions, 0, source.len() as u32);
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
/// comment collectors build different literals: one acorn collected (a `<script>`
/// or template-expression comment, through its `onComment` wrapper) is
/// `{type, value, start, end, loc}`; an in-tag one its template reader collected,
/// `{type, start, end, value, loc}` with `character` in its `loc` — the
/// `from_template_reader` axis keys both differences. tsv writes `loc` immediately
/// after `end` in both, from the document's one line table.
fn write_root_comment(w: &mut JsonWriter, comment: &Comment, ctx: &Ctx<'_>) {
    let span = comment.span;
    w.raw("{\"type\":\"");
    w.raw(if comment.is_block { "Block" } else { "Line" });
    if comment.from_template_reader {
        w.raw("\",\"start\":");
        w.start_end(ctx.pos(span.start), ctx.pos(span.end));
        if let Some(lines) = ctx.positions.lines() {
            // `character` in each point, the shape Svelte's own `locate-character`
            // positions take
            let (start, end) = lines.span_positions(span.start, span.end);
            w.loc_field::<true>(start, end);
        }
        w.raw(",\"value\":");
        w.string(&comment.wire_value(ctx.source));
    } else {
        w.raw("\",\"value\":");
        w.string(&comment.wire_value(ctx.source));
        w.span_start_end_field(ctx.positions, span.start, span.end);
    }
    w.raw("}");
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

/// A generic template expression island: fused when comment-free, else with the
/// island's online attach driving `leadingComments` / `trailingComments` off this
/// emit's own node opens and closes (`attach_expression`).
///
/// The attach is keyed on `expr`'s own span — a JSDoc cast's covers its `(`…`)` — and
/// reads the bare grouping pairs around it back off the source, since the canonical root
/// is the outermost pair and the parse ended past its `)`; see `attach_expression`.
///
/// Call this directly only for a window that is **deliberately asymmetric** — a block head,
/// whose attach runs from the `{#` to the end of its clause rather than to the expression's
/// own end. Everything else wants [`write_braced_island`], which keys the window on the
/// node's own span and cannot be handed two offsets from different nodes.
fn write_generic_island(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if ctx.any_comment_in(container_start, range_end) {
        let attach =
            attach_expression(ctx.attach_inputs(), container_start, expr.span(), range_end);
        write_expression_embedded(w, expr, ctx.embed(attach.mode()));
    } else {
        write_expression_embedded(w, expr, ctx.embed(CommentMode::Off));
    }
}

/// An expression island whose comment-attach window is the **braces it was written in** —
/// what every `{…}` tag wants, and the only window that reproduces Svelte's own attach:
/// `parse_expression_at` filters the comment set to `comment.start >= index`, where `index`
/// is where that expression's own parse began (`1-parse/acorn.js`, `get_comment_handlers`).
///
/// One `Span` argument rather than the two loose offsets, because the defect this guards is
/// keying the window on some *other* node: passing `<svelte:element this={x} />`'s enclosing
/// element span puts a comment earlier in the tag head inside the window, attaching it to the
/// expression as a `leadingComments` entry the canonical parser never emits.
/// A directive passes its whole attribute span rather than the braces — an equivalent
/// window, since no comment can sit between a directive head and its `{` (both parsers
/// reject there) and none can follow the `}` inside the span's tail (at most a closing
/// quote). A block head is the deliberate exception — its window runs from the `{#` to the
/// end of the clause, not to the expression's end — and calls [`write_generic_island`]
/// directly.
fn write_braced_island(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    window: Span,
    ctx: &Ctx<'_>,
) {
    write_generic_island(w, expr, window.start, window.end, ctx);
}

/// The `,"name_loc":` field and the shared `NameLocation` shape it holds:
/// `start`/`end` each `{line, column, character}` (all three, always) — the `loc`
/// object's character-bearing form, spelled once for every writer by
/// `StageRun::loc_object`. Char-space via one fused translation per endpoint. Only the
/// loc-bearing wire has one: the span-only wire every binding ships carries no
/// `name_loc`.
///
/// Emitted as one **staged run** (`JsonWriter::stage_run`), the field **key inside
/// it** rather than a `raw` at the call sites: every caller emits the same key, which
/// is also why the callers name this `_field` — it emits the key, so it may not be
/// spliced into a position where some other key precedes the object.
///
/// A name never crosses a line, so only its start is resolved to one
/// ([`LocationMapper::one_line_span_positions`]): the line table here is Svelte's,
/// where only `\n` starts a line, and every name span stops at whitespace, which
/// includes `\n` — an element or component name, an attribute name and a directive's
/// head (`on:click|once`) are each read to Svelte whitespace, `/`, `>` or `=` at the
/// latest, and a shorthand attribute's (`{x}`) is its identifier's span, which holds
/// no whitespace at all. So the end's line is the start's, which `loc_object` appends
/// again from the digits already in hand rather than converting it a second time.
fn write_name_loc_field(w: &mut JsonWriter, span: Span, lines: LocationMapper<'_>) {
    let (start, end) = lines.one_line_span_positions(span.start, span.end);
    debug_assert_eq!(start.1.line, end.1.line, "a name span lies on one line");
    let mut run = w.stage_run();
    run.loc_object::<true>(",\"name_loc\":", start, end);
    run.flush();
}

/// The `,"name":` key a Svelte node's name follows.
const NAME_KEY: &[u8; 8] = b",\"name\":";

/// Emit an element's `,"name":` field.
///
/// A `RegularElement`'s name is escape-free by the grammar the parser admits it under, so
/// it is written as the key and the quoted slice in one fixed-width append
/// ([`JsonWriter::string_escape_free_led`]), with no escape scan. The element parser's name
/// gate (`is_valid_tag_name`) admits a name that is not component-shaped only as a valid
/// element name: an ASCII letter followed by ASCII letters and digits and a `-`-led
/// custom-element tail of `PCENChar`s, a `![a-zA-Z]+` declaration, or an ASCII
/// `prefix:local` name — and no character of those is a `"`, a `\` or a control. A
/// component name — a name with no `:` whose lead is uppercase or that holds a `.` — is read
/// to whitespace, `/` or `>`, so it CAN hold one (`<A\b/>`), and is scanned
/// ([`write_scanned_name_field`]).
///
/// `inline(never)`: one out-of-line copy of the window write, whose arms leave by tail
/// call, so it saves no register.
#[inline(never)]
fn write_element_name_field(w: &mut JsonWriter, elem: &internal::Element<'_>, ctx: &Ctx<'_>) {
    let name = name_bytes(elem.name_span, ctx);
    match elem.kind {
        internal::ElementKind::Html => w.string_escape_free_led(NAME_KEY, name),
        internal::ElementKind::Component => write_scanned_name_field(w, name),
    }
}

/// A `,"name":` field for a name no grammar vouches for — an attribute's, a component's:
/// the escape scan, then the key and the quoted name as one fixed-width append when it
/// comes back clean ([`JsonWriter::string_led`]).
///
/// `inline(never)`: one shared copy of the short scan and the window write for its two
/// callers (the attribute and the component name); the word loop and the escaping arm leave
/// by tail call.
#[inline(never)]
fn write_scanned_name_field(w: &mut JsonWriter, name: &[u8]) {
    w.string_led(NAME_KEY, name);
}

/// The source bytes of a name span, without the char-boundary checks a `&str` slice pays:
/// a name span covers whole characters, which debug builds assert. ⚠️ Invariant, not a
/// check: in release a span that split a character would be copied into the wire as invalid
/// UTF-8 where the `&str` slice would have panicked — every name span is a lexed run's, so
/// none does.
#[inline]
fn name_bytes<'s>(span: Span, ctx: &Ctx<'s>) -> &'s [u8] {
    debug_assert!(
        ctx.source.is_char_boundary(span.start as usize)
            && ctx.source.is_char_boundary(span.end as usize),
        "a name span covers whole characters"
    );
    &ctx.source.as_bytes()[span.range()]
}

/// Emits a `RegularElement` (HTML) or `Component` node.
fn write_element(w: &mut JsonWriter, elem: &internal::Element<'_>, ctx: &Ctx<'_>) {
    // Staged burst; ends before the `name` field (module doc, Staged runs). The
    // opening literal is whole per kind, so each arm's copy is one constant's width.
    let mut run = w.stage_run();
    match elem.kind {
        internal::ElementKind::Component => run.raw("{\"type\":\"Component\",\"start\":"),
        internal::ElementKind::Html => run.raw("{\"type\":\"RegularElement\",\"start\":"),
    }
    run.u32(ctx.pos(elem.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(elem.span.end));
    run.flush();
    w.span_loc(ctx.positions, elem.span.start, elem.span.end);
    write_element_name_field(w, elem, ctx);
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, elem.name_span, lines);
    }
    // The constant stretch between two of the element's dynamic parts is one append,
    // and an empty attribute list rides the one that opens the fragment.
    if elem.attributes.is_empty() {
        w.raw(",\"attributes\":[],\"fragment\":{\"type\":\"Fragment\",\"nodes\":[");
    } else {
        w.raw(",\"attributes\":[");
        write_items(w, elem.attributes, |w, a| write_attribute_node(w, a, ctx));
        w.raw("],\"fragment\":{\"type\":\"Fragment\",\"nodes\":[");
    }
    let nodes = elem.fragment.nodes;
    // A `<textarea>`'s content is read with the attribute-value sequence
    // machinery in the canonical parser, whose `Text` literal leads with the
    // positions (`{start, end, type, raw, data}`). Asked only of an element with
    // children: an empty one writes the same bytes under either name.
    if !nodes.is_empty() && elem.name(ctx.source) == "textarea" {
        write_items(w, nodes, |w, n| match n {
            internal::FragmentNode::Text(text) => write_sequence_text(w, text, ctx),
            _ => write_fragment_node(w, n, ctx),
        });
    } else {
        write_items(w, nodes, |w, n| write_fragment_node(w, n, ctx));
    }
    // The node array, the fragment, the element.
    w.raw("]}}");
}

/// The items of a JSON array, comma-separated, without its brackets — for a caller
/// whose `[` and `]` ride the constants on either side ([`write_array`] writes them
/// itself).
#[inline]
fn write_items<T>(w: &mut JsonWriter, items: &[T], mut f: impl FnMut(&mut JsonWriter, &T)) {
    let mut first = true;
    for item in items {
        if !first {
            w.raw(",");
        }
        first = false;
        f(w, item);
    }
}

/// Emits a special-element node (`svelte:element`, `svelte:component`, …).
/// `tag`/`expression` are skip-if-none.
fn write_special_element(w: &mut JsonWriter, elem: &internal::SpecialElement<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"");
    w.raw(elem.kind.node_type());
    w.raw("\",\"start\":");
    w.span_start_end(ctx.positions, elem.span.start, elem.span.end);
    w.raw(",\"name\":");
    // Escape-free `&'static str` (`svelte:head`, `slot`, `title`, …) → skip the
    // string-escape scan.
    w.token(elem.kind.tag_name());
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, elem.name_span, lines);
    }
    w.raw(",\"attributes\":");
    write_array(w, elem.attributes, |w, a| write_attribute_node(w, a, ctx));
    w.raw(",\"fragment\":");
    write_fragment(w, &elem.fragment, ctx);
    // `<svelte:element this={…}>` tag. A plain-string `this="x"` is a
    // Svelte-style `Literal` (single-quoted `raw`) that carries no
    // expression parse, so no template comment can attach — emit it fused.
    // Every other `this={…}` is a braced island keyed on its own `{…}` span.
    if let Some(tag) = elem.kind.tag() {
        w.raw(",\"tag\":");
        write_special_tag(w, tag, ctx);
    }
    // `<svelte:component this={…}>` expression — a generic island.
    if let Some(tag) = elem.kind.expression() {
        w.raw(",\"expression\":");
        write_braced_island(w, tag.expression, tag.span, ctx);
    }
    w.raw("}");
}

/// A `<svelte:element this={…}>` tag. The plain-string form (`this="x"`) is a Svelte-style
/// `Literal` (`{type, value, raw, start, end}` — single-quoted `raw`) fused
/// directly; the braced form is a generic island keyed on **its own `{…}` span**, like every
/// other expression island.
///
/// That window is the binding's braces and not the enclosing element's span, which is what
/// Svelte's attach reduces to: `parse_expression_at` filters the comment set to
/// `comment.start >= index`, and `index` is where the expression's own parse began
/// (`1-parse/acorn.js`, `get_comment_handlers`). Keyed on the element instead, a comment
/// earlier in the tag head (`<svelte:element /* c */ this={x} />`) precedes the expression
/// inside the window and attaches to it as a `leadingComments` entry the canonical parser
/// never emits.
///
/// The form is read off [`internal::SpecialThis`], never sniffed back out of the source: it
/// is the parser's own answer, and a second way to ask the same question is a second way to
/// get it wrong.
fn write_special_tag(w: &mut JsonWriter, this: &internal::SpecialThis<'_>, ctx: &Ctx<'_>) {
    match this {
        internal::SpecialThis::Plain { content, span } => {
            w.raw("{\"type\":\"Literal\",\"value\":");
            w.string(content);
            w.raw(",\"raw\":");
            // Svelte reports the raw as a single-quoted string regardless of source.
            w.string(&format!("'{content}'"));
            w.span_start_end_field(ctx.positions, span.start, span.end);
            w.raw("}");
        }
        internal::SpecialThis::Braced(tag) => {
            write_braced_island(w, tag.expression, tag.span, ctx);
        }
    }
}

/// Emits an `ExpressionTag` node (fragment `{expr}`).
fn write_expression_tag(w: &mut JsonWriter, tag: &internal::ExpressionTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ExpressionTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"expression\":");
    write_braced_island(w, tag.expression, tag.span, ctx);
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
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"expression\":");
    write_identifier_expression_with_character(w, tag.expression, ctx.embed(CommentMode::Off));
    w.raw("}");
}

/// Emits a `Text` node (fragment context: `type, start, end, raw, data`).
/// Raw-content element text (`TextDecoding::Raw` — a nested `<script>`/
/// `<style>`) comes from a different canonical construction site whose
/// literal leads with the positions and puts `data` first:
/// `{start, end, type, data, raw}`. Only the fragment arm stages its header —
/// the raw arm is one text node per nested `<script>`/`<style>`, far too rare
/// to earn a staged run's inlined emitters (module doc, Staged runs).
fn write_text(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    if matches!(text.decoding, internal::TextDecoding::Raw) {
        w.span_start_end_object(ctx.positions, text.span.start, text.span.end);
        // `data`, then `raw`: a `Raw` decoding is no decode at all, so the two
        // are always the same bytes and the pair escapes them once.
        debug_assert_eq!(text.data(ctx.source), text.raw(ctx.source));
        w.raw(",\"type\":\"Text\",\"data\":");
        w.string_pair(text.raw(ctx.source), ",\"raw\":");
        w.raw("}");
        return;
    }
    // Staged burst; ends at the dynamic `raw` (module doc, Staged runs).
    let mut run = w.stage_run();
    run.raw("{\"type\":\"Text\",\"start\":");
    run.u32(ctx.pos(text.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(text.span.end));
    if ctx.positions.has_locations() {
        run.flush();
        w.span_loc(ctx.positions, text.span.start, text.span.end);
        w.raw(",\"raw\":");
    } else {
        run.raw(",\"raw\":");
        run.flush();
    }
    write_raw_then_data(w, text, ctx);
    w.raw("}");
}

/// A `Text`'s `raw` value, `,"data":`, then its `data` value — the tail every
/// `raw`-first `Text` shape shares.
///
/// `data` is `raw` whenever the text holds no `&` (nearly every template text), and
/// then the two values are the same bytes: [`JsonWriter::string_pair`] escapes them
/// once and copies the escaped form. Only a text whose `data` is decoded escapes two
/// strings.
///
/// A text made only of collapsible whitespace — the indentation between two tags,
/// over half of all texts — takes [`JsonWriter::string_pair_whitespace`], which knows
/// every byte's escape: `[ \t\n\r]` holds no `&`, so its `data` is its `raw`.
///
/// A flag test ahead of one of two out-of-line bodies, so neither pays for
/// the other's registers: inlined here, either body took callee-saved registers that
/// were saved at entry, ahead of the test, on every text.
fn write_raw_then_data(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    if text.is_collapsible_ws_only {
        write_whitespace_raw_then_data(w, text, ctx);
    } else {
        write_content_raw_then_data(w, text, ctx);
    }
}

/// [`write_raw_then_data`] for a text made only of collapsible whitespace: `raw` and
/// `data` as one [`JsonWriter::string_pair_whitespace`] window.
#[inline(never)]
fn write_whitespace_raw_then_data(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    // Bytes, not `raw`: a whitespace-only text is ASCII, so the `str` slice's
    // char-boundary checks buy nothing.
    w.string_pair_whitespace(&ctx.source.as_bytes()[text.raw_span.range()], b",\"data\":");
}

/// [`write_raw_then_data`] for a text that holds content: `raw` escaped once and copied
/// when `data` borrows it, two strings when `data` is decoded.
#[inline(never)]
fn write_content_raw_then_data(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    let raw = text.raw(ctx.source);
    // Asked inline rather than through `data()`: this is every raw-first `Text` with
    // content the writer emits (all but a raw-content element's, which `write_text`
    // writes itself).
    if text.data_is_raw(raw) {
        w.string_pair(raw, ",\"data\":");
    } else {
        w.string(raw);
        w.raw(",\"data\":");
        w.string(&text.data(ctx.source));
    }
}

/// A `Text` as Svelte's attribute-value sequence literal, `{start, end, type, raw, data}`
/// — an attribute value's text part, and a `<textarea>`'s content, which Svelte reads as
/// the same sequence.
fn write_sequence_text(w: &mut JsonWriter, text: &internal::Text, ctx: &Ctx<'_>) {
    w.span_start_end_object(ctx.positions, text.span.start, text.span.end);
    w.raw(",\"type\":\"Text\",\"raw\":");
    write_raw_then_data(w, text, ctx);
    w.raw("}");
}

/// Emits a `Comment` node (HTML `<!-- … -->`).
fn write_html_comment(w: &mut JsonWriter, comment: &internal::HtmlComment, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"Comment\",\"start\":");
    w.span_start_end(ctx.positions, comment.span.start, comment.span.end);
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
        w.span_start_end_object(ctx.positions, block.span.start, block.span.end);
        w.raw(",\"type\":\"IfBlock\",\"elseif\":true");
    } else {
        w.raw("{\"type\":\"IfBlock\",\"elseif\":false,\"start\":");
        w.span_start_end(ctx.positions, block.span.start, block.span.end);
    }
    let range_end = fragment_first_start(&block.consequent).unwrap_or(block.span.end);
    w.raw(",\"test\":");
    write_generic_island(w, block.test, block.span.start, range_end, ctx);
    w.raw(",\"consequent\":");
    write_fragment(w, &block.consequent, ctx);
    w.raw(",\"alternate\":");
    write_optional_fragment(w, block.alternate.as_ref(), ctx);
    w.raw("}");
}

/// Emits an `EachBlock` node. `context` is a pattern island
/// ([`write_pattern_island`] carries any binding/annotation comments);
/// `index`/`key`/`fallback` are skip-if-none.
fn write_each_block(w: &mut JsonWriter, block: &internal::EachBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"EachBlock\",\"start\":");
    w.span_start_end(ctx.positions, block.span.start, block.span.end);
    let range_end = fragment_first_start(&block.body).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_generic_island(w, block.expression, block.span.start, range_end, ctx);
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
        // The key's window is INSIDE its own parens, not at the block's `{#` — canonical
        // filters each parse's comments to `start >= index`, and the key's
        // `read_expression` begins after the `(` (only whitespace can sit between).
        // Anchored on the head, this window reaches back over the CONTEXT PATTERN and
        // claims a comment written inside it, which the pattern island also attaches. The
        // key's own `(`…`)` are block syntax, not a grouping pair around the root, so the
        // window stops short of both (no comment can follow the `)` before the `}`).
        write_generic_island(w, key.expression, key.span.start + 1, key.span.end - 1, ctx);
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
    w.span_start_end(ctx.positions, block.span.start, block.span.end);
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
    write_generic_island(w, block.expression, block.span.start, range_end, ctx);
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
    w.span_start_end(ctx.positions, block.span.start, block.span.end);
    let range_end = fragment_first_start(&block.fragment).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_generic_island(w, block.expression, block.span.start, range_end, ctx);
    w.raw(",\"fragment\":");
    write_fragment(w, &block.fragment, ctx);
    w.raw("}");
}

/// Emits a `SnippetBlock` node. The snippet name carries `character` (like a
/// shorthand attribute); `typeParams` is skip-if-none, right after
/// `expression` (Svelte assigns it before reading the parameters).
fn write_snippet_block(w: &mut JsonWriter, block: &internal::SnippetBlock<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"SnippetBlock\",\"start\":");
    w.span_start_end(ctx.positions, block.span.start, block.span.end);
    let range_end = fragment_first_start(&block.body).unwrap_or(block.span.end);
    w.raw(",\"expression\":");
    write_snippet_name(w, block.expression, block.span.start, range_end, ctx);
    if let Some(type_params) = block.type_params_raw {
        w.raw(",\"typeParams\":");
        w.string(type_params);
    }
    w.raw(",\"parameters\":");
    write_snippet_parameters(
        w,
        ctx.snippet_wire_parameters(block.span.start)
            .unwrap_or(block.parameters),
        block.span.start,
        range_end,
        ctx,
    );
    w.raw(",\"body\":");
    write_fragment(w, &block.body, ctx);
    w.raw("}");
}

/// The snippet name identifier — Svelte injects `character` into its `loc`. A
/// leading comment (`{#snippet /* c */ name(…)}`) can attach, so the
/// comment-bearing case runs the island's online attach; the comment-free common
/// case fuses directly.
fn write_snippet_name(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if ctx.any_comment_in(container_start, range_end) {
        let attach =
            attach_expression(ctx.attach_inputs(), container_start, expr.span(), range_end);
        write_identifier_expression_with_character(w, expr, ctx.embed(attach.mode()));
    } else {
        write_identifier_expression_with_character(w, expr, ctx.embed(CommentMode::Off));
    }
}

/// Snippet parameters. Comment-free (the common case): each fuses. Otherwise the
/// whole list shares ONE attach (`attach_expression_list` — one queue, each
/// inter-parameter comment claimed once per acorn's same-line rule) and every
/// parameter emits through it. No wrapper-end suppression: canonical parses the list in
/// a function context whose wrapper ends past every param.
fn write_snippet_parameters(
    w: &mut JsonWriter,
    parameters: &[tsv_ts::ast::internal::Expression<'_>],
    container_start: u32,
    range_end: u32,
    ctx: &Ctx<'_>,
) {
    if !parameters.is_empty() && ctx.any_comment_in(container_start, range_end) {
        // Svelte keeps this parse's grouping pairs on the wire (no `remove_parens`)
        let attach =
            attach_expression_list(ctx.attach_inputs(), container_start, range_end, None, false);
        write_array(w, parameters, |w, p| {
            write_expression_embedded(w, p, ctx.embed(attach.mode()));
        });
    } else {
        write_array(w, parameters, |w, p| {
            write_expression_embedded(w, p, ctx.embed(CommentMode::Off));
        });
    }
}

//
// Tags
//

/// Emits an `HtmlTag` node (`{@html expr}`).
fn write_html_tag(w: &mut JsonWriter, tag: &internal::HtmlTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"HtmlTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"expression\":");
    write_braced_island(w, tag.expression, tag.span, ctx);
    w.raw("}");
}

/// Emits a `RenderTag` node (`{@render expr}`).
fn write_render_tag(w: &mut JsonWriter, tag: &internal::RenderTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"RenderTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"expression\":");
    write_braced_island(w, tag.expression, tag.span, ctx);
    w.raw("}");
}

/// Emits a `DebugTag` node (`{@debug a, b}`).
///
/// A multi-identifier tag is ONE canonical acorn parse (a `SequenceExpression`
/// wrapper, discarded after identifier extraction), so its comment attach runs
/// once across the list with the wrapper-end trailing suppression, each element's own
/// grouping pairs run as silent frames. A single identifier is itself the parse root and
/// takes the generic-island path (root-fallback trailing).
fn write_debug_tag(w: &mut JsonWriter, tag: &internal::DebugTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"DebugTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"identifiers\":");
    // The internal entries keep what the author wrote — a JSDoc cast around the
    // whole comma list (`{@debug /** @type {B} */ (b, c)}`) is ONE entry so the
    // printer can reproduce the cast — but the wire is the canonical parse,
    // where `remove_parens` uncovers the sequence and the reader flattens it:
    // splice such an entry into the sequence's elements. The elements keep
    // their own casts (emission unwraps a cast to its inner node), and their
    // paren-inclusive spans are what give the discarded-wrapper bounds below
    // canonical's own values. The cast-free common case borrows the parsed
    // slice untouched.
    let spliced = if tag.identifiers.iter().any(|entry| {
        matches!(
            entry.unwrap_jsdoc_casts().kind,
            tsv_ts::ExpressionKind::SequenceExpression(_)
        )
    }) {
        let mut flat = Vec::with_capacity(tag.identifiers.len() + 1);
        for entry in tag.identifiers {
            match &entry.unwrap_jsdoc_casts().kind {
                tsv_ts::ExpressionKind::SequenceExpression(seq) => {
                    flat.extend(seq.expressions.iter().map(|e| (*e).clone()));
                }
                _ => flat.push(entry.clone()),
            }
        }
        Some(flat)
    } else {
        None
    };
    let identifiers = spliced.as_deref().unwrap_or(tag.identifiers);
    // `[_, _, ..]` is the multi-identifier case: a single identifier (which the pattern
    // excludes) has no wrapper at all.
    if let [_, _, ..] = identifiers
        && ctx.any_comment_in(tag.span.start, tag.span.end)
    {
        // Under `preserveParens` each element the sequence holds is the element's own
        // outermost grouping pair, if it has one (`{@debug (a), b}`) — `remove_parens` runs
        // only after the walk — so the pairs tsv's parse discarded are read back off the
        // source, run as silent frames around their element (each claims as the pair would
        // and emits nothing), and bound the discarded wrapper: it spans the first element's
        // `(` to the last element's `)`, not first identifier to last.
        let attach_inputs = ctx.attach_inputs();
        let element_parens: Vec<Vec<Span>> = identifiers
            .iter()
            .map(|id| {
                grouping_parens_around(attach_inputs, tag.span.start, id.span(), tag.span.end)
            })
            .collect();
        let outer = |i: usize| {
            element_parens[i]
                .first()
                .copied()
                .unwrap_or_else(|| identifiers[i].span())
        };
        let wrapper = Span::new(outer(0).start, outer(identifiers.len() - 1).end);
        let attach = attach_expression_list(
            attach_inputs,
            tag.span.start,
            tag.span.end,
            Some(wrapper),
            true,
        );
        let mode = attach.mode();
        write_array(
            w,
            identifiers.iter().zip(&element_parens),
            |w, (id, parens)| {
                mode.with_silent_parens(parens, || {
                    write_expression_embedded(w, id, ctx.embed(mode));
                });
            },
        );
    } else {
        write_array(w, identifiers, |w, id| {
            write_braced_island(w, id, tag.span, ctx);
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
/// document fuses directly; a document with template comments builds one attach per
/// canonical acorn parse — `read_pattern`'s synthetic `pattern = 1` for the id (plus
/// its `: T` annotation's own, when typed) and `read_expression`'s for the init — so a
/// comment in one window can never reach another's tree.
fn write_const_tag(w: &mut JsonWriter, tag: &internal::ConstTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"ConstTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"declaration\":");
    // The declaration `end` is always `tag.span.end - 1` — canonical Svelte
    // hard-codes `parser.index - 1` (the byte before the closing `}`).
    let decl_end = tag.span.end - 1;
    let declarator_end = const_declarator_end(tag, ctx);
    // Scoped comment pre-check: a comment attaching to this tag necessarily starts
    // inside its span, so no comment in `[tag.span.start, tag.span.end)` means every
    // attach queue would be empty — fuse directly (Off ≡ an empty queue).
    if !ctx.any_comment_in(tag.span.start, tag.span.end) {
        write_const_declaration(
            w,
            tag,
            decl_end,
            declarator_end,
            (CommentMode::Off, CommentMode::Off),
            CommentMode::Off,
            ctx,
        );
    } else {
        // The document has template comments: each of the tag's acorn parses takes its
        // own attach — the id's (pattern, annotation) pair, then the init, split at the
        // end of the binding (see `attach_const_tag_init`).
        let id_attach = attach_binding_pattern(tag.id, ctx.attach_inputs());
        let init_attach = attach_const_tag_init(tag, ctx.attach_inputs());
        write_const_declaration(
            w,
            tag,
            decl_end,
            declarator_end,
            id_attach.modes(),
            init_attach.mode(),
            ctx,
        );
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
            if !is_svelte_ws(ch) {
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
/// declaration's `end` byte (`tag.span.end - 1`) and `declarator_end` the
/// declarator's (`const_declarator_end`).
///
/// The id and the init take **separate** attaches because canonical runs them as
/// two acorn parses with two comment sets — a single one would let a comment in
/// the id's window reach the init's tree (see `attach_const_tag_init`).
fn write_const_declaration(
    w: &mut JsonWriter,
    tag: &internal::ConstTag<'_>,
    decl_end: u32,
    declarator_end: u32,
    (id_mode, id_annotation_mode): (CommentMode<'_>, CommentMode<'_>),
    init_mode: CommentMode<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw(
        "{\"type\":\"VariableDeclaration\",\"kind\":\"const\",\"declarations\":[{\"type\":\"VariableDeclarator\",\"id\":",
    );
    write_pattern_embedded(w, tag.id, ctx.embed_pattern(id_mode, id_annotation_mode));
    w.raw(",\"init\":");
    write_expression_embedded(w, tag.init, ctx.embed(init_mode));
    w.span_start_end_field(ctx.positions, tag.id.span().start, declarator_end);
    w.raw("}],\"start\":");
    w.span_start_end(ctx.positions, tag.span.start + 2, decl_end);
    w.raw("}");
}

/// Emits a `DeclarationTag` node (`{const …}` / `{let …}`).
///
/// The `declaration` is a real TS `VariableDeclaration`, emitted with its own
/// span `end` in both states (canonical keeps acorn's end for DeclarationTag —
/// unlike `ConstTag`, no `-1` rewrite). The comment-free document fuses via
/// `write_variable_declaration_embedded`; a comment-bearing one runs the island's
/// online attach over the whole tag, so comments attach across the **whole**
/// `VariableDeclaration` tree (every declarator and its id/init) per acorn's
/// recursive attachment — attaching only to the first init left a comment leading
/// a later declarator (`{let a = 1, /* c */ b}`) unattached.
fn write_declaration_tag(w: &mut JsonWriter, tag: &internal::DeclarationTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"DeclarationTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"declaration\":");
    // Scoped comment pre-check (see `write_const_tag`): no comment inside this
    // tag's span means the attach queue is empty, so fuse directly.
    if !ctx.any_comment_in(tag.span.start, tag.span.end) {
        write_variable_declaration_embedded(w, &tag.declaration, ctx.embed(CommentMode::Off));
    } else {
        let attach = attach_statement(
            ctx.attach_inputs(),
            tag.span.start,
            tag.declaration.span.end,
            tag.span.end,
        );
        write_variable_declaration_embedded(w, &tag.declaration, ctx.embed(attach.mode()));
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
    // Staged burst; ends before the `name` field (module doc, Staged runs).
    let mut run = w.stage_run();
    run.raw("{\"type\":\"Attribute\",\"start\":");
    run.u32(ctx.pos(attr.span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(attr.span.end));
    run.flush();
    w.span_loc(ctx.positions, attr.span.start, attr.span.end);
    write_scanned_name_field(w, name_bytes(attr.name_span, ctx));
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, attr.name_span, lines);
    }
    // A boolean attribute's `true` closes the node in the same constant.
    let Some(values) = attr.value else {
        w.raw(",\"value\":true}");
        return;
    };
    w.raw(",\"value\":");
    write_attribute_value_field(w, values, ctx);
    w.raw("}");
}

/// Emit the value an attribute was written with — a boolean attribute has none, and
/// [`write_attribute`] writes its `true`: a bare `{expr}` (plain object), or a
/// text/quoted sequence (array).
fn write_attribute_value_field(
    w: &mut JsonWriter,
    values: &[internal::AttributeValue<'_>],
    ctx: &Ctx<'_>,
) {
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
                if matches!(&tag.expression.kind, tsv_ts::ast::internal::ExpressionKind::Identifier(id)
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
        internal::AttributeValue::Text(text) => write_sequence_text(w, text, ctx),
        internal::AttributeValue::ExpressionTag(tag) => write_expression_tag(w, tag, ctx),
    }
}

/// Emits a `SpreadAttribute` node (`{...expr}`).
fn write_spread_attribute(
    w: &mut JsonWriter,
    spread: &internal::SpreadAttribute<'_>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"SpreadAttribute\",\"start\":");
    w.span_start_end(ctx.positions, spread.span.start, spread.span.end);
    w.raw(",\"expression\":");
    write_braced_island(w, spread.expression, spread.span, ctx);
    w.raw("}");
}

/// Emits an `AttachTag` node (`{@attach expr}`).
fn write_attach_tag(w: &mut JsonWriter, tag: &internal::AttachTag<'_>, ctx: &Ctx<'_>) {
    w.raw("{\"type\":\"AttachTag\",\"start\":");
    w.span_start_end(ctx.positions, tag.span.start, tag.span.end);
    w.raw(",\"expression\":");
    write_braced_island(w, tag.expression, tag.span, ctx);
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
    w.span_start_end_object(ctx.positions, span.start, span.end);
    w.raw(",\"type\":\"");
    w.raw(node_type);
    w.raw("\",\"name\":");
    w.string(name_span.extract(ctx.source));
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, head_span, lines);
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
        write_braced_island(w, e, span, ctx);
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
            write_optional_directive_expression(w, d.expression, d.span, ctx);
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
    write_optional_directive_expression(w, d.expression, d.span, ctx);
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
/// is a synthetic `Identifier` with Svelte field order (`start, end, type,
/// name`) that never carries a comment, emitted fused.
fn write_directive_value_expression(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    has_expression_tag: bool,
    span: Span,
    ctx: &Ctx<'_>,
) {
    if has_expression_tag {
        write_braced_island(w, expr, span, ctx);
    } else {
        // Shorthand: the parser builds this as a synthetic `Identifier`.
        #[expect(clippy::unreachable)]
        let tsv_ts::ast::internal::ExpressionKind::Identifier(id) = &expr.kind else {
            unreachable!("shorthand directive expression is always an Identifier");
        };
        w.span_start_end_object(ctx.positions, id.span.start, id.span.end);
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
        d.expression,
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
        d.expression,
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

/// A `<style>` `StyleSheet`. `children` fuse via `tsv_css`'s
/// `write_css_children`, which hands back the sibling `comments` run its walk
/// gathered; `attributes` and the preceding comment fuse too (the `<style>`
/// envelope is never visited by the template attach passes).
fn write_style_sheet(
    w: &mut JsonWriter,
    style: &internal::Style<'_>,
    preceding_comment: Option<&internal::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"StyleSheet\",\"start\":");
    w.span_start_end(ctx.positions, style.span.start, style.span.end);
    w.raw(",\"attributes\":");
    write_value_attributes(w, style.attributes, ctx);
    w.raw(",\"children\":");
    let css_comments = write_css_children(w, &style.css_stylesheet, ctx.source, ctx.positions);
    w.raw(",\"comments\":");
    write_css_comments(w, &css_comments, ctx.source, ctx.positions);
    w.raw(",\"content\":{\"start\":");
    w.span_start_end(
        ctx.positions,
        style.content_span.start,
        style.content_span.end,
    );
    w.raw(",\"styles\":");
    w.string(style.content_span.extract(ctx.source));
    w.raw(",\"comment\":");
    // Same `{type:"Comment", start, end, data}` shape as a fragment HTML comment.
    write_or_null(w, preceding_comment, |w, c| write_html_comment(w, c, ctx));
    w.raw("}}");
}

/// A `<script>` block. `content` always fuses via `write_program_embedded`; the
/// parser variant and (when needed) the island's online comment attach handle the
/// acorn quirks:
///
/// - **Parser variant**: component-global (Svelte's single `this.ts`), read through
///   `Ctx::embed` like every expression island's — a TS component emits acorn-typescript's
///   wire for *every* script, a non-TS one vanilla acorn's (omit
///   `importKind`/`exportKind="value"`, always emit `attributes`, and the expression
///   quirks — see `Ctx::vanilla_acorn` in `tsv_ts`).
/// - **Comments**: a script whose `Program` carries comments (its own or a
///   preceding HTML comment) runs acorn's leading/trailing attach online off this
///   emit (`attach_script`); the common comment-free case fuses without one.
///
/// The `Program`'s `loc` is its content span's, like every other node's — Svelte's own
/// sits at the `<script>` tag (`read/script.js`, for its sourcemaps), which tsv does not
/// reproduce.
fn write_script(
    w: &mut JsonWriter,
    script: &internal::Script<'_>,
    html_leading_comment: Option<&internal::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    w.raw("{\"type\":\"Script\",\"start\":");
    w.span_start_end(ctx.positions, script.span.start, script.span.end);
    w.raw(",\"context\":");
    // Escape-free `&'static str` (`default` / `module`) → skip the escape scan.
    w.token(script.context.as_str());
    w.raw(",\"content\":");
    // A script whose `Program` carries comments (its own or a preceding HTML
    // comment) needs acorn's leading/trailing attach, which runs online off this
    // emit's own node opens and closes. The common case (no comments) fuses with
    // no attach at all.
    let attach = if script.content.comments.is_empty() && html_leading_comment.is_none() {
        None
    } else {
        Some(attach_script(
            script,
            ctx.attach_inputs(),
            html_leading_comment,
        ))
    };
    let mode = attach
        .as_ref()
        .map_or(CommentMode::Off, CommentAttach::mode);
    write_program_embedded(w, &script.content, ctx.embed(mode));
    w.raw(",\"attributes\":");
    write_value_attributes(w, script.attributes, ctx);
    w.raw("}");
}

/// A `<svelte:options>`: everything fuses. Field order: `start, end,
/// attributes` then the skip-if-none `runes, immutable, css, accessors,
/// preserveWhitespace, namespace, customElement` (no `type`).
fn write_svelte_options(w: &mut JsonWriter, options: &internal::SvelteOptions<'_>, ctx: &Ctx<'_>) {
    let attrs = options.attributes;
    w.span_start_end_object(ctx.positions, options.span.start, options.span.end);
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
    use tsv_ts::ast::internal::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    w.raw("{");
    let mut first_prop = true;
    for prop in props_obj.properties {
        let ObjectProperty::Property(p) = prop else {
            continue;
        };
        let (
            Expression {
                kind: ExpressionKind::Identifier(key),
                ..
            },
            Expression {
                kind: ExpressionKind::ObjectExpression(inner),
                ..
            },
        ) = (&p.key, &p.value)
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
            let (
                Expression {
                    kind: ExpressionKind::Identifier(ikey),
                    ..
                },
                Expression {
                    kind: ExpressionKind::Literal(lit),
                    ..
                },
            ) = (&ip.key, &ip.value)
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
    use tsv_ts::ast::internal::{Expression, ExpressionKind, LiteralValue, ObjectProperty};
    let Some(values) = find_option_values(attrs, "customElement", ctx.source) else {
        return;
    };
    for v in values {
        // `customElement={{ tag: '…', props: {…}, shadow: …, extend: … }}`
        if let internal::AttributeValue::ExpressionTag(expr) = v
            && let ExpressionKind::ObjectExpression(obj) = &expr.expression.kind
        {
            let mut tag: Option<&Expression<'_>> = None;
            let mut props: Option<&Expression<'_>> = None;
            let mut shadow: Option<&Expression<'_>> = None;
            let mut extend: Option<&Expression<'_>> = None;
            for prop in obj.properties {
                if let ObjectProperty::Property(p) = prop
                    && let ExpressionKind::Identifier(key) = &p.key.kind
                {
                    let slot = match key.name(ctx.source) {
                        "tag" => &mut tag,
                        "props" => &mut props,
                        "shadow" => &mut shadow,
                        "extend" => &mut extend,
                        _ => continue,
                    };
                    if slot.is_none() {
                        *slot = Some(p.value);
                    }
                }
            }

            w.raw(",\"customElement\":{");
            let mut first = true;
            // `tag`: the string-literal value.
            if let Some(Expression {
                kind: ExpressionKind::Literal(lit),
                ..
            }) = tag
                && let LiteralValue::String(cooked) = &lit.value
            {
                json_comma(w, &mut first);
                w.raw("\"tag\":");
                w.string(cooked.resolve(lit.span, ctx.source));
            }
            // `props`: statically-evaluated plain object.
            if let Some(Expression {
                kind: ExpressionKind::ObjectExpression(props_obj),
                ..
            }) = props
            {
                json_comma(w, &mut first);
                w.raw("\"props\":");
                write_custom_element_props(w, props_obj, ctx);
            }
            // `shadow`: the string `'open'`/`'none'`, or the raw `ObjectExpression` AST.
            match shadow {
                Some(Expression {
                    kind: ExpressionKind::Literal(lit),
                    ..
                }) => {
                    if let LiteralValue::String(cooked) = &lit.value {
                        json_comma(w, &mut first);
                        w.raw("\"shadow\":");
                        w.string(cooked.resolve(lit.span, ctx.source));
                    }
                }
                Some(
                    shadow_expr @ Expression {
                        kind: ExpressionKind::ObjectExpression(_),
                        ..
                    },
                ) => {
                    json_comma(w, &mut first);
                    w.raw("\"shadow\":");
                    write_expression_embedded(w, shadow_expr, ctx.embed(CommentMode::Off));
                }
                _ => {}
            }
            // `extend`: the raw expression AST.
            if let Some(extend_expr) = extend {
                json_comma(w, &mut first);
                w.raw("\"extend\":");
                write_expression_embedded(w, extend_expr, ctx.embed(CommentMode::Off));
            }
            w.raw("}");
            return;
        }
        // Plain text or string literal: `customElement="tag-name"` → `{tag}`.
        let tag_str = match v {
            internal::AttributeValue::Text(text) => Some(text.data(ctx.source)),
            internal::AttributeValue::ExpressionTag(expr) => {
                if let ExpressionKind::Literal(lit) = &expr.expression.kind
                    && let LiteralValue::String(cooked) = &lit.value
                {
                    Some(Cow::Borrowed(cooked.resolve(lit.span, ctx.source)))
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
/// emitted fused via `tsv_ts`'s `write_pattern_embedded` (a simple identifier's
/// `character` in final char space).
///
/// Patterns DO collect comments — `parse_ts_pattern` and, for `{#each}`, the
/// separately-read `parse_ts_type_annotation` both extend `expression_comments`
/// — and canonical attaches each one to its adjacent node inside the subtree of
/// the parse that read it, so a comment-bearing binding attaches over the same two
/// windows `{@const}`'s id takes (`attach_binding_pattern`: the pattern, then its
/// `: T`). The pre-check is the union of the two rather than the enclosing
/// block's span: a comment attaching here
/// necessarily starts inside the binding, and asking wider would only build an
/// attach with nothing in its queue.
fn write_pattern_island(
    w: &mut JsonWriter,
    expr: &tsv_ts::ast::internal::Expression<'_>,
    ctx: &Ctx<'_>,
) {
    let window = pattern_comment_window(expr);
    if !ctx.any_comment_in(window.start, window.end) {
        write_pattern_embedded(
            w,
            expr,
            ctx.embed_pattern(CommentMode::Off, CommentMode::Off),
        );
        return;
    }
    let attach = attach_binding_pattern(expr, ctx.attach_inputs());
    let (mode, annotation_mode) = attach.modes();
    write_pattern_embedded(w, expr, ctx.embed_pattern(mode, annotation_mode));
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

// The tests that read `name_loc` / `loc` need `tsv_lang`'s `locations` feature — on in every
// workspace-wide test run (`tsv_cli` and `tsv_debug` enable it), off in a bare
// `cargo test -p tsv_svelte` — and are gated on it one by one; the rest grade the span-only
// wire and run either way.
#[cfg(test)]
mod tests {
    use serde_json::Value;

    /// Parse full Svelte source and return its span-only wire.
    fn convert_svelte(source: &str) -> Value {
        let arena = bumpalo::Bump::new();
        // Test inputs are hardcoded valid sources; a parse failure should panic
        let root = crate::parse(source, &arena).expect("parse");
        serde_json::from_slice(&crate::convert_ast_json_bytes(&root, source)).expect("wire")
    }

    /// Parse full Svelte source and return its loc-bearing wire.
    #[cfg(feature = "locations")]
    fn convert_svelte_with_locations(source: &str) -> Value {
        let arena = bumpalo::Bump::new();
        let root = crate::parse(source, &arena).expect("parse");
        serde_json::from_slice(&crate::convert_ast_json_bytes_with_locations(&root, source))
            .expect("wire")
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

    /// Every node's `name_loc` from the wire, as `(start, end)` `{line, column, character}`.
    #[cfg(feature = "locations")]
    fn name_locs(node: &Value, out: &mut Vec<(Value, Value)>) {
        match node {
            Value::Object(fields) => {
                if let Some(loc) = fields.get("name_loc") {
                    out.push((loc["start"].clone(), loc["end"].clone()));
                }
                fields.values().for_each(|v| name_locs(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| name_locs(v, out)),
            _ => {}
        }
    }

    /// `write_name_loc_field` resolves only a name's start to a line and derives the end
    /// from it — sound because no name span holds a `\n`, the one byte Svelte's line table
    /// opens a line at. Graded here over every name shape that carries a `name_loc`
    /// (element, component, special element, attribute, directive head with modifiers),
    /// each written directly before and after every line-terminator spelling of either
    /// class — `\n`, `\r`, `\r\n`, `<LS>`, `<PS>` — and after a multibyte name or a
    /// multibyte line ahead of it: each endpoint must sit at the line and column an
    /// independent count over the source's UTF-16 units gives its `character` (a line
    /// opens after each `\n`), and each end on its start's line. The oracle is the test's
    /// own, so the grade holds without debug assertions; the writer's debug assertion also
    /// re-derives both endpoints the unfused way on every name the test suite parses.
    #[test]
    #[cfg(feature = "locations")]
    fn name_loc_end_shares_its_start_line() {
        for term in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}", "\t", " "] {
            for lead in ["", "é\n", "😀", "\r\n\n"] {
                let sources = [
                    format!("{lead}<div{term}class=\"a\"{term}id={term}b></div>"),
                    format!("{lead}<Foo.Bar{term}x{term}/>"),
                    format!("{lead}<svelte:head{term}></svelte:head>"),
                    format!("{lead}<a{term}on:click|once|preventDefault={{f}}{term}>é</a>"),
                    format!("{lead}<p{term}bind:value={{v}}{term}class:é={{c}}></p>"),
                    format!("{lead}<input{term}data-é{term}/>"),
                    format!("{lead}<div{term}{{x}}{term}{{é}}></div>"),
                ];
                for source in &sources {
                    let units: Vec<u16> = source.encode_utf16().collect();
                    let mut locs = Vec::new();
                    name_locs(&convert_svelte_with_locations(source), &mut locs);
                    assert!(!locs.is_empty(), "{source:?} carries a name_loc");
                    for (start, end) in locs {
                        assert_eq!(start["line"], end["line"], "{source:?}");
                        for point in [&start, &end] {
                            let character =
                                point["character"].as_u64().expect("character") as usize;
                            let before = &units[..character];
                            let line_start = before
                                .iter()
                                .rposition(|&u| u == u16::from(b'\n'))
                                .map_or(0, |i| i + 1);
                            let line =
                                1 + before.iter().filter(|&&u| u == u16::from(b'\n')).count();
                            assert_eq!(point["line"], line, "{source:?} {point}");
                            assert_eq!(
                                point["column"],
                                character - line_start,
                                "{source:?} {point}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// One element holding each shape the element and attribute writers tell apart.
    const SHAPES: &str = concat!(
        r#"<div a b="c" d="{e}" f={g} {h} i="j{k}">t{x}<!--c-->"#,
        r#"<textarea>y{z}</textarea><textarea></textarea><Foo /></div>"#,
    );

    /// The element and attribute writers fold the constants around their dynamic parts —
    /// an empty attribute list, a boolean attribute's `true`, a lone text's array, each
    /// header's positions — so the bytes are pinned whole, for every such shape at once:
    /// a boolean, lone-text, quoted-tag, bare-tag, shorthand and mixed attribute; a text,
    /// a tag and a comment child; a `<textarea>` with a sequence text and an empty one;
    /// a childless component.
    #[test]
    fn element_and_attribute_shapes_write_these_bytes() {
        let arena = bumpalo::Bump::new();
        let root = crate::parse(SHAPES, &arena).expect("parse");
        let wire = crate::convert_ast_json_bytes(&root, SHAPES);
        let expected = concat!(
            r#"{"css":null,"js":[],"start":0,"end":111,"type":"Root","fragment":{"type":"Fragment","#,
            r#""nodes":[{"type":"RegularElement","start":0,"end":111,"name":"div","#,
            r#""attributes":[{"type":"Attribute","start":5,"end":6,"name":"a","value":true},"#,
            r#"{"type":"Attribute","start":7,"end":12,"name":"b","value":[{"start":10,"end":11,"#,
            r#""type":"Text","raw":"c","data":"c"}]},"#,
            r#"{"type":"Attribute","start":13,"end":20,"name":"d","value":[{"type":"ExpressionTag","#,
            r#""start":16,"end":19,"expression":{"type":"Identifier","start":17,"end":18,"#,
            r#""name":"e"}}]},"#,
            r#"{"type":"Attribute","start":21,"end":26,"name":"f","value":{"type":"ExpressionTag","#,
            r#""start":23,"end":26,"expression":{"type":"Identifier","start":24,"end":25,"name":"g"}}},"#,
            r#"{"type":"Attribute","start":27,"end":30,"name":"h","value":{"type":"ExpressionTag","#,
            r#""start":28,"end":29,"expression":{"type":"Identifier","name":"h","start":28,"end":29}}},"#,
            r#"{"type":"Attribute","start":31,"end":39,"name":"i","value":[{"start":34,"end":35,"#,
            r#""type":"Text","raw":"j","data":"j"},"#,
            r#"{"type":"ExpressionTag","start":35,"end":38,"expression":{"type":"Identifier","#,
            r#""start":36,"end":37,"name":"k"}}]}],"fragment":{"type":"Fragment","#,
            r#""nodes":[{"type":"Text","start":40,"end":41,"raw":"t","data":"t"},"#,
            r#"{"type":"ExpressionTag","start":41,"end":44,"expression":{"type":"Identifier","#,
            r#""start":42,"end":43,"name":"x"}},"#,
            r#"{"type":"Comment","start":44,"end":52,"data":"c"},"#,
            r#"{"type":"RegularElement","start":52,"end":77,"name":"textarea","attributes":[],"#,
            r#""fragment":{"type":"Fragment","nodes":[{"start":62,"end":63,"type":"Text","raw":"y","#,
            r#""data":"y"},"#,
            r#"{"type":"ExpressionTag","start":63,"end":66,"expression":{"type":"Identifier","#,
            r#""start":64,"end":65,"name":"z"}}]}},"#,
            r#"{"type":"RegularElement","start":77,"end":98,"name":"textarea","attributes":[],"#,
            r#""fragment":{"type":"Fragment","nodes":[]}},"#,
            r#"{"type":"Component","start":98,"end":105,"name":"Foo","attributes":[],"#,
            r#""fragment":{"type":"Fragment","nodes":[]}}]}}]},"options":null,"comments":[]}"#
        );
        assert_eq!(String::from_utf8(wire).expect("UTF-8 wire"), expected);
    }

    /// `node` with every position moved `by` code units along its line: `start`, `end`,
    /// `character` and `column`, wherever one is a number.
    fn shifted(node: &Value, by: u64) -> Value {
        match node {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| {
                        let moved = match (key.as_str(), value.as_u64()) {
                            ("start" | "end" | "character" | "column", Some(n)) => {
                                Value::from(n + by)
                            }
                            _ => shifted(value, by),
                        };
                        (key.clone(), moved)
                    })
                    .collect(),
            ),
            Value::Array(items) => Value::Array(items.iter().map(|v| shifted(v, by)).collect()),
            other => other.clone(),
        }
    }

    /// Every header writes its `start` / `end` into a fixed-width window, so each is
    /// walked across every decimal width a document reaches: the same element behind a
    /// one-line lead of every length that puts one of its offsets on a power of ten must
    /// be the unpadded element with its positions moved by the lead — in both wire
    /// variants. (A value past eight digits takes the writer's own wide arm, graded with
    /// `JsonWriter::start_end_head`.)
    #[test]
    fn positions_survive_every_digit_width() {
        let wires = |source: &str| {
            let arena = bumpalo::Bump::new();
            let root = crate::parse(source, &arena).expect("parse");
            [
                #[cfg(feature = "locations")]
                crate::convert_ast_json_bytes_with_locations(&root, source),
                crate::convert_ast_json_bytes(&root, source),
            ]
            .map(|wire| serde_json::from_slice::<Value>(&wire).expect("wire"))
        };
        let bare = wires(SHAPES);
        let lead_shell = "<i></i>".len();
        for power in [10usize, 100, 1_000, 10_000, 100_000] {
            let widest = power + 1;
            let narrowest = power.saturating_sub(SHAPES.len() + 1).max(lead_shell);
            for lead in narrowest..=widest {
                let source = format!("<i>{}</i>{SHAPES}", "p".repeat(lead - lead_shell));
                for (padded, bare) in wires(&source).iter().zip(&bare) {
                    assert_eq!(
                        padded["fragment"]["nodes"][1],
                        shifted(&bare["fragment"]["nodes"][0], lead as u64),
                        "lead {lead}"
                    );
                }
            }
        }
    }

    /// `write_name_loc_field` writes the end's line as the start's digits again: graded
    /// at each decimal width of the line, over every name shape, against the line the
    /// source puts the name on.
    #[test]
    #[cfg(feature = "locations")]
    fn name_loc_line_repeats_at_every_digit_width() {
        for line in [
            1usize, 9, 10, 99, 100, 999, 1_000, 9_999, 10_000, 99_999, 100_000,
        ] {
            let source = format!(
                "{}<div class=\"a\" {{b}} on:click|once={{f}}><Foo.Bar /></div><svelte:head></svelte:head>",
                "\n".repeat(line - 1)
            );
            let mut locs = Vec::new();
            name_locs(&convert_svelte_with_locations(&source), &mut locs);
            assert_eq!(locs.len(), 6, "line {line}");
            for (start, end) in locs {
                assert_eq!(start["line"], line, "line {line}");
                assert_eq!(end["line"], line, "line {line}");
            }
        }
    }

    /// `write_raw_then_data` hands [`JsonWriter::string_pair_whitespace`] every text whose
    /// `is_collapsible_ws_only` flag is set, and that window's escape table restates the
    /// flag's class, `[ \t\n\r]`, because `tsv_lang` cannot see this crate's
    /// [`is_collapsible_ws`](crate::ast::internal::is_collapsible_ws). Graded here, where
    /// both are in reach: every byte the class admits must write byte-identical to
    /// `serde_json` — a lone byte and a run on the window's arm, a run on its long arm, each
    /// into a full buffer and one with room — so a widened class fails this test rather
    /// than writing an unescaped control byte into a release build's wire.
    #[test]
    fn whitespace_window_covers_the_collapsible_class() {
        for byte in (0..=u8::MAX).filter(|&b| crate::ast::internal::is_collapsible_ws(b)) {
            for len in [1, 16, 17] {
                let text = String::from_utf8(vec![byte; len]).expect("an ASCII run");
                let quoted = serde_json::to_string(&text).expect("serde_json serializes a str");
                let expected = format!("{quoted},\"data\":{quoted}");
                for cap in [0, 256] {
                    let mut w = super::JsonWriter::with_capacity(cap);
                    w.string_pair_whitespace(text.as_bytes(), b",\"data\":");
                    assert_eq!(
                        String::from_utf8(w.into_bytes()).expect("UTF-8 wire"),
                        expected,
                        "{byte:#04x} x {len} (capacity {cap})"
                    );
                }
            }
        }
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
