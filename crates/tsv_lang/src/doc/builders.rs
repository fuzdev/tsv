//! Builder functions for constructing Doc trees

use super::types::{Doc, DocContext, DocText, GroupId, LineKind};

/// Create a text doc from a static string (zero allocation)
///
/// Use this for punctuation, keywords, and other compile-time known text.
/// For dynamic content (identifiers, formatted values), use `text_owned()`.
#[inline]
pub fn text(s: &'static str) -> Doc {
    Doc::Text(DocText::Static(s))
}

/// Create a text doc from an owned string
///
/// Use this for dynamic content like formatted values or transformed content.
/// For static strings, prefer `text()` which avoids allocation.
/// For interned identifiers, prefer `symbol()` which defers resolution.
#[inline]
pub fn text_owned(s: String) -> Doc {
    Doc::Text(DocText::Owned(s))
}

/// Create an empty doc that produces no output
///
/// Use this when a function must return a Doc but has nothing to emit.
/// Zero allocation - uses a static empty string.
#[inline]
pub fn empty() -> Doc {
    Doc::Text(DocText::Static(""))
}

/// Create a text doc from a symbol ID (deferred resolution)
///
/// Use this for interned identifiers and other strings that come from
/// a string interner. The symbol is resolved to text at print time,
/// avoiding string allocation during doc building.
///
/// The `id` should be the raw u32 value from a `DefaultSymbol`.
#[inline]
pub fn symbol(id: u32) -> Doc {
    Doc::Text(DocText::Symbol(id))
}

/// Create a normal line break (space if fits, newline if doesn't)
#[inline]
pub fn line() -> Doc {
    Doc::Line(LineKind::Normal)
}

/// Create a soft line that disappears in flat mode (no space)
#[inline]
pub fn softline() -> Doc {
    Doc::Line(LineKind::Soft)
}

/// Create a hard line break (always breaks, never becomes a space)
#[inline]
pub fn hardline() -> Doc {
    Doc::Line(LineKind::Hard)
}

/// Create a literal line break (just newline, no indentation)
/// Used for blank line preservation where we want an empty line
#[inline]
pub fn literalline() -> Doc {
    Doc::Line(LineKind::Literal)
}

/// Create a group (try to fit on one line, break all if doesn't fit)
pub fn group(doc: Doc) -> Doc {
    Doc::Group {
        contents: Box::new(doc),
        expanded_states: None,
        id: None,
    }
}

/// Create a group with an ID for tracking whether it broke
///
/// The ID allows `indent_if_break()` to check if this specific group broke,
/// enabling deferred indentation decisions. Matches Prettier's `group(doc, { id })`.
///
/// Example (Fluid assignment):
/// ```ignore
/// group_with_id(
///     concat(vec![left, text(" ="), indent(line())]),
///     GroupId::Assignment
/// )
/// ```
pub fn group_with_id(doc: Doc, id: GroupId) -> Doc {
    Doc::Group {
        contents: Box::new(doc),
        expanded_states: None,
        id: Some(id),
    }
}

/// Create a conditional group that tries multiple alternative layouts
///
/// Based on prettier's `conditionalGroup`:
/// - states[0] is tried first in flat mode
/// - If it doesn't fit, states[1], states[2], ... are tried in flat mode
/// - If none fit, the last state is used in break mode
///
/// Example: For member chains, you might use:
/// ```ignore
/// conditional_group(vec![
///     // State 0: everything on one line
///     concat(vec![base, text(".method1"), text(".method2")]),
///     // State 1: chain breaks, inner expressions stay flat
///     concat(vec![base, indent(concat(vec![hardline(), text(".method1"), hardline(), text(".method2")]))]),
/// ])
/// ```
pub fn conditional_group(mut states: Vec<Doc>) -> Doc {
    assert!(
        !states.is_empty(),
        "conditional_group requires at least one state"
    );
    // Move first state to contents, keep rest in expanded_states
    // This avoids cloning states[0] - it now lives only in contents
    let first = states.remove(0);
    Doc::Group {
        contents: Box::new(first),
        expanded_states: Some(Box::new(states)), // Now contains states[1..]
        id: None,
    }
}

/// Increase indentation for nested doc
pub fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

/// Decrease indentation for doc
pub fn dedent(doc: Doc) -> Doc {
    Doc::Dedent(Box::new(doc))
}

/// Set absolute indentation level for doc
///
/// Unlike `indent`/`dedent` which are relative, `align` sets the indent to an
/// absolute value. Used for template literal interpolations where content must
/// be indented to match the template's visual position regardless of nesting.
pub fn align(n: usize, doc: Doc) -> Doc {
    Doc::Align {
        n,
        contents: Box::new(doc),
    }
}

