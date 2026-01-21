//! Helper functions for building and analyzing Doc trees

use super::builders::{concat, empty, hardline, if_break, indent, line, softline, text};
use super::types::{Doc, LineKind};

// =============================================================================
// Analysis Utilities
// =============================================================================

/// Check if a doc will definitely break (contains hardline)
///
/// This is used for cascading breaks: if an inner doc will break,
/// outer groups should also break. For example, nested objects where
/// the inner object has a source newline after `{`.
///
/// Based on prettier's `willBreak()` utility.
pub fn will_break(doc: &Doc) -> bool {
    match doc {
        Doc::Text(_) => false,
        Doc::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
        Doc::Indent(inner) | Doc::Dedent(inner) => will_break(inner),
        Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => will_break(contents),
        Doc::IndentIfBreak { contents, .. } => will_break(contents),
        Doc::Group { contents, .. } => will_break(contents),
        // IfBreak is conditional - it doesn't force break mode, just chooses between docs
        Doc::IfBreak { .. } => false,
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(will_break),
        Doc::WithContext { doc, .. } => will_break(doc),
        // LineSuffix content doesn't affect breaking decisions
        Doc::LineSuffix(_) => false,
        Doc::LineSuffixBoundary => false,
        // BreakParent forces the group to break
        Doc::BreakParent => true,
    }
}

/// Check if a doc can break (contains any line elements)
///
/// This is used for assignment layout decisions: if the left-hand side can break,
/// we may need different formatting for the right-hand side.
///
/// Returns true if the doc contains any Line elements (soft, hard, or literal).
/// This matches Prettier's `canBreak()` utility which searches for DOC_TYPE_LINE.
///
/// # Example
/// ```ignore
/// let id_doc = self.build_expression_doc(&declarator.id);
/// let can_break_left = can_break(&id_doc);
/// if can_break_left && is_arrow_function(init) {
///     // Use break-lhs layout
/// }
/// ```
pub fn can_break(doc: &Doc) -> bool {
    match doc {
        // Any Line element means the doc can break
        Doc::Line(_) => true,
        // Recurse into containers
        Doc::Indent(inner) | Doc::Dedent(inner) => can_break(inner),
        Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => can_break(contents),
        Doc::IndentIfBreak { contents, .. } => can_break(contents),
        Doc::Group {
            contents,
            expanded_states,
            ..
        } => {
            can_break(contents)
                || expanded_states
                    .as_ref()
                    .is_some_and(|states| states.iter().any(can_break))
        }
        Doc::IfBreak {
            break_doc,
            flat_doc,
        } => can_break(break_doc) || can_break(flat_doc),
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(can_break),
        Doc::WithContext { doc, .. } => can_break(doc),
        Doc::LineSuffix(inner) => can_break(inner),
        // Text and boundaries can't break
        Doc::Text(_) | Doc::LineSuffixBoundary => false,
        // BreakParent forces breaking
        Doc::BreakParent => true,
    }
}

// =============================================================================
// Join Helpers
// =============================================================================

/// Build a doc from items with a static string separator between them
///
/// This is useful for building comma-separated or space-separated lists.
///
/// # Example
/// ```ignore
/// let docs = vec![text("a"), text("b"), text("c")];
/// let result = join(docs, ", ");
/// // Renders as: "a, b, c"
/// ```
pub fn join(docs: Vec<Doc>, separator: &'static str) -> Doc {
    if docs.is_empty() {
        return empty();
    }
    let mut parts = Vec::with_capacity(docs.len() * 2 - 1);
    for (i, doc) in docs.into_iter().enumerate() {
        if i > 0 {
            parts.push(text(separator));
        }
        parts.push(doc);
    }
    concat(parts)
}

/// Build a doc from items with a Doc separator between them
///
/// More flexible than `join()` - accepts any Doc as separator.
/// Use this when you need separators like `line()`, `softline()`,
/// or `concat(vec![text(","), line()])`.
///
/// # Example
/// ```ignore
/// // Comma-separated with line breaks
/// let docs = vec![text("a"), text("b"), text("c")];
/// let sep = concat(vec![text(","), line()]);
/// let result = join_doc(docs, sep);
/// // In break mode renders as: "a,\nb,\nc"
/// ```
pub fn join_doc(docs: Vec<Doc>, separator: Doc) -> Doc {
    if docs.is_empty() {
        return empty();
    }
    let mut parts = Vec::with_capacity(docs.len() * 2 - 1);
    for (i, doc) in docs.into_iter().enumerate() {
        if i > 0 {
            parts.push(separator.clone());
        }
        parts.push(doc);
    }
    concat(parts)
}

/// Join docs with separator, adding trailing separator only when breaking
///
/// This is the common pattern for lists with trailing commas:
/// - Flat: `a, b, c`
/// - Break: `a,\nb,\nc,` (trailing comma)
///
/// # Example
/// ```ignore
/// let docs = vec![text("a"), text("b"), text("c")];
/// let sep = concat(vec![text(","), line()]);
/// let result = join_trailing(docs, sep);
/// // Flat: "a, b, c"
/// // Break: "a,\nb,\nc,"
/// ```
pub fn join_trailing(docs: Vec<Doc>, separator: Doc) -> Doc {
    if docs.is_empty() {
        return empty();
    }
    let mut parts = Vec::with_capacity(docs.len() * 2);
    for (i, doc) in docs.into_iter().enumerate() {
        if i > 0 {
            parts.push(separator.clone());
        }
        parts.push(doc);
    }
    // Add trailing separator only when breaking
    // Extract just the punctuation part (e.g., "," from ",\n")
    let trailing = extract_trailing_punctuation(&separator);
    parts.push(if_break(trailing, text("")));
    concat(parts)
}

