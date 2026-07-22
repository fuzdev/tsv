//! Writer-mode conversion: emit compact wire JSON directly from the internal AST.
//!
//! This is the **sole emission path** for the TS wire JSON: it walks the
//! *internal* AST once and writes the final JSON bytes as it goes, never
//! materializing a typed public tree — the hot path behind
//! `convert_ast_json_bytes`/`_string` (FFI/WASM parse bindings, CLI compact
//! output; `convert_ast_json` parses these bytes back into a `Value`).
//!
//! **Byte-identity**: the wire JSON is a faithful emission of the acorn quirk
//! catalog — each node's field order, `skip_serializing_if` behavior, `null`s
//! for non-skipped `Option`s, and scalar formatting match acorn-typescript's
//! JSON exactly (the shape each fixture's `expected.json` records).
//!
//! Scalar formatting delegates to `serde_json` wherever its output is not
//! trivially reproducible: dynamic strings (`to_writer` runs `serde_json`'s
//! exact string-escape logic) and non-integral `f64` (ryu). Static tokens
//! (node types, operators, kinds) are known escape-free and written verbatim;
//! integers have a unique decimal form and are hand-formatted.
//!
//! Three conversion-time mutations of already-converted children become
//! pre-computed decisions threaded down as flags (see `ExprFlags` in
//! `expressions`): the `?.<T>()` callee-optional force, the unparenthesized
//! decorator spine optional-strip, and the `TSParameterProperty`
//! assignment-pattern span override. The super-class
//! `TSInstantiationExpression` wrap is decided before its fields are emitted.

use super::super::internal;
use super::{Schema, bigint_to_decimal};
use tsv_lang::{LocationMapper, Position, Span};
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
mod skeleton;
mod statements;
mod types;

pub use comments::{AttachedComment, WriterComments};
use declarations::{write_decorator, write_type_parameter_declaration};
pub use skeleton::{SkeletonRecorder, SkeletonTree};
use statements::{write_statement, write_variable_declaration};
use types::{write_type_annotation, write_type_parameter_instantiation};

/// Convert an internal `Program` straight to its compact wire-JSON bytes.
///
/// One AST walk, no intermediate tree. The mapper decides the offset space:
/// identity → byte space (the embedded byte-space skeletons `tsv_svelte` builds),
/// real map → UTF-16 code units (the shipped char-space wire).
///
/// Returns `Vec<u8>` rather than `String`: every emitted byte comes from `&str`
/// slices and ASCII fragments, so the output is valid UTF-8 by construction,
/// but proving that to a `String` costs an O(output) validation scan (~15×
/// source bytes). Byte-oriented boundaries (FFI, the CLI's stdout) take the
/// bytes as-is; `&str` boundaries (`convert_ast_json_string` → WASM/N-API)
/// pay the one validation at the edge.
pub fn write_program_json(
    program: &internal::Program<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    schema: Schema,
    locations: bool,
) -> Vec<u8> {
    let mut ctx = Ctx::new(source, loc);
    ctx.vanilla_acorn = schema.is_svelte_script();
    ctx.emit_loc = locations;
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    write_program(&mut w, program, &ctx, schema);
    w.into_bytes()
}

