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
//! `Attribute` / element / `Text` node headers, the JSON mount's (`write_json.rs`'s
//! [`write_element_header`], [`write_attribute_header`], [`write_text_header`]) — assemble in the writer's scratch
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
//! `ExpressionTag` and the directive heads are deliberately left unstaged: their `start`/`end`
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
//! the shape the canonical Svelte parser's `expected.json` records; key order is
//! tsv's own, one per node type (docs/conformance_svelte.md §Key Order).
//!
//! # One walk, two products
//!
//! This file is mounted once per sink: as `convert::write`, the JSON copy, and behind `_tape`
//! as `convert::write_tape::write`, the tape copy (`docs/tape_format.md` §A direct writer),
//! which writes a document's whole tape — its `<script>`s and template expressions through
//! `tsv_ts`'s tape copy (`__private`'s `*_tape` embedded writers), its `<style>` through
//! `tsv_css`'s — so the tape is one document, its refs true offsets into the source less a
//! leading byte-order mark, as the wire's offsets are. `tsv_ts`'s convention, unchanged: a site
//! states its JSON literal once, through a site macro (`write_macros.rs`), naming the entry its
//! keys belong to; the JSON copy writes the literal, the tape copy replays the ops a
//! compile-time reader made of it (`tsv_lang::tape::literal_ops`), and nothing is looked up by
//! name at run time. What a Svelte head adds is the macros' module doc's: a record whose keys
//! lead its `type` (`Root`, the directives, `<svelte:options>`) opens at its `{`, and a head's
//! `start` / `end` are written as they lie. Each mount provides by name what it writes its own
//! way (`write_json.rs`, `write_tape.rs`): the staged heads, the name fields, a text's `raw` /
//! `data`, a root comment, `customElement`'s data, and its sink's list helpers and embedded
//! writers. `cargo test --test tape_roundtrip` grades the tape copy against the JSON copy over
//! every Svelte fixture document.

// The functions each mount writes its own way, and the embedded `tsv_ts` / `tsv_css` writers of
// its sink (`write_json.rs`, `write_tape.rs`).
use super::comment_attachment::{
    AttachInputs, attach_binding_pattern, attach_const_tag_init, attach_expression,
    attach_expression_list, attach_script, attach_statement, grouping_parens_around,
    is_template_comment, pattern_comment_window,
};
use super::special::{bool_option, component_is_typescript, find_option_values, text_value};
use super::{
    write_array, write_attribute_header, write_css_children, write_css_comments,
    write_custom_element_field, write_element_header, write_element_name_field,
    write_expression_embedded, write_identifier_expression_with_character, write_name_loc_field,
    write_or_null, write_pattern_embedded, write_program_embedded, write_raw_then_data,
    write_root_comment, write_scanned_name_field, write_text_header, write_value_array,
    write_variable_declaration_embedded,
};
use crate::ast;
use crate::whitespace::is_svelte_ws;
use tsv_lang::store::Run;
use tsv_lang::{Comment, Span, Wire, WirePositions, WireTables};
use tsv_ts::__private::{CommentAttach, CommentMode, EmbedWriter};

json_only! {
    /// The JSON copy's sink.
    pub(super) type Sink<'a> = tsv_lang::JsonWriter;
}
tape! {
    /// The tape copy's sink.
    pub(super) type Sink<'a> = tsv_lang::tape::TapeWriter<'a>;
}

/// The document's `Root` and the context every writer function shares, built once at each
/// copy's entry — `$root`, `$comments` (the template comments `$ctx` borrows) and `$ctx`
/// named by the entry, which opens its own sink after. A macro rather than a shared function,
/// so the JSON copy's entry is the one body it would be with no tape.
macro_rules! document_ctx {
    ($parsed:ident, $source:ident, $positions:expr => $root:ident, $comments:ident, $ctx:ident) => {
        let $root = $parsed.root;
        // Template comments (outside `<script>` content spans) are the only comments
        // the template attach passes move; everything else stays where it is.
        let script_spans = crate::script_content_spans($root);
        let $comments: Vec<&Comment> = $root
            .comments
            .iter()
            .filter(|c| is_template_comment(c, &script_spans))
            .collect();

        let $ctx = Ctx {
            source: $source,
            positions: $positions,
            svelte: $parsed.view,
            view: $parsed.view.ts(),
            css: $parsed.view.css(),
            snippet_wire_parameters: $parsed.view.wire_params($root.snippet_wire_parameters),
            comments: &$comments,
            // Component-global: `lang="ts"` on any script makes *every* script emit the
            // acorn-typescript wire shape (Svelte's single `this.ts` flag).
            component_is_ts: component_is_typescript($parsed.view, $root, $source),
        };
    };
}

json_only! {
    /// Convert an internal Svelte `Root` straight to its compact wire-JSON bytes.
    ///
    /// One AST walk, no intermediate `serde_json::Value` for the spine. On `Wire::Span`
    /// it writes the span-only wire every binding ships — only `start`/`end` offsets, no
    /// `loc` and no `name_loc`; on `Wire::Loc` it adds every line/column object, the
    /// loc-bearing wire `tsv parse --locations` prints.
    pub(crate) fn write_root_bytes(
        parsed: ast::ParsedRootRef<'_>,
        source: &str,
        wire: Wire,
    ) -> Vec<u8> {
        // One set of tables for the whole document — template, every acorn island,
        // `<script>` bodies, root comments and `<style>` — in the coordinates
        // `crate::WIRE_COORDINATES` states.
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
        document_ctx!(parsed, source, tables.positions() => root, template_comments, ctx);
        let capacity = tsv_lang::estimated_json_capacity(source.len(), wire);
        let mut w = tsv_lang::JsonWriter::with_capacity(capacity);
        write_root(&mut w, root, &ctx);
        w.into_bytes()
    }
}

tape! {
    /// The span-only wire's TAPE of an internal `Root`, written into `sink` by the same walk as
    /// `write_root_bytes` — every embedded `<script>`, template expression and `<style>` in one
    /// tape document — in the coordinates `crate::WIRE_COORDINATES` states (a leading
    /// byte-order mark elided, as Svelte's `parse` strips it). Finish it with
    /// `sink.finish_words()` / `sink.finish_bytes()`.
    pub(crate) fn write_root_tape(
        parsed: ast::ParsedRootRef<'_>,
        source: &str,
        sink: &mut tsv_lang::tape::TapeSink,
    ) {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, Wire::Span);
        document_ctx!(parsed, source, tables.positions() => root, template_comments, ctx);
        let mut w = sink.writer(source, ctx.positions, crate::WIRE_COORDINATES.bom);
        write_root(&mut w, root, &ctx);
    }
}

