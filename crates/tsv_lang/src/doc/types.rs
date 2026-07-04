//! Core types for the doc builder

use crate::Span;

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

impl GroupId {
    /// Number of variants. Sizes the renderer's inline `[Option<Mode>; COUNT]`
    /// group-mode map (indexed by `id as usize`), which replaces a per-render
    /// `HashMap`. Keep in sync when adding a variant — a stale (too-small) value
    /// would index out of bounds, caught immediately by the fixture suite.
    pub(crate) const COUNT: usize = 6;
}

/// Context for doc rendering - provides hints about trailing punctuation
/// that affect how content is rendered.
///
/// This allows fills to make better packing decisions by knowing about
/// punctuation that will be added by the parent (e.g., semicolons in CSS,
/// commas in object properties).
///
/// These flags are deliberate per-fill render policies set by the language
/// printers (Svelte boundary rules, CSS trailing punctuation); a flag bag is
/// the intended shape, so the `excessive_bools` lint is allowed here.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct DocContext {
    /// Reserve N chars when checking if content fits.
    ///
    /// This prevents greedy fills from packing to exactly printWidth,
    /// which would be exceeded when the parent adds trailing punctuation.
    ///
    /// Example: CSS declarations add ";" after the value, so reserve 1 char.
    pub trailing_reserve: usize,

    /// When set, the fill's FIRST item, if it renders at the start of its line (i.e. it was
    /// pushed to its own line by a preceding break — it "dropped"), forces the separator after it
    /// to break, so the next item takes its own line.
    ///
    /// Scoped to the Svelte after-element fold of a *sandwiched* inline element/component: a wide
    /// inline child that drops to its own line owns that line — the trailing text after it wraps to
    /// the next line rather than hugging the dropped child's `>`. Off for every other fill (text
    /// word-wrap and CSS value lists pack greedily after a dropped item), so the flag never affects
    /// them.
    pub break_after_dropped_first: bool,

    /// When set, the fill's trailing separator (its terminal `line`, the only one reaching the
    /// "content + separator" render case) measures the *immediately following* node — the next
    /// item on the render stack — as a WHOLE flat unit, instead of letting that node's own internal
    /// break point short-circuit the fit check. A wide inline element that would not fit flat after
    /// the separator then forces the separator to break, dropping the element to its own line whole
    /// — rather than packing it onto the text line, where it would break its own tag in place.
    ///
    /// Scoped to the Svelte text→flow-element boundary fill (a text run whose next sibling is a
    /// flowing inline element/component, ended with a trailing `line`). Off for every other fill, so
    /// a small element after text still packs and CSS/value-list fills are unaffected. This is the
    /// leading-boundary counterpart of [`Self::break_after_dropped_first`]: both re-couple the
    /// width-driven drop decision to the boundary rule at render position so the space- and
    /// newline-authored forms converge to one fixed point.
    pub break_before_wide_flow: bool,

    /// When set, the fill's FIRST item, if it sits mid-line (right after a small prefix such as a
    /// parent inline element's `>`) and does not fit on its own line *either* (wider than printWidth
    /// even at line start), is rendered **in place** — it breaks internally — rather than dropped to
    /// the next line. Dropping a too-wide-anyway first item only strands a spurious break before it
    /// (a `>⏎<child` dangle that the next pass collapses → non-idempotent); rendering in place keeps
    /// the child hugging the prefix, matching the newline-authored form.
    ///
    /// Scoped to the Svelte after-element fold (`fill([element, line, words…])`), whose first item
    /// is always a breakable inline element/component. Off for every other fill, so text word-wrap
    /// and CSS value lists still drop a too-wide item onto its own line.
    pub hug_wide_first: bool,

    /// When set, a fill item that wraps at line start (the after-element fold's wide element) lets
    /// the *terminal* trailing text hug the dangled closing `>` (`</tag⏎> tail`) if it fits there,
    /// instead of forcing it onto its own line. The separator after the wrapped element is chosen
    /// by the actual resulting column — flat (hug) when the next item still fits, else break.
    ///
    /// Scoped to the Svelte after-element fold whose trailing text is *terminal* (`!trailing_line`
    /// — no following flowing element). This is how tsv respects an author's *space* boundary after
    /// a wide inline element, mirroring how short inline elements already keep `<el>x</el> tail`
    /// inline; a *newline*-authored boundary still takes its own line (the text node carries the
    /// newline, so it never reaches this fold). Off for every other fill — CSS value lists and
    /// non-terminal text (`trailing_line`, the non-convergent cascade) keep their own-line break.
    pub hug_terminal_after_break: bool,
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

    /// Resolve a [`DocText::SourceSpan`] to its verbatim source slice.
    ///
    /// The default implementation panics — a bare interner carries no source, so
    /// docs containing `SourceSpan` nodes must be rendered through a source-aware
    /// resolver ([`SourceTextResolver`]). Mirrors the `Symbol`-without-resolver
    /// programming-error contract.
    ///
    /// # Panics
    /// Panics if the resolver carries no source (the default).
    fn resolve_source_span(&self, _span: Span) -> &str {
        #[allow(clippy::unimplemented)]
        {
            unimplemented!("SourceSpan in Doc but resolver carries no source")
        }
    }
}

