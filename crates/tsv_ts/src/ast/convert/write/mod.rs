//! Writer-mode conversion: emit compact wire JSON directly from the internal AST.
//!
//! This is the **sole emission path** for the TS wire JSON: it walks the
//! *internal* AST once and writes the final JSON bytes as it goes, never
//! materializing a typed public tree. It writes both wires: the span-only one
//! behind `convert_ast_json_bytes` / `convert_ast_json_string` (every FFI/WASM/N-API
//! parse binding, the CLI's default output) and the loc-bearing one behind
//! `convert_ast_json_bytes_with_locations` (`tsv parse --locations`) — `--pretty` is a
//! re-indent of either's bytes.
//!
//! **Byte-identity**: the wire JSON is a faithful emission of the acorn quirk
//! catalog — each node's field order, `skip_serializing_if` behavior, `null`s
//! for non-skipped `Option`s, and scalar formatting match acorn-typescript's
//! JSON exactly (the shape each fixture's `expected.json` records).
//!
//! Dynamic strings are escaped by `JsonWriter::string`, byte-identical to
//! `serde_json`'s string serialization; only non-integral `f64` delegates to
//! `serde_json` (ryu). Static tokens (node types, operators, kinds) are known
//! escape-free and written verbatim; integers have a unique decimal form and
//! are hand-formatted.
//!
//! Three conversion-time mutations of already-converted children become
//! pre-computed decisions threaded down as flags (see `ExprFlags` in
//! `expressions`): the `?.<T>()` callee-optional force, the unparenthesized
//! decorator spine optional-strip, and the `TSParameterProperty`
//! assignment-pattern span override. The super-class
//! `TSInstantiationExpression` wrap is decided before its fields are emitted.

use super::super::internal;
use super::{Schema, bigint_to_decimal};
use tsv_lang::{Span, StageRun, Wire, WirePositions, WireTables};
// The JSON-scalar substrate is shared across the three language writers (so the
// Svelte writer can compose embedded TS/CSS emission into one buffer). Only the
// TS-specific node emitters (`node_header`, field helpers, `Ctx`) live here.
pub(super) use tsv_lang::{JsonWriter, write_array, write_or_null};

mod comments;
mod control_flow;
mod declarations;
mod expressions;
mod functions;
mod modules;
mod patterns;
mod statements;
mod types;

pub use comments::{CommentAttach, IslandComments};
use declarations::{write_decorator, write_type_parameter_declaration};
use statements::{write_statement, write_variable_declaration};
use types::{write_type_annotation, write_type_parameter_instantiation};

/// Convert an internal `Program` straight to its compact wire-JSON bytes on `wire`, in
/// the coordinates `crate::WIRE_COORDINATES` states.
///
/// One AST walk, no intermediate tree, the byte→UTF-16 translation fused in (identity
/// on ASCII).
///
/// Returns `Vec<u8>` rather than `String`: every emitted byte comes from `&str`
/// slices and ASCII fragments, so the output is valid UTF-8 by construction,
/// but proving that to a `String` costs an O(output) validation scan, and the
/// output is several times the source (`tsv_lang::estimated_json_capacity`).
/// Byte-oriented boundaries (FFI, the CLI's stdout) take the bytes as-is; `&str`
/// boundaries (`convert_ast_json_string` → WASM/N-API) pay the one validation at
/// the edge.
pub(crate) fn write_program_bytes(
    program: &internal::Program<'_>,
    source: &str,
    wire: Wire,
) -> Vec<u8> {
    let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
    let ctx = Ctx::new(source, tables.positions(), Schema::Acorn, CommentMode::Off);
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len(), wire));
    write_program(&mut w, program, &ctx);
    w.into_bytes()
}

/// Emit an embedded TS expression's wire JSON into a caller-owned writer, for
/// `tsv_svelte` composing template `{expr}` / directive / block expression
/// emission into its own buffer. Shares the host document's
/// `WirePositions` (spans are host-file coordinates, and the line table is the
/// host document's).
///
/// `comments` is this emission's comment role: `Attach` for a comment-bearing
/// template expression island, where acorn's leading/trailing attach runs
/// online off this emit's own node opens and closes and each node emits its
/// assigned comments at its close; `Off` for every ordinary emission.
#[inline]
pub fn write_expression_embedded(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    env: EmbedWriter<'_>,
) {
    let ctx = Ctx::from_embed(env);
    expressions::write_expression(w, expr, &ctx);
}

/// Emit an embedded standalone `VariableDeclaration`'s wire JSON, for
/// `tsv_svelte`'s `{const …}` / `{let …}` declaration tag. Shares the host
/// document's `WirePositions` (spans are host-file coordinates). `comments` as in
/// `write_expression_embedded`.
#[inline]
pub fn write_variable_declaration_embedded(
    w: &mut JsonWriter,
    var_decl: &internal::VariableDeclaration<'_>,
    env: EmbedWriter<'_>,
) {
    let ctx = Ctx::from_embed(env);
    write_variable_declaration(w, var_decl, &ctx, false);
}

/// Emit an embedded expression whose top-level `Identifier` carries an injected
/// `character` in its `loc` (the fused `inject_loc_character`), for the Svelte
/// shorthand attribute (`{name}`) and snippet name. The `character` is injected
/// only on a top-level `Identifier`, so any other expression emits exactly as
/// `write_expression_embedded` (character a no-op). `comments` is `Attach` for a
/// comment-bearing snippet name (`{#snippet /* c */ name(…)}`), where a
/// leading comment attaches to the `Identifier`.
#[inline]
pub fn write_identifier_expression_with_character(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    env: EmbedWriter<'_>,
) {
    let ctx = Ctx::from_embed(env);
    write_identifier_expression_with_character_in(w, expr, &ctx);
}

/// The shared body of the shorthand/snippet-name identifier emission.
fn write_identifier_expression_with_character_in(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    ctx: &Ctx<'_>,
) {
    if let internal::ExpressionKind::Identifier(id) = &expr.kind {
        write_identifier_parts_with_character(
            w,
            id.span,
            id.ident_name(),
            id.optional,
            id.type_annotation(),
            id.decorators(),
            ctx,
        );
    } else {
        expressions::write_expression(w, expr, ctx);
    }
}

