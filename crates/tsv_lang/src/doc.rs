//! Document builder primitives for prettier-compatible formatting
//!
//! This module implements a declarative document builder architecture inspired by
//! prettier's doc builder (see prettier/doc.js). Instead of imperatively deciding
//! when to break lines, formatters describe the document structure using primitives
//! like `group()`, `line`, and `indent()`, and let the rendering algorithm decide
//! how to lay out the content based on the print width.
//!
//! ## Core Concepts
//!
//! - **Doc**: An abstract document tree describing how content should be formatted
//! - **Mode**: Flat (try to fit on one line) vs Break (use line breaks)
//! - **fits()**: Algorithm to check if a doc fits in remaining width
//! - **print_doc()**: Convert a Doc tree to a final formatted string
//!
//! ## Architecture Note: Command Stack with Look-Ahead
//!
//! Like prettier's printer, this implementation uses a command stack approach.
//! When checking if a group fits (`fits()`), we pass the remaining command stack
//! so the algorithm can look ahead at what comes after the current group.
//!
//! This is critical for correct breaking decisions. For example:
//! ```text
//! (veryLongExpr || anotherLongExpr)!.method()
//! ```
//!
//! Without look-ahead, `fits()` would check if `(veryLongExpr || anotherLongExpr)`
//! fits and say "yes" (90 chars fits in 91 remaining). But the actual line includes
//! `!.method()` which pushes it over the limit.
//!
//! With look-ahead, `fits()` checks the group + everything after it, correctly
//! deciding to break.
//!
//! ## Example
//!
//! ```rust
//! use tsv_lang::doc::*;
//! use tsv_lang::PrintConfig;
//!
//! // Build a doc for an element with attributes
//! let doc = concat(vec![
//!     text("<"),
//!     text("Component"),
//!     indent(group(concat(vec![
//!         line(),
//!         text("prop1=\"value1\""),
//!         line(),
//!         text("prop2=\"value2\""),
//!         dedent(line()),
//!     ]))),
//!     text("/>"),
//! ]);
//!
//! // Render to string
//! let config = PrintConfig::default();
//! let output = print_doc(&doc, &config);
//! ```
//!
//! If the content fits within `print_width`, it renders on one line:
//! ```html
//! <Component prop1="value1" prop2="value2" />
//! ```
//!
//! If it doesn't fit, it breaks:
//! ```html
//! <Component
//!     prop1="value1"
//!     prop2="value2"
//! />
//! ```

use crate::PrintConfig;
use smallvec::SmallVec;

/// Context for doc rendering - provides hints about trailing punctuation
/// and width constraints that affect how fills pack content.
///
/// This allows fills to make better packing decisions by knowing about
/// punctuation that will be added by the parent (e.g., semicolons in CSS,
/// commas in object properties).
#[derive(Debug, Clone, Default)]
pub struct DocContext {
    /// Reserve N chars when checking if content fits.
    ///
    /// This prevents greedy fills from packing to exactly printWidth,
    /// which would be exceeded when the parent adds trailing punctuation.
    ///
    /// Example: CSS declarations add ";" after the value, so reserve 1 char.
    pub trailing_reserve: usize,
}

/// Trait for resolving symbol IDs to strings at print time
///
/// This enables deferred symbol resolution - Docs can store symbol IDs
/// instead of allocated strings, and resolution happens during printing.
/// This eliminates allocations for identifier text in the doc tree.
///
/// The resolver is language-agnostic (uses raw u32 IDs) so tsv_lang
/// doesn't need to depend on string_interner.
pub trait TextResolver {
    /// Resolve a symbol ID to its string representation
    ///
    /// # Panics
    /// May panic if the ID is invalid (not from this resolver's interner)
    fn resolve(&self, id: u32) -> &str;
}

/// Text content in a Doc - static, owned, or a symbol to resolve at print time
#[derive(Debug, Clone)]
pub enum DocText {
    /// Static string literal - no allocation, just stores pointer
    /// Used for punctuation, keywords, and other compile-time known text
    Static(&'static str),
    /// Dynamically generated text - requires allocation
    /// Used for formatted values, transformed content
    Owned(String),
    /// Symbol ID to be resolved at print time - no allocation during doc building
    /// Used for identifiers and other interned strings
    Symbol(u32),
}

impl DocText {
    /// Try to get the string content directly.
    ///
    /// Returns `Some(&str)` for Static and Owned variants.
    /// Returns `None` for Symbol variant (use `resolve()` with a TextResolver instead).
    #[inline]
    pub fn try_as_str(&self) -> Option<&str> {
        match self {
            DocText::Static(s) => Some(s),
            DocText::Owned(s) => Some(s),
            DocText::Symbol(_) => None,
        }
    }

    /// Resolve text content using the provided resolver
    ///
    /// For Static and Owned, returns the string directly.
    /// For Symbol, uses the resolver to look up the interned string.
    #[inline]
    pub fn resolve<'a, R: TextResolver + ?Sized>(&'a self, resolver: &'a R) -> &'a str {
        match self {
            DocText::Static(s) => s,
            DocText::Owned(s) => s,
            DocText::Symbol(id) => resolver.resolve(*id),
        }
    }

    /// Check if this is a Symbol variant (needs resolver)
    #[inline]
    pub const fn is_symbol(&self) -> bool {
        matches!(self, DocText::Symbol(_))
    }
}

/// Resolve DocText to a string, using resolver if provided
///
/// For Static and Owned text, returns directly.
/// For Symbol text, uses the resolver (panics if resolver is None).
///
/// # Panics
///
/// Panics if a Symbol is encountered but no resolver was provided.
/// This indicates a bug - docs containing symbols must use resolved print functions.
#[inline]
#[allow(clippy::expect_used)] // Intentional: Symbol without resolver is a programming error
fn resolve_text<'a, R: TextResolver + ?Sized>(
    text: &'a DocText,
    resolver: Option<&'a R>,
) -> &'a str {
    match text {
        DocText::Static(s) => s,
        DocText::Owned(s) => s,
        DocText::Symbol(id) => resolver
            .expect("Symbol encountered in Doc but no TextResolver provided")
            .resolve(*id),
    }
}

