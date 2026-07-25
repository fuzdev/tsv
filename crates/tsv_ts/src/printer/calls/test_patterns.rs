// Test function pattern detection for TypeScript call expressions
//
// Detects test framework patterns that Prettier keeps on a single line:
// Jest, Mocha, Jasmine, Playwright, Vitest patterns

use super::super::Printer;
use super::arg_comments::has_inter_argument_comments_slice;
use crate::ast::internal::{self, IdentName};
use smallvec::SmallVec;
use tsv_lang::doc::arena::DocId;

/// Test function patterns that Prettier keeps on a single line
/// Includes: Jest, Mocha, Jasmine, Playwright, Vitest patterns
pub(super) const TEST_CALL_PATTERNS: &[&str] = &[
    // Core test functions
    "it",
    "it.only",
    "it.skip",
    "describe",
    "describe.only",
    "describe.skip",
    "test",
    "test.only",
    "test.skip",
    "test.fixme", // Playwright 3.7
    "test.step",
    // Playwright describe variants
    "test.describe",
    "test.describe.only",
    "test.describe.skip",
    "test.describe.fixme", // Playwright 3.7
    "test.describe.parallel",
    "test.describe.parallel.only",
    "test.describe.serial",
    "test.describe.serial.only",
    // Focus/skip prefixes
    "skip",
    "xit",
    "xdescribe",
    "xtest",
    "fit",
    "fdescribe",
    "ftest",
];

/// Get the name channel (+ span start) of an identifier if it's a simple identifier
fn get_identifier_name<'a>(expr: &internal::Expression<'a>) -> Option<(IdentName<'a>, u32, u32)> {
    if let internal::Expression::Identifier(id) = expr {
        Some((id.ident_name(), id.span.start, id.span.end))
    } else {
        None
    }
}

/// Build the flat, break-free callee doc (e.g. `test.describe.only`) for the
/// test-call layout, straight from the span-identity member-chain parts of a simple
/// identifier or non-computed, non-optional member chain — no intermediate
/// `String`. Returns `None` for anything else (computed access, optional chains,
/// or non-identifier callees), so the caller falls back to the general callee doc.
///
/// Emitting `source_span` name + `.` doc nodes is byte-identical to the old
/// resolve-and-`join(".")` `String`: identifiers never contain `.`, and
/// the concatenated text nodes carry no break point, so the flat callee still
/// never breaks at `.skip` — at zero heap allocation. `is_test_call` matches the
/// pattern list against the same chain parts directly (also no allocation) via
/// the shared [`get_member_chain_parts`], so the two stay in lockstep on which
/// callees qualify.
///
/// Each `.` goes through the shared dotted-pair printer, which emits the gaps
/// around it — the positions an author can comment in (`describe /* c */.only`,
/// `test./* c */ skip`). Joining the parts with a bare `d.text(".")` scans none of
/// them and drops what's there. It stays break-free: with no comment in a gap the
/// pair is the same three text nodes as before.
pub(super) fn build_test_callee_flat_doc(
    expr: &internal::Expression<'_>,
    printer: &Printer<'_>,
) -> Option<DocId> {
    let parts = get_member_chain_parts(expr)?;
    // Parts come out leaf→root; reverse to root→leaf (`test.describe.only`), which is
    // also the AST's own association — each pair's left is everything before its dot.
    let mut iter = parts.iter().rev();
    // A single-part callee (`it`) is the bare name: the loop never runs.
    let &(name, start, mut prev_end) = iter.next()?;
    let mut doc = printer.ident_name_doc(name, start);
    for &(name, start, end) in iter {
        doc = printer.build_dotted_pair_doc(
            doc,
            printer.ident_name_doc(name, start),
            prev_end,
            start,
        );
        prev_end = end;
    }
    Some(doc)
}

