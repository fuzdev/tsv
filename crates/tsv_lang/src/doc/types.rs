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
    /// Type parameter `extends` constraint: `<T extends Long>` breaks after
    /// `extends` and indents the constraint when it overflows.
    TypeParameterConstraint,
    /// Type parameter `=` default: `<T = Long>` breaks after `=` and indents
    /// the default when it overflows.
    TypeParameterDefault,
    /// Curried arrow-function chain: the joined signature heads
    /// (`(a) => (b) => …`) break as a unit when they don't fit, and the
    /// terminal body's `indent_if_break` keys on this group so it indents only
    /// when the heads broke.
    ArrowChain,
    /// Svelte block-tag head (`{#if …}`, `{#each …}`, …): the breakable head
    /// expression breaks as a unit when it exceeds print width, and the closing
    /// `}` keys on this group via `if_break` so it dangles on its own line only
    /// when the head broke. The dangle's `if_break` is read immediately after the
    /// head group resolves (before the body), so a shared variant is safe under
    /// block nesting.
    BlockHead,
}

/// Context for doc rendering - provides hints about trailing punctuation
/// that affect how content is rendered.
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

/// Sentinel value for cached_width: text contains a newline.
/// Used by fits to early-return without resolving the string.
pub const TEXT_WIDTH_HAS_NEWLINE: u16 = u16::MAX;

/// Sentinel value for cached_width: width not yet computed.
/// Used for owned strings that may be expensive to measure upfront.
pub const TEXT_WIDTH_NOT_COMPUTED: u16 = u16::MAX - 1;

/// Text content in a Doc - static, owned, or a symbol to resolve at print time
#[derive(Debug, Clone)]
pub enum DocText {
    /// Static string literal - no allocation, just stores pointer.
    /// Second field is precomputed visual width (u16::MAX = contains newline).
    Static(&'static str, u16),
    /// Dynamically generated text - requires allocation.
    /// Second field is precomputed visual width (u16::MAX = contains newline).
    Owned(String, u16),
    /// Symbol ID to be resolved at print time - no allocation during doc building.
    /// No cached width — identifiers are ASCII, fast path handles them.
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
            DocText::Static(s, _) => Some(s),
            DocText::Owned(s, _) => Some(s),
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
            DocText::Static(s, _) => s,
            DocText::Owned(s, _) => s,
            DocText::Symbol(id) => resolver.resolve(*id),
        }
    }

    /// Check if this is a Symbol variant (needs resolver)
    #[inline]
    pub const fn is_symbol(&self) -> bool {
        matches!(self, DocText::Symbol(_))
    }

    /// Get the cached visual width, if available.
    ///
    /// Returns `Some(w)` for Static/Owned with precomputed width (u16::MAX = newline).
    /// Returns `None` for Symbol or when width was not precomputed (TEXT_WIDTH_NOT_COMPUTED).
    #[inline]
    pub const fn cached_width(&self) -> Option<u16> {
        match self {
            DocText::Static(_, w) | DocText::Owned(_, w) => {
                if *w == TEXT_WIDTH_NOT_COMPUTED {
                    None
                } else {
                    Some(*w)
                }
            }
            DocText::Symbol(_) => None,
        }
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
        DocText::Static(s, _) => s,
        DocText::Owned(s, _) => s,
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

/// Rendering mode for a doc
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Try to fit on one line (soft lines become spaces)
    Flat,
    /// Use line breaks (soft lines become newlines)
    Break,
}
