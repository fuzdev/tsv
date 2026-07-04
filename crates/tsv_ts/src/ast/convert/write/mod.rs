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
use string_interner::DefaultStringInterner;
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
mod statements;
mod types;

pub use comments::WriterComments;
use declarations::{write_decorator, write_type_parameter_declaration};
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
) -> Vec<u8> {
    let interner = program.interner.borrow();
    let mut ctx = Ctx::new(source, loc, &interner);
    ctx.vanilla_acorn = schema.is_svelte_script();
    let mut w = JsonWriter::with_capacity(tsv_lang::estimated_json_capacity(source.len()));
    write_program(&mut w, program, &ctx, schema);
    w.into_bytes()
}

/// Emit an embedded TS expression's wire JSON into a caller-owned writer, for
/// `tsv_svelte` composing template `{expr}` / directive / block expression
/// emission into its own buffer. Shares the host document's interner and
/// `LocationMapper` (spans are host-file coordinates); with a real map it emits
/// final char-space positions directly.
pub fn write_expression_embedded(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) {
    let ctx = Ctx::new(source, loc, interner);
    expressions::write_expression(w, expr, &ctx);
}

/// `write_expression_embedded` with a per-node comment map — the fused form of a
/// comment-bearing template expression island (`{expr}`, block test, directive,
/// `{@debug}` id, spread). Each node emits any attached leading/trailing comments
/// at its close.
pub fn write_expression_embedded_with_comments(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    comments: &WriterComments,
) {
    let mut ctx = Ctx::new(source, loc, interner);
    ctx.comments = Some(comments);
    expressions::write_expression(w, expr, &ctx);
}

/// Emit an embedded standalone `VariableDeclaration`'s wire JSON, for
/// `tsv_svelte`'s `{const …}` / `{let …}` declaration tag. Shares the host
/// document's interner and
/// `LocationMapper` (spans are host-file coordinates), emitting final char-space
/// positions directly.
pub fn write_variable_declaration_embedded(
    w: &mut JsonWriter,
    var_decl: &internal::VariableDeclaration<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) {
    let ctx = Ctx::new(source, loc, interner);
    write_variable_declaration(w, var_decl, &ctx, false);
}

/// `write_variable_declaration_embedded` with a per-node comment map — the fused
/// form of a comment-bearing `{@const}` / `{const}` / `{let}` declaration (the
/// document has a template comment). Attached comments emit at each node's close.
pub fn write_variable_declaration_embedded_with_comments(
    w: &mut JsonWriter,
    var_decl: &internal::VariableDeclaration<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    comments: &WriterComments,
) {
    let mut ctx = Ctx::new(source, loc, interner);
    ctx.comments = Some(comments);
    write_variable_declaration(w, var_decl, &ctx, false);
}

/// Emit an embedded expression whose top-level `Identifier` carries an injected
/// `character` in its `loc` (the fused `inject_loc_character`), for the Svelte
/// shorthand attribute (`{name}`) and snippet name. The `character` is injected
/// only on a top-level `Identifier`, so any other expression emits exactly as
/// `write_expression_embedded` (character a no-op). No type-annotation-`loc`
/// stripping (unlike a block pattern).
pub fn write_identifier_expression_with_character(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) {
    let ctx = Ctx::new(source, loc, interner);
    write_identifier_expression_with_character_in(w, expr, &ctx);
}