/// The per-document environment every writer function shares.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) source: &'a str,
    /// Reads the template's lists and strings, and hands out the two views below.
    pub(super) svelte: ast::SvelteView<'a>,
    /// Resolves the ids, runs and pool text of the document's TypeScript (its scripts and
    /// template expressions).
    pub(super) view: tsv_ts::ast::AstView<'a>,
    /// Reads the stores of the top-level `<style>`'s stylesheet, for its children's walk.
    pub(super) css: tsv_css::ast::CssView<'a>,
    /// The document's positions: the byte→UTF-16 map, and — on the wire that carries
    /// `loc` — its one line table, which every node, `name_loc`, root comment,
    /// embedded island and `<style>` node reads.
    pub(super) positions: WirePositions<'a>,
    /// `Root::snippet_wire_parameters`: the preserved-paren parameter lists the wire emits
    /// in place of a head's paren-free `SnippetBlock::parameters`.
    pub(super) snippet_wire_parameters: &'a [ast::SnippetWireParameters],
    /// Template comments, sorted by position (empty on the common no-comment
    /// template — the whole spine then fuses).
    pub(super) comments: &'a [&'a Comment],
    /// Whether the component parses as TypeScript (component-global, from the first
    /// lang-bearing `<script>` — see `component_is_typescript`). Selects the parser variant
    /// every embedded writer reads (`EmbedWriter::vanilla_acorn`, via `Ctx::embed`), so a
    /// plain `<script>` beside a `lang="ts"` sibling still emits the acorn-typescript
    /// import/export shape.
    pub(super) component_is_ts: bool,
}

impl<'a> Ctx<'a> {
    json_only! {
        /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII — for the JSON
        /// copy's staged heads (the tape's take the offsets through the sink).
        #[inline]
        pub(super) fn pos(&self, byte: u32) -> u32 {
            self.positions.pos(byte)
        }
    }

    /// The preserved-paren parameter list of the `{#snippet}` block starting at
    /// `snippet_start`, when its head holds a grouping pair (`Root::snippet_wire_parameters`,
    /// ascending by that key).
    fn snippet_wire_parameters(&self, snippet_start: u32) -> Option<&'a [tsv_ts::ast::Expression]> {
        let wire = self.snippet_wire_parameters;
        wire.binary_search_by_key(&snippet_start, |entry| entry.snippet_start)
            .ok()
            .map(|i| self.view.exprs(wire[i].parameters))
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
    pub(super) fn embed(&self, mode: CommentMode<'a>) -> EmbedWriter<'a> {
        EmbedWriter {
            source: self.source,
            positions: self.positions,
            view: self.view,
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
fn fragment_first_start(view: ast::SvelteView<'_>, fragment: ast::Fragment) -> Option<u32> {
    view.nodes(fragment.nodes).first().map(|n| n.span().start)
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
    view: ast::SvelteView<'a>,
    root: &'a ast::Root,
    tag_start: u32,
    source: &str,
) -> Option<&'a ast::HtmlComment> {
    let mut expected_end = tag_start;
    for node in view.nodes(root.fragment.nodes).iter().rev() {
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
            ast::FragmentNode::Comment(comment) => return Some(comment),
            ast::FragmentNode::Text(text)
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
fn write_root(w: &mut Sink<'_>, root: &ast::Root, ctx: &Ctx<'_>) {
    let source = ctx.source;

    let find_preceding_comment =
        |tag_start: u32| preceding_comment(ctx.svelte, root, tag_start, source);

    // `Root`'s keys lead its `type`: the record opens at its `{`
    open_keys!(w, "Root", "{\"css\":");
    write_or_null(w, root.css.as_ref(), |w, style| {
        let style_comment = find_preceding_comment(style.span.start);
        write_style_sheet(w, style, style_comment, ctx);
    });
    keys_span!(
        w,
        "Root",
        ",\"js\":[],\"start\":",
        0,
        source.len() as u32,
        ctx
    );
    raw!(w, "Root", ",\"type\":\"Root\",\"fragment\":");
    write_fragment(w, root.fragment, ctx);
    raw!(w, "Root", ",\"options\":");
    write_or_null(w, root.options.as_ref(), |w, opts| {
        write_svelte_options(w, opts, ctx);
    });
    raw!(w, "Root", ",\"comments\":");
    write_array(w, root.comments.iter(), |w, c| {
        write_root_comment(w, c, ctx);
    });
    // Svelte assigns `module` before `instance` on the root.
    if let Some(script) = &root.module {
        let comment = find_preceding_comment(script.span.start);
        raw!(w, "Root", ",\"module\":");
        write_script(w, script, comment, ctx);
    }
    if let Some(script) = &root.instance {
        let comment = find_preceding_comment(script.span.start);
        raw!(w, "Root", ",\"instance\":");
        write_script(w, script, comment, ctx);
    }
    close!(w);
}

/// Emits a `Fragment` node.
fn write_fragment(w: &mut Sink<'_>, fragment: ast::Fragment, ctx: &Ctx<'_>) {
    raw!(w, "{\"type\":\"Fragment\",\"nodes\":");
    write_array(w, ctx.svelte.nodes(fragment.nodes), |w, n| {
        write_fragment_node(w, n, ctx);
    });
    close!(w);
}

/// Emit a fragment node, dispatching on its variant.
fn write_fragment_node(w: &mut Sink<'_>, node: &ast::FragmentNode, ctx: &Ctx<'_>) {
    match node {
        ast::FragmentNode::Element(elem) => write_element(w, elem, ctx),
        ast::FragmentNode::SpecialElement(elem) => write_special_element(w, elem, ctx),
        ast::FragmentNode::ExpressionTag(tag) => write_expression_tag(w, tag, ctx),
        ast::FragmentNode::Text(text) => write_text(w, text, ctx),
        ast::FragmentNode::Comment(comment) => write_html_comment(w, comment, ctx),
        ast::FragmentNode::IfBlock(block) => write_if_block(w, block, ctx),
        ast::FragmentNode::EachBlock(block) => write_each_block(w, block, ctx),
        ast::FragmentNode::AwaitBlock(block) => write_await_block(w, block, ctx),
        ast::FragmentNode::KeyBlock(block) => write_key_block(w, block, ctx),
        ast::FragmentNode::SnippetBlock(block) => write_snippet_block(w, block, ctx),
        ast::FragmentNode::HtmlTag(tag) => write_html_tag(w, tag, ctx),
        ast::FragmentNode::ConstTag(tag) => write_const_tag(w, tag, ctx),
        ast::FragmentNode::DeclarationTag(tag) => write_declaration_tag(w, tag, ctx),
        ast::FragmentNode::DebugTag(tag) => write_debug_tag(w, tag, ctx),
        ast::FragmentNode::RenderTag(tag) => write_render_tag(w, tag, ctx),
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
    w: &mut Sink<'_>,
    expr: &tsv_ts::ast::Expression,
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
    w: &mut Sink<'_>,
    expr: &tsv_ts::ast::Expression,
    window: Span,
    ctx: &Ctx<'_>,
) {
    write_generic_island(w, expr, window.start, window.end, ctx);
}

/// The source bytes of a name span, without the char-boundary checks a `&str` slice pays:
/// a name span covers whole characters, which debug builds assert. ⚠️ Invariant, not a
/// check: in release a span that split a character would be copied into the wire as invalid
/// UTF-8 where the `&str` slice would have panicked — every name span is a lexed run's, so
/// none does.
#[inline]
pub(super) fn name_bytes<'s>(span: Span, ctx: &Ctx<'s>) -> &'s [u8] {
    debug_assert!(
        ctx.source.is_char_boundary(span.start as usize)
            && ctx.source.is_char_boundary(span.end as usize),
        "a name span covers whole characters"
    );
    &ctx.source.as_bytes()[span.range()]
}

/// Emits a `RegularElement` (HTML) or `Component` node.
fn write_element(w: &mut Sink<'_>, elem: &ast::Element, ctx: &Ctx<'_>) {
    // The head, through the mount's staged burst, which ends before the `name` field (module
    // doc, Staged runs); the tape entry it opened.
    tape! { let e = write_element_header(w, elem, ctx); }
    json_only! { write_element_header(w, elem, ctx); }
    write_element_name_field(w, elem, ctx);
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, elem.name_span, lines);
    }
    // The constant stretch between two of the element's dynamic parts is one append,
    // and an empty attribute list rides the one that opens the fragment.
    let attributes = ctx.svelte.attributes(elem.attributes);
    if attributes.is_empty() {
        raw!(
            w,
            @in e => "Component" | "RegularElement",
            ",\"attributes\":[],\"fragment\":{\"type\":\"Fragment\",\"nodes\":["
        );
    } else {
        raw!(w, @in e => "Component" | "RegularElement", ",\"attributes\":[");
        write_items(w, attributes, |w, a| write_attribute_node(w, a, ctx));
        raw!(
            w,
            @in e => "Component" | "RegularElement",
            "],\"fragment\":{\"type\":\"Fragment\",\"nodes\":["
        );
    }
    let nodes = ctx.svelte.nodes(elem.fragment.nodes);
    write_items(w, nodes, |w, n| write_fragment_node(w, n, ctx));
    // The node array, the fragment, the element.
    raw!(w, "]}}");
}

/// The items of a JSON array, comma-separated, without its brackets — for a caller
/// whose `[` and `]` ride the constants on either side ([`write_array`] writes them
/// itself).
#[inline]
fn write_items<T>(w: &mut Sink<'_>, items: &[T], mut f: impl FnMut(&mut Sink<'_>, &T)) {
    let mut first = true;
    for item in items {
        if !first {
            comma!(w);
        }
        first = false;
        f(w, item);
    }
}

/// A key literal of a special element (`$e`, the record's run-time entry), which may be any
/// of them.
macro_rules! special_raw {
    ($w:ident, $e:ident, $s:literal) => {
        raw!(
            $w,
            @in $e => "SvelteHead" | "SvelteWindow" | "SvelteBody" | "SvelteDocument"
                | "SvelteElement" | "SvelteComponent" | "SvelteSelf" | "SlotElement"
                | "SvelteFragment" | "SvelteBoundary" | "TitleElement",
            $s
        )
    };
}

/// Emits a special-element node (`svelte:element`, `svelte:component`, …).
/// `tag`/`expression` are skip-if-none.
fn write_special_element(w: &mut Sink<'_>, elem: &ast::SpecialElement, ctx: &Ctx<'_>) {
    json_only! {
        w.raw("{\"type\":\"");
        w.raw(elem.kind.node_type());
        w.raw("\",\"start\":");
    }
    tape! {
        let e = special_element_entry(&elem.kind);
        #[cfg(debug_assertions)]
        assert_eq!(
            ::tsv_lang::tape::shapes::ENTRIES[usize::from(e)].ty,
            Some(elem.kind.node_type()),
            "a header's tape entry is its JSON type's"
        );
        w.t_open(e);
    }
    w.span_start_end(ctx.positions, elem.span.start, elem.span.end);
    special_raw!(w, e, ",\"name\":");
    // Escape-free `&'static str` (`svelte:head`, `slot`, `title`, …) → skip the
    // string-escape scan.
    w.token(elem.kind.tag_name());
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, elem.name_span, lines);
    }
    special_raw!(w, e, ",\"attributes\":");
    write_array(w, ctx.svelte.attributes(elem.attributes), |w, a| {
        write_attribute_node(w, a, ctx);
    });
    special_raw!(w, e, ",\"fragment\":");
    write_fragment(w, elem.fragment, ctx);
    // `<svelte:element this={…}>` tag. A plain-string `this="x"` is a
    // Svelte-style `Literal` (single-quoted `raw`) that carries no
    // expression parse, so no template comment can attach — emit it fused.
    // Every other `this={…}` is a braced island keyed on its own `{…}` span.
    if let Some(tag) = elem.kind.tag() {
        raw!(w, "SvelteElement", ",\"tag\":");
        write_special_tag(w, tag, ctx);
    }
    // `<svelte:component this={…}>` expression — a generic island.
    if let Some(tag) = elem.kind.expression() {
        raw!(w, "SvelteComponent", ",\"expression\":");
        write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
    }
    close!(w);
}

