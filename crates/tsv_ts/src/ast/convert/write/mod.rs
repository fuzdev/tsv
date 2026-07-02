//! Writer-mode conversion: emit compact wire JSON directly from the internal AST.
//!
//! This is the third emission mode of the acorn quirk catalog, next to the typed
//! conversion (`convert_program`) and the `Value` translation walk
//! (`convert_ast_json`). It walks the *internal* AST once and writes the final
//! JSON bytes as it goes, never materializing the typed public tree — the hot
//! path behind `convert_ast_json_string` (FFI/WASM parse bindings, CLI compact
//! output).
//!
//! **Byte-identity contract**: every function here must emit exactly the bytes
//! `serde_json::to_string` produces for the corresponding `convert_*` result —
//! same field order (the public struct's declaration order), same
//! `skip_serializing_if` behavior, same `null`s for non-skipped `Option`s, and
//! the same scalar formatting. Each `write_*` function mirrors its `convert_*`
//! twin in the sibling `convert` submodules; change them in lockstep. The
//! fixture suite's string-path identity check (writer vs `Value` oracle, every
//! fixture plus synthesized multibyte variants) enforces the lockstep.
//!
//! Scalar formatting delegates to `serde_json` wherever its output is not
//! trivially reproducible: dynamic strings (`to_writer` runs the exact escape
//! logic the typed path uses) and non-integral `f64` (ryu). Static tokens
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
use tsv_lang::{LocationMapper, Span};

mod control_flow;
mod declarations;
mod expressions;
mod functions;
mod modules;
mod patterns;
mod statements;
mod types;

use declarations::{write_decorator, write_type_parameter_declaration};
use statements::write_statement;
use types::{write_type_annotation, write_type_parameter_instantiation};

/// Convert an internal `Program` straight to its compact wire-JSON bytes.
///
/// The writer twin of `convert_program` + `serde_json::to_string`: byte-identical
/// output, one AST walk, no intermediate tree. The mapper decides the offset
/// space exactly as it does for `convert_program` (identity → byte space, real
/// map → UTF-16 code units).
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
    let ctx = Ctx {
        source,
        loc,
        interner: &interner,
    };
    let mut w = JsonWriter {
        buf: Vec::with_capacity(tsv_lang::estimated_json_capacity(source.len())),
    };
    write_program(&mut w, program, &ctx, schema);
    w.buf
}

/// Mirrors `convert_program`.
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
    w.raw("}");
}

/// The per-document environment every writer function shares (the writer's
/// analogue of convert's `(source, loc, interner)` triple).
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub(super) source: &'a str,
    pub(super) loc: LocationMapper<'a>,
    pub(super) interner: &'a DefaultStringInterner,
}

/// Compact-JSON output buffer.
///
/// All writes are infallible (`Vec<u8>` backing). The escape-sensitive entry
/// points are `string` (full JSON escaping via `serde_json`) and `token`
/// (quoted verbatim — static ASCII tokens only, debug-asserted).
pub(super) struct JsonWriter {
    buf: Vec<u8>,
}

impl JsonWriter {
    /// Verbatim JSON structure fragment (`{"key":`, `,`, `]`…). No escaping.
    #[inline]
    pub(super) fn raw(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    /// A quoted static token (node type, operator, kind, keyword). These are
    /// compile-time ASCII strings that never contain `"`, `\`, or control
    /// characters, so they skip the escape scan.
    #[inline]
    pub(super) fn token(&mut self, s: &str) {
        debug_assert!(
            s.bytes().all(|b| b != b'"' && b != b'\\' && b >= 0x20),
            "token must be escape-free: {s:?}"
        );
        self.buf.push(b'"');
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'"');
    }

    /// A dynamic string value, JSON-escaped and quoted. Delegates to
    /// `serde_json` so the escape set matches the typed serialization exactly.
    #[inline]
    #[allow(clippy::expect_used)]
    pub(super) fn string(&mut self, s: &str) {
        serde_json::to_writer(&mut self.buf, s).expect("Vec<u8> write is infallible");
    }

