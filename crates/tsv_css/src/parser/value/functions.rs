use super::parser::ValueParser;
use crate::ast::internal::CssValue;
use tsv_lang::Span;

/// Parse function arguments as comma-separated values
///
/// Uses ValueParser for accurate span tracking through same-source recursion.
/// This ensures nested function arguments maintain correct byte positions.
///
/// # Arguments
/// * `args_str` - The function arguments string (e.g., "90deg, red 01%, blue 02%")
/// * `parent_span` - The span of the arguments in the full CSS document
///
/// # Returns
/// Vector of parsed argument values
pub fn parse_function_arguments(args_str: &str, parent_span: Span) -> Vec<CssValue> {
    let trimmed = args_str.trim();

    if trimmed.is_empty() {
        return vec![];
    }

    // Calculate adjusted span for trimmed args (same logic as parse_value_from_source)
    let trim_start_offset = args_str.len() - args_str.trim_start().len();
    let trim_end_offset = args_str.len() - args_str.trim_end().len();
    let adjusted_span = Span {
        start: parent_span.start + trim_start_offset as u32,
        end: parent_span.end - trim_end_offset as u32,
    };

    let parser = ValueParser::new(trimmed, adjusted_span);
    let value = parser.parse();

    // Function arguments are comma-separated, so we expect CommaSeparated
    // But if there's only one argument, it might be a single value
    match value {
        CssValue::CommaSeparated { values, .. } => values,
        single => vec![single],
    }
}