/// Document primitive - abstract representation of formatted output
#[derive(Debug, Clone)]
pub enum Doc {
    /// Text content to output (static or owned)
    Text(DocText),

    /// Line break - behavior depends on mode:
    /// - In Flat mode: becomes a space (unless `soft = true`)
    /// - In Break mode: becomes newline + indentation
    /// - `hard = true`: unconditional line break (always breaks)
    /// - `soft = true`: disappears in flat mode (no space)
    /// - `literal = true`: just newline, NO indentation (for blank lines)
    Line {
        hard: bool,
        soft: bool,
        literal: bool,
    },

    /// Increase indentation level for nested content
    Indent(Box<Doc>),

    /// Decrease indentation level
    Dedent(Box<Doc>),

    /// Try to fit content on one line; if doesn't fit, break ALL lines in group
    /// This is the key primitive for prettier's "all-or-nothing" breaking
    ///
    /// When `expanded_states` is Some, this is a "conditional group" that tries
    /// multiple alternative layouts (like prettier's `conditionalGroup`):
    /// 1. First tries states[0] in flat mode
    /// 2. If that doesn't fit, tries states[1], states[2], ... in flat mode
    /// 3. If none fit, uses the last state in break mode
    Group {
        contents: Box<Doc>,
        /// Alternative layouts to try before breaking (prettier's expandedStates)
        expanded_states: Option<Vec<Doc>>,
    },

    /// Conditional rendering based on whether parent group breaks
    /// - If parent breaks: render `break_doc`
    /// - If parent fits: render `flat_doc`
    IfBreak {
        break_doc: Box<Doc>,
        flat_doc: Box<Doc>,
    },

    /// Sequence of docs - rendered one after another
    Concat(Vec<Doc>),

    /// Greedy line packing - fills each line with as much as fits
    ///
    /// Unlike Group (all-or-nothing breaking), Fill packs items left-to-right,
    /// breaking to a new line only when the next item wouldn't fit.
    ///
    /// Parts should alternate: [content, separator, content, separator, ...]
    /// Separators are typically `line()` or `softline()`.
    ///
    /// Example: `fill(vec![text("a"), line(), text("b"), line(), text("c")])`
    /// At width 5: "a b c" (all fit)
    /// At width 3: "a b\nc" (c doesn't fit with b)
    Fill(Vec<Doc>),

    /// Wrap a doc with rendering context (hints for width and punctuation).
    ///
    /// This is primarily used with Fill docs to prevent greedy-fill bugs
    /// where fills pack to exactly printWidth, then parent adds punctuation
    /// causing overflow.
    ///
    /// Example:
    /// ```ignore
    /// // CSS: reserve 1 char for trailing semicolon
    /// let context = DocContext { trailing_reserve: 1 };
    /// with_context(fill(values), context)
    /// ```
    WithContext { doc: Box<Doc>, context: DocContext },
}

/// Rendering mode for a doc
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Try to fit on one line (soft lines become spaces)
    Flat,
    /// Use line breaks (soft lines become newlines)
    Break,
}

// =============================================================================
// Builder Helpers
// =============================================================================

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
        Doc::Line { hard, literal, .. } => *hard || *literal,
        Doc::Indent(inner) | Doc::Dedent(inner) => will_break(inner),
        Doc::Group { contents, .. } => will_break(contents),
        Doc::IfBreak { break_doc, .. } => will_break(break_doc),
        Doc::Concat(docs) | Doc::Fill(docs) => docs.iter().any(will_break),
        Doc::WithContext { doc, .. } => will_break(doc),
    }
}

// =============================================================================
// Rendering Algorithm
// =============================================================================

/// A command in the printer's command stack.
/// Holds the context (indent, mode) and reference to a doc to process.
#[derive(Debug, Clone, Copy)]
struct Command<'a> {
    indent: usize,
    mode: Mode,
    doc: &'a Doc,
}

/// Check if a doc fits in the remaining width, looking ahead at remaining commands.
///
/// This simulates rendering the doc without actually building the string,
/// tracking the remaining width character-by-character. Returns `true` if
/// the doc fits, `false` otherwise.
///
/// The key innovation from prettier: when the current doc is exhausted,
/// continue checking from `rest_commands`. This enables look-ahead:
/// a group can see what comes after it and make correct breaking decisions.
///
/// Based on prettier's `fits()` algorithm (prettier/src/document/printer/printer.js:52-168).
fn fits_with_lookahead<'a, R: TextResolver + ?Sized>(
    doc: &'a Doc,
    mode: Mode,
    rest_commands: &[Command<'a>],
    remaining_width: isize,
    _config: &PrintConfig,
    resolver: Option<&R>,
) -> bool {
    if remaining_width == isize::MAX {
        return true; // Infinite width always fits
    }

    let mut remaining = remaining_width;

    // Local stack for processing the doc itself
    let mut stack: SmallVec<[(&Doc, Mode); 16]> = SmallVec::new();
    stack.push((doc, mode));

    // Index into rest_commands (we traverse from end to start, like prettier)
    let mut rest_idx = rest_commands.len();

    while remaining >= 0 {
        // Pop from local stack, or pull from rest_commands if empty
        let Some((current_doc, current_mode)) = stack.pop() else {
            if rest_idx == 0 {
                // eprintln!("    fits_with_lookahead: initial_remaining={}, final_remaining={}, result=true",
                //           initial_remaining, remaining);
                return true; // Everything checked, it fits
            }
            rest_idx -= 1;
            let cmd = &rest_commands[rest_idx];
            stack.push((cmd.doc, cmd.mode));
            continue;
        };

        match current_doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                remaining -= string_width(s) as isize;
                // Don't return early on negative - let the while condition catch it
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    // Line break found - rest fits on next line
                    return true;
                }
                // In flat mode: soft line disappears, regular line becomes space
                if !soft {
                    remaining -= 1;
                }
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // Match prettier: when in break mode with expanded states,
                // use the most expanded state (takes least space on current line)
                // See prettier printer.js lines 126-130
                let doc_to_check = if current_mode == Mode::Break {
                    if let Some(states) = expanded_states {
                        states.last().unwrap_or(contents)
                    } else {
                        contents
                    }
                } else {
                    contents
                };
                stack.push((doc_to_check, current_mode));
            }

            Doc::Indent(inner) | Doc::Dedent(inner) => {
                // Indent/dedent don't affect width in fits() check
                // (indentation only matters at line breaks, which end fits() early)
                stack.push((inner, current_mode));
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if current_mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                stack.push((chosen, current_mode));
            }

            Doc::Concat(docs) => {
                // Process docs in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::Fill(parts) => {
                // For fits() check, Fill behaves like Concat - just check all parts fit
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode));
                }
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply trailing_reserve for any doc type.
                // This ensures both Fill and non-Fill docs (like Concat in conditional_group)
                // respect the trailing reserve constraint.
                remaining -= context.trailing_reserve as isize;
                stack.push((doc.as_ref(), current_mode));
            }
        }
    }

    false // remaining < 0, doesn't fit
}