/// Emit an embedded Svelte block pattern (`{#each … as ctx}`,
/// `{:then value}`/`{:catch error}`, `{@const id = …}`) into a caller-owned
/// writer.
///
/// A **simple identifier** gets `character` in its `loc` (`inject_loc_character`)
/// — Svelte reports it on the identifiers `read_identifier` creates directly.
///
/// A trailing `: T` is Svelte's second parse, so it is a second comment island:
/// `comments` is `Attach` for a comment-carrying destructure pattern
/// (`{@const { b = /* c */ 1 } = expr}`), whose canonical parse is a synthetic
/// `(pattern = 1)` acorn expression whose comment attach covers the pattern
/// subtree, and the annotation attaches under `annotation_comments` instead — see
/// `Ctx::annotation_comments`.
#[inline]
pub fn write_pattern_embedded(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    env: EmbedWriter<'_>,
) {
    let mut ctx = Ctx::from_embed(env);
    if let Some(ann) = crate::pattern_type_annotation(expr) {
        ctx.pattern_ann_span = ann.span;
    }
    match &expr.kind {
        internal::ExpressionKind::Identifier(id) => {
            // Simple identifier: inject `character` on its own `loc`.
            write_identifier_parts_with_character(
                w,
                id.span,
                id.ident_name(),
                id.optional,
                id.type_annotation(),
                id.decorators(),
                &ctx,
            );
        }
        // A destructure, or any other non-identifier pattern: `inject_loc_character`
        // is a no-op (it only touches a top-level `Identifier`).
        _ => expressions::write_expression(w, expr, &ctx),
    }
}

/// Emit the `Program` node.
fn write_program(w: &mut JsonWriter, program: &internal::Program<'_>, ctx: &Ctx<'_>) {
    node_header(w, "Program", program.span, ctx);
    w.raw(",\"body\":");
    write_body_array(w, program.body, ctx, |w, s| write_statement(w, s, ctx));
    w.raw(",\"sourceType\":");
    w.token(program.goal.source_type());
    close_node(w, "Program", program.span, ctx);
}

/// Emit an embedded `<script>` `Program`'s wire JSON into a caller-owned writer —
/// for `tsv_svelte` composing a `<script>` block's `content` into its own buffer.
/// Shares the host document's `WirePositions` (spans are host-file coordinates)
/// and threads the `Schema`; the node is emitted exactly as a standalone
/// `Program` is, its `loc` included (the content span's, under the host
/// document's line table).
pub fn write_program_embedded(
    w: &mut JsonWriter,
    program: &internal::Program<'_>,
    env: ProgramWriter<'_>,
) {
    let ProgramWriter {
        source,
        positions,
        schema,
        comments,
    } = env;
    let ctx = Ctx::new(source, positions, schema, comments);
    write_program(w, program, &ctx);
}

/// The comment role of an emission (the Svelte comment-attach paths).
///
/// `Off` for every ordinary emission — the hot path pays one never-taken
/// discriminant compare per node open and close. `Attach` is a comment-bearing
/// island: acorn's leading/trailing attach runs **online**, driven by this
/// emit's own node opens and closes, and each node emits the comments it was
/// assigned at its own close (see [`CommentAttach`]).
#[derive(Clone, Copy)]
pub enum CommentMode<'a> {
    Off,
    Attach(&'a CommentAttach<'a>),
}

/// The per-document inputs the four "plain" embedded writers share
/// (`write_expression_embedded`, `write_pattern_embedded`,
/// `write_variable_declaration_embedded`,
/// `write_identifier_expression_with_character`) — the source text, positions,
/// comment role and parser variant each one funnels into a `Ctx`.
///
/// Bundled into one `Copy` value so the call sites stop re-threading the same
/// arguments. It is an entry-boundary value: each writer destructures it into a
/// stack `Ctx` (`Ctx::from_embed`) and the per-node walk threads `&Ctx`, so it
/// is copied once per island rather than once per node. `ProgramWriter` is its
/// sibling for a `<script>`'s `Program`, and says there why that one is separate.
#[derive(Clone, Copy)]
pub struct EmbedWriter<'a> {
    pub source: &'a str,
    pub positions: WirePositions<'a>,
    pub comments: CommentMode<'a>,
    /// The canonical parser for this document — see `Ctx::vanilla_acorn`. It is
    /// component-global, so an island carries the same variant as the
    /// component's `<script>` blocks.
    pub vanilla_acorn: bool,
    /// The block pattern's trailing `: T` — its comment role, see
    /// `Ctx::annotation_comments`. `Off` for every other entry.
    pub annotation_comments: CommentMode<'a>,
}

/// The per-document inputs `write_program_embedded` takes — `EmbedWriter`'s
/// sibling for a `<script>`'s `Program`.
///
/// A separate bundle rather than a sixth `EmbedWriter` field because it carries the
/// `Schema`, from which the parser variant is *derived*, not passed, and an
/// `annotation_comments` role has no meaning here — only a block pattern can carry a
/// second parse.
#[derive(Clone, Copy)]
pub struct ProgramWriter<'a> {
    pub source: &'a str,
    pub positions: WirePositions<'a>,
    pub schema: Schema,
    pub comments: CommentMode<'a>,
}

/// The per-document environment every writer function shares (`source` and the
/// `WirePositions`).
///
/// `pattern_ann_span` is inert (the empty span) for every ordinary emission, where
/// the annotation test is one never-taken compare.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) source: &'a str,
    pub(super) positions: WirePositions<'a>,
    /// The span of a Svelte block pattern's **top-level** `TSTypeAnnotation` — the
    /// one Svelte reads with a *second* acorn parse (`read_type_annotation`'s `_ as `
    /// trick), which therefore emits under `annotation_comments`.
    ///
    /// `Span::new(u32::MAX, u32::MAX)` when inactive: never equal to a real
    /// annotation's span.
    pub(super) pattern_ann_span: Span,
    /// This pass's comment role (Svelte comment-attach paths). `Off` for every
    /// ordinary emission, so the hot path pays only a never-taken compare per
    /// node open and close.
    pub(super) comments: CommentMode<'a>,
    /// The canonical parser for this document is **vanilla acorn** (a Svelte
    /// non-`lang="ts"` component), not acorn-typescript. Drives the
    /// vanilla-only wire quirks: `options` rather than `arguments` for an
    /// `ImportExpression`'s second argument (vanilla acorn always emits the
    /// field, `null` when absent; acorn-typescript emits a skip-if-empty
    /// `arguments` array instead), and `value`-before-`kind` on get/set
    /// `Property` nodes (acorn-typescript's get/set path assigns `kind` first).
    /// `false` for standalone TS and every `lang="ts"` component.
    ///
    /// The fact is **component-global** (Svelte's single `this.ts`), so it is
    /// not `<script>`-scoped: every expression island — `{expr}`, an attribute
    /// or directive value, `{@const}`, a `{#snippet}` body — carries the same
    /// variant, and reaches this field through `EmbedWriter`.
    pub(super) vanilla_acorn: bool,
    /// The block pattern's trailing `: T` is a second acorn parse, so it is a second
    /// comment island too: the top-level annotation (`pattern_ann_span`) is emitted under
    /// this mode instead of `comments`. Svelte's `add_comments` runs once per parse over
    /// that parse's own comments, so a comment inside the pattern can never attach to the
    /// annotation, and one inside the annotation can never fall back to the pattern root —
    /// which one attach over both let happen wherever a walk left a comment unclaimed.
    /// `Off` for every ordinary emission.
    pub(super) annotation_comments: CommentMode<'a>,
}