tape! {
    /// A special element's tape entry, resolved at build time per kind.
    fn special_element_entry(kind: &ast::SpecialElementKind) -> u16 {
        use ast::SpecialElementKind as K;
        match kind {
            K::SvelteHead => entry!("SvelteHead"),
            K::SvelteWindow => entry!("SvelteWindow"),
            K::SvelteBody => entry!("SvelteBody"),
            K::SvelteDocument => entry!("SvelteDocument"),
            K::SvelteElement { .. } => entry!("SvelteElement"),
            K::SvelteComponent { .. } => entry!("SvelteComponent"),
            K::SvelteSelf => entry!("SvelteSelf"),
            K::SlotElement => entry!("SlotElement"),
            K::SvelteFragment => entry!("SvelteFragment"),
            K::SvelteBoundary => entry!("SvelteBoundary"),
            K::TitleElement => entry!("TitleElement"),
        }
    }
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
/// The form is read off [`ast::SpecialThis`], never sniffed back out of the source: it
/// is the parser's own answer, and a second way to ask the same question is a second way to
/// get it wrong.
fn write_special_tag(w: &mut Sink<'_>, this: &ast::SpecialThis, ctx: &Ctx<'_>) {
    match this {
        ast::SpecialThis::Plain(text) => {
            let content = text.data(ctx.source);
            let span = text.span;
            // Svelte builds this `Literal` `{type, value, raw, start, end}`; tsv's wire
            // gives it acorn's one `Literal` order.
            head!(
                w,
                "Literal/SvelteFusedLiteral",
                "{\"type\":\"Literal\",\"start\":",
                span.start,
                span.end,
                ctx
            );
            raw!(w, "Literal/SvelteFusedLiteral", ",\"value\":");
            w.string(&content);
            raw!(w, "Literal/SvelteFusedLiteral", ",\"raw\":");
            // Svelte reports the raw as a single-quoted string regardless of source.
            w.string(&format!("'{content}'"));
            close!(w);
        }
        ast::SpecialThis::Braced(tag) => {
            write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
        }
    }
}

/// Emits an `ExpressionTag` node (fragment `{expr}`).
fn write_expression_tag(w: &mut Sink<'_>, tag: &ast::ExpressionTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "ExpressionTag",
        "{\"type\":\"ExpressionTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "ExpressionTag", ",\"expression\":");
    write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
    close!(w);
}