/// Check if a doc fits in the remaining width (legacy API without look-ahead)
///
/// Note: For most formatting scenarios, the command-stack-based renderer
/// uses `fits_with_lookahead` directly. This function is kept for the
/// public API and Fill algorithm.
pub fn fits(doc: &Doc, width: usize, mode: Mode, config: &PrintConfig) -> bool {
    fits_with_lookahead::<dyn TextResolver>(doc, mode, &[], width as isize, config, None)
}

/// Check if a doc fits in the remaining width with symbol resolution
pub fn fits_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    width: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: &R,
) -> bool {
    fits_with_lookahead(doc, mode, &[], width as isize, config, Some(resolver))
}

/// Check if multiple docs fit sequentially in the remaining width
///
/// This is an optimization to avoid cloning docs just to check combined width.
/// Used by render_fill() to check if content + separator + next_content all fit.
fn fits_multi<R: TextResolver + ?Sized>(
    docs: &[&Doc],
    width: usize,
    mode: Mode,
    _config: &PrintConfig,
    resolver: Option<&R>,
) -> bool {
    if width == usize::MAX {
        return true;
    }

    let mut stack: SmallVec<[(&Doc, Mode, isize); 16]> = SmallVec::new();
    let mut remaining_width = width as isize;

    // Push docs in reverse order (will be processed first-to-last)
    for doc in docs.iter().rev() {
        stack.push((doc, mode, 0));
    }

    while let Some((current_doc, current_mode, indent_delta)) = stack.pop() {
        match current_doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                remaining_width -= string_width(s) as isize;
                if remaining_width < 0 {
                    return false;
                }
            }

            Doc::Line { hard, soft, .. } => {
                if current_mode == Mode::Break || *hard {
                    return true;
                }
                if !*soft {
                    remaining_width -= 1;
                    if remaining_width < 0 {
                        return false;
                    }
                }
            }

            Doc::Group { contents, .. } => {
                stack.push((contents, current_mode, indent_delta));
            }

            Doc::Indent(inner) => {
                stack.push((inner, current_mode, indent_delta + 1));
            }

            Doc::Dedent(inner) => {
                stack.push((inner, current_mode, indent_delta - 1));
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if current_mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                stack.push((chosen, current_mode, indent_delta));
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }

            Doc::Fill(parts) => {
                for doc in parts.iter().rev() {
                    stack.push((doc, current_mode, indent_delta));
                }
            }

            Doc::WithContext { doc, context } => {
                // Apply trailing_reserve for any doc type.
                remaining_width -= context.trailing_reserve as isize;
                if remaining_width < 0 {
                    return false;
                }
                stack.push((doc.as_ref(), current_mode, indent_delta));
            }
        }
    }

    // Match prettier's fits() semantics: `while (remainingWidth >= 0)`
    // Content that uses exactly remaining width does fit
    remaining_width >= 0
}

// =============================================================================
// Width Calculation Helpers
// =============================================================================

/// Calculate available width for fitting check
///
/// This centralizes the width calculation logic used across TypeScript, CSS, and Svelte
/// formatters. It accounts for indentation and any trailing characters that will follow
/// the content being checked.
///
/// # Arguments
/// * `config` - Print configuration (contains print_width and tab_width)
/// * `indent_level` - Current indentation level
/// * `current_column` - Position on current line (0 if start of line)
/// * `trailing_chars` - Space to reserve for trailing punctuation (e.g., 1 for ";")
///
/// # Example
/// ```ignore
/// // Check if object fits after "const x = " on a line
/// let available = available_width(&config, 0, 10, 1); // 10 chars used, reserve 1 for ";"
/// ```
pub fn available_width(
    config: &PrintConfig,
    indent_level: usize,
    current_column: usize,
    trailing_chars: usize,
) -> usize {
    let indent_width = indent_level * config.tab_width;
    let used = indent_width.max(current_column) + trailing_chars;
    config.print_width.saturating_sub(used)
}

/// Check if a doc fits given the context
///
/// Convenience wrapper around `fits()` that handles width calculation.
///
/// # Arguments
/// * `doc` - Document to check
/// * `config` - Print configuration
/// * `indent_level` - Current indentation level
/// * `current_column` - Position on current line (0 if start of line)
/// * `trailing_chars` - Space to reserve for trailing punctuation
pub fn fits_at(
    doc: &Doc,
    config: &PrintConfig,
    indent_level: usize,
    current_column: usize,
    trailing_chars: usize,
) -> bool {
    let available = available_width(config, indent_level, current_column, trailing_chars);
    fits(doc, available, Mode::Flat, config)
}

