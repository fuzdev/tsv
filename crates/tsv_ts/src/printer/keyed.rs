//! The address keys of the printer's memos, typed end to end.
//!
//! Two memos key an AST node by its address in its store: the chain share map
//! ([`Printer::chain_share_key_of`]) and the leading-edge descent's memo (`HeadMemo`). An
//! address is the identity they need — a span cannot stand in, since a node and its sole
//! member can share one — but every AST row is `Copy`, so the address of a COPY is a stack
//! slot, and the next copy (of another node) reuses it: a later lookup then hits the first
//! node's entry and prints the wrong doc or answer, silently, with no type error to catch it.
//!
//! So a key is a [`NodeAddr`], whose field is private to this module: the only way to make
//! one is [`NodeAddr::of`], bounded by the sealed [`Keyed`] trait, which the `keyed_rows!`
//! list below implements for exactly the types some memo keys. That refuses at compile time
//! a `&&Expression` (the address of the reference, not the row — what a `match` on a
//! `&&Expression` binds) and an id slot (the address of the id inside its row, not of the
//! row it names). What the bound cannot see — a stack copy of a keyed type — is the
//! store-range guard's: in debug builds, [`NodeAddr::of`] asserts the address lies in a
//! store row (`AstStores::holds_address`). The pattern is the experimental typechecker's
//! address map's, sealed per crate, since a seal can only be implemented where it lives.

use crate::ast::{AstView, Expression, FunctionExpression, TSType, TSTypeParameterInstantiation};

#[cfg(doc)]
use super::Printer;

/// The address of a keyed AST node, read in place through the parse's stores: a memo key, or
/// its node half. Stable while the stores live; meaningful only against the stores it was
/// taken from — which a memo never outlives (each lives within one outermost chain window or
/// descent).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct NodeAddr(usize);

/// The seal of [`Keyed`]: a trait nothing outside this module can name.
mod sealed {
    pub(crate) trait Sealed {}
}

/// A type whose address a printer memo keys. Sealed, and implemented by `keyed_rows!` for
/// exactly the types some [`NodeAddr::of`] call site keys, so every other referent — a
/// reference (`&&Expression`), an id, any other value — is refused at compile time.
pub(crate) trait Keyed: sealed::Sealed {}

/// Seal and mark each listed AST type as [`Keyed`].
macro_rules! keyed_rows {
    ($($row:ident),* $(,)?) => {
        $(
            impl sealed::Sealed for $row {}
            impl Keyed for $row {}
        )*
    };
}

keyed_rows!(
    Expression,
    FunctionExpression,
    TSType,
    TSTypeParameterInstantiation
);

impl NodeAddr {
    /// The address key of `node`: a reference-to-integer cast (no dereference, so
    /// `unsafe_code = "forbid"` holds).
    ///
    /// `T: Keyed` is the type-level half of the keying rule; the store-range guard is the
    /// runtime half (debug builds): `node` must lie in a row of the stores `view` reads — a
    /// reference an accessor returned, or a field of it — never a copy on the stack.
    ///
    /// The guard is compiled out with `#[cfg]` rather than `debug_assert!`: the dead branch a
    /// release `debug_assert!` leaves in the MIR moved the shipped printer's inlining, and this
    /// key must cost nothing over the raw cast it replaced.
    #[inline]
    pub(super) fn of<T: Keyed>(view: AstView<'_>, node: &T) -> Self {
        let address = std::ptr::from_ref(node) as usize;
        #[cfg(debug_assertions)]
        assert!(
            view.stores().holds_address(address),
            "{address:#x} lies in no store row: a copied AST node was keyed by address"
        );
        #[cfg(not(debug_assertions))]
        let _ = view;
        Self(address)
    }

    /// The address as the integer the doc arena's share map keys by (`tsv_lang`'s
    /// `ShareKey`, which cannot name this crate's AST).
    #[inline]
    pub(super) const fn share_bits(self) -> usize {
        self.0
    }
}