/// `write_identifier_expression_with_character` with a per-node comment map — the
/// fused form of a comment-bearing snippet name (`{#snippet /* c */ name(…)}`),
/// where a leading comment attaches to the `Identifier`.
pub fn write_identifier_expression_with_character_and_comments(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    comments: &WriterComments,
) {
    let mut ctx = Ctx::new(source, loc, interner);
    ctx.comments = Some(comments);
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
pub fn write_pattern_embedded(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
) {
    write_pattern_embedded_impl(w, expr, source, loc, interner, None);
}

/// `write_pattern_embedded` with a per-node comment map — a destructure
/// pattern *can* carry comments (`{@const { b = /* c */ 1 } = expr}`):
/// canonical parses it as a synthetic `(pattern = 1)` acorn expression whose
/// comment attach covers the pattern subtree. Attached comments emit at each
/// node's close.
pub fn write_pattern_embedded_with_comments(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    comments: &WriterComments,
) {
    write_pattern_embedded_impl(w, expr, source, loc, interner, Some(comments));
}

fn write_pattern_embedded_impl(
    w: &mut JsonWriter,
    expr: &internal::Expression<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    comments: Option<&WriterComments>,
) {
    let mut ctx = Ctx::new(source, loc, interner);
    ctx.comments = comments;
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
            let line = loc.pos_and_position(expr.span().start).1.line;
            if line > 1 {
                ctx.pattern_line = line;
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
/// interner and `LocationMapper` (spans are host-file coordinates), threads the
/// `Schema`, and — unlike a standalone `Program` — emits the node's own `loc`
/// from `loc_override` rather than deriving it from `program.span`.
///
/// Svelte reports the `Program` `loc` against the `<script>` **tag** (start line,
/// column 0) and the tag's closing `</script>`, not the content span; the caller
/// supplies those two final char-space `Position`s (the offset-translated form of
/// Svelte's byte-space override). `start`/`end` offsets still come from
/// `program.span` via `loc.pos`, and the body/`sourceType` are emitted exactly as
/// the standalone program writer does — so an eligible (comment-free, `lang="ts"`,
/// no preceding HTML comment) script's `content` matches the standalone
/// `Program` emission.
#[allow(clippy::too_many_arguments)]
pub fn write_program_embedded(
    w: &mut JsonWriter,
    program: &internal::Program<'_>,
    source: &str,
    loc: LocationMapper<'_>,
    interner: &DefaultStringInterner,
    schema: Schema,
    loc_override: (Position, Position),
    comments: Option<&WriterComments>,
) {
    let mut ctx = Ctx::new(source, loc, interner);
    ctx.vanilla_acorn = schema.is_svelte_script();
    ctx.comments = comments;
    let (start_pos, end_pos) = loc_override;
    w.raw("{\"type\":\"Program\",\"start\":");
    w.u32(loc.pos(program.span.start));
    w.raw(",\"end\":");
    w.u32(loc.pos(program.span.end));
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start_pos.line);
    w.raw(",\"column\":");
    w.usize(start_pos.column);
    w.raw("},\"end\":{\"line\":");
    w.usize(end_pos.line);
    w.raw(",\"column\":");
    w.usize(end_pos.column);
    w.raw("}},\"body\":");
    write_array(w, program.body, |w, s| write_statement(w, s, &ctx, schema));
    w.raw(",\"sourceType\":");
    w.token(program.goal.source_type());
    close_node(w, "Program", program.span, &ctx);
}

/// The per-document environment every writer function shares (`source`, the
/// `LocationMapper`, and the interner).
///
/// `pattern_line` / `pattern_ann_span` are the two Svelte block-pattern quirks
/// (`write_pattern_embedded`): they are inert (`0` / the empty span) for every
/// ordinary emission, so the hot path pays only a never-taken compare per
/// position (or per annotation).
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) source: &'a str,
    pub(super) loc: LocationMapper<'a>,
    pub(super) interner: &'a DefaultStringInterner,
    /// Block-pattern `read_pattern` `+1`-column quirk: the (1-based) line on
    /// which the pattern starts, or `0` when inactive. A node's `loc` column is
    /// bumped `+1` on this line only, reproducing `adjust_read_pattern_columns`.
    pub(super) pattern_line: usize,
    /// Block-pattern quirk: the span of the pattern's **top-level**
    /// `TSTypeAnnotation`, whose `loc` is omitted (Svelte's `read_context`
    /// synthesizes that node itself, without `loc`; nested annotations keep
    /// theirs). The empty span (never a real annotation) when inactive.
    pub(super) pattern_ann_span: Span,
    /// Per-node attached comments (Svelte comment-attach paths — a
    /// comment-bearing `<script>` `Program` or template expression). `None` for
    /// every ordinary emission, so the hot path pays only a never-taken compare
    /// per node close.
    pub(super) comments: Option<&'a WriterComments>,
    /// The canonical parser for this document is **vanilla acorn** (a Svelte
    /// non-`lang="ts"` component), not acorn-typescript. Drives the
    /// vanilla-only wire quirks: `,"options":null` on every `ImportExpression`
    /// (vanilla acorn always emits it; acorn-typescript omits it), and
    /// `value`-before-`kind` on get/set `Property` nodes (acorn-typescript's
    /// get/set path assigns `kind` first). `false` for standalone TS and every
    /// `lang="ts"` component.
    pub(super) vanilla_acorn: bool,
}