/// A shorthand attribute `{name}`'s `ExpressionTag`: Svelte injects `character`
/// into the identifier's `loc`. The shorthand form requires `tag.span == id.span`
/// (the identifier *is* the tag), so no comment can lie between the braces and the
/// name — attach is always a no-op here — and the whole tag fuses.
fn write_shorthand_expression_tag(w: &mut Sink<'_>, tag: &ast::ExpressionTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "ExpressionTag",
        "{\"type\":\"ExpressionTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "ExpressionTag", ",\"expression\":");
    write_identifier_expression_with_character(
        w,
        ctx.view.expr(tag.expression),
        ctx.embed(CommentMode::Off),
    );
    close!(w);
}

/// Emits a `Text` node: `type, start, end, raw, data`, the order Svelte builds a
/// fragment's text in. (Its attribute-value sequence builds `{start, end, type, raw,
/// data}` and a raw-content element's text — a nested `<script>`/`<style>` —
/// `{start, end, type, data, raw}`; tsv's wire keeps the one order.)
fn write_text(w: &mut Sink<'_>, text: &ast::Text, ctx: &Ctx<'_>) {
    // The head, through the mount's staged burst, which ends at the dynamic `raw` (module
    // doc, Staged runs).
    write_text_header(w, text, ctx);
    write_raw_then_data(w, text, ctx);
    close!(w);
}

/// Emits a `Comment` node (HTML `<!-- … -->`).
fn write_html_comment(w: &mut Sink<'_>, comment: &ast::HtmlComment, ctx: &Ctx<'_>) {
    head!(
        w,
        "Comment",
        "{\"type\":\"Comment\",\"start\":",
        comment.span.start,
        comment.span.end,
        ctx
    );
    raw!(w, "Comment", ",\"data\":");
    w.string(comment.content(ctx.source));
    close!(w);
}

//
// Blocks
//

/// Emits an `IfBlock` node: `{type, elseif, start, end, …}`, the order Svelte builds a
/// root `{#if}` in. (It builds an `{:else if}` block `{start, end, type, elseif, …}`;
/// tsv's wire keeps the one order.)
fn write_if_block(w: &mut Sink<'_>, block: &ast::IfBlock, ctx: &Ctx<'_>) {
    head!(
        w,
        "IfBlock",
        pick block.elseif,
        "{\"type\":\"IfBlock\",\"elseif\":true,\"start\":",
        "{\"type\":\"IfBlock\",\"elseif\":false,\"start\":",
        block.span.start,
        block.span.end,
        ctx
    );
    let range_end = fragment_first_start(ctx.svelte, block.consequent).unwrap_or(block.span.end);
    raw!(w, "IfBlock", ",\"test\":");
    write_generic_island(
        w,
        ctx.view.expr(block.test),
        block.span.start,
        range_end,
        ctx,
    );
    raw!(w, "IfBlock", ",\"consequent\":");
    write_fragment(w, block.consequent, ctx);
    raw!(w, "IfBlock", ",\"alternate\":");
    write_optional_fragment(w, block.alternate.as_ref(), ctx);
    close!(w);
}

/// Emits an `EachBlock` node. `context` is a pattern island
/// ([`write_pattern_island`] carries any binding/annotation comments);
/// `index`/`key`/`fallback` are skip-if-none.
fn write_each_block(w: &mut Sink<'_>, block: &ast::EachBlock, ctx: &Ctx<'_>) {
    head!(
        w,
        "EachBlock",
        "{\"type\":\"EachBlock\",\"start\":",
        block.span.start,
        block.span.end,
        ctx
    );
    let range_end = fragment_first_start(ctx.svelte, block.body).unwrap_or(block.span.end);
    raw!(w, "EachBlock", ",\"expression\":");
    write_generic_island(
        w,
        ctx.view.expr(block.expression),
        block.span.start,
        range_end,
        ctx,
    );
    raw!(w, "EachBlock", ",\"body\":");
    write_fragment(w, block.body, ctx);
    raw!(w, "EachBlock", ",\"context\":");
    write_or_null(w, block.context.as_ref(), |w, c| {
        write_pattern_island(w, ctx.view.expr(*c), ctx);
    });
    if let Some(index) = block.index {
        raw!(w, "EachBlock", ",\"index\":");
        w.string(index.extract(ctx.source));
    }
    if let Some(key) = &block.key {
        raw!(w, "EachBlock", ",\"key\":");
        // The key's window is INSIDE its own parens, not at the block's `{#` — canonical
        // filters each parse's comments to `start >= index`, and the key's
        // `read_expression` begins after the `(` (only whitespace can sit between).
        // Anchored on the head, this window reaches back over the CONTEXT PATTERN and
        // claims a comment written inside it, which the pattern island also attaches. The
        // key's own `(`…`)` are block syntax, not a grouping pair around the root, so the
        // window stops short of both (no comment can follow the `)` before the `}`).
        write_generic_island(
            w,
            ctx.view.expr(key.expression),
            key.span.start + 1,
            key.span.end - 1,
            ctx,
        );
    }
    if let Some(fallback) = &block.fallback {
        raw!(w, "EachBlock", ",\"fallback\":");
        write_fragment(w, *fallback, ctx);
    }
    close!(w);
}

/// Emits an `AwaitBlock` node. `value`/`error` are pattern islands; every
/// `Option` → `null` when absent (no skip).
fn write_await_block(w: &mut Sink<'_>, block: &ast::AwaitBlock, ctx: &Ctx<'_>) {
    head!(
        w,
        "AwaitBlock",
        "{\"type\":\"AwaitBlock\",\"start\":",
        block.span.start,
        block.span.end,
        ctx
    );
    let range_end = [
        block.pending.as_ref(),
        block.then.as_ref(),
        block.catch.as_ref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|fragment| fragment_first_start(ctx.svelte, *fragment))
    .min()
    .unwrap_or(block.span.end);
    raw!(w, "AwaitBlock", ",\"expression\":");
    write_generic_island(
        w,
        ctx.view.expr(block.expression),
        block.span.start,
        range_end,
        ctx,
    );
    raw!(w, "AwaitBlock", ",\"value\":");
    write_or_null(w, block.value.as_ref(), |w, v| {
        write_pattern_island(w, ctx.view.expr(*v), ctx);
    });
    raw!(w, "AwaitBlock", ",\"error\":");
    write_or_null(w, block.error.as_ref(), |w, e| {
        write_pattern_island(w, ctx.view.expr(*e), ctx);
    });
    raw!(w, "AwaitBlock", ",\"pending\":");
    // Svelte's block form always has a pending Fragment (empty or not); the inline
    // `then`/`catch` shorthand has `null`. `pending` holds only non-empty content,
    // so an empty block-form pending (`{#await x}{/await}`, `{#await x}{:then v}…`)
    // is `None` here yet must still emit an (empty) Fragment — hence the flag.
    match block.pending.as_ref() {
        Some(fragment) => write_fragment(w, *fragment, ctx),
        None if block.pending_block => write_fragment(w, ast::Fragment { nodes: Run::EMPTY }, ctx),
        None => raw!(w, "null"),
    }
    raw!(w, "AwaitBlock", ",\"then\":");
    write_optional_fragment(w, block.then.as_ref(), ctx);
    raw!(w, "AwaitBlock", ",\"catch\":");
    write_optional_fragment(w, block.catch.as_ref(), ctx);
    close!(w);
}

