use super::parser::ValueParser;
use crate::ast::{CssStores, CssValue};
use tsv_lang::Span;
use tsv_lang::store::{Run, push_run_of_one};

/// Parse function arguments as comma-separated values
///
/// Uses ValueParser for accurate span tracking through same-source recursion.
/// This ensures nested function arguments maintain correct byte positions.
///
/// # Arguments
/// * `args_str` - The function arguments string (e.g., "90deg, red 01%, blue 02%")
/// * `parent_span` - The span of the arguments in the full CSS document
/// * `stores` - The stylesheet's stores, carried into the arguments' parse
///
/// # Returns
/// The run of parsed argument values
pub(crate) fn parse_function_arguments(
    args_str: &str,
    parent_span: Span,
    stores: &mut CssStores,
) -> Run<CssValue> {
    // Strip leading whitespace by offset, then trailing on the remaining slice — one
    // boundary scan per side (vs a `.trim()` plus a separate `.trim_start()`/`.trim_end()`
    // recomputing the same two boundaries). The trailing trim stops at an escaped
    // whitespace (`var(--a, 50px\ )`), which would otherwise strand the backslash onto
    // the closing `)` — see `trim_end_preserving_escape`.
    let trim_start_offset = args_str.len() - crate::escapes::trim_start_css(args_str).len();
    let trimmed = crate::escapes::trim_end_preserving_escape(&args_str[trim_start_offset..]);

    if trimmed.is_empty() {
        return Run::EMPTY;
    }

    // Calculate adjusted span for trimmed args (same logic as parse_value_from_source_into)
    let trim_end_offset = args_str.len() - trim_start_offset - trimmed.len();
    let adjusted_span = Span {
        start: parent_span.start + trim_start_offset as u32,
        end: parent_span.end - trim_end_offset as u32,
    };

    // A function's argument list IS a group, which is what lets a `:` inside it be an
    // operator (`f(a: 1.5)`) where the same byte at a declaration's top level is not.
    let parser = ValueParser::new_within(trimmed, adjusted_span, true);
    let value = parser.parse(stores);

    // Function arguments are comma-separated, so we expect CommaSeparated
    // But if there's only one argument, it might be a single value
    match value {
        // Already a run of the stores — return it directly (zero copy)
        CssValue::CommaSeparated { values, .. } => values,
        single => push_run_of_one(&mut stores.runs.values, single),
    }
}