/// Get the member chain parts from an expression
/// Returns parts reversed, e.g. `["skip", "test"]` for `test.skip`.
fn get_member_chain_parts<'a>(
    expr: &internal::Expression<'a>,
) -> Option<SmallVec<[(IdentName<'a>, u32, u32); 8]>> {
    let mut parts: SmallVec<[(IdentName<'a>, u32, u32); 8]> = SmallVec::new();

    match expr {
        internal::Expression::Identifier(id) => {
            parts.push((id.ident_name(), id.span.start, id.span.end));
            Some(parts)
        }
        internal::Expression::MemberExpression(member) => {
            // Don't match computed or optional chains (a[b] or a?.b)
            if member.computed || member.optional {
                return None;
            }

            // Get property name
            let prop_name = get_identifier_name(member.property)?;
            parts.push(prop_name);

            // Recursively get object parts
            let mut object_parts = get_member_chain_parts(member.object)?;
            parts.append(&mut object_parts);

            Some(parts)
        }
        _ => None,
    }
}

/// Whether the test-call FLAT layout applies to `call` — [`is_test_call`] plus the one
/// thing that layout cannot do: print a comment sitting in an argument gap.
///
/// The flat layout joins the argument docs with `", "` and appends a trailing-comment
/// suffix after the last argument; it never opens the `(`→first-argument gap or any
/// inter-argument gap, so a comment there is DROPPED outright (the hazard-4 shape in
/// docs/comments.md — an alternate-layout builder that emits only its children's docs).
/// A layout that cannot print a comment must not be chosen for a call that has one, so
/// such a call falls through and expands like any other.
///
/// **To-emit axis**, deliberately: a GLUED block comment is owned by the argument it
/// precedes and rides inside that argument's own doc, so the flat layout does print it
/// — and keeping the flat layout there matches prettier. The question here is exactly
/// "would a comment be dropped?", which is the to-emit axis's question. A comment
/// trailing the whole call is likewise fine — the layout emits that itself.
///
/// Both routing sites ask this rather than [`is_test_call`] (`calls/mod.rs`'s
/// chain-bypass and `call_formatting.rs`'s layout branch), so they cannot disagree about
/// which calls take the flat form. See conformance_prettier.md §Comment relocation.
pub(super) fn test_call_flat_layout_applies(
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
    paren_open: u32,
) -> bool {
    if !is_test_call(call, printer) {
        return false;
    }
    // Zero-comment fast gate: ONE binary search over `[paren_open, last argument's start)`,
    // which strictly contains every gap the check below looks at — the `(`→first-argument
    // gap and each inter-argument gap all end at or before the last argument. So with no
    // comment in it they are provably all empty.
    //
    // The window deliberately stops at the LAST argument rather than at the call's end: the
    // last argument of a test call is its callback body, which is where a test file's
    // comments overwhelmingly live, so a gate spanning it fires on nearly every call and
    // gates nothing. Canonical reference: `build_params_doc_with_comments`.
    let Some(last_start) = call.arguments.last().map(|a| a.span().start) else {
        return true;
    };
    !(printer.has_comments_to_emit_between(paren_open, last_start)
        && test_call_gaps_have_comments(call, printer, paren_open))
}

/// Whether any argument gap of `call` holds a comment this call would have to emit —
/// the `(`→first-argument gap or an inter-argument gap. Only the gaps, never an
/// argument's interior: a comment inside an argument is printed by that argument's own
/// doc.
fn test_call_gaps_have_comments(
    call: &internal::CallExpression<'_>,
    printer: &Printer<'_>,
    paren_open: u32,
) -> bool {
    call.arguments
        .first()
        .is_some_and(|first| printer.has_comments_to_emit_between(paren_open, first.span().start))
        || has_inter_argument_comments_slice(call.arguments, printer)
}

/// Check if a call expression is a test function call that should stay on one line
pub(super) fn is_test_call(call: &internal::CallExpression<'_>, printer: &Printer<'_>) -> bool {
    // Optional calls (`describe?.(...)`) are never test calls — they format like
    // a normal call (wrap when long), preserving the `?.`. Mirrors prettier's
    // isTestCall guard (`utilities/test-libraries.js`: `node.optional` → false).
    if call.optional {
        return false;
    }

    // Must have 2-3 arguments
    let arg_count = call.arguments.len();
    if !(2..=3).contains(&arg_count) {
        return false;
    }

    // First argument must be a string or template literal
    let first_is_string = match &call.arguments[0] {
        internal::Expression::Literal(lit) => {
            matches!(lit.value, internal::LiteralValue::String { .. })
        }
        internal::Expression::TemplateLiteral(_) => true,
        _ => false,
    };
    if !first_is_string {
        return false;
    }

    // Second argument must be a function expression (arrow or regular)
    let second_is_function = matches!(
        &call.arguments[1],
        internal::Expression::ArrowFunctionExpression(_)
            | internal::Expression::FunctionExpression(_)
    );
    if !second_is_function {
        return false;
    }

    // Third argument (if present) must be a number (timeout)
    if arg_count == 3 {
        let third_is_number = match &call.arguments[2] {
            internal::Expression::Literal(lit) => {
                matches!(lit.value, internal::LiteralValue::Number(_))
            }
            _ => false,
        };
        if !third_is_number {
            return false;
        }
    }

    // Check if the callee matches a known test pattern. Compare the resolved
    // member-chain parts against the pattern list directly — span-identity slices,
    // a stack-only `SmallVec` — instead of building and immediately discarding a
    // dotted callee `String` on every test-shaped call (the hot waste this path
    // used to pay). `callee_chain_string` stays for the actual-test-call
    // flat-layout path, which needs the owned text.
    let Some(parts) = get_member_chain_parts(call.callee) else {
        return false;
    };
    // Parts come out leaf→root; reverse to root→leaf to match the dotted
    // patterns (`test.describe.only`). Identifiers never contain `.`, so a
    // per-segment compare against `pattern.split('.')` is exact.
    let names: SmallVec<[&str; 8]> = parts
        .iter()
        .rev()
        .map(|&(name, name_start, _)| name.resolve(name_start, printer.source))
        .collect();
    TEST_CALL_PATTERNS
        .iter()
        .any(|pattern| pattern.split('.').eq(names.iter().copied()))
}
