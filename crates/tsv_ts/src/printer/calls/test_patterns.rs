// Test function pattern detection for TypeScript call expressions
//
// Detects test framework patterns that Prettier keeps on a single line:
// Jest, Mocha, Jasmine, Playwright, Vitest patterns

use super::super::Printer;
use crate::ast::internal;
use smallvec::SmallVec;
use string_interner::DefaultSymbol;
use tsv_lang::{InfallibleResolve, SymbolResolver};

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

/// Get the name of an identifier if it's a simple identifier
fn get_identifier_name(expr: &internal::Expression<'_>) -> Option<DefaultSymbol> {
    if let internal::Expression::Identifier(id) = expr {
        Some(id.name)
    } else {
        None
    }
}

/// Build the dotted callee string (e.g. `test.describe.only`) for a simple
/// identifier or a non-computed, non-optional member chain. Returns `None` for
/// anything else (computed access, optional chains, or non-identifier callees).
///
/// Used by the test-call layout for the flat callee text. `is_test_call` matches
/// the pattern list against the chain parts directly (no allocation) via the
/// shared [`get_member_chain_parts`], so the two stay in lockstep on which
/// callees qualify.
pub(super) fn callee_chain_string(
    expr: &internal::Expression<'_>,
    printer: &Printer<'_>,
) -> Option<String> {
    let parts = get_member_chain_parts(expr)?;
    Some(
        parts
            .iter()
            .rev()
            .map(|sym| printer.resolve_symbol(*sym))
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// Get the member chain parts from an expression
/// Returns parts reversed, e.g. `["skip", "test"]` for `test.skip`.
fn get_member_chain_parts(expr: &internal::Expression<'_>) -> Option<SmallVec<[DefaultSymbol; 8]>> {
    let mut parts: SmallVec<[DefaultSymbol; 8]> = SmallVec::new();

    match expr {
        internal::Expression::Identifier(id) => {
            parts.push(id.name);
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
    // member-chain parts against the pattern list directly — one interner borrow,
    // a stack-only `SmallVec` — instead of building and immediately discarding a
    // dotted callee `String` on every test-shaped call (the hot waste this path
    // used to pay). `callee_chain_string` stays for the actual-test-call
    // flat-layout path, which needs the owned text.
    let Some(parts) = get_member_chain_parts(call.callee) else {
        return false;
    };
    let interner = printer.interner().borrow();
    // Parts come out leaf→root; reverse to root→leaf to match the dotted
    // patterns (`test.describe.only`). Identifiers never contain `.`, so a
    // per-segment compare against `pattern.split('.')` is exact.
    let names: SmallVec<[&str; 8]> = parts
        .iter()
        .rev()
        .map(|&sym| interner.resolve_infallible(sym))
        .collect();
    TEST_CALL_PATTERNS
        .iter()
        .any(|pattern| pattern.split('.').eq(names.iter().copied()))
}