impl<'a> Ctx<'a> {
    /// The per-document context for a whole-`Program` writer, and the one place
    /// `Schema` becomes the `vanilla_acorn` fact.
    ///
    /// Every per-document field is set in the initializer, like `from_embed`:
    /// a field that a caller must remember to overwrite afterwards is a default
    /// waiting to be inherited by the next caller, which is exactly how the
    /// embedded path shipped `vanilla_acorn: false` to every expression island.
    #[inline]
    fn new(
        source: &'a str,
        positions: WirePositions<'a>,
        schema: Schema,
        comments: CommentMode<'a>,
    ) -> Self {
        Ctx {
            source,
            positions,
            pattern_ann_span: Span::new(u32::MAX, u32::MAX),
            comments,
            vanilla_acorn: schema.is_svelte_script(),
            annotation_comments: CommentMode::Off,
        }
    }

    /// The per-document context for an embedded writer: the shared `EmbedWriter`
    /// inputs plus the inert pattern-annotation default. Sets every per-document
    /// field in the initializer (no post-construction re-assignment), so with
    /// the entry writers inlined the `EmbedWriter` aggregate scalar-replaces
    /// away.
    #[inline]
    fn from_embed(env: EmbedWriter<'a>) -> Self {
        Ctx {
            source: env.source,
            positions: env.positions,
            pattern_ann_span: Span::new(u32::MAX, u32::MAX),
            comments: env.comments,
            vanilla_acorn: env.vanilla_acorn,
            annotation_comments: env.annotation_comments,
        }
    }

    /// Byte offset → emitted (UTF-16 code unit) offset; identity on ASCII.
    #[inline]
    pub(super) fn pos(&self, byte: u32) -> u32 {
        self.positions.pos(byte)
    }
}

/// Close a node object: run this node's online comment attach — which emits any
/// `leadingComments`/`trailingComments` it was assigned — then the closing `}`.
/// The type and span mirror the node's own `node_header` call.
/// `CommentMode::Off` (every ordinary emission) makes this exactly
/// `w.raw("}")` after one never-taken branch.
#[inline]
pub(super) fn close_node(w: &mut JsonWriter, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    if let CommentMode::Attach(attach) = ctx.comments {
        attach.close_and_emit(w, node_type, span, ctx.positions);
    }
    w.raw("}");
}

/// Emit a node with no fields beyond the universal prefix (`ThisExpression`,
/// `Super`, keyword types, …).
#[inline]
pub(super) fn write_bare_node(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    node_header(w, node_type, span, ctx);
    close_node(w, node_type, span, ctx);
}

/// Report a node open to the online comment attach (`CommentMode::Attach` only
/// — one never-taken compare for every ordinary emission), which shifts this
/// node's leading comments off the queue. `node_header` calls this; the
/// hand-written header sites (the name-first `Identifier`, the widened-`end`
/// pattern header) call it directly so every wire node reaches the attach.
///
/// ⚠️ **Never gate this on whether the wire carries `loc`.** The attach is driven by node
/// opens and knows nothing about line/column, so a node skipped here on one wire re-binds
/// its comments to whatever opens next on that wire alone, and the two wires then attach
/// comments differently. The fixture gate pins the span-only wire's attachment, `check:loc`
/// holds the loc wire's to it, and `tests/loc_definition.rs` holds the span-only wire to
/// the loc wire stripped, byte for byte, over every fixture input.
#[inline]
pub(super) fn attach_open(node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    if let CommentMode::Attach(attach) = ctx.comments {
        attach.open(node_type, span);
    }
}