    /// A non-integral `f64` (the rare literal tail) — `serde_json`'s ryu
    /// formatting, matching `serde_json::Number` serialization.
    #[inline]
    #[allow(clippy::expect_used)]
    pub(super) fn f64(&mut self, n: f64) {
        serde_json::to_writer(&mut self.buf, &n).expect("Vec<u8> write is infallible");
    }

    #[inline]
    pub(super) fn u64(&mut self, n: u64) {
        // Two-digit-pair formatting (itoa's approach): halves the divisions.
        // The writer emits six integers per node, so this is hot.
        const DEC_PAIRS: [u8; 200] = {
            let mut t = [0u8; 200];
            let mut i = 0;
            while i < 100 {
                t[i * 2] = b'0' + (i / 10) as u8;
                t[i * 2 + 1] = b'0' + (i % 10) as u8;
                i += 1;
            }
            t
        };
        let mut tmp = [0u8; 20];
        let mut i = tmp.len();
        let mut n = n;
        while n >= 100 {
            let pair = (n % 100) as usize * 2;
            n /= 100;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        }
        if n >= 10 {
            let pair = n as usize * 2;
            i -= 2;
            tmp[i] = DEC_PAIRS[pair];
            tmp[i + 1] = DEC_PAIRS[pair + 1];
        } else {
            i -= 1;
            tmp[i] = b'0' + n as u8;
        }
        self.buf.extend_from_slice(&tmp[i..]);
    }

    #[inline]
    pub(super) fn i64(&mut self, n: i64) {
        if n < 0 {
            self.buf.push(b'-');
        }
        self.u64(n.unsigned_abs());
    }

    #[inline]
    pub(super) fn u32(&mut self, n: u32) {
        self.u64(u64::from(n));
    }

    #[inline]
    pub(super) fn usize(&mut self, n: usize) {
        self.u64(n as u64);
    }

    #[inline]
    pub(super) fn bool(&mut self, b: bool) {
        self.raw(if b { "true" } else { "false" });
    }

    #[inline]
    pub(super) fn null(&mut self) {
        self.raw("null");
    }
}

/// Emit a JSON array: `[` + comma-separated items + `]`.
#[inline]
pub(super) fn write_array<T>(
    w: &mut JsonWriter,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut JsonWriter, T),
) {
    w.raw("[");
    let mut first = true;
    for item in items {
        if !first {
            w.raw(",");
        }
        first = false;
        f(w, item);
    }
    w.raw("]");
}

/// Emit a nullable node value: the item through `f`, or `null` — the writer's
/// shape for every `Option` field *without* `skip_serializing_if`.
#[inline]
pub(super) fn write_or_null<T>(
    w: &mut JsonWriter,
    item: Option<&T>,
    f: impl FnOnce(&mut JsonWriter, &T),
) {
    match item {
        Some(v) => f(w, v),
        None => w.null(),
    }
}

/// Emit a node with no fields beyond the universal prefix (`ThisExpression`,
/// `Super`, keyword types, …).
#[inline]
pub(super) fn write_bare_node(w: &mut JsonWriter, node_type: &str, span: Span, ctx: &Ctx<'_>) {
    node_header(w, node_type, span, ctx);
    w.raw("}");
}

