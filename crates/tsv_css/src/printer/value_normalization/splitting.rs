// Value splitting and whitespace normalization: top-level splits that respect
// parens/quotes/comments, plus prettier-style whitespace collapsing.

/// Normalize CSS whitespace in extracted source text
///
/// Single-pass normalization that:
/// - Collapses consecutive whitespace to single spaces
/// - Removes spaces after opening parentheses: `( expr` → `(expr`
/// - Removes spaces before closing parentheses: `expr )` → `expr)`
/// - Preserves content inside quoted strings (`'...'` and `"..."`)
/// - Preserves content inside CSS comments (`/* ... */`)
///
/// This matches Prettier's normalization for calc(), var(), and other CSS functions.
///
/// # Example
/// ```ignore
/// assert_eq!(
///     normalize_css_whitespace("10px  /* test */  20px"),
///     "10px /* test */ 20px"
/// );
/// assert_eq!(
///     normalize_css_whitespace("var( --a, /* comment */ red )"),
///     "var(--a, /* comment */ red)"
/// );
/// assert_eq!(
///     normalize_css_whitespace("url( 'path with spaces' )"),
///     "url('path with spaces')"
/// );
/// ```
pub(crate) fn normalize_css_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut string_delim = '\0';
    let mut in_comment = false;
    let mut pending_space = false;

    while let Some(ch) = chars.next() {
        // Check for comment start (outside strings)
        if !in_string && !in_comment && ch == '/' && chars.peek() == Some(&'*') {
            // Add space before comment if preceded by non-whitespace (except `(`)
            // This normalizes `foo,/*` → `foo, /*`
            if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            result.push('/');
            chars.next(); // consume '*'
            result.push('*');
            in_comment = true;
            continue;
        }

        // Check for comment end
        if in_comment && ch == '*' && chars.peek() == Some(&'/') {
            result.push('*');
            chars.next(); // consume '/'
            result.push('/');
            in_comment = false;
            pending_space = true; // Space needed before next token
            continue;
        }

        // Inside comment - preserve everything
        if in_comment {
            result.push(ch);
            continue;
        }

        // String delimiter handling (outside comments)
        if !in_string && (ch == '\'' || ch == '"') {
            if pending_space && !result.is_empty() && !result.ends_with('(') {
                result.push(' ');
            }
            pending_space = false;
            in_string = true;
            string_delim = ch;
            result.push(ch);
            continue;
        }

        if in_string && ch == string_delim {
            in_string = false;
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Inside string - preserve everything
        if in_string {
            result.push(ch);
            continue;
        }

        // Opening paren - skip following whitespace
        if ch == '(' {
            if pending_space && !result.is_empty() {
                result.push(' ');
            }
            pending_space = false;
            result.push(ch);
            // Skip whitespace after opening paren
            while chars.peek().is_some_and(|&c| c.is_whitespace()) {
                chars.next();
            }
            continue;
        }

        // Closing paren - remove trailing whitespace
        if ch == ')' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = false;
            continue;
        }

        // Comma - no space before, single space after (CSS never wants a space
        // before a comma, e.g. a media-query list `projection, tv`).
        if ch == ',' {
            while result.ends_with(' ') {
                result.pop();
            }
            result.push(ch);
            pending_space = true;
            continue;
        }

        // Whitespace - mark pending (collapse consecutive)
        if ch.is_whitespace() {
            if !result.is_empty() && !result.ends_with('(') {
                pending_space = true;
            }
            continue;
        }

        // Regular character - add pending space if needed
        if pending_space && !result.is_empty() {
            result.push(' ');
            pending_space = false;
        }
        result.push(ch);
    }

    result.trim().to_string()
}

/// Normalize spacing in a value containing comments (alias for backward compatibility)
#[inline]
pub(crate) fn normalize_value_spacing(value: &str) -> String {
    normalize_css_whitespace(value)
}

/// Extract the content between a function's parentheses from source
///
/// Given source like `property: func_name(arg1, arg2)` and func_name `func_name`,
/// returns `Some("arg1, arg2")`. Returns `None` if the function can't be found.
pub(crate) fn extract_function_args<'a>(source: &'a str, func_name: &str) -> Option<&'a str> {
    let func_start = source.find(func_name)?;
    let after_name = &source[func_start + func_name.len()..];
    let open_paren = after_name.find('(')?;

    let inner_start = func_start + func_name.len() + open_paren + 1;
    let inner_content = &source[inner_start..];

    // Find closing paren (handle nested parens)
    let mut depth = 1;
    for (i, c) in inner_content.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&inner_content[..i]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Split by top-level spaces, preserving content inside parentheses, quotes, and comments
