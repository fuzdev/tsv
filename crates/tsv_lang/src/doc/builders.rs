//! Builder functions for constructing Doc trees

use super::types::{Doc, DocContext, DocText};

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

/// Create a soft line break (space if fits, newline if doesn't)
pub fn line() -> Doc {
    Doc::Line {
        hard: false,
        soft: false,
        literal: false,
    }
}

/// Create a soft line that disappears in flat mode (no space)
pub fn softline() -> Doc {
    Doc::Line {
        hard: false,
        soft: true,
        literal: false,
    }
}

/// Create a hard line break (always breaks, never becomes a space)
pub fn hardline() -> Doc {
    Doc::Line {
        hard: true,
        soft: false,
        literal: false,
    }
}

/// Create a literal line break (just newline, no indentation)
/// Used for blank line preservation where we want an empty line
pub fn literalline() -> Doc {
    Doc::Line {
        hard: true,
        soft: false,
        literal: true,
    }
}

/// Create a group (try to fit on one line, break all if doesn't fit)
pub fn group(doc: Doc) -> Doc {
    Doc::Group {
        contents: Box::new(doc),
        expanded_states: None,
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
pub fn conditional_group(states: Vec<Doc>) -> Doc {
    assert!(
        !states.is_empty(),
        "conditional_group requires at least one state"
    );
    Doc::Group {
        contents: Box::new(states[0].clone()),
        expanded_states: Some(states),
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

/// Conditional rendering based on parent group breaking
pub fn if_break(break_doc: Doc, flat_doc: Doc) -> Doc {
    Doc::IfBreak {
        break_doc: Box::new(break_doc),
        flat_doc: Box::new(flat_doc),
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