impl<'a> Ctx<'a> {
    /// The base per-document context (no pattern quirks active).
    #[inline]
    fn new(source: &'a str, loc: LocationMapper<'a>, interner: &'a DefaultStringInterner) -> Self {
        Ctx {
            source,
            loc,
            interner,
            pattern_line: 0,
            pattern_ann_span: Span::new(0, 0),
            comments: None,
            vanilla_acorn: false,
        }
    }
}

/// Close a node object: emit any attached `leadingComments`/`trailingComments`
/// (fused) for this node's byte span + type, then the closing `}`. The type and
/// span mirror the node's own `node_header` call. A `None` comment map (every
/// ordinary emission) makes this exactly `w.raw("}")` after one never-taken
/// branch.
#[inline]
pub(super) fn close_node(w: &mut JsonWriter, node_type: &str, span: Span, ctx: &Ctx<'_>) {
    if let Some(wc) = ctx.comments {
        wc.emit(w, node_type, span.start, span.end, ctx.loc);
    }
    w.raw("}");
}

/// Apply the block-pattern `+1`-column adjustment: a node's `loc` column is
/// bumped by one on `ctx.pattern_line` only (inert when `pattern_line == 0`,
/// which never equals a real 1-based line).
#[inline]
pub(super) fn adjusted_column(ctx: &Ctx<'_>, line: usize, column: usize) -> usize {
    if line == ctx.pattern_line {
        column + 1
    } else {
        column
    }
}

/// Emit a node with no fields beyond the universal prefix (`ThisExpression`,
/// `Super`, keyword types, …).
#[inline]
pub(super) fn write_bare_node(w: &mut JsonWriter, node_type: &str, span: Span, ctx: &Ctx<'_>) {
    node_header(w, node_type, span, ctx);
    close_node(w, node_type, span, ctx);
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
pub(super) fn node_header(w: &mut JsonWriter, node_type: &str, span: Span, ctx: &Ctx<'_>) {
    node_header_impl::<false>(w, node_type, span, ctx);
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
    node_type: &str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    debug_assert!(
        node_type
            .bytes()
            .all(|b| b != b'"' && b != b'\\' && b >= 0x20),
        "node type must be escape-free: {node_type:?}"
    );
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\"");
    position_fields::<CHARACTER>(w, span, ctx);
}

/// The `,"start":…,"end":…,"loc":{…}` position fields (final char space) —
/// the tail of `node_header_impl`, also emitted after a leading `name` for
/// the Svelte-constructed identifiers whose fields precede the positions.
fn position_fields<const CHARACTER: bool>(w: &mut JsonWriter, span: Span, ctx: &Ctx<'_>) {
    let (start_pos, start) = ctx.loc.pos_and_position(span.start);
    let (end_pos, end) = ctx.loc.pos_and_position(span.end);
    w.raw(",\"start\":");
    w.u32(start_pos);
    w.raw(",\"end\":");
    w.u32(end_pos);
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, start.line, start.column));
    if CHARACTER {
        w.raw(",\"character\":");
        w.u32(start_pos);
    }
    w.raw("},\"end\":{\"line\":");
    w.usize(end.line);
    w.raw(",\"column\":");
    w.usize(adjusted_column(ctx, end.line, end.column));
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
/// `name_start`); escaped names resolve the interned decoded form (an escaped
/// identifier's `\u{78}` source form decodes to `x`). Both arms write the wire
/// value directly; no allocation.
#[inline]
pub(super) fn write_name(
    w: &mut JsonWriter,
    name: internal::IdentName,
    name_start: u32,
    ctx: &Ctx<'_>,
) {
    use tsv_lang::InfallibleResolve;
    match name.escaped {
        Some(sym) => w.string(ctx.interner.resolve_infallible(sym)),
        None => {
            let start = name_start as usize;
            w.string(&ctx.source[start..start + name.raw_len as usize]);
        }
    }
}

/// Emit a numeric literal value the way acorn's JSON does: non-finite as
/// `null` (JSON has no Infinity/NaN — an overflow literal like `1e999`),
/// integral doubles as expanded shortest-round-trip integers (JS
/// `JSON.stringify` semantics), everything else as ryu.
///
/// Known divergence (pre-existing, carried over from the typed converter):
/// integral doubles in `(u64::MAX, 1e21)` — e.g. `1e20` — emit ryu's `1e+20`
/// where JS prints expanded digits.
/// TODO: emit JS-style expanded text for integral doubles beyond `u64` up to
/// 1e21 (fixtures-first).
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
    name: internal::IdentName,
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
    name: internal::IdentName,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
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
    name: internal::IdentName,
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