/// Conditional rendering based on parent group breaking
pub fn if_break(break_doc: Doc, flat_doc: Doc) -> Doc {
    Doc::IfBreak {
        break_doc: Box::new(break_doc),
        flat_doc: Box::new(flat_doc),
    }
}

/// Conditionally indent based on whether a specific group broke
///
/// Optimized version of `if_break(indent(doc), doc)` that checks a specific
/// group ID instead of the immediate parent. Matches Prettier's `indentIfBreak`.
///
/// - If group[id] broke: applies indent to contents
/// - If group[id] stayed flat: renders contents without indent
/// - If negate=true: reverses the logic
///
/// This enables deferred indentation - the decision is made after we know
/// whether the referenced group broke during rendering.
///
/// Example (Fluid assignment):
/// ```ignore
/// group(concat(vec![
///     group(left),
///     text(" ="),
///     group_with_id(indent(line()), GroupId::Assignment),  // Marker group
///     line_suffix_boundary(),
///     indent_if_break(right, GroupId::Assignment, false),  // Checks marker
/// ]))
/// ```
pub fn indent_if_break(doc: Doc, group_id: GroupId, negate: bool) -> Doc {
    Doc::IndentIfBreak {
        contents: Box::new(doc),
        group_id,
        negate,
    }
}

/// Concatenate multiple docs into a sequence
pub fn concat(docs: Vec<Doc>) -> Doc {
    Doc::Concat(docs)
}

/// Create a fill doc for greedy line packing
///
/// Fill packs as many items as possible on each line before breaking.
/// Parts should alternate between content and separators:
///
/// ```ignore
/// fill(vec![
///     text("item1"), line(),  // content, separator
///     text("item2"), line(),  // content, separator
///     text("item3"),          // final content (no trailing separator)
/// ])
/// ```
///
/// Separators should be Line variants (`line()`, `softline()`).
pub fn fill(parts: Vec<Doc>) -> Doc {
    Doc::Fill(parts)
}

/// Wrap a doc with rendering context
///
/// Provides hints about trailing punctuation and width constraints
/// to help fills make better packing decisions.
///
/// Example:
/// ```ignore
/// // Reserve 1 char for CSS semicolon
/// let context = DocContext { trailing_reserve: 1 };
/// with_context(fill(parts), context)
/// ```
pub fn with_context(doc: Doc, context: DocContext) -> Doc {
    Doc::WithContext {
        doc: Box::new(doc),
        context,
    }
}

/// Wrap a doc with a base indent override
///
/// This overrides `config.base_indent_offset` for position calculations within
/// this doc subtree. Used for template expression content where the wrapper
/// (e.g., Svelte's script post-processor) won't add its usual indentation.
///
/// Example:
/// ```ignore
/// // Template expression content - Svelte won't add +1 indent
/// with_base_indent_override(expression_doc, 0)
/// ```
pub fn with_base_indent_override(doc: Doc, base_offset: usize) -> Doc {
    Doc::WithContext {
        doc: Box::new(doc),
        context: DocContext {
            trailing_reserve: 0,
            base_indent_override: Some(base_offset),
        },
    }
}

/// Content to print at the end of the current line
///
/// LineSuffix is NOT included in width calculations during `fits()`,
/// allowing lines to exceed print width when they have trailing comments.
///
/// Example:
/// ```ignore
/// // Trailing comment doesn't affect line break decisions
/// concat(vec![
///     text("value"),
///     line_suffix(concat(vec![text(" "), text("// comment")])),
/// ])
/// ```
pub fn line_suffix(doc: Doc) -> Doc {
    Doc::LineSuffix(Box::new(doc))
}

/// Force pending LineSuffix content to be flushed
///
/// Prevents LineSuffix from bleeding across group boundaries.
pub fn line_suffix_boundary() -> Doc {
    Doc::LineSuffixBoundary
}

/// Force parent group to break
///
/// When encountered during printing, marks the enclosing group as broken.
/// This is useful when a child element should force multiline layout.
///
/// # Example
/// ```ignore
/// // Force the for loop header to break when there's a trailing comment
/// group(concat([
///     text("for ("),
///     indent(concat([softline(), content])),
///     softline(),
///     text(")"),
///     break_parent(),  // Forces the group to break mode
/// ]))
/// ```
pub fn break_parent() -> Doc {
    Doc::BreakParent
}