/// A [`TextResolver`] that wraps an inner resolver (the interner) and adds the
/// document source, so [`DocText::SourceSpan`] nodes resolve to verbatim source
/// slices. This is how `source` reaches the render path **without** putting a
/// lifetime on `DocArena` (the span lives in the lifetime-less arena; the source
/// is supplied transiently at render). A printer that emits `SourceSpan` builds
/// one of these around its interner + source and passes it to the resolved
/// render entry points in place of the bare interner.
pub struct SourceTextResolver<'a, R: TextResolver + ?Sized> {
    /// The underlying symbol resolver (typically the interner).
    pub inner: &'a R,
    /// The document source the spans index into (the host document — all spans
    /// recorded by the printer are absolute into this string).
    pub source: &'a str,
}

impl<R: TextResolver + ?Sized> TextResolver for SourceTextResolver<'_, R> {
    #[inline]
    fn resolve(&self, id: u32) -> &str {
        self.inner.resolve(id)
    }

    #[inline]
    fn resolve_source_span(&self, span: Span) -> &str {
        span.extract(self.source)
    }
}

/// Sentinel value for cached_width: text contains a newline.
/// Used by fits to early-return without resolving the string.
pub const TEXT_WIDTH_HAS_NEWLINE: u16 = u16::MAX;

/// Sentinel value for cached_width: width not yet computed.
/// Used for owned strings that may be expensive to measure upfront.
pub const TEXT_WIDTH_NOT_COMPUTED: u16 = u16::MAX - 1;

/// A slice into the [`super::DocArena`]'s text pool — the arena-owned `String`
/// holding every dynamically-built text body ([`DocText::Pooled`],
/// [`super::DocNode::MultilineText`]). Offsets are byte indices into that pool,
/// resolved at render time against the pool borrowed from the same arena the
/// node lives in (the pool-keyed sibling of the source-keyed
/// [`DocText::SourceSpan`]). Storing a span instead of an owned `String` keeps
/// `DocNode` free of drop glue, so the arena's `reset()`/drop never walk the
/// node store running destructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSpan {
    /// Byte offset of the text's start in the arena text pool.
    pub start: u32,
    /// Byte length of the text.
    pub len: u32,
}

impl PoolSpan {
    /// Resolve to the text slice within `pool` (the owning arena's text pool).
    #[inline]
    pub fn slice(self, pool: &str) -> &str {
        &pool[self.start as usize..(self.start + self.len) as usize]
    }
}

