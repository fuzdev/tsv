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
//! catalog — key presence, `skip_serializing_if` behavior, `null`s for
//! non-skipped `Option`s, and scalar formatting match acorn-typescript's JSON
//! (the shape each fixture's `expected.json` records); key order is tsv's own,
//! one per node type (docs/conformance_svelte.md §Key Order).
//!
//! Dynamic strings are escaped by `JsonWriter::string`, byte-identical to
//! `serde_json`'s string serialization; only non-integral `f64` goes to
//! `JsonWriter::f64` (`zmij`, the formatter `serde_json` calls). Static tokens
//! (node types, operators, kinds) are known escape-free and written verbatim;
//! integers have a unique decimal form and are hand-formatted.
//!
//! # One walk, two products
//!
//! This body is mounted once per sink: as `convert::write`, the JSON copy, and behind `_tape`
//! as `convert::write_tape::write`, the tape copy (`docs/tape_format.md` §A direct writer). A
//! site states its JSON literal once, through a site macro (`write_macros.rs`): the JSON copy
//! writes it, the tape copy replays the ops a compile-time reader made of it
//! (`tsv_lang::tape::literal_ops`) — records, presence bits, list brackets, and each value as
//! the field its key declared lays it down. Each mount defines `tape!` / `json_only!` before
//! mounting, and provides by name the functions it writes its own way (`write_json.rs`,
//! `write_tape.rs`): the staged runs, the name window, a literal's fused fields, a number's
//! spelling. crates/tsv_ts/CLAUDE.md §Distinctives says how to edit a site.
//!
//! Three conversion-time mutations of already-converted children become
//! pre-computed decisions threaded down as flags (see `ExprFlags` in
//! `expressions`): the `?.<T>()` callee-optional force, the unparenthesized
//! decorator spine optional-strip, and the `TSParameterProperty`
//! assignment-pattern span override. The super-class
//! `TSInstantiationExpression` wrap is decided before its fields are emitted.

use super::bigint_to_decimal;
// The functions each mount writes its own way (`write_json.rs`, `write_tape.rs`).
use super::{
    ChildKey, Ent, header_run, write_chain_any, write_identifier_plain, write_literal_fields_any,
    write_name_field, write_number_value,
};
use crate::ast;
pub(super) use crate::ast::convert::comments::{CommentAttach, CommentMode, EmbedWriter};
use tsv_lang::{Span, Wire, WirePositions, WireTables};

json_only! {
    /// The JSON copy's sink. The JSON-scalar substrate is shared across the three language
    /// writers (so the Svelte writer can compose embedded TS/CSS emission into one buffer);
    /// only the TS-specific node emitters (`node_header`, field helpers, `Ctx`) live here.
    type Sink<'a> = JsonWriter;
    pub(super) use tsv_lang::{JsonWriter, write_array, write_or_null};
}
tape! {
    /// The tape copy's sink, and its list helpers (`write_tape.rs`).
    type Sink<'a> = tsv_lang::tape::TapeWriter<'a>;
    pub(super) use super::{write_array, write_or_null};
}

mod control_flow;
mod declarations;
pub(super) mod expressions;
mod functions;
mod modules;
mod patterns;
mod statements;
mod types;

use declarations::{write_decorator, write_type_parameter_declaration};
use statements::{write_statement, write_variable_declaration};
use types::{write_type_annotation, write_type_parameter_instantiation};

json_only! {
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
        parsed: ast::ParsedRef<'_>,
        source: &str,
        wire: Wire,
    ) -> Vec<u8> {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, wire);
        let ctx = Ctx::new(source, tables.positions(), CommentMode::Off, parsed.view);
        let capacity = tsv_lang::estimated_json_capacity(source.len(), wire);
        let mut w = JsonWriter::with_capacity(capacity);
        write_program(&mut w, parsed.program, &ctx);
        w.into_bytes()
    }
}

