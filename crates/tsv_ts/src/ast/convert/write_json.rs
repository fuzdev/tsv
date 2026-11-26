//! The wire writer's JSON-copy helpers: the writer functions whose JSON form has no tape
//! counterpart worth sharing a body with — the staged runs (a node header, a plain identifier,
//! a call/member spine), the name field's fixed-width window, a literal's fused fields and a
//! number's JS spelling. The JSON mount (`mod.rs`) provides them to `write/` by name, as the
//! tape mount (`write_tape.rs`) provides its own; the shared body calls each by that name, so
//! neither copy carries the other's form.

use super::CommentMode;
use super::write::expressions::{
    ChainTail, ExprFlags, chain_link, write_chain_general, write_chain_tails,
    write_expression_inner,
};
use super::write::{Ctx, write_identifier_parts, write_literal_fields};
use crate::ast;
use smallvec::SmallVec;
use tsv_lang::{JsonWriter, Span, StageRun};

/// The header's bytes — `{"type":"X"` and the position fields of `span` — as one
/// staged run: the part of a header that does not report to the comment attach.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn header_run<const CHARACTER: bool>(
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
/// the tail of `header_run`.
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
    let (start, end) = lines.span_positions(span.start, span.end);
    run.raw(",\"start\":");
    run.u32(start.0);
    run.raw(",\"end\":");
    run.u32(end.0);
    run.loc_object::<CHARACTER>(",\"loc\":", start, end);
}