/// [`write_array`] over a container's `body` / `elements` / `properties`, telling
/// the online attach which element is its parent's last — acorn's
/// `is_last_in_body`, which a child needs at its own close and so cannot derive
/// for itself ([`CommentAttach::mark_last_body_element`], which says why it is a
/// MARK on the element as EMITTED rather than a span read at the container's
/// open).
///
/// The three plain-slice containers — `Program`, `BlockStatement`,
/// `ObjectExpression` — share it; `ArrayExpression`'s elements can be holes and
/// take [`write_body_array_holes`].
///
/// The mode test is hoisted out of the loop rather than asked per element, so an
/// ordinary emission — which has no last element to mark — takes the bare
/// [`write_array`], with no index to carry and no per-element compare, and asks
/// the mode once per container instead of once per last element. Measured
/// **flat** on the wire-write wall (`json_profile` over three real `src` trees,
/// interleaved rounds), at **−536 B `.text`**; it is here as hygiene — the same
/// "one never-taken compare, and only where one is needed" shape as
/// [`attach_open`] — not as a win.
#[inline]
pub(super) fn write_body_array<'a, T: 'a>(
    w: &mut JsonWriter,
    items: &'a [T],
    ctx: &Ctx<'_>,
    mut emit: impl FnMut(&mut JsonWriter, &'a T),
) {
    let CommentMode::Attach(attach) = ctx.comments else {
        write_array(w, items, |w, item| emit(w, item));
        return;
    };
    let len = items.len();
    write_array(w, items.iter().enumerate(), |w, (i, item)| {
        if i + 1 == len {
            attach.mark_last_body_element();
        }
        emit(w, item);
    });
}

/// [`write_body_array`] for `ArrayExpression`, whose `elements` may hold `null`.
///
/// A **trailing hole** (`[a,,]`) leaves the array with no last-in-body element at
/// all: acorn tests `elements.indexOf(node) === elements.length - 1`, and that last
/// index holds `null`, which is no node. Marking only a `Some` last element is
/// exactly that rule. (`write_expression_holes` is the unmarked twin, for
/// `ArrayPattern` — a type acorn's `is_last_in_body` test does not name.)
///
/// The mode test is hoisted as in [`write_body_array`], for the same reason.
#[inline]
pub(super) fn write_body_array_holes<'a, T: 'a>(
    w: &mut JsonWriter,
    items: &'a [Option<T>],
    ctx: &Ctx<'_>,
    mut emit: impl FnMut(&mut JsonWriter, &'a T),
) {
    let CommentMode::Attach(attach) = ctx.comments else {
        write_array(w, items, |w, item| match item {
            Some(item) => emit(w, item),
            None => w.null(),
        });
        return;
    };
    let len = items.len();
    write_array(w, items.iter().enumerate(), |w, (i, item)| match item {
        Some(item) => {
            if i + 1 == len {
                attach.mark_last_body_element();
            }
            emit(w, item);
        }
        None => w.null(),
    });
}

/// Emit the universal node prefix: `{"type":"X","start":N,"end":N,"loc":{…}`.
///
/// Leaves the object open — the caller appends its remaining fields and the
/// closing `}`. `span` is the span every one of `start`/`end`/`loc` derives
/// from (start/end are the fused char-space positions, `loc` their
/// line/column form, absent on the span-only wire); TS emits no
/// `Position.character`, so it is always omitted. Static fragments are
/// pre-fused into the fewest buffer writes — this runs once per node.
#[inline]
pub(super) fn node_header(w: &mut JsonWriter, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    node_header_impl::<false>(w, node_type, span, ctx);
}

/// A header whose wire **`end` is widened past the node's own span** — a Svelte
/// **block** binding pattern (`{#each xs as { a }: T}`, `{:then { a }: T}`).
///
/// acorn parses such a pattern bare, after which Svelte's `read_pattern`
/// (`1-parse/read/context.js`) patches `expression.end = typeAnnotation.end`. acorn
/// parsing the same pattern as a real *signature parameter* extends the span itself —
/// and there the internal span already covers the annotation, so the `max` below is a
/// no-op. The internal span therefore records the **bare** pattern (which the comment
/// attach keys on) and the widened `end` is recovered here from the annotation; `loc`
/// follows the wire `start`/`end`, as on every node.
///
/// Cold by construction: only reached by a destructuring pattern that actually
/// carries an annotation, so the hot per-node path keeps its branch-free
/// monomorphized header.
pub(super) fn node_header_wide_end(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: Span,
    wire_end: u32,
    ctx: &Ctx<'_>,
) {
    let wire_end = wire_end.max(span.end);
    #[cfg(debug_assertions)]
    debug_assert_overhanging_node_is_island_root(node_type, span, wire_end, ctx);
    attach_open(node_type, span, ctx);
    header_run::<false>(w, node_type, Span::new(span.start, wire_end), ctx);
}

/// Debug-only, ahead of a node's open: a node whose subtree ends at `subtree_end`, past
/// its own `span` — a typed Svelte block binding, whose `typeAnnotation` hangs off the
/// bare pattern — must be its comment-bearing island's root. It is the one shape that
/// breaks what the online attach's subtree skip leans on (a non-root node's children end
/// no later than it does; `comments.rs`'s module doc), and the root is the one node the
/// skip never needs it of, so the assumption is checked here, where the shape is made.
#[cfg(debug_assertions)]
fn debug_assert_overhanging_node_is_island_root(
    node_type: &'static str,
    span: Span,
    subtree_end: u32,
    ctx: &Ctx<'_>,
) {
    if subtree_end > span.end
        && let CommentMode::Attach(attach) = ctx.comments
    {
        attach.debug_assert_opens_island_root(node_type, span);
    }
}

/// Shared body of `node_header` and the name-first identifier emission;
/// `CHARACTER` (the fused `inject_loc_character`, injected into
/// `loc.start`/`loc.end` for the top-level `Identifier` of a simple block
/// pattern / shorthand) is a compile-time constant, so each wrapper
/// monomorphizes to its own straight-line emission (no runtime branch on the
/// per-node hot path).
fn node_header_impl<const CHARACTER: bool>(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    attach_open(node_type, span, ctx);
    header_run::<CHARACTER>(w, node_type, span, ctx);
}

/// The header's bytes — `{"type":"X"` and the position fields of `span` — as one
/// staged run: the part of a header that does not report to the comment attach.
#[expect(clippy::inline_always)]
#[inline(always)]
fn header_run<const CHARACTER: bool>(
    w: &mut JsonWriter,
    node_type: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    debug_assert!(
        node_type
            .bytes()
            .all(|b| b != b'"' && b != b'\\' && b >= 0x20),
        "node type must be escape-free: {node_type:?}"
    );
    let mut run = w.stage_run();
    run.raw("{\"type\":\"");
    run.short(node_type);
    run.raw("\"");
    position_fields::<CHARACTER>(&mut run, span, ctx);
    run.flush();
}

/// The `,"start":…,"end":…,"loc":{…}` position fields (final char space) —
/// the tail of `header_run`, also emitted after a leading `name` for the
/// Svelte-constructed identifiers whose fields precede the positions.
///
/// Emits into the caller's **staged run**, which the caller opens and flushes
/// around it — the whole header reaches the output buffer as one append (see
/// [`JsonWriter::stage_run`] for why).
///
/// `inline(always)` because it is handed the run by reference: outlined, the
/// run's length would live in memory across the call, which is the cost
/// holding it by value removes (see [`StageRun`]).
#[expect(clippy::inline_always)]
#[inline(always)]
fn position_fields<const CHARACTER: bool>(run: &mut StageRun<'_>, span: Span, ctx: &Ctx<'_>) {
    let Some(lines) = ctx.positions.lines() else {
        // The span-only wire: offsets only, no `loc` (and no `character`, which
        // lives inside `loc`). Only the byte→char `pos` is needed, so the
        // per-node line/column lookup is skipped entirely.
        run.raw(",\"start\":");
        run.u32(ctx.pos(span.start));
        run.raw(",\"end\":");
        run.u32(ctx.pos(span.end));
        return;
    };
    let ((start_pos, start), (end_pos, end)) = lines.span_positions(span.start, span.end);
    run.raw(",\"start\":");
    run.u32(start_pos);
    run.raw(",\"end\":");
    run.u32(end_pos);
    run.raw(",\"loc\":{\"start\":{\"line\":");
    let start_line = run.usize_kept(start.line);
    run.raw(",\"column\":");
    run.usize(start.column);
    if CHARACTER {
        run.raw(",\"character\":");
        run.u32(start_pos);
    }
    run.raw("},\"end\":{\"line\":");
    // Most nodes end on the line they start on, and that line's digits are
    // already in hand.
    if end.line == start.line {
        run.repeat(start_line, end.line);
    } else {
        run.usize(end.line);
    }
    run.raw(",\"column\":");
    run.usize(end.column);
    if CHARACTER {
        run.raw(",\"character\":");
        run.u32(end_pos);
    }
    run.raw("}}");
}

/// Emit `,"typeParameters":<declaration>` when present (skip-if-none field).
#[inline]
pub(super) fn write_type_parameters_field(
    w: &mut JsonWriter,
    type_parameters: Option<&internal::TSTypeParameterDeclaration<'_>>,
    ctx: &Ctx<'_>,
) {
    if let Some(tp) = type_parameters {
        w.raw(",\"typeParameters\":");
        write_type_parameter_declaration(w, tp, ctx);
    }
}

/// Emit `,"typeArguments":<instantiation>` when present (skip-if-none field).
#[inline]
pub(super) fn write_type_arguments_field(
    w: &mut JsonWriter,
    type_arguments: Option<&internal::TSTypeParameterInstantiation<'_>>,
    ctx: &Ctx<'_>,
) {
    if let Some(ta) = type_arguments {
        w.raw(",\"typeArguments\":");
        write_type_parameter_instantiation(w, ta, ctx);
    }
}

/// Emit `,"typeAnnotation":<annotation>` when present (skip-if-none field;
/// also the wire name of `TSMethodSignature`/signature-declaration return
/// types).
#[inline]
pub(super) fn write_type_annotation_field(
    w: &mut JsonWriter,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    ctx: &Ctx<'_>,
) {
    if let Some(ta) = type_annotation {
        w.raw(",\"typeAnnotation\":");
        write_type_annotation(w, ta, ctx);
    }
}

/// Emit `,"returnType":<annotation>` when present (skip-if-none field).
#[inline]
pub(super) fn write_return_type_field(
    w: &mut JsonWriter,
    return_type: Option<&internal::TSTypeAnnotation<'_>>,
    ctx: &Ctx<'_>,
) {
    if let Some(rt) = return_type {
        w.raw(",\"returnType\":");
        write_type_annotation(w, rt, ctx);
    }
}

/// Emit `,"expression":false,"generator":<bool>,"async":<bool>` — a non-arrow
/// function's three flags (`expression` is only ever `true` on an arrow). The
/// constant `expression` field, the `generator` field and the `async` key fold
/// into one literal per `generator` value.
#[inline]
pub(super) fn write_function_flags_fields(w: &mut JsonWriter, generator: bool, r#async: bool) {
    w.raw_pick(
        generator,
        b",\"expression\":false,\"generator\":true,\"async\":",
        b",\"expression\":false,\"generator\":false,\"async\":",
    );
    w.bool(r#async);
}

/// Emit `,"importKind":"type"|"value"`. `"type"` is always written; `"value"` is
/// omitted under vanilla acorn (Svelte non-`lang="ts"` context, see
/// `Ctx::vanilla_acorn`) and written under acorn-typescript. Each emitting arm
/// appends the whole field as one literal rather than the key followed by a
/// runtime-chosen token.
#[inline]
pub(super) fn write_import_kind_field(
    w: &mut JsonWriter,
    import_kind: internal::ImportKind,
    ctx: &Ctx<'_>,
) {
    match import_kind {
        internal::ImportKind::Type => w.raw(",\"importKind\":\"type\""),
        internal::ImportKind::Value if ctx.vanilla_acorn => {}
        internal::ImportKind::Value => w.raw(",\"importKind\":\"value\""),
    }
}

/// Emit `,"exportKind":"type"|"value"`, with the same omission rule as
/// [`write_import_kind_field`].
#[inline]
pub(super) fn write_export_kind_field(
    w: &mut JsonWriter,
    export_kind: internal::ExportKind,
    ctx: &Ctx<'_>,
) {
    match export_kind {
        internal::ExportKind::Type => w.raw(",\"exportKind\":\"type\""),
        internal::ExportKind::Value if ctx.vanilla_acorn => {}
        internal::ExportKind::Value => w.raw(",\"exportKind\":\"value\""),
    }
}

/// Emit an identifier's `,"name":` field — the single name-emission funnel.
/// Span-identity names are the raw source slice (the leading `raw_len` bytes at
/// `name_start`); escaped names are the decoded `&'arena str` (an escaped
/// identifier's `\u{78}` source form decodes to `x`). Both arms write the wire
/// value directly; no allocation.
///
/// The span-identity arm skips the escape scan: an `IdentName` with no
/// `escaped` form promises its source bytes hold nothing JSON escapes (see
/// [`internal::IdentName`] — a lexed `IdentifierName` by the grammar, any other
/// slice by its constructor's check), so the key and the quoted name are one
/// fixed-width append. The decoded arm keeps the scan — it is rare, and it is
/// also where a constructor parks a name it cannot vouch for.
///
/// `inline(never)`, and the pair of choices is measured, not stylistic: this is
/// the one out-of-line copy of the window write, and because both arms leave by
/// tail call (the decoded one to [`write_escaped_name_field`], the window's cold
/// grow and long paths to theirs) it saves no register at all. Left to the
/// inliner, the body folded into its callers and reshaped THEIR inlining — the
/// `Identifier` field writer came back out of line, a call and seven register
/// saves per name — which cost ~0.45% of the parse→JSON path's instructions on
/// TypeScript against this shape; `inline(always)` there was no better.
#[inline(never)]
pub(super) fn write_name_field(
    w: &mut JsonWriter,
    name: internal::IdentName<'_>,
    name_start: u32,
    ctx: &Ctx<'_>,
) {
    match name.escaped {
        Some(s) => write_escaped_name_field(w, s),
        None => {
            let start = name_start as usize;
            let end = start + name.raw_len as usize;
            debug_assert!(
                ctx.source.is_char_boundary(start) && ctx.source.is_char_boundary(end),
                "a span-identity name covers whole characters"
            );
            w.string_escape_free_led(NAME_KEY, &ctx.source.as_bytes()[start..end]);
        }
    }
}

/// The `,"name":` key an identifier's name follows.
const NAME_KEY: &[u8; 8] = b",\"name\":";

/// [`write_name_field`]'s decoded arm, out of line: it is rare, and inline its
/// key's append — a call on its grow path — made every name write keep its
/// values in callee-saved registers.
#[inline(never)]
fn write_escaped_name_field(w: &mut JsonWriter, name: &str) {
    w.raw_fixed(NAME_KEY);
    w.string(name);
}

/// Emit a numeric literal value the way acorn's JSON does: non-finite as
/// `null` (JSON has no Infinity/NaN — an overflow literal like `1e999`),
/// integral doubles below `1e21` as their expanded shortest-round-trip integer
/// digits and integral doubles at/above `1e21` in exponential form (JS
/// `Number::toString` / `JSON.stringify` semantics), everything else as ryu —
/// which matches JS except the one non-integral decade handled below.
pub(super) fn write_number_value(w: &mut JsonWriter, n: f64) {
    if !n.is_finite() {
        // ±Inf → null, matching JSON.stringify (a parsed literal is never NaN).
        w.null();
        return;
    }
    if n.fract() == 0.0 {
        // Below 2^53 every integral f64 is exact, so the shortest round-trip
        // representation *is* the integer's own digits — write them directly,
        // no format!/parse round trip.
        if n.abs() < 9_007_199_254_740_992.0 {
            w.i64(n as i64);
            return;
        }
        // Above 2^53 the shortest representation can denote the double with
        // fewer significant digits than the exact integer (JS prints that
        // expanded form), so go through Display + parse.
        let shortest = format!("{n}");
        if let Ok(v) = shortest.parse::<i64>() {
            w.i64(v);
            return;
        }
        if let Ok(v) = shortest.parse::<u64>() {
            w.u64(v);
            return;
        }
        // Beyond u64 but below 1e21, JS `Number::toString` still prints the
        // expanded integer (the spec's `k <= n <= 21` case); `shortest` —
        // Rust's shortest-round-trip Display — already holds those exact digits.
        // At/above 1e21 JS switches to exponential, where Rust's Display would
        // wrongly keep expanding, so that range falls through to ryu (`w.f64`).
        if n.abs() < 1e21 {
            w.raw(&shortest);
            return;
        }
    } else {
        // Non-integral. JS `Number::toString` uses fixed notation down to the
        // spec's `n = -5` (|x| in [1e-6, 1e-5)), whereas ryu switches to
        // scientific one decade earlier — the sole non-integral divergence.
        // In that single decade the point sits at position -5, so the fixed
        // form is `0.` + five zeros + the shortest significant digits.
        let a = n.abs();
        if (1e-6..1e-5).contains(&a) {
            // `{a:e}` is the shortest round-trip scientific form (`d[.ddd]e-6`);
            // its mantissa digits are exactly `s` in the spec.
            let sci = format!("{a:e}");
            let mantissa = sci.split('e').next().unwrap_or(&sci);
            let mut out = String::with_capacity(8 + mantissa.len());
            if n.is_sign_negative() {
                out.push('-');
            }
            out.push_str("0.00000");
            out.extend(mantissa.chars().filter(|&c| c != '.'));
            w.raw(&out);
            return;
        }
    }
    w.f64(n);
}

/// Emits a `Literal` node.
///
/// The common literals take [`write_literal_fields_fused`]'s single write; any
/// it declines takes [`write_literal_fields`].
pub(super) fn write_literal(w: &mut JsonWriter, lit: &internal::Literal<'_>, ctx: &Ctx<'_>) {
    node_header(w, "Literal", lit.span, ctx);
    #[cfg(debug_assertions)]
    let fields_from = w.as_bytes().len();
    if write_literal_fields_fused(w, lit, ctx) {
        #[cfg(debug_assertions)]
        {
            let mut full = JsonWriter::with_capacity(0);
            write_literal_fields(&mut full, lit, ctx);
            debug_assert_eq!(
                String::from_utf8_lossy(&w.as_bytes()[fields_from..]),
                String::from_utf8_lossy(full.as_bytes()),
                "the fused literal fields must be the full path's bytes"
            );
        }
    } else {
        write_literal_fields(w, lit, ctx);
    }
    close_node(w, "Literal", lit.span, ctx);
}

/// A `Literal`'s `value` and `raw` fields as one write, for the literals whose
/// two fields are one constant or one source token: a string with no escape to
/// decode, a plain decimal integer, `true`, `false` and `null`. Returns `false`,
/// having written nothing, for every other literal — and for a string or number
/// the writer's own conditions decline ([`JsonWriter::string_value_raw`],
/// [`JsonWriter::number_value_raw`]).
///
/// A numeric token is tested as text, not by its parsed value: one made only of
/// decimal digits with no leading zero is the shortest form of the integer it
/// names, so the value the parser stored prints as those same digits.
#[inline]
fn write_literal_fields_fused(
    w: &mut JsonWriter,
    lit: &internal::Literal<'_>,
    ctx: &Ctx<'_>,
) -> bool {
    match lit.value {
        internal::LiteralValue::String(internal::StringCooked::Verbatim) => {
            w.string_value_raw(lit.span.extract(ctx.source).as_bytes())
        }
        internal::LiteralValue::Number(_) => {
            w.number_value_raw(lit.span.extract(ctx.source).as_bytes())
        }
        internal::LiteralValue::Boolean(true) => {
            w.raw(",\"value\":true,\"raw\":\"true\"");
            true
        }
        internal::LiteralValue::Boolean(false) => {
            w.raw(",\"value\":false,\"raw\":\"false\"");
            true
        }
        internal::LiteralValue::Null => {
            w.raw(",\"value\":null,\"raw\":\"null\"");
            true
        }
        internal::LiteralValue::String(internal::StringCooked::Decoded(_))
        | internal::LiteralValue::BigInt => false,
    }
}

/// A `Literal`'s `value`, `raw` and (for a BigInt) `bigint` fields, field by
/// field — every literal's general emission.
fn write_literal_fields(w: &mut JsonWriter, lit: &internal::Literal<'_>, ctx: &Ctx<'_>) {
    w.raw(",\"value\":");
    // `bigint` is emitted only for BigInt literals (`skip_serializing_if` on
    // `Option`), and shares the decimal string with `value`.
    let mut bigint: Option<String> = None;
    match &lit.value {
        internal::LiteralValue::Number(n) => write_number_value(w, *n),
        internal::LiteralValue::String(cooked) => {
            w.string(cooked.resolve(lit.span, ctx.source));
        }
        internal::LiteralValue::BigInt => {
            let decimal = bigint_to_decimal(lit.bigint_digits(ctx.source));
            w.string(&decimal);
            bigint = Some(decimal);
        }
        internal::LiteralValue::Boolean(b) => w.bool(*b),
        internal::LiteralValue::Null => w.null(),
    }
    let raw = lit.span.extract(ctx.source);
    if let internal::LiteralValue::String(_) = lit.value {
        w.raw(",\"raw\":");
        w.string(raw);
    } else {
        // A numeric, BigInt, boolean or `null` token is escape-free by grammar —
        // digits, letters, `.`, `_`, and a sign only inside an exponent — so its
        // raw text takes the name field's unscanned write.
        w.string_escape_free_led(b",\"raw\":", raw.as_bytes());
    }
    if let Some(decimal) = bigint {
        w.raw(",\"bigint\":");
        w.string(&decimal);
    }
}

/// Shared `Identifier` node emission. Emits the `Identifier` fields: `name`,
/// then `optional` (only when true), `typeAnnotation` (only when present),
/// `decorators` (only when non-empty).
pub(super) fn write_identifier_parts(
    w: &mut JsonWriter,
    span: Span,
    name: internal::IdentName<'_>,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "Identifier", span, ctx);
    write_identifier_fields(w, span, name, optional, type_annotation, decorators, ctx);
}

/// `write_identifier_parts` with `character` injected into the node's `loc`
/// (the fused `inject_loc_character`) — the top-level `Identifier` of a simple
/// Svelte block pattern / shorthand. Svelte's parser constructs these
/// identifiers itself, with `name` ahead of the positions
/// (`{type, name, start, end, loc}`), unlike acorn-parsed identifiers.
pub(super) fn write_identifier_parts_with_character(
    w: &mut JsonWriter,
    span: Span,
    name: internal::IdentName<'_>,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    // A typed simple block binding is the bare `Identifier` the annotation hangs
    // off, past its span.
    #[cfg(debug_assertions)]
    debug_assert_overhanging_node_is_island_root(
        "Identifier",
        span,
        type_annotation.map_or(span.end, |ta| ta.span.end),
        ctx,
    );
    attach_open("Identifier", span, ctx);
    w.raw("{\"type\":\"Identifier\"");
    write_name_field(w, name, span.start, ctx);
    // `name` is escape-sensitive and precedes the positions, so it can't join
    // the staged run — the run opens after it and covers the positions alone.
    let mut run = w.stage_run();
    position_fields::<true>(&mut run, span, ctx);
    run.flush();
    write_identifier_tail(w, span, optional, type_annotation, decorators, ctx);
}

/// The `Identifier` fields after the node header: `name`, then the tail.
#[inline]
fn write_identifier_fields(
    w: &mut JsonWriter,
    span: Span,
    name: internal::IdentName<'_>,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    write_name_field(w, name, span.start, ctx);
    write_identifier_tail(w, span, optional, type_annotation, decorators, ctx);
}

/// The skip-if-empty `Identifier` fields (`optional` / `typeAnnotation` /
/// `decorators`) and the closing `}`.
#[inline]
fn write_identifier_tail(
    w: &mut JsonWriter,
    span: Span,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    if optional {
        w.raw(",\"optional\":true");
    }
    write_type_annotation_field(w, type_annotation, ctx);
    if let Some(decs) = decorators
        && !decs.is_empty()
    {
        w.raw(",\"decorators\":");
        write_array(w, decs, |w, d| write_decorator(w, d, ctx));
    }
    close_node(w, "Identifier", span, ctx);
}

/// Emits a plain `Identifier` node: no optional flag, no type annotation, no
/// decorators — regardless of what the binding carries.
///
/// The common identifier — a short unescaped name, no comment attach running —
/// is written as a single staged run, header through closing `}`: the node has
/// no child, so nothing has to interrupt the run, and the name's key, quotes
/// and close ride the header's one append instead of a window write and a
/// `}` append of their own.
///
/// `inline(never)` to keep it out of the recursive writers' frames: once the name
/// write went out of line this body became small enough to inline, and each inlined
/// copy parked an `IdentName` in its caller's frame — `write_statement`'s grew from
/// 88 to 168 bytes, a frame paid at every level of statement nesting. Kept out of
/// line it costs ~0.03% of the parse→JSON path's instructions on TypeScript.
#[inline(never)]
pub(super) fn write_identifier_plain(
    w: &mut JsonWriter,
    id: &internal::Identifier<'_>,
    ctx: &Ctx<'_>,
) {
    let name = id.ident_name();
    let name_len = name.raw_len as usize;
    if name.escaped.is_some()
        || name_len > StageRun::SHORT_MAX
        || !matches!(ctx.comments, CommentMode::Off)
    {
        write_identifier_parts(w, id.span, name, false, None, None, ctx);
        return;
    }
    // The whole node as one staged run. It is `write_identifier_parts`'s bytes
    // under the three conditions just tested: a span-identity name is its
    // source bytes, which hold nothing JSON escapes (`write_name_field`), so it
    // can be staged where a scanned string could not; a name no longer than
    // `StageRun::SHORT_MAX` is copied inline, and cannot overrun the scratch
    // behind a header; and with no comment attach the node's open and close
    // report to nothing, so the close is the bare `}`.
    let start = id.span.start as usize;
    let name = &ctx.source.as_bytes()[start..start + name_len];
    debug_assert!(
        name.iter().all(|&b| b != b'"' && b != b'\\' && b >= 0x20),
        "a span-identity name holds nothing JSON escapes"
    );
    let mut run = w.stage_run();
    run.raw("{\"type\":\"Identifier\"");
    position_fields::<false>(&mut run, id.span, ctx);
    run.raw(",\"name\":\"");
    run.short_bytes(name);
    run.raw("\"}");
    run.flush();
}

/// An `Identifier` carrying only the binding's `optional` flag (function and
/// method ids, entity-name-as-expression nodes) — no type annotation or
/// decorators.
#[inline]
pub(super) fn write_identifier_with_optional(
    w: &mut JsonWriter,
    id: &internal::Identifier<'_>,
    ctx: &Ctx<'_>,
) {
    write_identifier_parts(w, id.span, id.ident_name(), id.optional, None, None, ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire variants a test runs: both, unless the build lacks the loc-bearing one
    /// (`tsv_lang`'s `locations` feature, off in a bare `cargo test -p tsv_ts`).
    const WIRES: &[Wire] = &[
        #[cfg(feature = "locations")]
        Wire::Loc,
        Wire::Span,
    ];

    /// Run `emit` under a standalone-TypeScript context over `source`, for one
    /// wire variant and one comment role.
    fn emitted(
        source: &str,
        wire: Wire,
        comments: CommentMode<'_>,
        emit: impl FnOnce(&mut JsonWriter, &Ctx<'_>),
    ) -> String {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
        let ctx = Ctx::new(source, tables.positions(), Schema::Acorn, comments);
        let mut w = JsonWriter::with_capacity(0);
        emit(&mut w, &ctx);
        String::from_utf8(w.into_bytes()).expect("the wire is UTF-8")
    }

    /// The sole statement of `source` as the `Identifier` its expression is.
    fn sole_identifier<'a>(program: &'a internal::Program<'a>) -> &'a internal::Identifier<'a> {
        let [statement] = program.body else {
            panic!("one statement expected");
        };
        let internal::StatementKind::ExpressionStatement(statement) = &statement.kind else {
            panic!("an expression statement expected");
        };
        let internal::ExpressionKind::Identifier(id) = &statement.expression.kind else {
            panic!("an identifier expected");
        };
        id
    }

    /// The staged header against `node_header_wide_end`, which writes the same
    /// header through the direct emitters — over a span whose two ends sit on
    /// different lines behind a multibyte character, and at node-type lengths
    /// either side of the staged run's inline copy (a real node type is at most
    /// 31 bytes; a longer one takes the run's out-of-line copy).
    #[test]
    fn node_header_matches_the_direct_emitters() {
        const TYPE: &str = "TSConstructSignatureDeclarationAndThenSomeMore";
        let source = "é;\n\n  abc(\n);\n";
        let start = source.find("abc").expect("present") as u32;
        let span = Span::new(start, source.len() as u32 - 2);
        for len in [1, 31, 32, 33, TYPE.len()] {
            let node_type: &'static str = &TYPE[..len];
            for &wire in WIRES {
                let staged = emitted(source, wire, CommentMode::Off, |w, ctx| {
                    node_header(w, node_type, span, ctx);
                });
                let direct = emitted(source, wire, CommentMode::Off, |w, ctx| {
                    node_header_wide_end(w, node_type, span, span.end, ctx);
                });
                assert_eq!(staged, direct, "type length {len}, {wire:?}");
                if wire != Wire::Span {
                    assert_eq!(
                        staged,
                        format!(
                            "{{\"type\":\"{node_type}\",\"start\":6,\"end\":12,\"loc\":{{\"start\":\
                             {{\"line\":3,\"column\":2}},\"end\":{{\"line\":4,\"column\":1}}}}"
                        )
                    );
                }
            }
        }
    }

    /// `write_identifier_plain`'s single-run arm against the general identifier
    /// emission it stands in for, and against the bytes spelled out, at each edge
    /// of its gate: the name length either side of `StageRun::SHORT_MAX`, a
    /// non-ASCII name (whose byte length is what the gate reads) either side of
    /// it too, a name longer than the whole staging scratch, and an escaped
    /// name, which carries its decoded form.
    #[test]
    fn plain_identifier_run_matches_the_general_emission() {
        let ascii = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        // Two bytes and one UTF-16 unit a character.
        let wide = "éèêëàâäîïôöùûüçñ".repeat(2);
        let cases: Vec<(String, String)> = [
            &ascii[..1],
            &ascii[..2],
            &ascii[..7],
            &ascii[..8],
            &ascii[..16],
            &ascii[..31],
            &ascii[..32],
            &ascii[..33],
            &ascii[..40],
            "$",
            "_$1",
            &wide[..2],
            &wide[..30],
            &wide[..32],
            &wide[..34],
        ]
        .into_iter()
        .map(|name| (name.to_owned(), name.to_owned()))
        .chain([(ascii[..50].repeat(8), ascii[..50].repeat(8))])
        .chain([
            ("\\u0061bc".to_owned(), "abc".to_owned()),
            ("a\\u{62}".to_owned(), "ab".to_owned()),
        ])
        .collect();
        for (written, name) in &cases {
            let source = format!("\n  {written};\n");
            let arena = bumpalo::Bump::new();
            let program = crate::parse(&source, &arena).expect("parses");
            let id = sole_identifier(&program);
            let end = 3 + written.encode_utf16().count();
            let column = end - 1;
            for &wire in WIRES {
                let plain = emitted(&source, wire, CommentMode::Off, |w, ctx| {
                    write_identifier_plain(w, id, ctx);
                });
                let general = emitted(&source, wire, CommentMode::Off, |w, ctx| {
                    write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
                });
                assert_eq!(plain, general, "{written:?}, {wire:?}");
                let loc = if wire != Wire::Span {
                    format!(
                        ",\"loc\":{{\"start\":{{\"line\":2,\"column\":2}},\"end\":\
                         {{\"line\":2,\"column\":{column}}}}}"
                    )
                } else {
                    String::new()
                };
                assert_eq!(
                    plain,
                    format!(
                        "{{\"type\":\"Identifier\",\"start\":3,\"end\":{end}{loc},\
                         \"name\":\"{name}\"}}"
                    ),
                    "{written:?}, {wire:?}"
                );
            }
        }
    }

    /// Under a comment attach the identifier keeps the general emission: its open
    /// and close are what hand it its comments, and the single-run arm reports
    /// neither.
    #[test]
    fn plain_identifier_under_a_comment_attach_emits_its_comments() {
        let source = "/* c */ name";
        let arena = bumpalo::Bump::new();
        let (expression, comments) =
            crate::parse_expression_with_comments(source, 0, &arena).expect("parses");
        let internal::ExpressionKind::Identifier(id) = &expression.kind else {
            panic!("an identifier expected");
        };
        let attach = || {
            CommentAttach::new(
                source,
                IslandComments {
                    queue: comments.iter().collect(),
                    root_parent_end: None,
                    root_fallback: true,
                    html_leading: None,
                },
            )
        };
        for &wire in WIRES {
            let island = attach();
            let plain = emitted(source, wire, island.mode(), |w, ctx| {
                write_identifier_plain(w, id, ctx);
            });
            let island = attach();
            let general = emitted(source, wire, island.mode(), |w, ctx| {
                write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
            });
            assert_eq!(plain, general, "{wire:?}");
            assert!(
                plain.contains(",\"name\":\"name\",\"leadingComments\":[{\"type\":\"Block\""),
                "the leading comment is emitted: {plain}"
            );
        }
    }
}