/// Emit an embedded TS expression's wire JSON into a caller-owned writer, for
/// `tsv_svelte` composing template `{expr}` / directive / block expression
/// emission into its own buffer. Shares the host document's
/// `LocationMapper` (spans are host-file coordinates); with a real map it emits
/// final char-space positions directly.
///
/// `comments` is the pass's comment role: `Emit` for the fused form of a
/// comment-bearing template expression island (each node emits its attached
/// leading/trailing comments at its close), `Record` for the byte-space
/// skeleton pass that builds an island's comment map.
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
/// document's `LocationMapper` (spans are host-file coordinates), emitting final char-space
/// positions directly. `comments` as in `write_expression_embedded`.
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
/// `write_expression_embedded` (character a no-op). No type-annotation-`loc`
/// stripping (unlike a block pattern). `comments` is `Emit` for the fused form
/// of a comment-bearing snippet name (`{#snippet /* c */ name(…)}`), where a
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
    if let internal::Expression::Identifier(id) = expr {
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
/// Reproduces Svelte's three `read_pattern` quirks fused, in final char space:
///
/// - **Destructure** (`ObjectPattern`/`ArrayPattern`): every node's `loc` column
///   is bumped `+1` on the pattern's start line when that line `> 1`
///   (`adjust_read_pattern_columns` — the synthetic `(`-wrapper acorn parses the
///   pattern under shifts that one line by a byte).
/// - **Simple identifier**: `character` is injected into the top-level
///   `Identifier`'s `loc` (`inject_loc_character`) — Svelte reports it on the
///   identifiers `read_identifier` creates directly.
/// - **Both**: `loc` is omitted on the pattern's **top-level** `TSTypeAnnotation`
///   only — the one Svelte's `read_context` synthesizes itself (no `loc`);
///   annotations nested inside it come from the acorn parse and keep `loc`.
///
/// `comments` is `Emit` for the fused form of a comment-carrying destructure
/// pattern (`{@const { b = /* c */ 1 } = expr}`): canonical parses it as a
/// synthetic `(pattern = 1)` acorn expression whose comment attach covers the
/// pattern subtree, and attached comments emit at each node's close.
#[inline]
pub fn write_pattern_embedded(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    env: EmbedWriter<'_>,
) {
    let mut ctx = Ctx::from_embed(env);
    // The pattern root's own annotation is the `read_context`-synthesized one
    // whose `loc` is omitted (a block-pattern root is always an identifier or a
    // destructure, so no other root shape can carry one).
    let top_annotation = match expr {
        internal::Expression::ObjectPattern(o) => o.type_annotation.as_ref(),
        internal::Expression::ArrayPattern(a) => a.type_annotation.as_ref(),
        internal::Expression::Identifier(id) => id.type_annotation(),
        _ => None,
    };
    if let Some(ann) = top_annotation {
        ctx.pattern_ann_span = ann.span;
    }
    match expr {
        internal::Expression::ObjectPattern(_) | internal::Expression::ArrayPattern(_) => {
            // Destructure: `+1`-column adjustment on the start line (when `> 1`).
            // Only affects column output, so skip the line lookup entirely on the
            // no-locations path (where it would only hit the stub `[0]` table).
            if env.emit_loc {
                let line = env.loc.pos_and_position(expr.span().start).1.line;
                if line > 1 {
                    ctx.pattern_line = line;
                }
            }
            expressions::write_expression(w, expr, &ctx);
        }
        internal::Expression::Identifier(id) => {
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
        // Any other non-destructure pattern: `inject_loc_character` is a no-op
        // (it only touches a top-level `Identifier`), and a block-pattern root
        // is always an identifier or a destructure, so no top-level annotation
        // can exist here.
        _ => expressions::write_expression(w, expr, &ctx),
    }
}

/// Emit the `Program` node.
fn write_program(
    w: &mut JsonWriter,
    program: &internal::Program<'_>,
    ctx: &Ctx<'_>,
    schema: Schema,
) {
    node_header(w, "Program", program.span, ctx);
    w.raw(",\"body\":");
    write_array(w, program.body, |w, s| write_statement(w, s, ctx, schema));
    w.raw(",\"sourceType\":");
    w.token(program.goal.source_type());
    close_node(w, "Program", program.span, ctx);
}

/// Emit an embedded `<script>` `Program`'s wire JSON into a caller-owned writer —
/// for `tsv_svelte` composing a
/// `<script>` block's `content` into its own buffer. Shares the host document's
/// `LocationMapper` (spans are host-file coordinates), threads the
/// `Schema`, and — unlike a standalone `Program` — emits the node's own `loc`
/// from `program_loc` rather than deriving it from `program.span`.
///
/// Svelte reports the `Program` `loc` against the `<script>` **tag** (start line,
/// column 0) and the tag's closing `</script>`, not the content span; the caller
/// supplies those two final char-space `Position`s via `ProgramLoc::Emit` (the
/// offset-translated form of Svelte's byte-space override), or `ProgramLoc::Omit`
/// for the no-locations wire. `start`/`end` offsets still come from `program.span`
/// via `loc.pos`, and the body/`sourceType` are emitted exactly as the standalone
/// program writer does — so an eligible (comment-free, `lang="ts"`, no preceding
/// HTML comment) script's `content` matches the standalone `Program` emission.
pub fn write_program_embedded(
    w: &mut JsonWriter,
    program: &internal::Program<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    schema: Schema,
    program_loc: ProgramLoc,
    comments: CommentMode<'_>,
) {
    let mut ctx = Ctx::new(source, loc);
    ctx.vanilla_acorn = schema.is_svelte_script();
    ctx.comments = comments;
    ctx.emit_loc = matches!(program_loc, ProgramLoc::Emit(..));
    record_open("Program", program.span, &ctx);
    w.raw("{\"type\":\"Program\",\"start\":");
    w.u32(loc.pos(program.span.start));
    w.raw(",\"end\":");
    w.u32(loc.pos(program.span.end));
    if let ProgramLoc::Emit(start_pos, end_pos) = program_loc {
        w.raw(",\"loc\":{\"start\":{\"line\":");
        w.usize(start_pos.line);
        w.raw(",\"column\":");
        w.usize(start_pos.column);
        w.raw("},\"end\":{\"line\":");
        w.usize(end_pos.line);
        w.raw(",\"column\":");
        w.usize(end_pos.column);
        w.raw("}}");
    }
    w.raw(",\"body\":");
    write_array(w, program.body, |w, s| write_statement(w, s, &ctx, schema));
    w.raw(",\"sourceType\":");
    w.token(program.goal.source_type());
    close_node(w, "Program", program.span, &ctx);
}

/// The comment role of one emission pass (Svelte comment-attach paths).
///
/// `Off` for every ordinary emission — the hot path pays one never-taken
/// discriminant compare per node open and close. `Emit` is the fused pass of a
/// comment-bearing island: each node close consults the precomputed
/// `WriterComments` map. `Record` is the byte-space skeleton pass that
/// *builds* that map's input: each node open/close is reported to the
/// `SkeletonRecorder`, which reconstructs the wire tree for the comment-attach
/// walk (no re-parse of the emitted bytes).
#[derive(Clone, Copy)]
pub enum CommentMode<'a> {
    Off,
    Emit(&'a WriterComments),
    Record(&'a SkeletonRecorder),
}

/// The embedded `Program` node's `loc` source (see `write_program_embedded`).
///
/// Fuses the former `loc_override` + `emit_loc` parameters into one value so the
/// "no `loc` but a meaningful override" state is unrepresentable — the caller no
/// longer builds a dummy `Position` pair just to satisfy the signature. `Omit`
/// is the no-locations wire, which drops `loc` from every node globally (it sets
/// `Ctx::emit_loc`), this `Program` included.
#[derive(Clone, Copy)]
pub enum ProgramLoc {
    /// No-locations wire: omit `loc` on the `Program` (and every node).
    Omit,
    /// Emit the `Program`'s `loc` from Svelte's tag-line `(start, end)` positions.
    Emit(Position, Position),
}

/// The per-document inputs the four "plain" embedded writers share
/// (`write_expression_embedded`, `write_pattern_embedded`,
/// `write_variable_declaration_embedded`,
/// `write_identifier_expression_with_character`) — the source text, offset
/// mapper, comment role, and `loc`-emission flag each one funnels into a `Ctx`.
///
/// Bundled into one `Copy` value (all fields are `Copy` — two references, an
/// enum, a bool) so the call sites stop re-threading the same four arguments.
/// It is an entry-boundary convenience only: each writer destructures it into a
/// stack `Ctx` (`Ctx::from_embed`) and the per-node walk threads `&Ctx` exactly
/// as before — the fused char-space emission never sees it, so this is output-
/// and hot-path-neutral. (`write_program_embedded` stays out of this set: it
/// carries `Schema` + `ProgramLoc`, and its `loc` flag lives in `ProgramLoc`.)
#[derive(Clone, Copy)]
pub struct EmbedWriter<'a> {
    pub source: &'a str,
    pub loc: LocationMapper<'a>,
    pub comments: CommentMode<'a>,
    pub emit_loc: bool,
}

/// The per-document environment every writer function shares (`source` and the
/// `LocationMapper`).
///
/// `pattern_line` / `pattern_ann_span` are the two Svelte block-pattern quirks
/// (`write_pattern_embedded`): they are inert (`0` / the empty span) for every
/// ordinary emission, so the hot path pays only a never-taken compare per
/// position (or per annotation).
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) source: &'a str,
    pub(super) loc: LocationMapper<'a>,
    /// Block-pattern `read_pattern` `+1`-column quirk: the (1-based) line on
    /// which the pattern starts, or `0` when inactive. A node's `loc` column is
    /// bumped `+1` on this line only, reproducing `adjust_read_pattern_columns`.
    pub(super) pattern_line: usize,
    /// Block-pattern quirk: the span of the pattern's **top-level**
    /// `TSTypeAnnotation`. Two things key on it.
    ///
    /// Its own `loc` is omitted — Svelte's `read_context` synthesizes that node
    /// itself, without `loc` (nested annotations keep theirs).
    ///
    /// And it **bounds the `+1` column bump**: Svelte reads the annotation with a
    /// *second*, separately-padded acorn parse (`read_type_annotation`'s `_ as `
    /// trick) that inserts no `(`, so the type nodes inside it are NOT shifted the
    /// way the pattern's own nodes are. The bump therefore stops at this span's
    /// start (inclusive — the pattern's own `loc.end` lands exactly there).
    ///
    /// `Span::new(u32::MAX, u32::MAX)` when inactive: never equal to a real
    /// annotation's span, and an unreachable upper bound, so both uses are inert.
    pub(super) pattern_ann_span: Span,
    /// This pass's comment role (Svelte comment-attach paths). `Off` for every
    /// ordinary emission, so the hot path pays only a never-taken compare per
    /// node open and close.
    pub(super) comments: CommentMode<'a>,
    /// The canonical parser for this document is **vanilla acorn** (a Svelte
    /// non-`lang="ts"` component), not acorn-typescript. Drives the
    /// vanilla-only wire quirks: `,"options":null` on every `ImportExpression`
    /// (vanilla acorn always emits it; acorn-typescript omits it), and
    /// `value`-before-`kind` on get/set `Property` nodes (acorn-typescript's
    /// get/set path assigns `kind` first). `false` for standalone TS and every
    /// `lang="ts"` component.
    pub(super) vanilla_acorn: bool,
    /// Whether to emit the per-node `loc` object (line/column). `true` for the
    /// default acorn/svelte drop-in wire; `false` for the opt-in `no-locations`
    /// variant (`start`/`end` offsets only — `loc` is derivable from them plus
    /// source, so nothing is lost). Constant for a whole document, so the
    /// per-node branch in `position_fields` predicts perfectly on the default
    /// path.
    pub(super) emit_loc: bool,
}