/// Emits a `KeyBlock` node.
fn write_key_block(w: &mut Sink<'_>, block: &ast::KeyBlock, ctx: &Ctx<'_>) {
    head!(
        w,
        "KeyBlock",
        "{\"type\":\"KeyBlock\",\"start\":",
        block.span.start,
        block.span.end,
        ctx
    );
    let range_end = fragment_first_start(ctx.svelte, block.fragment).unwrap_or(block.span.end);
    raw!(w, "KeyBlock", ",\"expression\":");
    write_generic_island(
        w,
        ctx.view.expr(block.expression),
        block.span.start,
        range_end,
        ctx,
    );
    raw!(w, "KeyBlock", ",\"fragment\":");
    write_fragment(w, block.fragment, ctx);
    close!(w);
}

/// Emits a `SnippetBlock` node. The snippet name carries `character` (like a
/// shorthand attribute); `typeParams` is skip-if-none, right after
/// `expression` (Svelte assigns it before reading the parameters).
fn write_snippet_block(w: &mut Sink<'_>, block: &ast::SnippetBlock, ctx: &Ctx<'_>) {
    head!(
        w,
        "SnippetBlock",
        "{\"type\":\"SnippetBlock\",\"start\":",
        block.span.start,
        block.span.end,
        ctx
    );
    let range_end = fragment_first_start(ctx.svelte, block.body).unwrap_or(block.span.end);
    raw!(w, "SnippetBlock", ",\"expression\":");
    write_snippet_name(
        w,
        ctx.view.expr(block.expression),
        block.span.start,
        range_end,
        ctx,
    );
    if let Some(type_params) = block.type_params_raw {
        raw!(w, "SnippetBlock", ",\"typeParams\":");
        w.string(ctx.svelte.text(type_params));
    }
    raw!(w, "SnippetBlock", ",\"parameters\":");
    write_snippet_parameters(
        w,
        ctx.snippet_wire_parameters(block.span.start)
            .unwrap_or_else(|| ctx.view.exprs(block.parameters)),
        block.span.start,
        range_end,
        ctx,
    );
    raw!(w, "SnippetBlock", ",\"body\":");
    write_fragment(w, block.body, ctx);
    close!(w);
}

/// The snippet name identifier — Svelte injects `character` into its `loc`. A
/// leading comment (`{#snippet /* c */ name(…)}`) can attach, so the
/// comment-bearing case runs the island's online attach; the comment-free common
/// case fuses directly.
fn write_snippet_name(
    w: &mut Sink<'_>,
    expr: &tsv_ts::ast::Expression,
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
    w: &mut Sink<'_>,
    parameters: &[tsv_ts::ast::Expression],
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
fn write_html_tag(w: &mut Sink<'_>, tag: &ast::HtmlTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "HtmlTag",
        "{\"type\":\"HtmlTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "HtmlTag", ",\"expression\":");
    write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
    close!(w);
}

/// Emits a `RenderTag` node (`{@render expr}`).
fn write_render_tag(w: &mut Sink<'_>, tag: &ast::RenderTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "RenderTag",
        "{\"type\":\"RenderTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "RenderTag", ",\"expression\":");
    write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
    close!(w);
}

/// Emits a `DebugTag` node (`{@debug a, b}`).
///
/// A multi-identifier tag is ONE canonical acorn parse (a `SequenceExpression`
/// wrapper, discarded after identifier extraction), so its comment attach runs
/// once across the list with the wrapper-end trailing suppression, each element's own
/// grouping pairs run as silent frames. A single identifier is itself the parse root and
/// takes the generic-island path (root-fallback trailing).
fn write_debug_tag(w: &mut Sink<'_>, tag: &ast::DebugTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "DebugTag",
        "{\"type\":\"DebugTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "DebugTag", ",\"identifiers\":");
    // The internal entries keep what the author wrote — a JSDoc cast around the
    // whole comma list (`{@debug /** @type {B} */ (b, c)}`) is ONE entry so the
    // printer can reproduce the cast — but the wire is the canonical parse,
    // where `remove_parens` uncovers the sequence and the reader flattens it:
    // splice such an entry into the sequence's elements. The elements keep
    // their own casts (emission unwraps a cast to its inner node), and their
    // paren-inclusive spans are what give the discarded-wrapper bounds below
    // canonical's own values. The cast-free common case borrows the parsed
    // slice untouched.
    let entries = ctx.view.expr_list(tag.identifiers);
    let spliced = if entries.iter().any(|&entry| {
        matches!(
            ctx.view
                .expr(entry)
                .unwrap_comment_bound_pairs(ctx.view)
                .kind,
            tsv_ts::ast::ExpressionKind::SequenceExpression(_)
        )
    }) {
        let mut flat = Vec::with_capacity(entries.len() + 1);
        for &entry in entries {
            match &ctx
                .view
                .expr(entry)
                .unwrap_comment_bound_pairs(ctx.view)
                .kind
            {
                tsv_ts::ast::ExpressionKind::SequenceExpression(seq) => {
                    flat.extend_from_slice(ctx.view.expr_list(seq.expressions));
                }
                _ => flat.push(entry),
            }
        }
        Some(flat)
    } else {
        None
    };
    let identifiers: &[tsv_ts::ast::ExprId] = spliced.as_deref().unwrap_or(entries);
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
                grouping_parens_around(
                    attach_inputs,
                    tag.span.start,
                    ctx.view.expr(*id).span(),
                    tag.span.end,
                )
            })
            .collect();
        let outer = |i: usize| {
            element_parens[i]
                .first()
                .copied()
                .unwrap_or_else(|| ctx.view.expr(identifiers[i]).span())
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
                    write_expression_embedded(w, ctx.view.expr(*id), ctx.embed(mode));
                });
            },
        );
    } else {
        write_array(w, identifiers, |w, id| {
            write_braced_island(w, ctx.view.expr(*id), tag.span, ctx);
        });
    }
    close!(w);
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
fn write_const_tag(w: &mut Sink<'_>, tag: &ast::ConstTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "ConstTag",
        "{\"type\":\"ConstTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "ConstTag", ",\"declaration\":");
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
        let id_attach =
            attach_binding_pattern(ctx.view, ctx.view.expr(tag.id), ctx.attach_inputs());
        let init_attach = attach_const_tag_init(ctx.view, tag, ctx.attach_inputs());
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
    close!(w);
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
fn const_declarator_end(tag: &ast::ConstTag, ctx: &Ctx<'_>) -> u32 {
    let close = (tag.span.end - 1) as usize; // the closing `}`
    let mut end = ctx.view.expr(tag.init).span().end;
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
    w: &mut Sink<'_>,
    tag: &ast::ConstTag,
    decl_end: u32,
    declarator_end: u32,
    (id_mode, id_annotation_mode): (CommentMode<'_>, CommentMode<'_>),
    init_mode: CommentMode<'_>,
    ctx: &Ctx<'_>,
) {
    // Svelte builds the pair `{type, kind, declarations, start, end}` /
    // `{type, id, init, start, end}`; tsv's wire gives each acorn's one order.
    head!(
        w,
        "VariableDeclaration",
        "{\"type\":\"VariableDeclaration\",\"start\":",
        tag.span.start + 2,
        decl_end,
        ctx
    );
    keys_span!(
        w,
        "VariableDeclaration",
        ",\"declarations\":[{\"type\":\"VariableDeclarator\",\"start\":",
        ctx.view.expr(tag.id).span().start,
        declarator_end,
        ctx
    );
    raw!(w, "VariableDeclarator", ",\"id\":");
    write_pattern_embedded(
        w,
        ctx.view.expr(tag.id),
        ctx.embed_pattern(id_mode, id_annotation_mode),
    );
    raw!(w, "VariableDeclarator", ",\"init\":");
    write_expression_embedded(w, ctx.view.expr(tag.init), ctx.embed(init_mode));
    raw!(w, "VariableDeclaration", "}],\"kind\":\"const\"}");
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
fn write_declaration_tag(w: &mut Sink<'_>, tag: &ast::DeclarationTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "DeclarationTag",
        "{\"type\":\"DeclarationTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "DeclarationTag", ",\"declaration\":");
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
    close!(w);
}

