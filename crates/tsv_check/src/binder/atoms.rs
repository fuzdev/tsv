//! The binder's program-scoped name interner.
//!
//! Binding needs cross-occurrence name identity: two `x` at different spans must
//! resolve to one symbol-table key. Span-identity identifier names give equality
//! only per occurrence, so at bind time each declared name resolves to a dense
//! [`Atom`] through one interner pass — the common case slices `source[name_span]`
//! (no allocation beyond the interner's own copy), escaped names go through the
//! parser's decoded channel.
//!
//! This is the checker's **own** interner (a fresh `string-interner` instance),
//! not the parser's per-document `SharedInterner` — their tenant lifecycles stay
//! decoupled. The reserved internal names tsgo mangles (`"default"`, `"export="`,
//! `"__constructor"`, ambient-module `"name"`, private `\xFE#…`) intern through
//! the same table on demand; the hot reserved ones are pre-interned so their
//! [`Atom`]s are `const`-cheap to compare.
//
// tsgo: internal/ast/symbol.go InternalSymbolName* (the mangled reserved names)

use string_interner::backend::StringBackend;
use string_interner::symbol::SymbolU32;
use string_interner::{StringInterner, Symbol};

/// A dense, program-scoped interned name identity.
///
/// Equal atoms mean equal names — the symbol-table key. Wraps the interner's
/// `u32` symbol so distinct-name lookups are integer compares.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Atom(u32);

/// The binder's name interner (its own `string-interner` instance).
pub struct Atoms {
    interner: StringInterner<StringBackend<SymbolU32>>,
    /// tsgo's `InternalSymbolNameDefault` — the forced name of every default
    /// export, so multiple default exports collide under one table key.
    default: Atom,
    /// tsgo's `InternalSymbolNameExportEquals` — the `export =` self-merge name.
    export_equals: Atom,
}

impl Atoms {
    /// Build the interner and pre-intern the hot reserved names.
    #[must_use]
    pub fn new() -> Atoms {
        let mut interner = StringInterner::<StringBackend<SymbolU32>>::new();
        let default = Atom(interner.get_or_intern("default").to_usize() as u32);
        let export_equals = Atom(interner.get_or_intern("export=").to_usize() as u32);
        Atoms { interner, default, export_equals }
    }

    /// Intern a name to its [`Atom`].
    pub fn intern(&mut self, name: &str) -> Atom {
        Atom(self.interner.get_or_intern(name).to_usize() as u32)
    }

    /// Resolve an [`Atom`] back to its name (for diagnostic display).
    #[must_use]
    pub fn resolve(&self, atom: Atom) -> &str {
        // Sound: every `Atom` was minted by this interner's `get_or_intern`.
        SymbolU32::try_from_usize(atom.0 as usize)
            .and_then(|sym| self.interner.resolve(sym))
            .unwrap_or("")
    }

    /// The forced-default-export name atom (`"default"`).
    #[must_use]
    pub fn default_export(&self) -> Atom {
        self.default
    }

    /// The `export =` self-merge name atom (`"export="`).
    #[must_use]
    pub fn export_equals(&self) -> Atom {
        self.export_equals
    }
}

impl Default for Atoms {
    fn default() -> Atoms {
        Atoms::new()
    }
}