///
/// Used for space-separated values like `var(--b) color-mix(...)`.
/// Returns individual values that can be wrapped independently.
pub(crate) fn split_by_space_preserving_parens(content: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;
    let mut in_comment = false;
    let mut in_quote = false;
    let mut quote_char = b'\0';
    let bytes = content.as_bytes();

    let mut i = 0;
    while i < content.len() {
        // Check for comment start (outside quotes)
        if !in_quote
            && !in_comment
            && i + 1 < content.len()
            && bytes[i] == b'/'
            && bytes[i + 1] == b'*'
        {
            in_comment = true;
            i += 2;
            continue;
        }
        // Check for comment end
        if in_comment && i + 1 < content.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }
        if in_comment {
            i += 1;
            continue;
        }

        // Handle quotes
        if !in_quote && (bytes[i] == b'\'' || bytes[i] == b'"') {
            in_quote = true;
            quote_char = bytes[i];
            i += 1;
            continue;
        }
        if in_quote && bytes[i] == quote_char {
            in_quote = false;
            i += 1;
            continue;
        }
        if in_quote {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b' ' | b'\t' if depth == 0 => {
                let part = &content[start..i];
                if !part.trim().is_empty() {
                    parts.push(part.trim());
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Don't forget the last part
    if start < content.len() {
        let part = &content[start..];
        if !part.trim().is_empty() {
            parts.push(part.trim());
        }
    }

    parts
}

/// Split function arguments by top-level commas, preserving nested parens, quotes, and comments
///
/// Used when extracting function arguments from source while preserving comments.
/// Handles nested parentheses correctly so `func(a, b)` inside an arg isn't split.
/// Skips over block comments so commas inside `/* a, b */` aren't treated as separators.
/// Skips over quoted strings so commas inside `"a, b"` aren't treated as separators.
pub(crate) fn split_args_by_comma(content: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth: u32 = 0;
    let mut start = 0;
    let mut in_comment = false;
    let mut in_quote = false;
    let mut quote_char = b'\0';
    let bytes = content.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        // Check for comment start (outside quotes)
        if !in_quote
            && !in_comment
            && i + 1 < bytes.len()
            && bytes[i] == b'/'
            && bytes[i + 1] == b'*'
        {
            in_comment = true;
            i += 2;
            continue;
        }

        // Check for comment end
        if in_comment && i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            in_comment = false;
            i += 2;
            continue;
        }

        // Skip content inside comments
        if in_comment {
            i += 1;
            continue;
        }

        // Handle quotes
        if !in_quote && (bytes[i] == b'\'' || bytes[i] == b'"') {
            in_quote = true;
            quote_char = bytes[i];
            i += 1;
            continue;
        }
        if in_quote && bytes[i] == quote_char {
            in_quote = false;
            i += 1;
            continue;
        }
        if in_quote {
            i += 1;
            continue;
        }

        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                args.push(&content[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    // Don't forget the last argument
    if start < content.len() {
        args.push(&content[start..]);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function_args() {
        assert_eq!(
            extract_function_args("prop: var(--a, red)", "var"),
            Some("--a, red")
        );
        assert_eq!(
            extract_function_args("prop: var(--a, /* comment */ red)", "var"),
            Some("--a, /* comment */ red")
        );
        // Nested parens
        assert_eq!(
            extract_function_args("prop: var(--a, calc(1 + 2))", "var"),
            Some("--a, calc(1 + 2)")
        );
        // Function not found
        assert_eq!(extract_function_args("prop: red", "var"), None);
    }

    #[test]
    fn test_split_args_by_comma() {
        assert_eq!(split_args_by_comma("a, b, c"), vec!["a", " b", " c"]);
        assert_eq!(split_args_by_comma("--a, red"), vec!["--a", " red"]);
        // Nested parens preserved
        assert_eq!(
            split_args_by_comma("--a, calc(1, 2)"),
            vec!["--a", " calc(1, 2)"]
        );
        // Single arg
        assert_eq!(split_args_by_comma("--a"), vec!["--a"]);
        // Empty
        assert_eq!(split_args_by_comma(""), Vec::<&str>::new());
        // Commas inside comments are NOT separators
        assert_eq!(
            split_args_by_comma("--a, /* with, comma */ red"),
            vec!["--a", " /* with, comma */ red"]
        );
        assert_eq!(
            split_args_by_comma("/* a, b */ value"),
            vec!["/* a, b */ value"]
        );
        // Commas inside quotes are NOT separators
        assert_eq!(
            split_args_by_comma(r#"--font, "Font, Name""#),
            vec!["--font", r#" "Font, Name""#]
        );
        assert_eq!(
            split_args_by_comma(r"'a, b', 'c, d'"),
            vec!["'a, b'", " 'c, d'"]
        );
    }
}
