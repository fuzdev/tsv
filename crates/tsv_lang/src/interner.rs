// String interner utilities shared across language parsers and printers

use crate::doc::TextResolver;
use string_interner::{DefaultStringInterner, DefaultSymbol, Symbol};

/// A caller-owned string interner — the third per-document reusable threaded
/// alongside the parse-time `bumpalo::Bump` and the format-time `DocArena`.
///
/// A newtype over the upstream `string_interner::DefaultStringInterner` that
/// keeps that type out of every public signature (as the retired
/// `Rc<RefCell<…>>` `SharedInterner` alias did) while exposing only the three
/// operations the pipeline needs: `get_or_intern` (`&mut`, at parse), and
/// `resolve_infallible` / the [`TextResolver`] impl (`&`, at format and
/// convert). Interning is **not** interior-mutable — parse takes `&mut Interner`
/// and format/convert take `&Interner`, and the borrow checker enforces that
/// the write phase ends before the read phase (which it always does: tsv is a
/// batch parse-then-format, never an incremental compiler).
///
/// Its tenants are tsv_svelte's element/attribute names and the rare
/// unicode-escaped / oversized identifier — tens of short strings per document,
/// nothing on the common path (identifier names are span-identity). `new()`
/// therefore allocates nothing.
#[derive(Debug, Default)]
pub struct Interner(DefaultStringInterner);

impl Interner {
    /// A fresh, empty interner. Allocates nothing — the common-path document
    /// interns no strings at all.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(DefaultStringInterner::new())
    }

    /// A fresh interner pre-sized for `capacity` distinct strings. Used by the
    /// Svelte parser, whose element/attribute names are a small fixed
    /// population covered by one up-front allocation.
    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(DefaultStringInterner::with_capacity(capacity))
    }

    /// Intern `string`, returning its stable symbol (parse-time; needs `&mut`).
    #[inline]
    pub fn get_or_intern(&mut self, string: &str) -> DefaultSymbol {
        self.0.get_or_intern(string)
    }

    /// Resolve a symbol to its string, panicking if it was not interned here.
    ///
    /// Symbols in tsv are always resolved by the same interner that created
    /// them — an invariant of the system, so a miss is a bug in our code, not a
    /// recoverable condition.
    ///
    /// # Panics
    ///
    /// Panics if the symbol was not interned by this interner.
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn resolve_infallible(&self, symbol: DefaultSymbol) -> &str {
        self.0
            .resolve(symbol)
            .expect("Symbol not found in interner - this is a bug")
    }

    /// Reset for reuse across files (the [`crate::doc::arena::DocArena::reset`]
    /// analogue for a per-thread reusable interner).
    ///
    /// `string_interner` 0.20 exposes no capacity-retaining `clear`, so this
    /// replaces the backing with a fresh empty interner. That allocates nothing
    /// — the common-path interner is already empty — so there is no retained
    /// capacity to lose, and reuse is sound (each file's symbols are fully
    /// consumed by its format/convert before the next `clear`).
    #[inline]
    pub fn clear(&mut self) {
        self.0 = DefaultStringInterner::new();
    }
}

/// Deferred symbol resolution during doc rendering.
///
/// `DocText::Symbol` nodes carry a raw `u32` id (a [`DefaultSymbol`] flattened
/// via [`SymbolToU32`]); the renderer resolves them through this impl.
impl TextResolver for Interner {
    #[inline]
    #[allow(clippy::expect_used)]
    fn resolve(&self, id: u32) -> &str {
        let symbol = DefaultSymbol::try_from_usize(id as usize)
            .expect("Invalid symbol ID in doc - should be from DefaultSymbol::to_usize()");
        self.resolve_infallible(symbol)
    }
}

/// Trait for printers that use string interning.
///
/// Provides common symbol-resolution helpers on top of a single required
/// `interner()` accessor. A printer holds a borrowed [`Interner`] and gains:
///
/// - `resolve_symbol()`: allocates a `String` for the symbol (use when
///   ownership is needed)
/// - `with_resolved_symbol()`: zero-allocation callback (preferred for hot
///   paths)
pub trait SymbolResolver {
    /// Get a reference to the string interner.
    ///
    /// The only required method; every other method defaults off it.
    fn interner(&self) -> &Interner;

    /// Resolve a symbol to a `String` (allocates).
    ///
    /// For hot paths that operate on the resolved string without needing
    /// ownership, prefer `with_resolved_symbol()`.
    ///
    /// # Panics
    ///
    /// Panics if the symbol is not found in the interner (should never happen
    /// in correctly functioning code).
    fn resolve_symbol(&self, symbol: DefaultSymbol) -> String {
        self.interner().resolve_infallible(symbol).to_string()
    }

    /// Execute a callback with a borrowed string for a symbol (zero-allocation).
    ///
    /// # Panics
    ///
    /// Panics if the symbol is not found in the interner (should never happen
    /// in correctly functioning code).
    #[inline]
    fn with_resolved_symbol<F, R>(&self, symbol: DefaultSymbol, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        f(self.interner().resolve_infallible(symbol))
    }
}

/// Extension trait for Symbol to provide u32 conversion for doc builder
///
/// The doc builder's `doc::symbol()` function takes `u32` IDs, but `DefaultSymbol::to_usize()`
/// returns `usize`. This trait provides a convenient conversion method to avoid repeated
/// `.to_usize() as u32` casts throughout printer code.
///
/// # Example
///
/// ```rust,ignore
/// use tsv_lang::SymbolToU32;
///
/// let id = sym.to_u32();  // Instead of: sym.to_usize() as u32
/// doc::symbol(id)
/// ```
pub trait SymbolToU32 {
    /// Convert symbol to u32 for doc builder
    fn to_u32(self) -> u32;
}

impl SymbolToU32 for DefaultSymbol {
    #[inline]
    fn to_u32(self) -> u32 {
        self.to_usize() as u32
    }
}