tape! {
    /// The span-only wire's TAPE of an internal `Program`, written into `sink` by the same walk
    /// as `write_program_bytes`, in the coordinates `crate::WIRE_COORDINATES` states (a leading
    /// byte-order mark counted, as acorn counts it). Finish it with `sink.finish_words()` /
    /// `sink.finish_bytes()`.
    pub(crate) fn write_program_tape(
        parsed: ast::ParsedRef<'_>,
        source: &str,
        sink: &mut tsv_lang::tape::TapeSink,
    ) {
        let tables = WireTables::new(source, crate::WIRE_COORDINATES, Wire::Span);
        let ctx = Ctx::new(source, tables.positions(), CommentMode::Off, parsed.view);
        let mut w = sink.writer(source, ctx.positions, crate::WIRE_COORDINATES.bom);
        write_program(&mut w, parsed.program, &ctx);
    }
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
pub fn write_expression_embedded(w: &mut Sink<'_>, expr: &ast::Expression, env: EmbedWriter<'_>) {
    let ctx = Ctx::from_embed(env);
    expressions::write_expression(w, expr, &ctx);
}

/// Emit an embedded standalone `VariableDeclaration`'s wire JSON, for
/// `tsv_svelte`'s `{const …}` / `{let …}` declaration tag. Shares the host
/// document's `WirePositions` (spans are host-file coordinates). `comments` as in
/// `write_expression_embedded`.
#[inline]
pub fn write_variable_declaration_embedded(
    w: &mut Sink<'_>,
    var_decl: &ast::VariableDeclaration,
    env: EmbedWriter<'_>,
) {
    let ctx = Ctx::from_embed(env);
    write_variable_declaration(w, var_decl, &ctx);
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
    w: &mut Sink<'_>,
    expr: &ast::Expression,
    env: EmbedWriter<'_>,
) {
    let ctx = Ctx::from_embed(env);
    write_identifier_expression_with_character_in(w, expr, &ctx);
}

/// The shared body of the shorthand/snippet-name identifier emission.
fn write_identifier_expression_with_character_in(
    w: &mut Sink<'_>,
    expr: &ast::Expression,
    ctx: &Ctx<'_>,
) {
    if let ast::ExpressionKind::Identifier(id) = &expr.kind {
        write_identifier_parts_with_character(
            w,
            id.span,
            id.ident_name(),
            id.optional,
            id.type_annotation(ctx.view),
            id.decorators(ctx.view),
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
/// `pattern = 1` acorn expression whose comment attach covers the pattern
/// subtree, and the annotation attaches under `annotation_comments` instead — see
/// `Ctx::annotation_comments`.
#[inline]
pub fn write_pattern_embedded(w: &mut Sink<'_>, expr: &ast::Expression, env: EmbedWriter<'_>) {
    let mut ctx = Ctx::from_embed(env);
    if let Some(ann) = crate::__private::pattern_type_annotation(ctx.view, expr) {
        ctx.pattern_ann_span = ann.span;
    }
    match &expr.kind {
        ast::ExpressionKind::Identifier(id) => {
            // Simple identifier: inject `character` on its own `loc`.
            write_identifier_parts_with_character(
                w,
                id.span,
                id.ident_name(),
                id.optional,
                id.type_annotation(ctx.view),
                id.decorators(ctx.view),
                &ctx,
            );
        }
        // A destructure, or any other non-identifier pattern: `inject_loc_character`
        // is a no-op (it only touches a top-level `Identifier`).
        _ => expressions::write_expression(w, expr, &ctx),
    }
}

/// Emit the `Program` node.
fn write_program(w: &mut Sink<'_>, program: &ast::Program, ctx: &Ctx<'_>) {
    node_header!(w, "Program", program.span, ctx);
    raw!(w, "Program", ",\"body\":");
    write_body_array(w, ctx.view.stmt_list(program.body), ctx, |w, s| {
        write_statement(w, ctx.view.stmt(*s), ctx);
    });
    raw!(w, "Program", ",\"sourceType\":");
    keyword!(
        w,
        token,
        program.goal => source_type,
        crate::Goal => [Module, Script],
        "Program"."sourceType"
    );
    close_node(w, "Program", program.span, ctx);
}

/// Emit an embedded `<script>` `Program`'s wire JSON into a caller-owned writer —
/// for `tsv_svelte` composing a `<script>` block's `content` into its own buffer.
/// Shares the host document's `WirePositions` (spans are host-file coordinates)
/// and takes the island's parser variant from `env.vanilla_acorn` (a plain
/// `<script>` in a non-TS component is vanilla acorn's: `importKind`/`exportKind`
/// omitted when `"value"`, `attributes` always emitted on import/export
/// declarations, plus every expression quirk the islands carry); the node is
/// emitted exactly as a standalone `Program` is, its `loc` included (the content
/// span's, under the host document's line table). `env.annotation_comments` is
/// inert here — only a block pattern carries a second parse.
pub fn write_program_embedded(w: &mut Sink<'_>, program: &ast::Program, env: EmbedWriter<'_>) {
    let ctx = Ctx::from_embed(env);
    write_program(w, program, &ctx);
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
    /// Resolves the tree's ids, runs and pool text.
    pub(super) view: ast::AstView<'a>,
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
    /// `arguments` array instead), `importKind`/`exportKind` omitted where
    /// `"value"`, and `attributes` always emitted on import/export declarations.
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
    /// The per-document context for a standalone TypeScript document — acorn-typescript's
    /// wire, so never vanilla acorn's.
    ///
    /// Every per-document field is set in the initializer, like `from_embed`:
    /// a field that a caller must remember to overwrite afterwards is a default
    /// waiting to be inherited by the next caller, which is exactly how the
    /// embedded path shipped `vanilla_acorn: false` to every expression island.
    #[inline]
    pub(super) fn new(
        source: &'a str,
        positions: WirePositions<'a>,
        comments: CommentMode<'a>,
        view: ast::AstView<'a>,
    ) -> Self {
        Ctx {
            source,
            positions,
            view,
            pattern_ann_span: Span::new(u32::MAX, u32::MAX),
            comments,
            vanilla_acorn: false,
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
            view: env.view,
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
///
/// On the tape the `}` is the record's close called directly, not a `raw!` literal: every node
/// closes here, so the literal interpreter's loop and dispatch were the single largest share of
/// its replays, for one op whose checks (a key left pending, the balance) the close itself makes.
#[inline]
pub(super) fn close_node(w: &mut Sink<'_>, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    if let CommentMode::Attach(attach) = ctx.comments {
        attach.close_and_emit(w, node_type, span, ctx.positions);
    }
    tape! { w.t_close(); }
    json_only! { w.raw("}"); }
}

/// Emit a node with no fields beyond the universal prefix (`ThisExpression`,
/// `Super`, keyword types, …): `node_type` on the JSON wire, `entry` (`ent!`) on the tape.
#[inline]
pub(super) fn write_bare_node(
    w: &mut Sink<'_>,
    node_type: &'static str,
    entry: Ent,
    span: Span,
    ctx: &Ctx<'_>,
) {
    json_only! { let () = entry; }
    node_header!(w, @dyn node_type => entry, span, ctx);
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
    w: &mut Sink<'_>,
    items: &'a [T],
    ctx: &Ctx<'_>,
    mut emit: impl FnMut(&mut Sink<'_>, &'a T),
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
    w: &mut Sink<'_>,
    items: &'a [Option<T>],
    ctx: &Ctx<'_>,
    mut emit: impl FnMut(&mut Sink<'_>, &'a T),
) {
    let CommentMode::Attach(attach) = ctx.comments else {
        write_array(w, items, |w, item| match item {
            Some(item) => emit(w, item),
            None => null_item!(w),
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
        None => null_item!(w),
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
pub(super) fn node_header(w: &mut Sink<'_>, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
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
    w: &mut Sink<'_>,
    node_type: &'static str,
    entry: Ent,
    span: Span,
    wire_end: u32,
    ctx: &Ctx<'_>,
) {
    let wire_end = wire_end.max(span.end);
    #[cfg(debug_assertions)]
    debug_assert_overhanging_node_is_island_root(node_type, span, wire_end, ctx);
    tape! {
        #[cfg(debug_assertions)]
        assert_eq!(
            ::tsv_lang::tape::shapes::ENTRIES[usize::from(entry)].ty,
            Some(node_type),
            "a header's tape entry is its JSON type's"
        );
        w.t_open(entry);
    }
    json_only! { let () = entry; }
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

/// Shared body of `node_header` and the `character`-bearing identifier emission;
/// `CHARACTER` (the fused `inject_loc_character`, injected into
/// `loc.start`/`loc.end` for the top-level `Identifier` of a simple block
/// pattern / shorthand) is a compile-time constant, so each wrapper
/// monomorphizes to its own straight-line emission (no runtime branch on the
/// per-node hot path).
fn node_header_impl<const CHARACTER: bool>(
    w: &mut Sink<'_>,
    node_type: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    attach_open(node_type, span, ctx);
    header_run::<CHARACTER>(w, node_type, span, ctx);
}

/// Emit `,"typeParameters":<declaration>` when present (skip-if-none field).
#[inline]
pub(super) fn write_type_parameters_field<const E: u16>(
    w: &mut Sink<'_>,
    type_parameters: Option<&ast::TSTypeParameterDeclaration>,
    ctx: &Ctx<'_>,
) {
    if let Some(tp) = type_parameters {
        raw!(w, @E, ",\"typeParameters\":");
        write_type_parameter_declaration(w, tp, ctx);
    }
}

/// Emit `,"typeArguments":<instantiation>` when present (skip-if-none field).
#[inline]
pub(super) fn write_type_arguments_field<const E: u16>(
    w: &mut Sink<'_>,
    type_arguments: Option<&ast::TSTypeParameterInstantiation>,
    ctx: &Ctx<'_>,
) {
    if let Some(ta) = type_arguments {
        raw!(w, @E, ",\"typeArguments\":");
        write_type_parameter_instantiation(w, ta, ctx);
    }
}

/// Emit `,"typeAnnotation":<annotation>` when present (skip-if-none field;
/// also the wire name of `TSMethodSignature`/signature-declaration return
/// types).
#[inline]
pub(super) fn write_type_annotation_field<const E: u16>(
    w: &mut Sink<'_>,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    ctx: &Ctx<'_>,
) {
    if let Some(ta) = type_annotation {
        raw!(w, @E, ",\"typeAnnotation\":");
        write_type_annotation(w, ta, ctx);
    }
}

/// Emit `,"returnType":<annotation>` when present (skip-if-none field).
#[inline]
pub(super) fn write_return_type_field<const E: u16>(
    w: &mut Sink<'_>,
    return_type: Option<&ast::TSTypeAnnotation>,
    ctx: &Ctx<'_>,
) {
    if let Some(rt) = return_type {
        raw!(w, @E, ",\"returnType\":");
        write_type_annotation(w, rt, ctx);
    }
}

/// Emit `,"expression":false,"generator":<bool>,"async":<bool>` — a non-arrow
/// function's three flags (`expression` is only ever `true` on an arrow). The
/// constant `expression` field, the `generator` field and the `async` key fold
/// into one literal per `generator` value.
#[inline]
pub(super) fn write_function_flags_fields<const E: u16>(
    w: &mut Sink<'_>,
    generator: bool,
    r#async: bool,
) {
    pick!(
        w,
        @E,
        generator,
        ",\"expression\":false,\"generator\":true,\"async\":",
        ",\"expression\":false,\"generator\":false,\"async\":"
    );
    w.bool(r#async);
}

/// Emit `,"importKind":"type"|"value"`. `"type"` is always written; `"value"` is
/// omitted under vanilla acorn (Svelte non-`lang="ts"` context, see
/// `Ctx::vanilla_acorn`) and written under acorn-typescript. Each emitting arm
/// appends the whole field as one literal rather than the key followed by a
/// runtime-chosen token.
#[inline]
pub(super) fn write_import_kind_field<const E: u16>(
    w: &mut Sink<'_>,
    import_kind: ast::ImportKind,
    ctx: &Ctx<'_>,
) {
    match import_kind {
        ast::ImportKind::Type => raw!(w, @E, ",\"importKind\":\"type\""),
        ast::ImportKind::Value if ctx.vanilla_acorn => {}
        ast::ImportKind::Value => raw!(w, @E, ",\"importKind\":\"value\""),
    }
}

/// Emit `,"exportKind":"type"|"value"`, with the same omission rule as
/// [`write_import_kind_field`].
#[inline]
pub(super) fn write_export_kind_field<const E: u16>(
    w: &mut Sink<'_>,
    export_kind: ast::ExportKind,
    ctx: &Ctx<'_>,
) {
    match export_kind {
        ast::ExportKind::Type => raw!(w, @E, ",\"exportKind\":\"type\""),
        ast::ExportKind::Value if ctx.vanilla_acorn => {}
        ast::ExportKind::Value => raw!(w, @E, ",\"exportKind\":\"value\""),
    }
}

/// Emits a `Literal` node.
///
/// Its fields are the mount's `write_literal_fields_any`: on the JSON wire the common
/// literals in one fused write (`write_json.rs`), any other through
/// [`write_literal_fields`].
pub(super) fn write_literal(w: &mut Sink<'_>, lit: &ast::Literal, ctx: &Ctx<'_>) {
    node_header!(w, "Literal", lit.span, ctx);
    write_literal_fields_any(w, lit, ctx);
    close_node(w, "Literal", lit.span, ctx);
}

/// A `Literal`'s `value`, `raw` and (for a BigInt) `bigint` fields, field by
/// field — every literal's general emission.
pub(super) fn write_literal_fields(w: &mut Sink<'_>, lit: &ast::Literal, ctx: &Ctx<'_>) {
    raw!(w, "Literal", ",\"value\":");
    // `bigint` is emitted only for BigInt literals (`skip_serializing_if` on
    // `Option`), and shares the decimal string with `value`.
    let mut bigint: Option<String> = None;
    match &lit.value {
        ast::LiteralValue::Number(n) => write_number_value(w, n.get()),
        ast::LiteralValue::String(cooked) => {
            w.string(cooked.resolve(lit.span, ctx.source, ctx.view));
        }
        ast::LiteralValue::BigInt => {
            let decimal = bigint_to_decimal(lit.bigint_digits(ctx.source));
            w.string(&decimal);
            bigint = Some(decimal);
        }
        ast::LiteralValue::Boolean(b) => w.bool(*b),
        ast::LiteralValue::Null => w.null(),
    }
    let raw = lit.span.extract(ctx.source);
    if let ast::LiteralValue::String(_) = lit.value {
        raw!(w, "Literal", ",\"raw\":");
        w.string(raw);
    } else {
        // A numeric, BigInt, boolean or `null` token is escape-free by grammar —
        // digits, letters, `.`, `_`, and a sign only inside an exponent — so its
        // raw text takes the name field's unscanned write.
        json_only! { w.string_escape_free_led(b",\"raw\":", raw.as_bytes()); }
        tape! {
            raw!(w, "Literal", ",\"raw\":");
            w.string(raw);
        }
    }
    if let Some(decimal) = bigint {
        raw!(w, "Literal", ",\"bigint\":");
        w.string(&decimal);
    }
}

/// Shared `Identifier` node emission. Emits the `Identifier` fields: `name`,
/// then `optional` (only when true), `typeAnnotation` (only when present),
/// `decorators` (only when non-empty).
pub(super) fn write_identifier_parts(
    w: &mut Sink<'_>,
    span: Span,
    name: ast::IdentName,
    optional: bool,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    decorators: Option<&[ast::Decorator]>,
    ctx: &Ctx<'_>,
) {
    node_header!(w, "Identifier", span, ctx);
    write_identifier_fields(w, span, name, optional, type_annotation, decorators, ctx);
}

/// `write_identifier_parts` with `character` injected into the node's `loc`
/// (the fused `inject_loc_character`) — the top-level `Identifier` of a simple
/// Svelte block pattern / shorthand. Svelte's parser constructs these identifiers
/// itself, with `name` ahead of the positions; tsv's wire gives them the one
/// `Identifier` order, acorn's.
pub(super) fn write_identifier_parts_with_character(
    w: &mut Sink<'_>,
    span: Span,
    name: ast::IdentName,
    optional: bool,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    decorators: Option<&[ast::Decorator]>,
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
    tape! { w.t_open(entry!("Identifier")); }
    node_header_impl::<true>(w, "Identifier", span, ctx);
    write_identifier_fields(w, span, name, optional, type_annotation, decorators, ctx);
}

/// The `Identifier` fields after the node header: `name`, then the tail.
#[inline]
fn write_identifier_fields(
    w: &mut Sink<'_>,
    span: Span,
    name: ast::IdentName,
    optional: bool,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    decorators: Option<&[ast::Decorator]>,
    ctx: &Ctx<'_>,
) {
    write_name_field(w, name, span.start, ctx);
    write_identifier_tail(w, span, optional, type_annotation, decorators, ctx);
}

/// The skip-if-empty `Identifier` fields (`optional` / `typeAnnotation` /
/// `decorators`) and the closing `}`.
#[inline]
fn write_identifier_tail(
    w: &mut Sink<'_>,
    span: Span,
    optional: bool,
    type_annotation: Option<&ast::TSTypeAnnotation>,
    decorators: Option<&[ast::Decorator]>,
    ctx: &Ctx<'_>,
) {
    if optional {
        raw!(w, "Identifier", ",\"optional\":true");
    }
    write_type_annotation_field::<{ at!("Identifier") }>(w, type_annotation, ctx);
    if let Some(decs) = decorators
        && !decs.is_empty()
    {
        raw!(w, "Identifier", ",\"decorators\":");
        write_array(w, decs, |w, d| write_decorator(w, d, ctx));
    }
    close_node(w, "Identifier", span, ctx);
}

/// An `Identifier` carrying only the binding's `optional` flag (function and
/// method ids, entity-name-as-expression nodes) — no type annotation or
/// decorators.
#[inline]
pub(super) fn write_identifier_with_optional(
    w: &mut Sink<'_>,
    id: &ast::Identifier,
    ctx: &Ctx<'_>,
) {
    write_identifier_parts(w, id.span, id.ident_name(), id.optional, None, None, ctx);
}