/// Emit the universal node prefix: `{"type":"X","start":N,"end":N,"loc":{…}`.
///
/// Leaves the object open — the caller appends its remaining fields and the
/// closing `}`. `span` is the span every one of `start`/`end`/`loc` derives
/// from (the invariant `create_location` relies on for fused translation);
/// the `loc` body mirrors `create_location` + the `SourceLocation`/`Position`
/// serialization (TS conversion never sets `Position.character`, so it is
/// always omitted). Static fragments are pre-fused into the fewest buffer
/// writes — this runs once per node.
pub(super) fn node_header(w: &mut JsonWriter, node_type: &str, span: Span, ctx: &Ctx<'_>) {
    debug_assert!(
        node_type
            .bytes()
            .all(|b| b != b'"' && b != b'\\' && b >= 0x20),
        "node type must be escape-free: {node_type:?}"
    );
    let (start_pos, start) = ctx.loc.pos_and_position(span.start);
    let (end_pos, end) = ctx.loc.pos_and_position(span.end);
    w.raw("{\"type\":\"");
    w.raw(node_type);
    w.raw("\",\"start\":");
    w.u32(start_pos);
    w.raw(",\"end\":");
    w.u32(end_pos);
    w.raw(",\"loc\":{\"start\":{\"line\":");
    w.usize(start.line);
    w.raw(",\"column\":");
    w.usize(start.column);
    w.raw("},\"end\":{\"line\":");
    w.usize(end.line);
    w.raw(",\"column\":");
    w.usize(end.column);
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

/// Mirrors `public::name_cow`: borrow the source slice when it equals the
/// resolved name, else the resolved name — the writer emits either directly
/// (no `Cow`, no allocation on either branch).
#[inline]
pub(super) fn write_name(
    w: &mut JsonWriter,
    span: Span,
    sym: string_interner::DefaultSymbol,
    ctx: &Ctx<'_>,
) {
    use tsv_lang::InfallibleResolve;
    let resolved = ctx.interner.resolve_infallible(sym);
    let raw = span.extract(ctx.source);
    w.string(if raw == resolved { raw } else { resolved });
}

/// Mirrors `serialize_literal_value` over `json_number_from_f64(n)`: integral
/// doubles emit as expanded shortest-round-trip integers (JS `JSON.stringify`
/// semantics), non-finite collapses to `0`, everything else is ryu.
pub(super) fn write_number_value(w: &mut JsonWriter, n: f64) {
    if !n.is_finite() {
        // `json_number_from_f64` maps NaN/±Inf to integer 0.
        w.raw("0");
        return;
    }
    if n.fract() == 0.0 {
        // Below 2^53 every integral f64 is exact, so the shortest round-trip
        // representation *is* the integer's own digits — skip the format!/parse
        // round trip the typed path pays.
        if n.abs() < 9_007_199_254_740_992.0 {
            w.i64(n as i64);
            return;
        }
        // Above 2^53 the shortest representation can denote the double with
        // fewer significant digits than the exact integer (JS prints that
        // expanded form), so go through Display + parse like the typed path.
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

/// Mirrors `convert_literal`.
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
    w.raw("}");
}

/// Shared `Identifier` node emission. Mirrors the `public::Identifier` field
/// set: `name`, then `optional` (only when true), `typeAnnotation` (only when
/// present), `decorators` (only when non-empty).
pub(super) fn write_identifier_parts(
    w: &mut JsonWriter,
    span: Span,
    sym: string_interner::DefaultSymbol,
    optional: bool,
    type_annotation: Option<&internal::TSTypeAnnotation<'_>>,
    decorators: Option<&[internal::Decorator<'_>]>,
    ctx: &Ctx<'_>,
) {
    node_header(w, "Identifier", span, ctx);
    w.raw(",\"name\":");
    write_name(w, span, sym, ctx);
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
    w.raw("}");
}

/// Mirrors `convert_identifier` (the plain form: no optional flag, no type
/// annotation, no decorators — regardless of what the binding carries).
#[inline]
pub(super) fn write_identifier_plain(
    w: &mut JsonWriter,
    id: &internal::Identifier<'_>,
    ctx: &Ctx<'_>,
) {
    write_identifier_parts(w, id.span, id.name, false, None, None, ctx);
}

/// An `Identifier` carrying only the binding's `optional` flag (function and
/// method ids, entity-name-as-expression nodes) — mirrors convert's inline
/// constructions with no type annotation or decorators.
#[inline]
pub(super) fn write_identifier_with_optional(
    w: &mut JsonWriter,
    id: &internal::Identifier<'_>,
    ctx: &Ctx<'_>,
) {
    write_identifier_parts(w, id.span, id.name, id.optional, None, None, ctx);
}