impl<'a> Ctx<'a> {
    /// The base per-document context (no pattern quirks active).
    #[inline]
    fn new(source: &'a str, loc: LocationMapper<'a>) -> Self {
        Ctx {
            source,
            loc,
            pattern_line: 0,
            pattern_ann_span: Span::new(u32::MAX, u32::MAX),
            comments: CommentMode::Off,
            vanilla_acorn: false,
            emit_loc: true,
        }
    }

    /// The per-document context for an embedded writer: the shared `EmbedWriter`
    /// inputs plus the inert pattern-quirk defaults. Sets `comments`/`emit_loc`
    /// in the initializer (no post-construction re-assignment), so with the
    /// entry writers inlined the `EmbedWriter` aggregate scalar-replaces away.
    #[inline]
    fn from_embed(env: EmbedWriter<'a>) -> Self {
        Ctx {
            source: env.source,
            loc: env.loc,
            pattern_line: 0,
            pattern_ann_span: Span::new(u32::MAX, u32::MAX),
            comments: env.comments,
            vanilla_acorn: false,
            emit_loc: env.emit_loc,
        }
    }
}

/// Close a node object: emit any attached `leadingComments`/`trailingComments`
/// (fused) for this node's byte span + type, then the closing `}`. The type and
/// span mirror the node's own `node_header` call. `CommentMode::Off` (every
/// ordinary emission) makes this exactly `w.raw("}")` after one never-taken
/// branch.
#[inline]
pub(super) fn close_node(w: &mut JsonWriter, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    match ctx.comments {
        CommentMode::Off => {}
        CommentMode::Emit(wc) => wc.emit(w, node_type, span.start, span.end, ctx.loc),
        CommentMode::Record(rec) => rec.close(node_type, span),
    }
    w.raw("}");
}