// =============================================================================
// Doc Building Helpers
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
        return concat(vec![]);
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
        return concat(vec![]);
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
        return concat(vec![]);
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

/// Convert a Doc tree to a formatted string (starting at column 0)
///
/// Note: This function does not support Symbol text. Use `print_doc_resolved` for docs with symbols.
pub fn print_doc(doc: &Doc, config: &PrintConfig) -> String {
    print_doc_at_column(doc, config, 0)
}

/// Convert a Doc tree to a formatted string with symbol resolution
pub fn print_doc_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    resolver: &R,
) -> String {
    print_doc_with_indent_resolved(doc, config, 0, 0, resolver)
}

/// Convert a Doc tree to a formatted string, starting at a specific column
///
/// Use this when the doc is being inserted into a line that already has content.
/// The `start_column` affects the width calculation for breaking decisions.
///
/// Note: This function does not support Symbol text. Use `print_doc_at_column_resolved` for docs with symbols.
pub fn print_doc_at_column(doc: &Doc, config: &PrintConfig, start_column: usize) -> String {
    print_doc_with_indent(doc, config, start_column, 0)
}

/// Convert a Doc tree to a formatted string, starting at a specific column, with symbol resolution
pub fn print_doc_at_column_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    resolver: &R,
) -> String {
    print_doc_with_indent_resolved(doc, config, start_column, 0, resolver)
}

/// Convert a Doc tree to a formatted string with both column and indent level specified
///
/// Use this when the doc is being inserted into content that already has both
/// column position and indentation context (e.g., Svelte template expressions).
///
/// Note: This function does not support Symbol text. Use `print_doc_with_indent_resolved` for docs with symbols.
pub fn print_doc_with_indent(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
) -> String {
    let mut output = String::new();
    let mut pos: usize = start_column;

    // Use command-stack-based rendering with look-ahead (no resolver)
    render_doc_iterative::<dyn TextResolver>(
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        None,
    );

    output
}

/// Convert a Doc tree to a formatted string with column, indent level, and symbol resolution
pub fn print_doc_with_indent_resolved<R: TextResolver + ?Sized>(
    doc: &Doc,
    config: &PrintConfig,
    start_column: usize,
    start_indent_level: usize,
    resolver: &R,
) -> String {
    let mut output = String::new();
    let mut pos: usize = start_column;

    // Use command-stack-based rendering with look-ahead
    render_doc_iterative(
        doc,
        &mut output,
        &mut pos,
        start_indent_level,
        config,
        Some(resolver),
    );

    output
}

/// Command-stack-based rendering implementation with look-ahead.
///
/// This is the core of the prettier-compatible printer. Instead of recursive
/// calls, we use a command stack. When checking if a group fits, we pass
/// the remaining commands so `fits()` can look ahead at what comes after.
fn render_doc_iterative<R: TextResolver + ?Sized>(
    doc: &Doc,
    output: &mut String,
    pos: &mut usize,
    start_indent_level: usize,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    // Command stack - process from end (LIFO)
    let mut commands: Vec<Command> = vec![Command {
        indent: start_indent_level,
        mode: Mode::Break,
        doc,
    }];

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                output.push_str(s);
                *pos += string_width(s);
            }

            Doc::Line {
                hard,
                soft,
                literal,
            } => {
                if cmd.mode == Mode::Break || *hard {
                    // Break: emit newline
                    output.push('\n');
                    if *literal {
                        // Literal line: just newline, no indentation (for blank lines)
                        *pos = 0;
                    } else {
                        // Normal line: newline + indentation
                        write_indentation(output, cmd.indent, config);
                        // Account for base_indent_offset (e.g., Svelte wrapper indentation)
                        let base_indent = config.base_indent_offset * config.tab_width;
                        *pos = indent_width(cmd.indent, config) + base_indent;
                    }
                } else {
                    // Flat mode: soft line disappears, regular line becomes space
                    if !*soft {
                        output.push(' ');
                        *pos += 1;
                    }
                }
            }

            Doc::Indent(inner) => {
                commands.push(Command {
                    indent: cmd.indent + 1,
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Dedent(inner) => {
                commands.push(Command {
                    indent: cmd.indent.saturating_sub(1),
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // If contents will definitely break (contains hardline), use break mode
                if will_break(contents) {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: Mode::Break,
                        doc: contents,
                    });
                } else if let Some(states) = expanded_states {
                    // conditionalGroup: try each state until one fits
                    // This is prettier's expandedStates algorithm (printer.js:288-333)
                    let remaining_width = config.print_width.saturating_sub(*pos) as isize;

                    // Try each state in flat mode until one fits
                    let mut found = false;
                    for (i, state) in states.iter().enumerate() {
                        if i == states.len() - 1 {
                            // Last state: use in break mode
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Break,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                        let state_fits = fits_with_lookahead(
                            state,
                            Mode::Flat,
                            &commands,
                            remaining_width,
                            config,
                            resolver,
                        );
                        if state_fits {
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Flat,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                    }

                    // Shouldn't happen if states is non-empty, but handle gracefully
                    if !found {
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: Mode::Break,
                            doc: contents,
                        });
                    }
                } else {
                    // Regular group: check if content fits - WITH LOOK-AHEAD
                    let remaining_width = config.print_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining_width,
                        config,
                        resolver,
                    ) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: chosen_mode,
                        doc: contents,
                    });
                }
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                commands.push(Command {
                    indent: cmd.indent,
                    mode: cmd.mode,
                    doc: chosen,
                });
            }

            Doc::Concat(docs) => {
                // Push in reverse order (stack is LIFO)
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: cmd.mode,
                        doc,
                    });
                }
            }

            Doc::Fill(parts) => {
                // Fill needs special handling for greedy packing
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                match doc.as_ref() {
                    Doc::Fill(parts) => {
                        // Apply context when rendering fill
                        render_fill_iterative(
                            parts, output, pos, cmd.indent, config, context, resolver,
                        );
                    }
                    _ => {
                        // For non-fill docs, just unwrap and continue
                        // (context only affects Fill rendering currently)
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: cmd.mode,
                            doc: doc.as_ref(),
                        });
                    }
                }
            }
        }
    }
}

