//! Core types for the doc builder

/// Group identifier for tracking which groups broke during rendering.
///
/// Enables `indent_if_break` to check if a specific group broke, allowing
/// deferred indentation decisions. Add new variants here as needed.
///
/// Prettier uses Symbol() for unique IDs; we use an enum for type safety.
/// Most formatting needs are handled by `conditional_group` without needing IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupId {
    /// Fluid assignment layout: `a = value`
    /// Used in assignment.rs for conditional right-hand side indentation
    Assignment,
}

/// Context for doc rendering - provides hints about trailing punctuation,
/// width constraints, and indent overrides that affect how content is rendered.
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

    /// Override base_indent_offset for position calculations within this context.
    ///
    /// When `Some(n)`, width calculations use `n` instead of `config.base_indent_offset`.
    /// This is used for template expression content where the wrapper (e.g., Svelte)
    /// won't add its usual indentation, so position calculations shouldn't account for it.
    ///
    /// When `None`, uses the default `config.base_indent_offset`.
    pub base_indent_override: Option<usize>,
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

/// Line break behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Normal line: space in flat mode, newline + indent in break mode
    Normal,
    /// Soft line: disappears in flat mode, newline + indent in break mode
    Soft,
    /// Hard line: always breaks with newline + indent (ignores flat mode)
    Hard,
    /// Literal line: always breaks with newline only, NO indentation
    /// Used for blank line preservation
    Literal,
}

/// Document primitive - abstract representation of formatted output
#[derive(Debug, Clone)]
pub enum Doc {
    /// Text content to output (static or owned)
    Text(DocText),

    /// Line break - behavior depends on kind and mode
    Line(LineKind),

    /// Increase indentation level for nested content
    Indent(Box<Doc>),

    /// Decrease indentation level
    Dedent(Box<Doc>),

    /// Set absolute indentation level for nested content
    ///
    /// Unlike Indent/Dedent which are relative, Align sets the indent to an
    /// absolute value. Used for template literal interpolations where we need
    /// content indented to match the template's visual position regardless of
    /// how deeply nested the template is in function bodies or other blocks.
    Align { n: usize, contents: Box<Doc> },

    /// Add alignment spaces to indentation (Prettier-style alignment)
    ///
    /// Unlike Indent which adds tab levels, AlignSpaces adds a fixed number
    /// of spaces after the tabs. Used for aligning closing delimiters with
    /// opening delimiters (e.g., `)` aligning with `(` in union types).
    ///
    /// Example: `| (A & {\n\t\t\t  })` - the `)` uses 2 spaces to align with `(`
    AlignSpaces { spaces: usize, contents: Box<Doc> },

    /// Try to fit content on one line; if doesn't fit, break ALL lines in group
    /// This is the key primitive for prettier's "all-or-nothing" breaking
    ///
    /// When `expanded_states` is Some, this is a "conditional group" that tries
    /// multiple alternative layouts (like prettier's `conditionalGroup`):
    /// 1. First tries states[0] in flat mode
    /// 2. If that doesn't fit, tries states[1], states[2], ... in flat mode
    /// 3. If none fit, uses the last state in break mode
    ///
    /// When `id` is Some, the group's mode (Flat/Break) is tracked in groupModeMap,
    /// allowing `IndentIfBreak` nodes to check if this specific group broke.
    ///
    /// Note: `expanded_states` is boxed to keep Doc enum small (32 bytes vs 40).
    /// Only `conditional_group` uses this field; regular `group()` has None.
    Group {
        contents: Box<Doc>,
        /// Alternative layouts to try before breaking (prettier's expandedStates)
        /// Boxed to reduce Doc enum size - only conditional_group uses this.
        expanded_states: Option<Box<Vec<Doc>>>,
        /// Optional ID for tracking this group's mode (prettier's GroupId)
        id: Option<GroupId>,
    },

    /// Conditional rendering based on whether parent group breaks
    /// - If parent breaks: render `break_doc`
    /// - If parent fits: render `flat_doc`
    IfBreak {
        break_doc: Box<Doc>,
        flat_doc: Box<Doc>,
    },

    /// Conditionally indent based on whether a specific group broke
    /// (optimized version of `ifBreak(indent(doc), doc, { groupId })`)
    ///
    /// Matches Prettier's `indentIfBreak` (indent-if-break.js):
    /// - If group[id] broke: indent(contents)
    /// - If group[id] stayed flat: contents (no indent)
    /// - If `negate` is true: reverse the logic
    ///
    /// This enables deferred indentation decisions - the indent is applied
    /// only after we know whether the referenced group broke.
    IndentIfBreak {
        contents: Box<Doc>,
        group_id: GroupId,
        negate: bool,
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

    /// Content to print at the end of the current line.
    ///
    /// LineSuffix is NOT included in width calculations during `fits()`,
    /// allowing lines to exceed print width when they have trailing comments.
    /// The content is buffered and printed after the current line's content.
    ///
    /// This matches prettier's `lineSuffix` for trailing comments.
    LineSuffix(Box<Doc>),

    /// Force any pending LineSuffix content to be flushed.
    ///
    /// This prevents LineSuffix from one group from bleeding into another.
    /// Typically placed before line breaks in sensitive contexts.
    LineSuffixBoundary,

    /// Force parent group to break.
    ///
    /// This propagates up through the doc tree during printing and marks
    /// any enclosing group as broken, forcing it to use break mode.
    ///
    /// Used when a child element (like a trailing comment) should force
    /// the entire parent construct to expand to multiple lines.
    BreakParent,
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
    /// Override for base_indent_offset in position calculations.
    /// When Some(n), uses n instead of config.base_indent_offset.
    /// Propagated to child commands when set via WithContext.
    pub base_indent_override: Option<usize>,
    /// Additional alignment spaces after tabs (Prettier-style alignment).
    /// Used for aligning closing delimiters with opening delimiters.
    pub align_spaces: usize,
}

impl<'a> Command<'a> {
    /// Create a command with the same context but a different doc
    #[inline]
    pub fn with_doc(&self, doc: &'a Doc) -> Self {
        Self { doc, ..*self }
    }

    /// Create a command with incremented indent
    ///
    /// Resets align_spaces to 0 because indent starts a new "scope" where
    /// alignment is relative to the new indent level, not the parent's alignment.
    #[inline]
    pub fn indented(&self, doc: &'a Doc) -> Self {
        Self {
            indent: self.indent + 1,
            align_spaces: 0, // Reset alignment when entering new indent level
            doc,
            ..*self
        }
    }

    /// Create a command with decremented indent
    #[inline]
    pub fn dedented(&self, doc: &'a Doc) -> Self {
        Self {
            indent: self.indent.saturating_sub(1),
            doc,
            ..*self
        }
    }

    /// Create a command with absolute indent level
    #[inline]
    pub fn with_indent(&self, indent: usize, doc: &'a Doc) -> Self {
        Self {
            indent,
            doc,
            ..*self
        }
    }

    /// Create a command with a specific mode
    #[inline]
    pub fn with_mode(&self, mode: Mode, doc: &'a Doc) -> Self {
        Self { mode, doc, ..*self }
    }

    /// Create a command with a base indent override
    #[inline]
    pub fn with_base_override(&self, base_indent_override: Option<usize>, doc: &'a Doc) -> Self {
        Self {
            doc,
            base_indent_override,
            ..*self
        }
    }

    /// Create a command with additional alignment spaces
    #[inline]
    pub fn with_align_spaces(&self, spaces: usize, doc: &'a Doc) -> Self {
        Self {
            doc,
            align_spaces: self.align_spaces + spaces,
            ..*self
        }
    }
}
