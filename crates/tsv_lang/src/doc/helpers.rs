//! Helper functions for building and analyzing Doc trees

use super::builders::{concat, empty, group, hardline, if_break, indent, line, softline, text};
use super::types::{Doc, LineKind};

//
// Analysis Utilities
//

/// Check if a doc will definitely break (contains hardline or should_break group)
///
/// This is used for cascading breaks: if an inner doc will break,
/// outer groups should also break. For example, nested objects where
/// the inner object has a source newline after `{`.
///
/// This function includes groups with `should_break: true` (from `group_break()`),
/// which represent content that prefers to break. Use `has_forced_break()` if you
/// only want to check for unavoidable breaks (hardlines, comments).
///
/// Based on prettier's `willBreak()` utility.
pub fn will_break(doc: &Doc) -> bool {
    match doc {
        Doc::Text(_) => false,
        Doc::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
        Doc::Indent(inner) | Doc::Dedent(inner) => will_break(inner),
        Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => will_break(contents),
        Doc::IndentIfBreak { contents, .. } => will_break(contents),
        Doc::Group {
            contents,
            should_break,
            ..
        } => *should_break || will_break(contents),
        // IfBreak is conditional - it doesn't force break mode, just chooses between docs
        Doc::IfBreak { .. } => false,
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(will_break),
        Doc::WithContext { doc, .. } => will_break(doc),
        // IsolatedGroup explicitly prevents break propagation
        Doc::IsolatedGroup { .. } => false,
        // LineSuffix content doesn't affect breaking decisions
        Doc::LineSuffix(_) => false,
        Doc::LineSuffixBoundary => false,
        // BreakParent forces the group to break
        Doc::BreakParent => true,
    }
}

/// Check if a doc will break when the parent is in Break mode.
///
/// This is the stricter version of `will_break()`. It returns true for:
/// Check if a doc has forced breaks (hardlines only, no should_break groups)
///
/// This is the stricter version of `will_break()`. It only returns true for
/// unavoidable breaks like hardlines and BreakParent, NOT for groups with
/// `should_break: true`.
///
/// Use this for decisions where you need to distinguish between:
/// - Content that MUST break (line comments, multiline strings) → use this function
/// - Content that PREFERS to break (source newlines) → use `will_break()`
///
/// Example: Call argument formatting should keep `fn('x', {a: 1})` hugged if the
/// object just has source newlines, but expand if it has line comments.
pub fn has_forced_break(doc: &Doc) -> bool {
    match doc {
        Doc::Text(_) => false,
        Doc::Line(kind) => matches!(kind, LineKind::Hard | LineKind::Literal),
        Doc::Indent(inner) | Doc::Dedent(inner) => has_forced_break(inner),
        Doc::Align { contents, .. } | Doc::AlignSpaces { contents, .. } => {
            has_forced_break(contents)
        }
        Doc::IndentIfBreak { contents, .. } => has_forced_break(contents),
        // For groups, only recurse into contents - ignore should_break
        Doc::Group { contents, .. } => has_forced_break(contents),
        Doc::IfBreak { .. } => false,
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(has_forced_break),
        Doc::WithContext { doc, .. } => has_forced_break(doc),
        // IsolatedGroup contains forced breaks but doesn't propagate them
        Doc::IsolatedGroup { .. } => false,
        Doc::LineSuffix(_) => false,
        Doc::LineSuffixBoundary => false,
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
        Doc::IsolatedGroup { contents, .. } => can_break(contents),
        Doc::LineSuffix(inner) => can_break(inner),
        // Text and boundaries can't break
        Doc::Text(_) | Doc::LineSuffixBoundary => false,
        // BreakParent forces breaking
        Doc::BreakParent => true,
    }
}