/// Render a fill doc using greedy line packing (iterative version)
fn render_fill_iterative<R: TextResolver + ?Sized>(
    parts: &[Doc],
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    config: &PrintConfig,
    context: &DocContext,
    resolver: Option<&R>,
) {
    let mut offset = 0;

    while offset < parts.len() {
        let remaining = config.print_width.saturating_sub(*pos);
        let content = &parts[offset];

        // Check if current content fits in flat mode
        // Apply trailing_reserve to prevent packing to exactly printWidth
        let available = remaining.saturating_sub(context.trailing_reserve);
        let content_fits = fits_with_lookahead(
            content,
            Mode::Flat,
            &[],
            available as isize,
            config,
            resolver,
        );

        // Case 1: Last item - render it (break to new line if it doesn't fit)
        if offset + 1 >= parts.len() {
            if content_fits {
                render_single_doc(
                    content,
                    output,
                    pos,
                    indent_level,
                    Mode::Flat,
                    config,
                    resolver,
                );
            } else {
                // Doesn't fit - break to new line first
                output.push('\n');
                write_indentation(output, indent_level, config);
                let base_indent = config.base_indent_offset * config.tab_width;
                *pos = indent_width(indent_level, config) + base_indent;
                render_single_doc(
                    content,
                    output,
                    pos,
                    indent_level,
                    Mode::Flat,
                    config,
                    resolver,
                );
            }
            break;
        }

        let separator = &parts[offset + 1];

        // Case 2: Only content + separator left (no next content)
        if offset + 2 >= parts.len() {
            render_single_doc(
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
            );
            let sep_mode = if content_fits {
                Mode::Flat
            } else {
                Mode::Break
            };
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                sep_mode,
                config,
                resolver,
            );
            break;
        }

        // Case 3: Full three-way decision
        let next_content = &parts[offset + 2];
        // Check if content + separator + next_content all fit
        // Use the same available width as content_fits check
        let both_fit = fits_multi(
            &[content, separator, next_content],
            available,
            Mode::Flat,
            config,
            resolver,
        );

        if both_fit {
            // Both fit: render content flat, separator flat
            render_single_doc(
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
            );
        } else if content_fits {
            // First fits, next doesn't: render content flat, break after
            render_single_doc(
                content,
                output,
                pos,
                indent_level,
                Mode::Flat,
                config,
                resolver,
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
        } else {
            // Neither fits: render content break, separator break
            render_single_doc(
                content,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
            render_single_doc(
                separator,
                output,
                pos,
                indent_level,
                Mode::Break,
                config,
                resolver,
            );
        }

        offset += 2;
    }
}

/// Render a single doc with specified mode (helper for Fill)
fn render_single_doc<R: TextResolver + ?Sized>(
    doc: &Doc,
    output: &mut String,
    pos: &mut usize,
    indent_level: usize,
    mode: Mode,
    config: &PrintConfig,
    resolver: Option<&R>,
) {
    // Use a small command stack for this single doc
    let mut commands: Vec<Command> = vec![Command {
        indent: indent_level,
        mode,
        doc,
    }];

    while let Some(cmd) = commands.pop() {
        match cmd.doc {
            Doc::Text(t) => {
                let s = resolve_text(t, resolver);
                output.push_str(s);
                *pos += string_width(s);
            }

            Doc::Line {
                hard,
                soft,
                literal,
            } => {
                if cmd.mode == Mode::Break || *hard {
                    output.push('\n');
                    if *literal {
                        *pos = 0;
                    } else {
                        write_indentation(output, cmd.indent, config);
                        let base_indent = config.base_indent_offset * config.tab_width;
                        *pos = indent_width(cmd.indent, config) + base_indent;
                    }
                } else if !*soft {
                    output.push(' ');
                    *pos += 1;
                }
            }

            Doc::Indent(inner) => {
                commands.push(Command {
                    indent: cmd.indent + 1,
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Dedent(inner) => {
                commands.push(Command {
                    indent: cmd.indent.saturating_sub(1),
                    mode: cmd.mode,
                    doc: inner,
                });
            }

            Doc::Group {
                contents,
                expanded_states,
            } => {
                // Within Fill, groups still make their own decisions
                // but we pass the outer mode context
                if will_break(contents) {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: Mode::Break,
                        doc: contents,
                    });
                } else if let Some(states) = expanded_states {
                    // conditionalGroup within Fill - try each state
                    let remaining = config.print_width.saturating_sub(*pos) as isize;
                    let mut found = false;
                    for (i, state) in states.iter().enumerate() {
                        if i == states.len() - 1 {
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Break,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                        if fits_with_lookahead(
                            state,
                            Mode::Flat,
                            &commands,
                            remaining,
                            config,
                            resolver,
                        ) {
                            commands.push(Command {
                                indent: cmd.indent,
                                mode: Mode::Flat,
                                doc: state,
                            });
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: Mode::Break,
                            doc: contents,
                        });
                    }
                } else {
                    let remaining = config.print_width.saturating_sub(*pos) as isize;
                    let chosen_mode = if fits_with_lookahead(
                        contents,
                        Mode::Flat,
                        &commands,
                        remaining,
                        config,
                        resolver,
                    ) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: chosen_mode,
                        doc: contents,
                    });
                }
            }

            Doc::IfBreak {
                break_doc,
                flat_doc,
            } => {
                let chosen = if cmd.mode == Mode::Break {
                    break_doc
                } else {
                    flat_doc
                };
                commands.push(Command {
                    indent: cmd.indent,
                    mode: cmd.mode,
                    doc: chosen,
                });
            }

            Doc::Concat(docs) => {
                for doc in docs.iter().rev() {
                    commands.push(Command {
                        indent: cmd.indent,
                        mode: cmd.mode,
                        doc,
                    });
                }
            }

            Doc::Fill(parts) => {
                // Nested fill - recurse with default context
                render_fill_iterative(
                    parts,
                    output,
                    pos,
                    cmd.indent,
                    config,
                    &DocContext::default(),
                    resolver,
                );
            }

            Doc::WithContext { doc, context } => {
                // Extract context and apply to inner doc
                match doc.as_ref() {
                    Doc::Fill(parts) => {
                        // Apply context when rendering fill
                        render_fill_iterative(
                            parts, output, pos, cmd.indent, config, context, resolver,
                        );
                    }
                    _ => {
                        // For non-fill docs, just unwrap and continue
                        commands.push(Command {
                            indent: cmd.indent,
                            mode: cmd.mode,
                            doc: doc.as_ref(),
                        });
                    }
                }
            }
        }
    }
}

