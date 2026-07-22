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
//! This is the checker's **own** interner — a small hand-rolled table over the
//! crate's `FxHashMap` (no external interning crate; the parser is span-identity
//! and holds no interner). The reserved internal names tsgo mangles (`"default"`,
//! `"export="`, `"__constructor"`, ambient-module `"name"`, private `\xFE#…`)
//! intern through the same table on demand; the hot reserved ones are pre-interned
//! so their [`Atom`]s are `const`-cheap to compare.
//
// tsgo: internal/ast/symbol.go InternalSymbolName* (the mangled reserved names)

use crate::hash::FxHashMap;

/// A dense interned name identity, valid within one file's bind.
///
/// Equal atoms mean equal names — the symbol-table key. Wraps a `u32` index so
/// distinct-name lookups are integer compares. Atoms from different files never
/// compare (each `bind_file` has its own interner).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Atom(u32);

/// The binder's per-file name interner (its own hand-rolled table).
pub struct Atoms {
    /// `Atom` index → owned name (the resolve channel). Owned rather than
    /// source-borrowed so bind products stay relocatable across programs/folds.
    names: Vec<Box<str>>,
    /// name → `Atom` (the get-or-intern channel).
    lookup: FxHashMap<Box<str>, Atom>,
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
        let mut atoms = Atoms {
            names: Vec::new(),
            lookup: FxHashMap::default(),
            default: Atom(0),
            export_equals: Atom(0),
        };
        atoms.default = atoms.intern("default");
        atoms.export_equals = atoms.intern("export=");
        atoms
    }

    /// Intern a name to its [`Atom`] — one string hash, then integer identity.
    pub fn intern(&mut self, name: &str) -> Atom {
        if let Some(&atom) = self.lookup.get(name) {
            return atom;
        }
        let atom = Atom(self.names.len() as u32);
        let owned: Box<str> = name.into();
        self.names.push(owned.clone());
        self.lookup.insert(owned, atom);
        atom
    }

    /// Resolve an [`Atom`] back to its name (for diagnostic display).
    #[must_use]
    pub fn resolve(&self, atom: Atom) -> &str {
        // Sound: every `Atom` was minted by this interner's `intern`.
        self.names.get(atom.0 as usize).map_or("", |name| &**name)
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