//
// Join Helpers
//

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
pub fn join(docs: impl IntoIterator<Item = Doc>, separator: &'static str) -> Doc {
    let iter = docs.into_iter();
    let (lower, _) = iter.size_hint();
    let mut parts = Vec::with_capacity(lower.saturating_mul(2).saturating_sub(1));
    for (i, doc) in iter.enumerate() {
        if i > 0 {
            parts.push(text(separator));
        }
        parts.push(doc);
    }
    if parts.is_empty() {
        empty()
    } else {
        concat(parts)
    }
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
pub fn join_doc(docs: impl IntoIterator<Item = Doc>, separator: Doc) -> Doc {
    let iter = docs.into_iter();
    let (lower, _) = iter.size_hint();
    let mut parts = Vec::with_capacity(lower.saturating_mul(2).saturating_sub(1));
    for (i, doc) in iter.enumerate() {
        if i > 0 {
            parts.push(separator.clone());
        }
        parts.push(doc);
    }
    if parts.is_empty() {
        empty()
    } else {
        concat(parts)
    }
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
pub fn join_trailing(docs: impl IntoIterator<Item = Doc>, separator: Doc) -> Doc {
    let iter = docs.into_iter();
    let (lower, _) = iter.size_hint();
    let mut parts = Vec::with_capacity(lower.saturating_mul(2));
    for (i, doc) in iter.enumerate() {
        if i > 0 {
            parts.push(separator.clone());
        }
        parts.push(doc);
    }
    if parts.is_empty() {
        return empty();
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

//
// Wrap Helpers
//

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

/// Wrap a doc in parentheses with indent-on-break structure
///
/// In flat mode: `(inner)`
/// In break mode: `(\n  inner\n)`
///
/// Useful for parenthesized expressions in chains where breaking at the parens
/// is preferred over breaking inside the expression.
#[inline]
pub fn parens_break(inner: Doc) -> Doc {
    group(concat(vec![
        text("("),
        indent(concat(vec![softline(), inner])),
        softline(),
        text(")"),
    ]))
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

//
// Indent Helpers
//

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

//
// Separator Helpers
//

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

//
// Multi-Level Indent
//

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

//
// Line Flattening
//

/// Remove all line breaks from a doc, forcing it to stay on a single line.
///
/// This is Prettier's `removeLines()` function. It transforms a doc by replacing:
/// - `line` (normal) → `" "` (space)
/// - `softline` → `""` (empty)
/// - `hardline` → `""` (empty, rare but handled)
/// - `literalline` → `""` (empty)
/// - `group` → converted to regular concat (loses group/conditional_group semantics)
///
/// Used for block conditions in Svelte templates where Prettier forces the expression
/// to stay on a single line. When the total line exceeds print_width, EARLIER content
/// must break instead of the block condition.
///
/// # Example
/// ```ignore
/// // For: {#if x.y.z}body{/if}
/// // The condition x.y.z is built as a conditional_group that could break at `.`
/// // After remove_lines(), it becomes flat text that cannot break
/// let expr_doc = build_expression_doc(&block.test);
/// let flat_expr_doc = remove_lines(expr_doc);
/// ```
pub fn remove_lines(doc: Doc) -> Doc {
    match doc {
        Doc::Text(_) => doc, // Text stays as-is

        Doc::Line(kind) => match kind {
            LineKind::Normal => text(" "), // line() → space
            LineKind::Soft => empty(),     // softline() → empty
            LineKind::Hard => empty(),     // hardline() → empty (unusual)
            LineKind::Literal => empty(),  // literalline() → empty
        },

        Doc::Indent(inner) => Doc::Indent(Box::new(remove_lines(*inner))),
        Doc::Dedent(inner) => Doc::Dedent(Box::new(remove_lines(*inner))),

        Doc::Align { n, contents } => Doc::Align {
            n,
            contents: Box::new(remove_lines(*contents)),
        },

        Doc::AlignSpaces { spaces, contents } => Doc::AlignSpaces {
            spaces,
            contents: Box::new(remove_lines(*contents)),
        },

        // Groups: the handling depends on `should_break`.
        //
        // - Groups with `should_break=true`: These MUST break (e.g., 3+ method chains).
        //   Keep their expanded_states intact so they can still wrap properly.
        //
        // - Groups with `should_break=false`: These are optional breaks. Flatten everything
        //   including expanded_states to prevent them from being chosen for breaking when
        //   there's other content that should break first.
        //
        // This gives us the behavior:
        // 1. `{expr1}{#if short.chain}` - expr1 breaks, short.chain stays flat
        // 2. `{#if very.long.chain.that.exceeds.width}` - stays flat (exceeds but that's ok)
        // 3. `{#if a.b().c().d()}` (3+ chain) - wraps because should_break=true
        Doc::Group {
            contents,
            expanded_states,
            id,
            should_break,
        } => {
            if should_break {
                // Must-break groups: keep expanded_states for proper wrapping
                let flat_contents = remove_lines(*contents);
                Doc::Group {
                    contents: Box::new(flat_contents),
                    expanded_states, // Keep as-is
                    id,
                    should_break,
                }
            } else {
                // Optional-break groups: flatten everything to prevent unwanted breaking
                let flat_contents = remove_lines(*contents);
                let flat_states = expanded_states
                    .map(|states| Box::new(states.into_iter().map(remove_lines).collect()));
                Doc::Group {
                    contents: Box::new(flat_contents),
                    expanded_states: flat_states,
                    id,
                    should_break,
                }
            }
        }

        Doc::IsolatedGroup { contents } => {
            // Keep isolated group wrapper but flatten contents
            Doc::IsolatedGroup {
                contents: Box::new(remove_lines(*contents)),
            }
        }

        Doc::IfBreak { flat_doc, .. } => {
            // In flattened mode, always use the flat version
            remove_lines(*flat_doc)
        }

        Doc::IndentIfBreak { contents, .. } => {
            // In flattened mode, no conditional indent (use contents without indent)
            remove_lines(*contents)
        }

        Doc::Concat(docs) => {
            let flattened: Vec<Doc> = docs.into_iter().map(remove_lines).collect();
            concat(flattened)
        }

        Doc::Fill(parts) => {
            // Fill becomes regular concat when flattened
            let flattened: Vec<Doc> = parts.into_iter().map(remove_lines).collect();
            concat(flattened)
        }

        Doc::WithContext { doc, context } => Doc::WithContext {
            doc: Box::new(remove_lines(*doc)),
            context,
        },

        Doc::LineSuffix(inner) => Doc::LineSuffix(Box::new(remove_lines(*inner))),
        Doc::LineSuffixBoundary => doc,
        Doc::BreakParent => empty(), // BreakParent becomes nothing when flattened
    }
}