/// Apply the block-pattern `+1`-column adjustment: a node's `loc` column is
/// bumped by one on `ctx.pattern_line` only (inert when `pattern_line == 0`,
/// which never equals a real 1-based line).
///
/// The bump reproduces the synthetic `(`-wrapper Svelte's `read_pattern` parses
/// the pattern under — so it applies only to nodes that came from THAT parse. A
/// trailing `: T` is read by a *second*, separately-padded parse with no inserted
/// `(` (`read_type_annotation`), so its type nodes keep their true columns:
/// `offset` past the annotation's start is left alone. The bound is inclusive —
/// the pattern's own `loc.end` sits exactly on the annotation's start — and inert
/// without one (`pattern_ann_span.start == u32::MAX`).
#[inline]
pub(super) fn adjusted_column(ctx: &Ctx<'_>, offset: u32, line: usize, column: usize) -> usize {
    if line == ctx.pattern_line && offset <= ctx.pattern_ann_span.start {
        column + 1
    } else {
        column
    }
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

/// Report a node open to the skeleton recorder (`CommentMode::Record` only —
/// one never-taken compare for every ordinary emission). `node_header` calls
/// this; the hand-written header sites (the embedded `Program`, the
/// name-first identifier, the `loc`-less block-pattern `TSTypeAnnotation`)
/// call it directly so every wire node reaches the recorder.
#[inline]
pub(super) fn record_open(node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    if let CommentMode::Record(rec) = ctx.comments {
        rec.open(node_type, span);
    }
}

/// Emit the universal node prefix: `{"type":"X","start":N,"end":N,"loc":{…}`.
///
/// Leaves the object open — the caller appends its remaining fields and the
/// closing `}`. `span` is the span every one of `start`/`end`/`loc` derives
/// from (start/end are the fused char-space positions, `loc` their
/// line/column form); TS emits no `Position.character`, so it is always
/// omitted. Static fragments are pre-fused into the fewest buffer writes —
/// this runs once per node.
#[inline]
pub(super) fn node_header(w: &mut JsonWriter, node_type: &'static str, span: Span, ctx: &Ctx<'_>) {
    node_header_impl::<false>(w, node_type, span, ctx);
}

/// A header whose wire **`end` offset is widened past its `loc`** — the one node
/// class where a node's byte range and its line/column range genuinely disagree.
///
/// A Svelte **block** binding pattern (`{#each xs as { a }: T}`, `{:then { a }: T}`)
/// is parsed bare by acorn, after which Svelte's `read_pattern`
/// (`1-parse/read/context.js`) patches `expression.end = typeAnnotation.end` and
/// never touches `expression.loc`. acorn parsing the same pattern as a real
/// *signature parameter* extends both — and there the internal span already covers
/// the annotation, so the `max` below is a no-op. The internal span therefore
/// records the **bare** pattern (which `loc` derives from) and the widened `end` is
/// recovered here from the annotation.
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
    record_open(node_type, span, ctx);
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\"");
    if !ctx.emit_loc {
        w.raw(",\"start\":");
        w.u32(ctx.loc.pos(span.start));
        w.raw(",\"end\":");
        w.u32(ctx.loc.pos(wire_end));
        return;
    }
    let (start_pos, start) = ctx.loc.pos_and_position(span.start);
    let (_, end) = ctx.loc.pos_and_position(span.end);
    w.raw(",\"start\":");
    w.u32(start_pos);
    w.raw(",\"end\":");
    w.u32(ctx.loc.pos(wire_end));
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, span.start, start.line, start.column));
    w.raw("},\"end\":{\"line\":");
    w.usize(end.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, span.end, end.line, end.column));
    w.raw("}}");
}

