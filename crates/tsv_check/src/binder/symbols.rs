//! Symbols, symbol flags, and symbol tables — the binder's substrate.
//!
//! Ported from tsgo's `internal/ast`: [`SymbolFlags`] is the bit table plus the
//! `*Excludes` conflict masks (a construct's flags cannot coexist in one table
//! with any flag in its excludes mask — the whole basis of the duplicate-identifier
//! cascade), reproduced so the merge-vs-conflict verdict matches tsgo by
//! construction. The port covers every flag the binder + merge classify with and
//! every `*Excludes` mask; it deliberately omits the flags no ported path reads —
//! `ConstEnumOnlyModule` (`1 << 28`, only a `getModuleInstanceState` refinement the
//! `module_instantiated` approximation folds away), `GlobalLookup` (`1 << 30`, the
//! name-resolver's global-scope marker), and the convenience composites
//! (`Module`, `ExportHasLocal`, `BlockScoped`, `PropertyOrAccessor`, `ClassMember`,
//! …) whose members are all present. A [`Symbol`] carries its accumulated flags, name [`Atom`], the
//! declaration list the cascade points errors at, and the `members`/`exports`
//! child tables containers own. Tables ([`TableId`] into the binder's pool) are
//! `Atom → SymbolId` maps.
//
// tsgo: internal/ast/symbolflags.go (the bit table + *Excludes masks),
//       internal/ast/symbol.go (Symbol shape)

use crate::binder::atoms::Atom;
use crate::ids::NodeId;
use smallvec::SmallVec;
use tsv_lang::Span;

/// A dense symbol identity into the binder's `symbols` vector.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SymbolId(pub u32);

impl SymbolId {
    /// The 0-based index this id addresses.
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A dense symbol-table identity into the binder's `tables` vector.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TableId(pub u32);

impl TableId {
    /// The 0-based index this id addresses.
    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// tsgo's `SymbolFlags` — a `u32` bitset whose bits classify a declaration and
/// whose `*Excludes` masks (below) decide same-table conflicts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SymbolFlags(pub u32);

// The full flag + `*Excludes` table is ported verbatim from tsgo; some masks are
// not yet referenced by the family cascade but are kept so the port stays a
// faithful, auditable mirror of `symbolflags.go`.
#[allow(clippy::unreadable_literal, dead_code)]
impl SymbolFlags {
    pub const NONE: SymbolFlags = SymbolFlags(0);
    pub const FUNCTION_SCOPED_VARIABLE: SymbolFlags = SymbolFlags(1 << 0);
    pub const BLOCK_SCOPED_VARIABLE: SymbolFlags = SymbolFlags(1 << 1);
    pub const PROPERTY: SymbolFlags = SymbolFlags(1 << 2);
    pub const ENUM_MEMBER: SymbolFlags = SymbolFlags(1 << 3);
    pub const FUNCTION: SymbolFlags = SymbolFlags(1 << 4);
    pub const CLASS: SymbolFlags = SymbolFlags(1 << 5);
    pub const INTERFACE: SymbolFlags = SymbolFlags(1 << 6);
    pub const CONST_ENUM: SymbolFlags = SymbolFlags(1 << 7);
    pub const REGULAR_ENUM: SymbolFlags = SymbolFlags(1 << 8);
    pub const VALUE_MODULE: SymbolFlags = SymbolFlags(1 << 9);
    pub const NAMESPACE_MODULE: SymbolFlags = SymbolFlags(1 << 10);
    pub const TYPE_LITERAL: SymbolFlags = SymbolFlags(1 << 11);
    pub const OBJECT_LITERAL: SymbolFlags = SymbolFlags(1 << 12);
    pub const METHOD: SymbolFlags = SymbolFlags(1 << 13);
    pub const CONSTRUCTOR: SymbolFlags = SymbolFlags(1 << 14);
    pub const GET_ACCESSOR: SymbolFlags = SymbolFlags(1 << 15);
    pub const SET_ACCESSOR: SymbolFlags = SymbolFlags(1 << 16);
    pub const SIGNATURE: SymbolFlags = SymbolFlags(1 << 17);
    pub const TYPE_PARAMETER: SymbolFlags = SymbolFlags(1 << 18);
    pub const TYPE_ALIAS: SymbolFlags = SymbolFlags(1 << 19);
    pub const EXPORT_VALUE: SymbolFlags = SymbolFlags(1 << 20);
    pub const ALIAS: SymbolFlags = SymbolFlags(1 << 21);
    pub const PROTOTYPE: SymbolFlags = SymbolFlags(1 << 22);
    pub const EXPORT_STAR: SymbolFlags = SymbolFlags(1 << 23);
    pub const OPTIONAL: SymbolFlags = SymbolFlags(1 << 24);
    pub const TRANSIENT: SymbolFlags = SymbolFlags(1 << 25);
    pub const ASSIGNMENT: SymbolFlags = SymbolFlags(1 << 26);
    pub const MODULE_EXPORTS: SymbolFlags = SymbolFlags(1 << 27);
    pub const REPLACEABLE_BY_METHOD: SymbolFlags = SymbolFlags(1 << 29);