//
// Attributes
//

/// Emit an attribute node, dispatching on its variant (attribute / spread /
/// attach / directive).
fn write_attribute_node(w: &mut Sink<'_>, node: &ast::AttributeNode, ctx: &Ctx<'_>) {
    match node {
        ast::AttributeNode::Attribute(a) => write_attribute(w, a, ctx),
        ast::AttributeNode::SpreadAttribute(s) => write_spread_attribute(w, s, ctx),
        ast::AttributeNode::AttachTag(t) => write_attach_tag(w, t, ctx),
        ast::AttributeNode::OnDirective(d) => write_on_directive(w, d, ctx),
        ast::AttributeNode::BindDirective(d) => write_bind_directive(w, d, ctx),
        ast::AttributeNode::ClassDirective(d) => write_class_directive(w, d, ctx),
        ast::AttributeNode::StyleDirective(d) => write_style_directive(w, d, ctx),
        ast::AttributeNode::UseDirective(d) => write_use_directive(w, d, ctx),
        ast::AttributeNode::TransitionDirective(d) => write_transition_directive(w, d, ctx),
        ast::AttributeNode::AnimateDirective(d) => write_animate_directive(w, d, ctx),
        ast::AttributeNode::LetDirective(d) => write_let_directive(w, d, ctx),
    }
}

/// Emits an `Attribute` node.
fn write_attribute(w: &mut Sink<'_>, attr: &ast::Attribute, ctx: &Ctx<'_>) {
    // The head, through the mount's staged burst, which ends before the `name` field (module
    // doc, Staged runs).
    write_attribute_header(w, attr, ctx);
    write_scanned_name_field(w, name_bytes(attr.name_span, ctx));
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, attr.name_span, lines);
    }
    // A boolean attribute's `true` closes the node in the same constant.
    let Some(values) = ctx.svelte.opt_attribute_values(attr.value) else {
        raw!(w, "Attribute", ",\"value\":true}");
        return;
    };
    raw!(w, "Attribute", ",\"value\":");
    write_attribute_value_field(w, values, ctx);
    close!(w);
}

/// Emit the value an attribute was written with — a boolean attribute has none, and
/// [`write_attribute`] writes its `true`: a bare `{expr}` (plain object), or a
/// text/quoted sequence (array).
fn write_attribute_value_field(w: &mut Sink<'_>, values: &[ast::AttributeValue], ctx: &Ctx<'_>) {
    let has_text = values
        .iter()
        .any(|v| matches!(v, ast::AttributeValue::Text(_)));
    let quoted = values.len() == 1
        && matches!(&values[0], ast::AttributeValue::ExpressionTag(tag)
            if preceded_by_quote(ctx.source, tag.span.start));

    if has_text || quoted {
        write_value_array(w, values, |w, v| write_attribute_value(w, v, ctx));
    } else if values.len() == 1 {
        // Single bare expression → plain object. A shorthand `{name}` (the tag
        // and its identifier share a span) injects `character`.
        match &values[0] {
            ast::AttributeValue::ExpressionTag(tag)
                if matches!(&ctx.view.expr(tag.expression).kind, tsv_ts::ast::ExpressionKind::Identifier(id)
                    if tag.span == id.span) =>
            {
                write_shorthand_expression_tag(w, tag, ctx);
            }
            v => write_attribute_value(w, v, ctx),
        }
    } else {
        write_value_array(w, values, |w, v| write_attribute_value(w, v, ctx));
    }
}

/// One attribute-value part (array element or bare-object body).
fn write_attribute_value(w: &mut Sink<'_>, value: &ast::AttributeValue, ctx: &Ctx<'_>) {
    match value {
        ast::AttributeValue::Text(text) => write_text(w, text, ctx),
        ast::AttributeValue::ExpressionTag(tag) => write_expression_tag(w, tag, ctx),
    }
}

/// Emits a `SpreadAttribute` node (`{...expr}`).
fn write_spread_attribute(w: &mut Sink<'_>, spread: &ast::SpreadAttribute, ctx: &Ctx<'_>) {
    head!(
        w,
        "SpreadAttribute",
        "{\"type\":\"SpreadAttribute\",\"start\":",
        spread.span.start,
        spread.span.end,
        ctx
    );
    raw!(w, "SpreadAttribute", ",\"expression\":");
    write_braced_island(w, ctx.view.expr(spread.expression), spread.span, ctx);
    close!(w);
}

/// Emits an `AttachTag` node (`{@attach expr}`).
fn write_attach_tag(w: &mut Sink<'_>, tag: &ast::AttachTag, ctx: &Ctx<'_>) {
    head!(
        w,
        "AttachTag",
        "{\"type\":\"AttachTag\",\"start\":",
        tag.span.start,
        tag.span.end,
        ctx
    );
    raw!(w, "AttachTag", ",\"expression\":");
    write_braced_island(w, ctx.view.expr(tag.expression), tag.span, ctx);
    close!(w);
}

//
// Directives
//