/// Shared body of `node_header` and the name-first identifier emission;
/// `CHARACTER` (the fused `inject_loc_character`, injected into
/// `loc.start`/`loc.end` for the top-level `Identifier` of a simple block
/// pattern / shorthand) is a compile-time constant, so each wrapper
/// monomorphizes to its own straight-line emission (no runtime branch on the
/// per-node hot path). The pattern `+1`-column adjustment applies in both
/// (it never actually co-occurs with character injection — destructure has
/// no character).
fn node_header_impl<const CHARACTER: bool>(
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
    record_open(node_type, span, ctx);
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\"");
    position_fields::<CHARACTER>(w, span, ctx);
}

/// The `,"start":…,"end":…,"loc":{…}` position fields (final char space) —
/// the tail of `node_header_impl`, also emitted after a leading `name` for
/// the Svelte-constructed identifiers whose fields precede the positions.
fn position_fields<const CHARACTER: bool>(w: &mut JsonWriter, span: Span, ctx: &Ctx<'_>) {
    if !ctx.emit_loc {
        // `no-locations` variant: offsets only, no `loc` (and no `character`,
        // which lives inside `loc`). Only the byte→char `pos` is needed, so the
        // per-node line/column lookup is skipped entirely.
        w.raw(",\"start\":");
        w.u32(ctx.loc.pos(span.start));
        w.raw(",\"end\":");
        w.u32(ctx.loc.pos(span.end));
        return;
    }
    let (start_pos, start) = ctx.loc.pos_and_position(span.start);
    let (end_pos, end) = ctx.loc.pos_and_position(span.end);
    w.raw(",\"start\":");
    w.u32(start_pos);
    w.raw(",\"end\":");
    w.u32(end_pos);
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, span.start, start.line, start.column));
    if CHARACTER {
        w.raw(",\"character\":");
        w.u32(start_pos);
    }
    w.raw("},\"end\":{\"line\":");
    w.usize(end.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, span.end, end.line, end.column));
    if CHARACTER {
        w.raw(",\"character\":");
        w.u32(end_pos);
    }
    w.raw("}}");
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