/// Emit an identifier's `,"name":` field — the single name-emission funnel.
/// Span-identity names are the raw source slice (the leading `raw_len` bytes at
/// `name_start`); escaped names are the decoded pool text (an escaped
/// identifier's `\u{78}` source form decodes to `x`). Both arms write the wire
/// value directly; no allocation.
///
/// The span-identity arm skips the escape scan: an `IdentName` with no
/// `escaped` form promises its source bytes hold nothing JSON escapes (see
/// [`ast::IdentName`] — a lexed `IdentifierName` by the grammar, any other
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
    name: ast::IdentName,
    name_start: u32,
    ctx: &Ctx<'_>,
) {
    match name.escaped {
        Some(id) => write_escaped_name_field(w, ctx.view.escaped_text(id)),
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
/// `Number::toString` / `JSON.stringify` semantics), everything else as
/// `JsonWriter::f64` writes it (`zmij`'s shortest round-trip digits, `serde_json`'s
/// spelling) — which matches JS except the one non-integral decade handled below.
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
        // wrongly keep expanding, so that range falls through to `w.f64`.
        if n.abs() < 1e21 {
            w.raw(&shortest);
            return;
        }
    } else {
        // Non-integral. JS `Number::toString` uses fixed notation down to the
        // spec's `n = -5` (|x| in [1e-6, 1e-5)), whereas `zmij` switches to
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

/// A `Literal`'s `value` and `raw` fields as one write, for the literals whose
/// two fields are one constant or one source token: a string with no escape to
/// decode, a plain decimal integer, `true`, `false` and `null`. Returns `false`,
/// having written nothing, for every other literal — and for a string or number
/// the writer's own conditions decline ([`JsonWriter::try_string_value_raw`],
/// [`JsonWriter::try_number_value_raw`]).
///
/// A numeric token is tested as text, not by its parsed value: one made only of
/// decimal digits with no leading zero is the shortest form of the integer it
/// names, so the value the parser stored prints as those same digits.
#[inline]
fn write_literal_fields_fused(w: &mut JsonWriter, lit: &ast::Literal, ctx: &Ctx<'_>) -> bool {
    match lit.value {
        ast::LiteralValue::String(ast::StringCooked::Verbatim) => {
            w.try_string_value_raw(lit.span.extract(ctx.source).as_bytes())
        }
        ast::LiteralValue::Number(_) => {
            w.try_number_value_raw(lit.span.extract(ctx.source).as_bytes())
        }
        ast::LiteralValue::Boolean(true) => {
            w.raw(",\"value\":true,\"raw\":\"true\"");
            true
        }
        ast::LiteralValue::Boolean(false) => {
            w.raw(",\"value\":false,\"raw\":\"false\"");
            true
        }
        ast::LiteralValue::Null => {
            w.raw(",\"value\":null,\"raw\":\"null\"");
            true
        }
        ast::LiteralValue::String(ast::StringCooked::Decoded(_)) | ast::LiteralValue::BigInt => {
            false
        }
    }
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
pub(super) fn write_identifier_plain(w: &mut JsonWriter, id: &ast::Identifier, ctx: &Ctx<'_>) {
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

/// A node header on the span-only wire (`{"type":"X","start":N,"end":M`), staged
/// into the caller's open run — the bytes [`header_run`] writes there, for a caller
/// that stages several nodes into one run. The caller made room
/// ([`SPAN_HEADER_MAX`]).
///
/// Only for a caller already known to run with no comment attach and on the
/// span-only wire: the node's open reports to no attach, and the header carries
/// no `loc`. Stated once in the assert rather than tested again per header.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn stage_span_header(
    run: &mut StageRun<'_>,
    node_type: &'static str,
    span: Span,
    ctx: &Ctx<'_>,
) {
    debug_assert!(
        matches!(ctx.comments, CommentMode::Off) && ctx.positions.lines().is_none(),
        "a staged header reports to no comment attach and carries no `loc`"
    );
    run.raw("{\"type\":\"");
    run.short(node_type);
    run.raw("\",\"start\":");
    run.u32(ctx.pos(span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(span.end));
}

/// The widest [`stage_span_header`] plus the child key a caller stages behind it:
/// `{"type":"`, the longest node type, `","start":`, an integer, `,"end":`, an
/// integer, and the longest child key (`,"expression":`). Each integer is counted
/// at `u64`'s width ([`STAGED_INTEGER_MAX`]), so the bound holds without an
/// argument about which offsets are reachable.
pub(super) const SPAN_HEADER_MAX: usize = "{\"type\":\"".len()
    + LONGEST_NODE_TYPE
    + "\",\"start\":".len()
    + STAGED_INTEGER_MAX
    + ",\"end\":".len()
    + STAGED_INTEGER_MAX
    + ",\"expression\":".len();

/// The longest node `type` the writer emits.
const LONGEST_NODE_TYPE: usize = "TSConstructSignatureDeclaration".len();

/// The widest integer a staged-width bound counts: `u64::MAX`'s digits, wider
/// than any `u32` offset, so no bound depends on which values are reachable.
const STAGED_INTEGER_MAX: usize = u64::MAX.ilog10() as usize + 1;

/// The name bytes of an `Identifier` that [`stage_identifier_plain`] may stage
/// whole, or `None` — the identifiers whose expression emission is
/// `write_identifier_plain`'s run arm.
///
/// That is an identifier with no binding extra and no `?` (the expression
/// dispatch's plain arm), and a name its run arm stages: unescaped, so its
/// source bytes are the name and hold nothing JSON escapes, and within
/// [`StageRun::SHORT_MAX`]. For a caller already known to run with no comment
/// attach, the run arm's third condition.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn plain_identifier_stage_name<'s>(
    id: &ast::Identifier,
    ctx: &Ctx<'s>,
) -> Option<&'s [u8]> {
    let name = id.ident_name();
    let name_len = name.raw_len as usize;
    if id.extra.is_some() || id.optional || name.escaped.is_some() || name_len > StageRun::SHORT_MAX
    {
        return None;
    }
    let start = id.span.start as usize;
    Some(&ctx.source.as_bytes()[start..start + name_len])
}

/// Stage a plain `Identifier` on the span-only wire, whose name
/// [`plain_identifier_stage_name`] returned, into the caller's open run —
/// `write_identifier_plain`'s run bytes. The caller made room
/// ([`PLAIN_IDENTIFIER_MAX`]).
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn stage_identifier_plain(
    run: &mut StageRun<'_>,
    span: Span,
    name: &[u8],
    ctx: &Ctx<'_>,
) {
    run.raw("{\"type\":\"Identifier\",\"start\":");
    run.u32(ctx.pos(span.start));
    run.raw(",\"end\":");
    run.u32(ctx.pos(span.end));
    run.raw(",\"name\":\"");
    run.short_bytes(name);
    run.raw("\"}");
}

/// The widest [`stage_identifier_plain`]: `{"type":"Identifier","start":`, an
/// integer, `,"end":`, an integer, `,"name":"`, a name of up to
/// [`StageRun::SHORT_MAX`] bytes and `"}`, each integer at `u64`'s width
/// ([`STAGED_INTEGER_MAX`]).
pub(super) const PLAIN_IDENTIFIER_MAX: usize = "{\"type\":\"Identifier\",\"start\":".len()
    + STAGED_INTEGER_MAX
    + ",\"end\":".len()
    + STAGED_INTEGER_MAX
    + ",\"name\":\"".len()
    + StageRun::SHORT_MAX
    + "\"}".len();

/// A `Literal`'s fields: the common literals as [`write_literal_fields_fused`]'s single write
/// (checked against the field-by-field write in debug builds), every other one field by field.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn write_literal_fields_any(w: &mut JsonWriter, lit: &ast::Literal, ctx: &Ctx<'_>) {
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
}

/// `write_chain`'s JSON form: the staged spine where nothing can
/// interrupt a run, the general one otherwise.
#[expect(clippy::inline_always)]
#[inline(always)]
pub(super) fn write_chain_any(
    w: &mut JsonWriter,
    expr: &ast::Expression,
    ctx: &Ctx<'_>,
    flags: ExprFlags,
) {
    if matches!(ctx.comments, CommentMode::Off) && ctx.positions.lines().is_none() {
        #[cfg(debug_assertions)]
        if GENERAL_CHAIN_REFERENCE.get() {
            write_chain_general(w, expr, ctx, flags);
            return;
        }
        #[cfg(debug_assertions)]
        let from = w.as_bytes().len();
        write_chain_staged(w, expr, ctx, flags);
        #[cfg(debug_assertions)]
        debug_assert_staged_chain_is_general(&w.as_bytes()[from..], expr, ctx, flags);
    } else {
        write_chain_general(w, expr, ctx, flags);
    }
}

#[cfg(debug_assertions)]
thread_local! {
    /// Set while [`debug_assert_staged_chain_is_general`] writes its reference: every
    /// chain then takes [`write_chain_general`], so the reference holds no staged bytes,
    /// and the chains nested in it are not checked again — each was checked where the
    /// staged write reached it, and a check per enclosing chain would make the cost
    /// exponential in the nesting.
    static GENERAL_CHAIN_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Debug-only: `staged` — the bytes [`write_chain_staged`] just wrote for `expr` — must
/// be the bytes [`write_chain_general`] writes for it — the form every comment-attach and
/// `loc` emission takes — so the two forms are tied on every chain a debug build stages.
/// Release builds compile out the reference write and the compare.
#[cfg(debug_assertions)]
fn debug_assert_staged_chain_is_general(
    staged: &[u8],
    expr: &ast::Expression,
    ctx: &Ctx<'_>,
    flags: ExprFlags,
) {
    /// Clears the flag on the way out, a panic in the reference write included — a
    /// caller that catches it (the fuzzer) would otherwise run every later chain
    /// unchecked.
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            GENERAL_CHAIN_REFERENCE.set(false);
        }
    }
    let mut general = JsonWriter::with_capacity(staged.len());
    {
        GENERAL_CHAIN_REFERENCE.set(true);
        let _reset = Reset;
        write_chain_general(&mut general, expr, ctx, flags);
    }
    debug_assert!(
        staged == general.as_bytes(),
        "the staged chain must be the general path's bytes\n  staged: {}\n general: {}",
        String::from_utf8_lossy(staged),
        String::from_utf8_lossy(general.as_bytes()),
    );
}