/// The head shared by every directive: `start, end, type, name, name_loc`. The directive's
/// `type` follows its offsets, so on the tape its record (entry `E`, `at!`) opens at its `{`.
fn write_directive_head<const E: u16>(
    w: &mut Sink<'_>,
    node_type: &str,
    span: Span,
    name_span: Span,
    head_span: Span,
    ctx: &Ctx<'_>,
) {
    head_object!(w, @E, span.start, span.end, ctx);
    json_only! {
        w.raw(",\"type\":\"");
        w.raw(node_type);
    }
    tape! {
        #[cfg(debug_assertions)]
        assert_eq!(
            ::tsv_lang::tape::shapes::ENTRIES[usize::from(E)].ty,
            Some(node_type),
            "a header's tape entry is its JSON type's"
        );
        #[cfg(not(debug_assertions))]
        let _ = node_type;
    }
    raw!(w, @E, "\",\"name\":");
    w.string(name_span.extract(ctx.source));
    if let Some(lines) = ctx.positions.lines() {
        write_name_loc_field(w, head_span, lines);
    }
}

/// The `modifiers` array (each modifier's source span → a JSON string of its bytes).
fn write_modifiers(w: &mut Sink<'_>, modifiers: &[Span], source: &str) {
    write_array(w, modifiers, |w, m| {
        tape! { w.t_list_ref(m.extract(source).as_bytes()); }
        json_only! { w.string(m.extract(source)); }
    });
}

/// An optional directive expression (`on:`/`use:`/`transition:`/`animate:`/`let:`):
/// a generic island when present, else `null`.
fn write_optional_directive_expression(
    w: &mut Sink<'_>,
    expression: Option<&tsv_ts::ast::Expression>,
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
    ($fn_name:ident, $ty:ident, $e:literal) => {
        fn $fn_name(w: &mut Sink<'_>, d: &ast::$ty, ctx: &Ctx<'_>) {
            write_directive_head::<{ at!($e) }>(
                w,
                stringify!($ty),
                d.span,
                d.name_span,
                d.head_span,
                ctx,
            );
            raw!(w, $e, ",\"expression\":");
            write_optional_directive_expression(w, ctx.view.opt_expr(d.expression), d.span, ctx);
            raw!(w, $e, ",\"modifiers\":");
            write_modifiers(w, ctx.svelte.modifiers(d.modifiers), ctx.source);
            close!(w);
        }
    };
}

expression_directive_writer!(write_on_directive, OnDirective, "OnDirective");
expression_directive_writer!(write_use_directive, UseDirective, "UseDirective");
expression_directive_writer!(
    write_animate_directive,
    AnimateDirective,
    "AnimateDirective"
);
expression_directive_writer!(write_let_directive, LetDirective, "LetDirective");

fn write_transition_directive(w: &mut Sink<'_>, d: &ast::TransitionDirective, ctx: &Ctx<'_>) {
    write_directive_head::<{ at!("TransitionDirective") }>(
        w,
        "TransitionDirective",
        d.span,
        d.name_span,
        d.head_span,
        ctx,
    );
    raw!(w, "TransitionDirective", ",\"expression\":");
    write_optional_directive_expression(w, ctx.view.opt_expr(d.expression), d.span, ctx);
    raw!(w, "TransitionDirective", ",\"modifiers\":");
    write_modifiers(w, ctx.svelte.modifiers(d.modifiers), ctx.source);
    raw!(w, "TransitionDirective", ",\"intro\":");
    w.bool(d.direction.has_intro());
    raw!(w, "TransitionDirective", ",\"outro\":");
    w.bool(d.direction.has_outro());
    close!(w);
}

/// `bind:`/`class:` share an expression: the explicit form (`bind:x={e}`) is a
/// generic island keyed on the directive span (a real expression parse, so
/// template comments can attach); the shorthand form (`bind:x`)
/// is a synthetic `Identifier` (Svelte builds it `{start, end, type, name}`; tsv's wire
/// gives it acorn's one `Identifier` order) that never carries a comment, emitted fused.
fn write_directive_value_expression(
    w: &mut Sink<'_>,
    expr: &tsv_ts::ast::Expression,
    has_expression_tag: bool,
    span: Span,
    ctx: &Ctx<'_>,
) {
    if has_expression_tag {
        write_braced_island(w, expr, span, ctx);
    } else {
        // Shorthand: the parser builds this as a synthetic `Identifier`.
        #[expect(clippy::unreachable)]
        let tsv_ts::ast::ExpressionKind::Identifier(id) = &expr.kind else {
            unreachable!("shorthand directive expression is always an Identifier");
        };
        head!(
            w,
            "Identifier",
            "{\"type\":\"Identifier\",\"start\":",
            id.span.start,
            id.span.end,
            ctx
        );
        raw!(w, "Identifier", ",\"name\":");
        w.string(id.name(ctx.source, ctx.view));
        close!(w);
    }
}

fn write_bind_directive(w: &mut Sink<'_>, d: &ast::BindDirective, ctx: &Ctx<'_>) {
    write_directive_head::<{ at!("BindDirective") }>(
        w,
        "BindDirective",
        d.span,
        d.name_span,
        d.head_span,
        ctx,
    );
    raw!(w, "BindDirective", ",\"expression\":");
    write_directive_value_expression(
        w,
        ctx.view.expr(d.expression),
        d.expression_tag_span.is_some(),
        d.span,
        ctx,
    );
    raw!(w, "BindDirective", ",\"modifiers\":");
    write_modifiers(w, ctx.svelte.modifiers(d.modifiers), ctx.source);
    close!(w);
}

fn write_class_directive(w: &mut Sink<'_>, d: &ast::ClassDirective, ctx: &Ctx<'_>) {
    write_directive_head::<{ at!("ClassDirective") }>(
        w,
        "ClassDirective",
        d.span,
        d.name_span,
        d.head_span,
        ctx,
    );
    raw!(w, "ClassDirective", ",\"expression\":");
    write_directive_value_expression(
        w,
        ctx.view.expr(d.expression),
        d.expression_tag_span.is_some(),
        d.span,
        ctx,
    );
    raw!(w, "ClassDirective", ",\"modifiers\":");
    write_modifiers(w, ctx.svelte.modifiers(d.modifiers), ctx.source);
    close!(w);
}

/// Emits a `StyleDirective` node. Field order: `start, end, type, name,
/// name_loc, modifiers, value`.
fn write_style_directive(w: &mut Sink<'_>, d: &ast::StyleDirective, ctx: &Ctx<'_>) {
    write_directive_head::<{ at!("StyleDirective") }>(
        w,
        "StyleDirective",
        d.span,
        d.name_span,
        d.head_span,
        ctx,
    );
    raw!(w, "StyleDirective", ",\"modifiers\":");
    write_modifiers(w, ctx.svelte.modifiers(d.modifiers), ctx.source);
    raw!(w, "StyleDirective", ",\"value\":");
    match &d.value {
        ast::StyleDirectiveValue::True => raw!(w, "true"),
        ast::StyleDirectiveValue::ExpressionTag(tag) => {
            // Quoted (`style:x="{e}"`) → array; bare (`style:x={e}`) → plain object.
            if preceded_by_quote(ctx.source, tag.span.start) {
                // a one-item array in the tagged-value field
                tape! {
                    w.t_value_list_open();
                    w.t_tag(tsv_lang::tape::TAG_OBJECT);
                }
                json_only! { w.raw("["); }
                write_expression_tag(w, tag, ctx);
                tape! { w.t_list_close(); }
                json_only! { w.raw("]"); }
            } else {
                write_expression_tag(w, tag, ctx);
            }
        }
        ast::StyleDirectiveValue::Parts(parts) => {
            write_value_array(w, ctx.svelte.attribute_values(*parts), |w, p| {
                write_attribute_value(w, p, ctx);
            });
        }
    }
    close!(w);
}

