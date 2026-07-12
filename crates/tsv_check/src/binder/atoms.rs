//! The binder's per-file name interner.
//!
//! Binding needs cross-occurrence name identity: two `x` at different spans must
//! resolve to one symbol-table key. Span-identity identifier names give equality
//! only per occurrence, so at bind time each declared name resolves to a dense
//! [`Atom`] through one interner pass — the common case slices `source[name_span]`
//! (no allocation beyond the interner's own copy), escaped names go through the
//! parser's decoded channel.
//!
//! **Scope: one file's bind.** Each `bind_file` runs a fresh instance, so an
//! [`Atom`] is comparable only within its own file — a deliberate deviation from
//! a program-scoped interner, keeping every bind product program-independent
//! (relocatable/cacheable across programs and lib folds). Cross-file name
//! identity is reconciled at merge time, today by resolving atoms to owned name
//! strings in `FileMerge`; a merge-time atom-remap table (old→new integer map)
//! is the planned replacement when multi-file volume makes the string bridge
//! measurable.
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

/// A dense interned name identity, valid within one file's bind.
///
/// Equal atoms mean equal names — the symbol-table key. Wraps the interner's
/// `u32` symbol so distinct-name lookups are integer compares. Atoms from
/// different files never compare (each `bind_file` has its own interner).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Atom(u32);

/// The binder's per-file name interner (its own `string-interner` instance).
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
        Atoms {
            interner,
            default,
            export_equals,
        }
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