    pub const ENUM: SymbolFlags = SymbolFlags(Self::REGULAR_ENUM.0 | Self::CONST_ENUM.0);
    pub const VARIABLE: SymbolFlags =
        SymbolFlags(Self::FUNCTION_SCOPED_VARIABLE.0 | Self::BLOCK_SCOPED_VARIABLE.0);
    pub const VALUE: SymbolFlags = SymbolFlags(
        Self::VARIABLE.0
            | Self::PROPERTY.0
            | Self::ENUM_MEMBER.0
            | Self::OBJECT_LITERAL.0
            | Self::FUNCTION.0
            | Self::CLASS.0
            | Self::ENUM.0
            | Self::VALUE_MODULE.0
            | Self::METHOD.0
            | Self::GET_ACCESSOR.0
            | Self::SET_ACCESSOR.0,
    );
    pub const TYPE: SymbolFlags = SymbolFlags(
        Self::CLASS.0
            | Self::INTERFACE.0
            | Self::ENUM.0
            | Self::ENUM_MEMBER.0
            | Self::TYPE_LITERAL.0
            | Self::TYPE_PARAMETER.0
            | Self::TYPE_ALIAS.0,
    );
    pub const ACCESSOR: SymbolFlags = SymbolFlags(Self::GET_ACCESSOR.0 | Self::SET_ACCESSOR.0);
    /// All flags except the `GlobalLookup` sentinel — the `export =` excludes.
    pub const ALL: SymbolFlags = SymbolFlags((1 << 30) - 1);

