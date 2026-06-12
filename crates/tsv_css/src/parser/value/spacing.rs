/// Determine if a space is needed between two CSS value parts
///
/// Used by formatters to add appropriate spacing between tokens.
/// Examples:
/// - "rgb" + "(" = no space (function call)
/// - "100" + "px" = no space (dimension)
/// - ")" + "format" = space (end of function, start of identifier)
/// - "100%" + "-" = space (operators need spaces in calc())
pub fn should_add_space_between(prev: &str, curr: &str) -> bool {
    let Some(prev_last) = prev.chars().last() else {
        return false;
    };
    let Some(curr_first) = curr.chars().next() else {
        return false;
    };

    // Never space around parens - they connect directly
    if curr_first == '(' || curr_first == ')' || prev_last == '(' {
        return false;
    }

    // Space after commas
    if prev_last == ',' {
        return true;
    }

    // Space around operators (-, +, *, /)
    if curr_first == '-' || curr_first == '+' || curr_first == '*' || curr_first == '/' {
        return true;
    }
    if (prev_last == '-' || prev_last == '+' || prev_last == '*' || prev_last == '/')
        && curr_first.is_alphanumeric()
    {
        return true;
    }

    // Space between adjacent alphanumeric/quotes: "red" "blue", ")" "format", "'val'" "unit"
    let prev_is_alnum = prev_last.is_alphanumeric();
    let curr_is_alnum = curr_first.is_alphanumeric();
    let prev_ends_paren_or_quote = prev_last == ')' || prev_last == '\'' || prev_last == '"';

    (prev_ends_paren_or_quote || prev_is_alnum) && curr_is_alnum
}