/// The `importKind`/`exportKind` value under the schema: `"value"` is omitted
/// in Svelte non-`lang="ts"` context, always present under acorn.
#[inline]
pub(super) fn kind_token(is_type: bool, schema: Schema) -> Option<&'static str> {
    if is_type {
        Some("type")
    } else if schema.is_svelte_script() {
        None
    } else {
        Some("value")
    }
}

/// Emit an identifier name — the single name-emission funnel. Span-identity
/// names are the raw source slice (the leading `raw_len` bytes at
/// `name_start`); escaped names are the decoded `&'arena str` (an escaped
/// identifier's `\u{78}` source form decodes to `x`). Both arms write the wire
/// value directly; no allocation.
#[inline]
pub(super) fn write_name(
    w: &mut JsonWriter,
    name: internal::IdentName<'_>,
    name_start: u32,
    ctx: &Ctx<'_>,
) {
    match name.escaped {
        Some(s) => w.string(s),
        None => {
            let start = name_start as usize;
            w.string(&ctx.source[start..start + name.raw_len as usize]);
        }
    }
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
pub(super) fn write_literal(w: &mut JsonWriter, lit: &internal::Literal<'_>, ctx: &Ctx<'_>) {
    node_header(w, "Literal", lit.span, ctx);
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
    w.raw(",\"raw\":");
    w.string(lit.span.extract(ctx.source));
    if let Some(decimal) = bigint {
        w.raw(",\"bigint\":");
        w.string(&decimal);
    }
    close_node(w, "Literal", lit.span, ctx);
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
    record_open("Identifier", span, ctx);
    w.raw("{\"type\":\"Identifier\",\"name\":");
    write_name(w, name, span.start, ctx);
    position_fields::<true>(w, span, ctx);
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
    w.raw(",\"name\":");
    write_name(w, name, span.start, ctx);
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
#[inline]
pub(super) fn write_identifier_plain(
    w: &mut JsonWriter,
    id: &internal::Identifier<'_>,
    ctx: &Ctx<'_>,
) {
    write_identifier_parts(w, id.span, id.ident_name(), false, None, None, ctx);
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