//
// Scripts, style, and shared helpers
//

/// A `<style>` `StyleSheet`. `children` fuse via `tsv_css`'s
/// `write_css_children`, which hands back the sibling `comments` run its walk
/// gathered; `attributes` and the preceding comment fuse too (the `<style>`
/// envelope is never visited by the template attach passes).
fn write_style_sheet(
    w: &mut Sink<'_>,
    style: &ast::Style,
    preceding_comment: Option<&ast::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    head!(
        w,
        "css:StyleSheet",
        "{\"type\":\"StyleSheet\",\"start\":",
        style.span.start,
        style.span.end,
        ctx
    );
    raw!(w, "css:StyleSheet", ",\"attributes\":");
    write_value_attributes(w, ctx.svelte.attributes(style.attributes), ctx);
    raw!(w, "css:StyleSheet", ",\"children\":");
    let css_comments = write_css_children(w, style.css_parsed(ctx.css), ctx.source, ctx.positions);
    raw!(w, "css:StyleSheet", ",\"comments\":");
    write_css_comments(w, &css_comments, ctx.source, ctx.positions);
    keys_span!(
        w,
        "css:StyleSheet",
        ",\"content\":{\"start\":",
        style.content_span.start,
        style.content_span.end,
        ctx
    );
    raw!(w, "~StyleContent", ",\"styles\":");
    w.string(style.content_span.extract(ctx.source));
    raw!(w, "~StyleContent", ",\"comment\":");
    // Same `{type:"Comment", start, end, data}` shape as a fragment HTML comment.
    write_or_null(w, preceding_comment, |w, c| write_html_comment(w, c, ctx));
    raw!(w, "}}");
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
    w: &mut Sink<'_>,
    script: &ast::Script,
    html_leading_comment: Option<&ast::HtmlComment>,
    ctx: &Ctx<'_>,
) {
    head!(
        w,
        "Script",
        "{\"type\":\"Script\",\"start\":",
        script.span.start,
        script.span.end,
        ctx
    );
    raw!(w, "Script", ",\"context\":");
    // Escape-free `&'static str` (`default` / `module`) → skip the escape scan; on the tape
    // the word's index, resolved at build time.
    keyword!(
        w,
        script.context => as_str,
        ast::ScriptContext => [Default, Module],
        "Script"."context"
    );
    raw!(w, "Script", ",\"content\":");
    // A script whose `Program` carries comments (its own or a preceding HTML
    // comment) needs acorn's leading/trailing attach, which runs online off this
    // emit's own node opens and closes. The common case (no comments) fuses with
    // no attach at all.
    let comments = ctx.view.comments(script.content.comments);
    let attach = if comments.is_empty() && html_leading_comment.is_none() {
        None
    } else {
        Some(attach_script(
            comments,
            ctx.attach_inputs(),
            html_leading_comment,
        ))
    };
    let mode = attach
        .as_ref()
        .map_or(CommentMode::Off, CommentAttach::mode);
    write_program_embedded(w, &script.content, ctx.embed(mode));
    raw!(w, "Script", ",\"attributes\":");
    write_value_attributes(w, ctx.svelte.attributes(script.attributes), ctx);
    close!(w);
}

/// A `<svelte:options>`: everything fuses. Field order: `start, end,
/// attributes` then the skip-if-none `runes, immutable, css, accessors,
/// preserveWhitespace, namespace, customElement` (no `type`).
fn write_svelte_options(w: &mut Sink<'_>, options: &ast::SvelteOptions, ctx: &Ctx<'_>) {
    let attrs = ctx.svelte.attributes(options.attributes);
    head_object!(
        w,
        "~SvelteOptions",
        options.span.start,
        options.span.end,
        ctx
    );
    raw!(w, "~SvelteOptions", ",\"attributes\":");
    write_value_attributes(w, attrs, ctx);
    if let Some(runes) = bool_option(ctx.svelte, attrs, "runes", ctx.source) {
        raw!(w, "~SvelteOptions", ",\"runes\":");
        w.bool(runes);
    }
    if let Some(immutable) = bool_option(ctx.svelte, attrs, "immutable", ctx.source) {
        raw!(w, "~SvelteOptions", ",\"immutable\":");
        w.bool(immutable);
    }
    if let Some(css) = find_option_values(ctx.svelte, attrs, "css", ctx.source)
        .and_then(|v| text_value(v, ctx.source))
    {
        raw!(w, "~SvelteOptions", ",\"css\":");
        w.string(&css);
    }
    if let Some(accessors) = bool_option(ctx.svelte, attrs, "accessors", ctx.source) {
        raw!(w, "~SvelteOptions", ",\"accessors\":");
        w.bool(accessors);
    }
    if let Some(preserve_whitespace) =
        bool_option(ctx.svelte, attrs, "preserveWhitespace", ctx.source)
    {
        raw!(w, "~SvelteOptions", ",\"preserveWhitespace\":");
        w.bool(preserve_whitespace);
    }
    if let Some(namespace) = find_option_values(ctx.svelte, attrs, "namespace", ctx.source)
        .and_then(|v| text_value(v, ctx.source))
    {
        raw!(w, "~SvelteOptions", ",\"namespace\":");
        w.string(&namespace);
    }
    write_custom_element_field(w, attrs, ctx);
    close!(w);
}

/// Attributes outside the fragment tree (`<script>`/`<style>`/`<svelte:options>`
/// tags): the template attach passes never visit them, so each fuses through the
/// same attribute writer the fragment path uses but with a comment-free context —
/// no expression-tag value can pick up a template comment.
fn write_value_attributes(w: &mut Sink<'_>, attributes: &[ast::AttributeNode], ctx: &Ctx<'_>) {
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
fn write_pattern_island(w: &mut Sink<'_>, expr: &tsv_ts::ast::Expression, ctx: &Ctx<'_>) {
    let window = pattern_comment_window(ctx.view, expr);
    if !ctx.any_comment_in(window.start, window.end) {
        write_pattern_embedded(
            w,
            expr,
            ctx.embed_pattern(CommentMode::Off, CommentMode::Off),
        );
        return;
    }
    let attach = attach_binding_pattern(ctx.view, expr, ctx.attach_inputs());
    let (mode, annotation_mode) = attach.modes();
    write_pattern_embedded(w, expr, ctx.embed_pattern(mode, annotation_mode));
}

/// A fragment or `null` (the `AwaitBlock` branch fields and `IfBlock`'s
/// `alternate`, no skip).
fn write_optional_fragment(w: &mut Sink<'_>, fragment: Option<&ast::Fragment>, ctx: &Ctx<'_>) {
    write_or_null(w, fragment, |w, f| write_fragment(w, *f, ctx));
}