/// Text content in a Doc - static, owned, or a symbol to resolve at print time
#[derive(Debug, Clone)]
pub enum DocText {
    /// Static string literal - no allocation, just stores pointer.
    /// Second field is precomputed visual width (u16::MAX = contains newline).
    Static(&'static str, u16),
    /// Dynamically generated text, stored in the arena's text pool — the
    /// drop-glue-free replacement for a per-node owned `String`. Resolved
    /// against the pool at render time (like `SourceSpan` against `source`).
    /// Second field is the precomputed visual width — **always** computed at
    /// build (a real width or [`TEXT_WIDTH_HAS_NEWLINE`], never
    /// [`TEXT_WIDTH_NOT_COMPUTED`]), so the fits walk never needs the pool:
    /// width queries answer from the node alone, and only the render loop
    /// (which borrows the pool once per render) reads the bytes. Pooled text
    /// is rare (~1.4% of Text nodes), so the eager measure is off the hot
    /// path by construction.
    Pooled(PoolSpan, u16),
    /// Verbatim source slice, resolved against `source` at print time — like
    /// `Symbol` but keyed on a span instead of an interner id. Second field is
    /// the precomputed visual width — always computed at build like `Pooled`
    /// (a real width or [`TEXT_WIDTH_HAS_NEWLINE`]), except identifier names
    /// (via `source_span_ident`), which defer to on-demand measurement
    /// ([`TEXT_WIDTH_NOT_COMPUTED`]). Lets a printer emit verbatim source text
    /// (comments, template chunks, already-canonical literals) with **no
    /// allocation and no copy** — the lifetime-free alternative to a borrowed
    /// `&'src str` (which would force `DocArena<'src>` and forfeit the
    /// cross-file arena `reset()` reuse). The span is resolved by a
    /// source-aware [`TextResolver`] (see [`SourceTextResolver`]); behaves
    /// identically to the pooled text it replaces in every doc transform (a
    /// `DocNode::Text` is matched generically).
    SourceSpan(Span, u16),
    /// Symbol ID to be resolved at print time - no allocation during doc building.
    /// No cached width — identifiers are ASCII, fast path handles them.
    Symbol(u32),
}

impl DocText {
    /// Get the cached visual width.
    ///
    /// Decodes the stored `u16` (a real width or one of the two sentinel
    /// values) into [`CachedWidth`], so callers can't mistake
    /// [`TEXT_WIDTH_HAS_NEWLINE`] for an actual width — every consumer must
    /// handle the newline case explicitly. `Symbol` is always
    /// [`CachedWidth::NotComputed`] (identifiers are measured on demand).
    #[inline]
    pub const fn cached_width(&self) -> CachedWidth {
        match self {
            DocText::Static(_, w) | DocText::Pooled(_, w) | DocText::SourceSpan(_, w) => match *w {
                TEXT_WIDTH_NOT_COMPUTED => CachedWidth::NotComputed,
                TEXT_WIDTH_HAS_NEWLINE => CachedWidth::HasNewline,
                w => CachedWidth::Width(w),
            },
            DocText::Symbol(_) => CachedWidth::NotComputed,
        }
    }
}

/// Decoded form of a [`DocText`] width slot — see [`DocText::cached_width`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachedWidth {
    /// Precomputed single-line visual width.
    Width(u16),
    /// The text contains a newline — there is no single-line width; fits
    /// treats the line as ending inside this text.
    HasNewline,
    /// Not precomputed (ASCII policy or `Symbol`) — measure on demand.
    NotComputed,
}

/// Resolve DocText to a string, using resolver if provided
///
/// For Static text, returns directly. For Pooled text, slices the arena text
/// pool the caller borrowed (render hoists it once per render). For Symbol
/// text, uses the resolver (panics if resolver is None).
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
    pool: &'a str,
) -> &'a str {
    match text {
        DocText::Static(s, _) => s,
        DocText::Pooled(span, _) => span.slice(pool),
        DocText::SourceSpan(span, _) => resolver
            .expect("SourceSpan encountered in Doc but no TextResolver provided")
            .resolve_source_span(*span),
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