    // --- *Excludes masks (verbatim from symbolflags.go) ---
    pub const FUNCTION_SCOPED_VARIABLE_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::VALUE.0 & !Self::FUNCTION_SCOPED_VARIABLE.0);
    pub const BLOCK_SCOPED_VARIABLE_EXCLUDES: SymbolFlags = Self::VALUE;
    pub const PARAMETER_EXCLUDES: SymbolFlags = Self::VALUE;
    pub const PROPERTY_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::VALUE.0 & !(Self::PROPERTY.0 | Self::ACCESSOR.0));
    pub const ENUM_MEMBER_EXCLUDES: SymbolFlags = SymbolFlags(Self::VALUE.0 | Self::TYPE.0);
    pub const FUNCTION_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::VALUE.0 & !(Self::FUNCTION.0 | Self::VALUE_MODULE.0 | Self::CLASS.0));
    pub const CLASS_EXCLUDES: SymbolFlags = SymbolFlags(
        (Self::VALUE.0 | Self::TYPE.0)
            & !(Self::VALUE_MODULE.0 | Self::INTERFACE.0 | Self::FUNCTION.0),
    );
    pub const INTERFACE_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::TYPE.0 & !(Self::INTERFACE.0 | Self::CLASS.0));
    pub const REGULAR_ENUM_EXCLUDES: SymbolFlags = SymbolFlags(
        (Self::VALUE.0 | Self::TYPE.0) & !(Self::REGULAR_ENUM.0 | Self::VALUE_MODULE.0),
    );
    pub const CONST_ENUM_EXCLUDES: SymbolFlags =
        SymbolFlags((Self::VALUE.0 | Self::TYPE.0) & !Self::CONST_ENUM.0);
    pub const VALUE_MODULE_EXCLUDES: SymbolFlags = SymbolFlags(
        Self::VALUE.0
            & !(Self::FUNCTION.0 | Self::CLASS.0 | Self::REGULAR_ENUM.0 | Self::VALUE_MODULE.0),
    );
    pub const NAMESPACE_MODULE_EXCLUDES: SymbolFlags = Self::NONE;
    pub const METHOD_EXCLUDES: SymbolFlags = SymbolFlags(Self::VALUE.0 & !Self::METHOD.0);
    pub const GET_ACCESSOR_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::VALUE.0 & !(Self::SET_ACCESSOR.0 | Self::PROPERTY.0));
    pub const SET_ACCESSOR_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::VALUE.0 & !(Self::GET_ACCESSOR.0 | Self::PROPERTY.0));
    pub const ACCESSOR_EXCLUDES: SymbolFlags = SymbolFlags(Self::VALUE.0 & !Self::PROPERTY.0);
    pub const TYPE_PARAMETER_EXCLUDES: SymbolFlags =
        SymbolFlags(Self::TYPE.0 & !Self::TYPE_PARAMETER.0);
    pub const TYPE_ALIAS_EXCLUDES: SymbolFlags = Self::TYPE;
    pub const ALIAS_EXCLUDES: SymbolFlags = Self::ALIAS;

    /// Whether any bit in `other` is set.
    #[inline]
    #[must_use]
    pub const fn intersects(self, other: SymbolFlags) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether every bit in `other` is set.
    #[inline]
    #[must_use]
    pub const fn contains(self, other: SymbolFlags) -> bool {
        self.0 & other.0 == other.0
    }

    /// Set the bits in `other`.
    #[inline]
    pub fn insert(&mut self, other: SymbolFlags) {
        self.0 |= other.0;
    }

    /// The union of two flag sets.
    #[inline]
    #[must_use]
    pub const fn union(self, other: SymbolFlags) -> SymbolFlags {
        SymbolFlags(self.0 | other.0)
    }
}

/// One declaration attached to a symbol: the node's dense id and the source span
/// the cascade points a diagnostic at (the declaration's *name* node, so the
/// squiggle sits on the identifier, matching tsgo's `getNameOfDeclaration`).
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // `node` is the future checker's declaration identity; the family cascade keys on `error_span`.
pub struct Decl {
    /// The declaration node's dense id (best-effort via the address map; not yet
    /// consumed by the cascade, which keys on `error_span`).
    pub node: NodeId,
    /// The span the diagnostic points at (the declaration name, or the node when
    /// it has no name).
    pub error_span: Span,
    /// The display name for the `{0}` message argument (the declaration's text).
    pub display: Atom,
    /// Whether this declaration is a *type* declaration (tsgo `IsTypeDeclaration`:
    /// class / interface / enum / type-alias / type-parameter). The merge phase's
    /// `undefined`-redeclaration check (TS2397) skips type declarations.
    pub is_type_decl: bool,
}

/// A bound symbol: accumulated flags, its table key, its declarations, and the
/// child tables a container owns.
#[derive(Clone, Debug)]
// `parent` mirrors tsgo's `Symbol` shape and is set by the bind but read by nothing
// yet (hence the allow); the cascade + merge resolution read
// `flags`/`name`/`decls`/`members`/`exports`.
#[allow(dead_code)]
pub struct Symbol {
    /// The accumulated classification flags.
    pub flags: SymbolFlags,
    /// The table key (interned name).
    pub name: Atom,
    /// The declarations that formed this symbol (most have one).
    pub decls: SmallVec<[Decl; 1]>,
    /// The `members` table (instance members of a class/interface/type-literal).
    pub members: Option<TableId>,
    /// The `exports` table (static members / module + enum exports).
    pub exports: Option<TableId>,
    /// The parent symbol (the container whose table this symbol lives in);
    /// recorded as tsgo does, unused by the cascade.
    pub parent: Option<SymbolId>,
}

impl Symbol {
    /// A fresh symbol with the given flags and name and no declarations.
    #[must_use]
    pub fn new(flags: SymbolFlags, name: Atom) -> Symbol {
        Symbol {
            flags,
            name,
            decls: SmallVec::new(),
            members: None,
            exports: None,
            parent: None,
        }
    }
}