// =============================================================================
// Utilities
// =============================================================================

/// Calculate visual width of a string
///
/// Uses a fast path for ASCII strings (O(1) via len()) since ~99% of
/// formatter output is ASCII. Falls back to char counting for non-ASCII.
///
/// TODO: For full Unicode support, should use a library like `unicode-width`.
/// For now, assumes non-ASCII multi-byte chars count as 1.
#[inline]
fn string_width(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    }
}

/// Write indentation to output
fn write_indentation(output: &mut String, level: usize, config: &PrintConfig) {
    for _ in 0..level {
        output.push_str(config.indent);
    }
}

/// Calculate width of indentation
fn indent_width(level: usize, config: &PrintConfig) -> usize {
    level * indent_str_width(config.indent, config.tab_width)
}

/// Calculate visual width of indentation string
fn indent_str_width(indent: &str, tab_width: usize) -> usize {
    indent
        .chars()
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text() {
        let doc = text("hello");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello");
    }

    #[test]
    fn test_concat() {
        let doc = concat(vec![text("hello"), text(" "), text("world")]);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello world");
    }

    #[test]
    fn test_line_in_flat_mode_fits() {
        // Short content should fit on one line
        let doc = group(concat(vec![text("a"), line(), text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_line_in_break_mode_doesnt_fit() {
        // Long content should break
        let doc = group(concat(vec![text("hello"), line(), text("world")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 8, // Too narrow for "hello world"
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "hello\nworld");
    }

    #[test]
    fn test_hardline_always_breaks() {
        let doc = concat(vec![text("a"), hardline(), text("b")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a\nb");
    }

    #[test]
    fn test_softline_disappears_in_flat_mode() {
        let doc = group(concat(vec![text("a"), softline(), text("b")]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "ab");
    }

    #[test]
    fn test_indent() {
        // Hardline must be INSIDE indent to get indented
        let doc = concat(vec![
            text("parent"),
            indent(concat(vec![hardline(), text("child")])),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "parent\n\tchild");
    }

    #[test]
    fn test_group_with_indent() {
        let doc = group(concat(vec![
            text("("),
            indent(concat(vec![line(), text("content")])),
            line(),
            text(")"),
        ]));

        // Fits: should be flat
        let config_wide = PrintConfig {
            indent: "  ",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "( content )");

        // Doesn't fit: should break
        let config_narrow = PrintConfig {
            indent: "  ",
            print_width: 8,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "(\n  content\n)");
    }

    #[test]
    fn test_if_break() {
        let doc = group(concat(vec![
            text("("),
            if_break(text(",\n"), text(", ")),
            text(")"),
        ]));

        // Fits: use flat_doc
        let config_wide = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "(, )");

        // Doesn't fit: use break_doc
        // Note: IfBreak needs parent group to break, so we need content that doesn't fit
        let doc_long = group(concat(vec![
            text("("),
            text("very long content that exceeds print width"),
            if_break(hardline(), text(" ")),
            text(")"),
        ]));

        let config_narrow = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        let result = print_doc(&doc_long, &config_narrow);
        assert!(result.contains('\n'));
    }

    #[test]
    fn test_dedent() {
        // Dedent affects line breaks INSIDE the dedent block
        let doc = indent(concat(vec![
            text("level1"),
            hardline(),
            text("still-level1"),
            dedent(concat(vec![hardline(), text("back-to-level0")])),
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 80,
            tab_width: 2,
            ..Default::default()
        };
        // First text at level 0, hardline at level 1, second text at level 1,
        // dedent's hardline at level 0, third text at level 0
        assert_eq!(
            print_doc(&doc, &config),
            "level1\n\tstill-level1\nback-to-level0"
        );
    }

    #[test]
    fn test_tab_width_calculation() {
        // Test that tabs are counted correctly for width
        assert_eq!(indent_str_width("\t", 2), 2);
        assert_eq!(indent_str_width("\t", 4), 4);
        assert_eq!(indent_str_width("  ", 2), 2);
        assert_eq!(indent_str_width("\t\t", 2), 4);
    }

    // ==========================================================================
    // Fill tests
    // ==========================================================================

    #[test]
    fn test_fill_all_fit() {
        // All items fit on one line
        let doc = fill(vec![text("a"), line(), text("b"), line(), text("c")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b c");
    }

    #[test]
    fn test_fill_greedy_packing() {
        // Items should pack greedily: "a b" fits, then "c" on next line
        let doc = fill(vec![text("aa"), line(), text("bb"), line(), text("cc")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 6, // "aa bb" = 5 chars, fits; "aa bb cc" = 8, doesn't fit
            tab_width: 2,
            ..Default::default()
        };
        // "aa bb" fits (5 chars), but "aa bb cc" (8 chars) doesn't
        // So: "aa bb\ncc"
        assert_eq!(print_doc(&doc, &config), "aa bb\ncc");
    }

    #[test]
    fn test_fill_single_item() {
        let doc = fill(vec![text("hello")]);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "hello");
    }

    #[test]
    fn test_fill_two_items() {
        let doc = fill(vec![text("a"), line(), text("b")]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 10,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_fill_with_indent() {
        // Fill inside indent should have indented continuation lines
        let doc = indent(fill(vec![
            text("aaa"),
            line(),
            text("bbb"),
            line(),
            text("ccc"),
        ]));
        let config = PrintConfig {
            indent: "\t",
            print_width: 10, // "\taaa bbb" = 9 chars (tab=2), fits; "\taaa bbb ccc" = 13, doesn't
            tab_width: 2,
            ..Default::default()
        };
        // At indent level 1, "aaa bbb" fits, "ccc" wraps
        assert_eq!(print_doc(&doc, &config), "aaa bbb\n\tccc");
    }

    #[test]
    fn test_fill_comma_separated() {
        // Simulate CSS comma-separated list: item1, item2, item3
        let doc = fill(vec![
            text("item1"),
            text(", "),
            text("item2"),
            text(", "),
            text("item3"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 20,
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "item1, item2, item3");
    }

    #[test]
    fn test_fill_long_comma_list_wraps() {
        // Long list should wrap with greedy packing
        let doc = fill(vec![
            text("aaaa"),
            concat(vec![text(","), line()]),
            text("bbbb"),
            concat(vec![text(","), line()]),
            text("cccc"),
            concat(vec![text(","), line()]),
            text("dddd"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 15, // "aaaa, bbbb" = 10, fits; add ", cccc" = 17, doesn't
            tab_width: 2,
            ..Default::default()
        };
        // "aaaa, bbbb" fits (10), "aaaa, bbbb, cccc" (17) doesn't
        // So: "aaaa, bbbb,\ncccc, dddd"
        assert_eq!(print_doc(&doc, &config), "aaaa, bbbb,\ncccc, dddd");
    }

    #[test]
    fn test_fill_none_fit() {
        // Each item is too long to fit with another
        let doc = fill(vec![
            text("verylongitem1"),
            line(),
            text("verylongitem2"),
            line(),
            text("verylongitem3"),
        ]);
        let config = PrintConfig {
            indent: "\t",
            print_width: 15,
            tab_width: 2,
            ..Default::default()
        };
        // Each item alone fits, but no two items fit together
        assert_eq!(
            print_doc(&doc, &config),
            "verylongitem1\nverylongitem2\nverylongitem3"
        );
    }

    #[test]
    fn test_fill_with_base_indent_offset() {
        // Test that base_indent_offset affects width calculations after newlines
        // Simulates TypeScript array inside Svelte <script> tag
        //
        // The base_indent_offset only affects position calculation AFTER a newline.
        // The first line is unaffected since we start at column 0.
        let doc = indent(fill(vec![
            text("1"),
            concat(vec![text(","), line()]),
            text("2"),
            concat(vec![text(","), line()]),
            text("3"),
            concat(vec![text(","), line()]),
            text("4"),
            concat(vec![text(","), line()]),
            text("5"),
            concat(vec![text(","), line()]),
            text("6"),
            concat(vec![text(","), line()]),
            text("7"),
            concat(vec![text(","), line()]),
            text("8"),
        ]));

        // Without base_indent_offset: width = 12, tab_width = 2
        // First line: fills until "1, 2, 3, 4," (10 chars), break after 4
        // After newline: pos = 2 (just local indent), remaining = 10
        // Continuation with +1 margin: remaining + 1 = 11, so "5, 6, 7, 8" (10) fits
        let config_no_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 0,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &config_no_offset),
            "1, 2, 3, 4,\n\t5, 6, 7, 8"
        );

        // With base_indent_offset=1: width = 12, tab_width = 2
        // base_indent_offset affects position calculation after newlines in render_single_doc
        // First line: same as without offset (base_indent only applies after newlines)
        // After newline: pos = 2 + 2 = 4 (local indent + base offset), remaining = 8
        // "5, 6, 7" = 7 chars fits, "5, 6, 7, 8" = 10 doesn't fit in 8
        // So: "5, 6, 7," then break, then "8"
        let config_with_offset = PrintConfig {
            indent: "\t",
            print_width: 12,
            tab_width: 2,
            base_indent_offset: 1,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &config_with_offset),
            "1, 2, 3, 4,\n\t5, 6, 7,\n\t8"
        );
    }

    #[test]
    fn test_fill_wraps_last_item_at_102_chars() {
        // Reproduce the CSS animation-name bug:
        // When the line reaches exactly 102 chars (printWidth + 2), the last item
        // should wrap to a new line, but it's staying on the same line.
        //
        // Structure: indent (6 chars) + items with commas
        // Items: "a0000000000, a1111111111, ... a5555555555, " = ~84 chars
        // Last item: "a6666666666666666" = 16 chars
        // Total: 6 + 84 + 16 = 106 chars → should wrap

        let items = vec![
            "a0000000000",
            "a1111111111",
            "a2222222222",
            "a3333333333",
            "a4444444444",
            "a5555555555",
            "a6666666666666666", // This long last item should wrap
        ];

        // Build fill doc: [item, ", ", item, ", ", ..., item]
        let mut parts = Vec::new();
        for (i, item) in items.iter().enumerate() {
            parts.push(text(*item));
            if i < items.len() - 1 {
                parts.push(concat(vec![text(","), line()]));
            }
        }

        let doc = fill(parts);

        let config = PrintConfig {
            indent: "\t",
            print_width: 100,
            tab_width: 2,
            base_indent_offset: 1, // Simulates CSS inside Svelte <style>
            ..Default::default()
        };

        // Simulate starting at indent position (3 tabs = 6 chars visual width)
        // This matches the real CSS case where we've already written:
        // <style>\n\tdiv {\n\t\tanimation-name:\n\t\t\t
        let start_column = 6; // 3 tabs × 2
        let indent_level = 3;
        let output = print_doc_with_indent(&doc, &config, start_column, indent_level);

        // Expected: last item wraps to new line
        // The long last item should NOT be on the same line as a5555555555
        assert!(
            !output.contains("a5555555555, a6666666666666666"),
            "Last item should wrap to new line, but found on same line as previous item"
        );

        // Should have the pattern: "a5555555555,\n\t\t\ta6666666666666666" (3 tabs for indent level 3)
        assert!(
            output.contains("a5555555555,\n\t\t\ta6666666666666666"),
            "Expected last item to be on its own line with proper indentation. Got:\n{}",
            output
        );
    }

    // ==========================================================================
    // Helper function tests
    // ==========================================================================

    #[test]
    fn test_join() {
        let docs = vec![text("a"), text("b"), text("c")];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "a, b, c");
    }

    #[test]
    fn test_join_empty() {
        let docs: Vec<Doc> = vec![];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "");
    }

    #[test]
    fn test_join_single() {
        let docs = vec![text("a")];
        let doc = join(docs, ", ");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "a");
    }

    #[test]
    fn test_join_doc_with_line() {
        // join_doc with line() separator
        let docs = vec![text("a"), text("b"), text("c")];
        let doc = group(join_doc(docs, line()));

        // Wide enough: fits on one line with spaces
        let config_wide = PrintConfig {
            print_width: 20,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "a b c");

        // Too narrow: breaks
        let config_narrow = PrintConfig {
            print_width: 3,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "a\nb\nc");
    }

    #[test]
    fn test_join_doc_with_comma_line() {
        // join_doc with comma + line separator (common pattern)
        let docs = vec![text("item1"), text("item2"), text("item3")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_doc(docs, sep));

        // Wide enough
        let config_wide = PrintConfig {
            print_width: 30,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_wide), "item1, item2, item3");

        // Too narrow
        let config_narrow = PrintConfig {
            print_width: 10,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config_narrow), "item1,\nitem2,\nitem3");
    }

    #[test]
    fn test_wrap() {
        let doc = wrap("(", text("content"), ")");
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "(content)");
    }

    #[test]
    fn test_parens() {
        let doc = parens(text("x"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "(x)");
    }

    #[test]
    fn test_brackets() {
        let doc = brackets(text("0"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "[0]");
    }

    #[test]
    fn test_braces() {
        let doc = braces(text("a: 1"));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "{a: 1}");
    }

    #[test]
    fn test_wrap_with_nested_content() {
        // Test wrap with more complex nested content
        let inner = concat(vec![text("a"), text(", "), text("b")]);
        let doc = brackets(inner);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "[a, b]");
    }

    #[test]
    fn test_nested_wraps() {
        // Test nested wraps: { [x] }
        let doc = braces(concat(vec![text(" "), brackets(text("x")), text(" ")]));
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "{ [x] }");
    }

    // ==========================================================================
    // Join trailing tests
    // ==========================================================================

    #[test]
    fn test_join_trailing_flat() {
        // When flat, no trailing comma
        let docs = vec![text("a"), text("b"), text("c")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a, b, c");
    }

    #[test]
    fn test_join_trailing_break() {
        // When breaking, adds trailing comma
        let docs = vec![text("a"), text("b"), text("c")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 3,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a,\nb,\nc,");
    }

    #[test]
    fn test_join_trailing_empty() {
        let docs: Vec<Doc> = vec![];
        let sep = concat(vec![text(","), line()]);
        let doc = join_trailing(docs, sep);
        let config = PrintConfig::default();
        assert_eq!(print_doc(&doc, &config), "");
    }

    #[test]
    fn test_join_trailing_single() {
        // Single item, no trailing comma in flat mode
        let docs = vec![text("a")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(join_trailing(docs, sep));
        let config = PrintConfig {
            print_width: 20,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a");
    }

    #[test]
    fn test_join_trailing_in_brackets() {
        // Common pattern: [a, b, c] or [\n  a,\n  b,\n  c,\n]
        let docs = vec![text("item1"), text("item2"), text("item3")];
        let sep = concat(vec![text(","), line()]);
        let doc = group(concat(vec![
            text("["),
            indent(concat(vec![softline(), join_trailing(docs, sep)])),
            softline(),
            text("]"),
        ]));

        // Wide: fits on one line
        let wide = PrintConfig {
            print_width: 30,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &wide), "[item1, item2, item3]");

        // Narrow: breaks with trailing comma
        let narrow = PrintConfig {
            print_width: 15,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(
            print_doc(&doc, &narrow),
            "[\n  item1,\n  item2,\n  item3,\n]"
        );
    }

    // ==========================================================================
    // Indent helper tests
    // ==========================================================================

    #[test]
    fn test_indent_line() {
        // Content that doesn't fit should break with indent
        let doc = group(concat(vec![text("prefix"), indent_line(text("indented"))]));
        let config = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "prefix\n  indented");
    }

    #[test]
    fn test_indent_line_fits() {
        // When content fits, line becomes space
        let doc = group(concat(vec![text("a"), indent_line(text("b"))]));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a b");
    }

    #[test]
    fn test_indent_softline_flat() {
        // Wide enough: softline disappears entirely
        let doc = group(concat(vec![text("a"), indent_softline(text("b"))]));
        let config = PrintConfig {
            print_width: 20,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "ab");
    }

    #[test]
    fn test_indent_softline_break() {
        // Too narrow: breaks with indent
        let doc = group(concat(vec![text("a"), indent_softline(text("b"))]));
        let config = PrintConfig {
            print_width: 1,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &config), "a\n  b");
    }

    #[test]
    fn test_indent_softline_in_parens() {
        // Common pattern: arguments in parentheses
        let doc = group(concat(vec![
            text("fn("),
            indent_softline(text("arg1, arg2")),
            softline(),
            text(")"),
        ]));

        // Fits: flat
        let wide = PrintConfig {
            print_width: 30,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &wide), "fn(arg1, arg2)");

        // Doesn't fit: breaks
        let narrow = PrintConfig {
            print_width: 10,
            indent: "  ",
            tab_width: 2,
            ..Default::default()
        };
        assert_eq!(print_doc(&doc, &narrow), "fn(\n  arg1, arg2\n)");
    }
}
