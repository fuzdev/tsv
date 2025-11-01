use super::lists::split_values_at_delimiter;
use crate::ast::internal::CssValue;

/// Parse function arguments as comma-separated values
pub fn parse_function_arguments(args_str: &str) -> Vec<CssValue> {
    split_values_at_delimiter(args_str, |c| c == ',')
}
