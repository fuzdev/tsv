//! Core types for the doc builder

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
pub(super) fn resolve_text<'a, R: TextResolver + ?Sized>(
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

/// A command in the printer's command stack.
/// Holds the context (indent, mode) and reference to a doc to process.
#[derive(Debug, Clone, Copy)]
pub(super) struct Command<'a> {
    pub indent: usize,
    pub mode: Mode,
    pub doc: &'a Doc,
}