/// Extract the punctuation part from a separator for trailing comma
///
/// For `concat([text(","), line()])` returns `text(",")`
/// For `text(",")` returns `text(",")`
fn extract_trailing_punctuation(separator: &Doc) -> Doc {
    match separator {
        Doc::Concat(parts) => {
            // Find the first text element (usually the punctuation)
            for part in parts.iter() {
                if let Doc::Text(doc_text) = part {
                    return Doc::Text(doc_text.clone());
                }
            }
            // Fallback: just use the whole separator
            separator.clone()
        }
        Doc::Text(_) => separator.clone(),
        _ => separator.clone(),
    }
}

// =============================================================================
// Wrap Helpers
// =============================================================================

/// Wrap a doc with open and close delimiters
///
/// # Example
/// ```ignore
/// let inner = text("content");
/// let result = wrap("(", inner, ")");
/// // Renders as: "(content)"
/// ```
#[inline]
pub fn wrap(open: &'static str, inner: Doc, close: &'static str) -> Doc {
    concat(vec![text(open), inner, text(close)])
}

/// Wrap a doc in parentheses
///
/// Convenience alias for `wrap("(", inner, ")")`
#[inline]
pub fn parens(inner: Doc) -> Doc {
    wrap("(", inner, ")")
}

/// Wrap a doc in square brackets
///
/// Convenience alias for `wrap("[", inner, "]")`
#[inline]
pub fn brackets(inner: Doc) -> Doc {
    wrap("[", inner, "]")
}

/// Wrap a doc in curly braces
///
/// Convenience alias for `wrap("{", inner, "}")`
#[inline]
pub fn braces(inner: Doc) -> Doc {
    wrap("{", inner, "}")
}

// =============================================================================
// Indent Helpers
// =============================================================================

/// Indent with leading line break
///
/// Shorthand for `indent(concat(vec![line(), inner]))`.
/// Use when content should break to a new indented line.
///
/// # Example
/// ```ignore
/// // Break after "=" and indent the value
/// concat(vec![text("x ="), indent_line(text("value"))])
/// // In break mode: "x =\n  value"
/// ```
#[inline]
pub fn indent_line(inner: Doc) -> Doc {
    indent(concat(vec![line(), inner]))
}

/// Indent with leading softline
///
/// Shorthand for `indent(concat(vec![softline(), inner]))`.
/// Use when content may break to a new indented line (softline disappears in flat mode).
///
/// # Example
/// ```ignore
/// // Arguments that may wrap
/// group(concat(vec![text("fn("), indent_softline(args), softline(), text(")")]))
/// // Flat: "fn(args)"
/// // Break: "fn(\n  args\n)"
/// ```
#[inline]
pub fn indent_softline(inner: Doc) -> Doc {
    indent(concat(vec![softline(), inner]))
}

// =============================================================================
// Separator Helpers
// =============================================================================

/// Comma followed by line break (common separator for lists)
///
/// Shorthand for `concat(vec![text(","), line()])`.
/// Use as separator in `join_doc` or `join_trailing` for comma-separated lists.
///
/// # Example
/// ```ignore
/// // Function arguments that wrap with trailing commas
/// join_trailing(arg_docs, comma_line())
/// // Flat: "a, b, c"
/// // Break: "a,\nb,\nc"
/// ```
#[inline]
pub fn comma_line() -> Doc {
    concat(vec![text(","), line()])
}

/// Comma followed by hardline (forced line break separator)
///
/// Shorthand for `concat(vec![text(","), hardline()])`.
/// Use as separator when elements should always be on separate lines.
///
/// # Example
/// ```ignore
/// // Array elements that are always multi-line
/// join_doc(element_docs, comma_hardline())
/// // Output: "a,\nb,\nc"
/// ```
#[inline]
pub fn comma_hardline() -> Doc {
    concat(vec![text(","), hardline()])
}

/// Trailing comma (only appears in break mode)
///
/// Shorthand for `if_break(text(","), text(""))`.
/// Use after the last element in a list to add a trailing comma only when broken.
///
/// # Example
/// ```ignore
/// // Array with optional trailing comma
/// group(concat(vec![text("["), inner, trailing_comma(), text("]")]))
/// // Flat: "[a, b, c]"
/// // Break: "[\n  a,\n  b,\n  c,\n]"
/// ```
#[inline]
pub fn trailing_comma() -> Doc {
    if_break(text(","), text(""))
}

// =============================================================================
// Multi-Level Indent
// =============================================================================

/// Apply N levels of indentation to a doc.
///
/// This wraps the inner doc in N nested `indent()` calls, useful when you need
/// to programmatically determine the indent depth at runtime.
///
/// # Example
/// ```ignore
/// // Indent by 3 levels
/// let doc = apply_indent_levels(text("content"), 3);
/// // Equivalent to: indent(indent(indent(text("content"))))
/// ```
#[inline]
pub fn apply_indent_levels(inner: Doc, levels: usize) -> Doc {
    let mut result = inner;
    for _ in 0..levels {
        result = indent(result);
    }
    result
}
