// String interner utilities shared across language printers

use std::cell::RefCell;
use std::rc::Rc;
use string_interner::{DefaultStringInterner, DefaultSymbol};

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
/// use tsv_lang::SymbolResolver;
///
/// struct MyPrinter<'a> {
///     interner: Rc<RefCell<DefaultStringInterner>>,
///     // ... other fields
/// }
///
/// impl<'a> SymbolResolver for MyPrinter<'a> {
///     fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>> {
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
    fn interner(&self) -> &Rc<RefCell<DefaultStringInterner>>;

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
            .resolve(symbol)
            .expect("Symbol not found in interner")
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
        let s = interner
            .resolve(symbol)
            .expect("Symbol not found in interner");
        f(s)
    }
}