/// `write_chain` with no comment attach, on the span-only wire, where a node's open and
/// close report to nothing and its header carries no `loc`: the bytes of several nodes
/// then run together with nothing between them, so they are staged into one run rather
/// than appended a fragment at a time.
///
/// Down the spine, every link's `ChainExpression` wrap, header and child key go into one
/// run held open across the links ([`stage_span_header`]). A base that is a plain
/// identifier is staged into it too ([`stage_identifier_plain`]); any other base flushes
/// the run and goes through the dispatch. Back up, the innermost tails the run can hold
/// whole ([`stage_chain_tail`]) are staged; the first that cannot flushes the run, and it
/// and every tail above it take [`write_chain_tails`]. The run spills to the output
/// buffer whenever the next step might not fit (`StageRun::spill_unless_room`), so a
/// spine of any length stages.
///
/// The general form's bytes, step for step: [`stage_span_header`] is `node_header`'s
/// span-only run, [`stage_identifier_plain`] `write_identifier_plain`'s, and each staged
/// tail the literal the general tail writes for the same flags.
///
/// The tails are read in place, by index, never iterated by value: `into_iter` moves the
/// `SmallVec`, inline buffer and all, into the iterator.
#[inline(never)]
fn write_chain_staged(w: &mut JsonWriter, expr: &ast::Expression, ctx: &Ctx<'_>, flags: ExprFlags) {
    let mut tails: SmallVec<[(ChainTail<'_>, Span, bool); 8]> = SmallVec::new();
    let (mut expr, mut flags) = (expr, flags);
    let mut run = w.stage_run();
    loop {
        let Some(link) = chain_link(ctx.view, expr, flags) else {
            if let ast::ExpressionKind::Identifier(id) = &expr.kind
                && !flags.force_optional
                && let Some(name) = plain_identifier_stage_name(id, ctx)
            {
                run.spill_unless_room::<PLAIN_IDENTIFIER_MAX>();
                stage_identifier_plain(&mut run, id.span, name, ctx);
            } else {
                run.flush();
                write_expression_inner(w, expr, ctx, flags);
                run = w.stage_run();
            }
            break;
        };
        run.spill_unless_room::<{ 2 * SPAN_HEADER_MAX }>();
        if link.needs_chain {
            stage_span_header(&mut run, "ChainExpression", expr.span, ctx);
            run.raw(",\"expression\":");
        }
        stage_span_header(&mut run, link.node_type, expr.span, ctx);
        run.short(link.child_key);
        tails.push((link.tail, expr.span, link.needs_chain));
        expr = ctx.view.expr(link.child);
        flags = link.child_flags;
    }
    let mut unstaged = tails.len();
    while let Some(entry) = unstaged.checked_sub(1).map(|i| &tails[i]) {
        if !stage_chain_tail(&mut run, entry, ctx) {
            break;
        }
        unstaged -= 1;
    }
    run.flush();
    write_chain_tails(w, &tails[..unstaged], ctx);
}

/// Stage one link's tail and its `ChainExpression` close into the open run, for the tails
/// whose every byte is known here: a non-computed member whose property is a plain
/// identifier ([`plain_identifier_stage_name`]), a call with no arguments and no type
/// arguments, and a non-null close. Returns `false`, having staged nothing, for any other
/// tail.
///
/// Each arm is `write_member_expression_tail` /
/// `write_call_expression_tail` /
/// `close_node` with those fields fixed: the property is the plain identifier the
/// dispatch writes for it, a call's argument list is `[]` and its type-arguments field
/// absent — which makes its `optional` field unconditional — and every close is the bare
/// `}`.
#[expect(clippy::inline_always)]
#[inline(always)]
fn stage_chain_tail(
    run: &mut StageRun<'_>,
    &(ref tail, _, needs_chain): &(ChainTail<'_>, Span, bool),
    ctx: &Ctx<'_>,
) -> bool {
    match *tail {
        ChainTail::Member {
            member,
            force,
            strip,
        } => {
            if member.computed {
                return false;
            }
            let ast::ExpressionKind::Identifier(id) = &ctx.view.expr(member.property).kind else {
                return false;
            };
            let Some(name) = plain_identifier_stage_name(id, ctx) else {
                return false;
            };
            run.spill_unless_room::<MEMBER_TAIL_MAX>();
            run.raw(",\"property\":");
            stage_identifier_plain(run, id.span, name, ctx);
            if strip {
                run.raw(",\"computed\":false}");
            } else if force || member.optional {
                run.raw(",\"computed\":false,\"optional\":true}");
            } else {
                run.raw(",\"computed\":false,\"optional\":false}");
            }
        }
        ChainTail::Call {
            call, force, strip, ..
        } => {
            if call.type_args.is_some() || !ctx.view.expr_list(call.arguments).is_empty() {
                return false;
            }
            run.spill_unless_room::<CALL_TAIL_MAX>();
            if strip {
                run.raw(",\"arguments\":[]}");
            } else if force || call.optional {
                run.raw(",\"arguments\":[],\"optional\":true}");
            } else {
                run.raw(",\"arguments\":[],\"optional\":false}");
            }
        }
        ChainTail::NonNull => {
            run.spill_unless_room::<NON_NULL_TAIL_MAX>();
            run.raw("}");
        }
    }
    if needs_chain {
        run.raw("}");
    }
    true
}

/// The widest staged member tail: `,"property":`, a plain identifier, the widest
/// `computed`/`optional` literal with the node's `}`, and a `ChainExpression`'s `}`.
const MEMBER_TAIL_MAX: usize = ",\"property\":".len()
    + PLAIN_IDENTIFIER_MAX
    + ",\"computed\":false,\"optional\":false}".len()
    + 1;

/// The widest staged call tail: the widest `arguments`/`optional` literal with the
/// node's `}`, and a `ChainExpression`'s `}`.
const CALL_TAIL_MAX: usize = ",\"arguments\":[],\"optional\":false}".len() + 1;

/// The staged non-null tail: the node's `}` and a `ChainExpression`'s.
const NON_NULL_TAIL_MAX: usize = 2;
