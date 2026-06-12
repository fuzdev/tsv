// String interner utilities shared across language printers

use crate::doc::TextResolver;
use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol, Symbol};

/// Shared, mutable interner reference threaded through parsers and printers.
///
/// Used for the embedded TS-in-Svelte path so the same string identity is
/// reused across crates. This alias hides the upstream
/// `string_interner::DefaultStringInterner` type from consumer signatures.
pub type SharedInterner = Rc<RefCell<DefaultStringInterner>>;

/// Extension trait for infallible symbol resolution.
///
/// Symbols in tsv are always resolved by the same interner that created them.
/// This is an invariant of the system - if violated, it's a bug in our code.
/// This trait provides `resolve_infallible()` which panics with a clear message
/// rather than returning `Option`.
///
/// # Example
///
/// ```rust,ignore
/// use tsv_lang::InfallibleResolve;
///
/// let interner = program.interner.borrow();
/// let name = interner.resolve_infallible(symbol).to_string();
/// ```
pub trait InfallibleResolve {
    /// Resolve a symbol to its string, panicking if not found.
    ///
    /// # Panics
    ///
    /// Panics if the symbol was not interned by this interner.
    /// This indicates a bug - symbols should only be resolved by the
    /// interner that created them.
    fn resolve_infallible(&self, symbol: DefaultSymbol) -> &str;
}

impl InfallibleResolve for DefaultStringInterner {
    #[allow(clippy::expect_used)]
    fn resolve_infallible(&self, symbol: DefaultSymbol) -> &str {
        self.resolve(symbol)
            .expect("Symbol not found in interner - this is a bug")
    }
}

/// Implement TextResolver for DefaultStringInterner to enable deferred symbol resolution in docs
///
/// This allows printers to borrow the interner and pass it to print_doc_resolved:
/// ```ignore
/// let interner = self.interner.borrow();
/// let output = doc::print_doc_resolved(&doc, &config, &*interner);
/// ```
impl TextResolver for DefaultStringInterner {
    #[allow(clippy::expect_used)]
    fn resolve(&self, id: u32) -> &str {
        let symbol = DefaultSymbol::try_from_usize(id as usize)
            .expect("Invalid symbol ID in doc - should be from DefaultSymbol::to_usize()");
        self.resolve_infallible(symbol)
    }
}

/// Trait for printers that use string interning
///
/// This trait provides common symbol resolution methods for printers that use
/// a shared string interner. By implementing this trait, printers automatically
/// gain access to efficient symbol resolution utilities.
///
/// # String Interning
///
/// String interning is a memory optimization technique where identical strings
/// are stored only once. Instead of duplicating strings, we store each unique
/// string once and reference it via a lightweight `Symbol` (essentially an integer).
///
/// # Methods
///
/// - `resolve_symbol()`: Allocates a String for the symbol (use when ownership needed)
/// - `with_resolved_symbol()`: Zero-allocation callback approach (preferred for hot paths)
///
/// # Example
///
/// ```rust,ignore
/// use tsv_lang::{SharedInterner, SymbolResolver};
///
/// struct MyPrinter<'a> {
///     interner: SharedInterner,
///     // ... other fields
/// }
///
/// impl<'a> SymbolResolver for MyPrinter<'a> {
///     fn interner(&self) -> &SharedInterner {
///         &self.interner
///     }
/// }
///
/// // Now you can use:
/// let name = printer.resolve_symbol(symbol);  // Allocates
/// printer.with_resolved_symbol(symbol, |s| {  // Zero-allocation
///     println!("Name: {}", s);
/// });
/// ```
pub trait SymbolResolver {
    /// Get reference to the string interner
    ///
    /// This is the only required method. All other methods have default
    /// implementations that use this interner reference.
    fn interner(&self) -> &SharedInterner;

    /// Resolve a symbol to a String (allocates)
    ///
    /// This method allocates a new String on every call. For hot paths where
    /// you need to perform multiple operations on the same symbol, prefer
    /// `with_resolved_symbol()` instead for zero-allocation access.
    ///
    /// # Panics
    ///
    /// Panics if the symbol is not found in the interner (should never happen
    /// in correctly functioning code).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let identifier = printer.resolve_symbol(symbol);
    /// println!("Identifier: {}", identifier);
    /// ```
    fn resolve_symbol(&self, symbol: DefaultSymbol) -> String {
        self.interner()
            .borrow()
            .resolve_infallible(symbol)
            .to_string()
    }

    /// Execute a callback with a borrowed string for a symbol (zero-allocation)
    ///
    /// This method is more efficient than `resolve_symbol()` when you need to
    /// perform operations on the resolved string without needing ownership.
    /// The string is borrowed from the interner and passed to your callback,
    /// avoiding any allocation.
    ///
    /// # Panics
    ///
    /// Panics if the symbol is not found in the interner (should never happen
    /// in correctly functioning code).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Zero-allocation string comparison
    /// printer.with_resolved_symbol(symbol, |s| {
    ///     if s == "const" {
    ///         // Handle const keyword
    ///     }
    /// });
    ///
    /// // Zero-allocation string writing
    /// printer.with_resolved_symbol(symbol, |s| {
    ///     printer.buffer.push_str(s);
    /// });
    /// ```
    #[inline]
    fn with_resolved_symbol<F, R>(&self, symbol: DefaultSymbol, f: F) -> R
    where
        F: FnOnce(&str) -> R,
    {
        let interner = self.interner().borrow();
        f(interner.resolve_infallible(symbol))
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
