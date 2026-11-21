//! The take-and-park protocol the language crates keep their stores warm by: a thread-local
//! slot each crate declares for its own store type, the generic take and park halves over it,
//! the free functions' capped park ([`ONE_SHOT_PARK_CAP_BYTES`], applied here alone among
//! the store slots), and [`Warm`], the same protocol over a slot the caller owns.
//!
//! A `thread_local!` cannot be generic, so each crate keeps its own `static PARKED` (a
//! [`Slot`]) and thin wrappers naming it; the protocol is stated once, here.
//!
//! # The protocol
//!
//! A take moves the stores out of the slot, or makes fresh ones when it is empty; a park
//! rewinds them in place ([`Parkable::reset`], keeping capacity) and puts them back. The slot is
//! empty between the two, so a nested call takes fresh stores, and a panic out of the holder
//! leaves the slot empty, as a new thread finds it — no held guard, nothing to restore (the
//! doc arena's slot, `doc::arena::with_doc_arena`, takes and parks for the same reasons). A
//! park drops whatever the slot holds. Every slot access is `LocalKey::try_with`, so a call
//! from a thread-local destructor, once the slot is gone, takes fresh stores and parks nothing.
//! The slot holds a `Box`, so a take and a park each move a pointer where the stores inline
//! would be copied whole both ways.
//!
//! The halves are `#[inline(always)]`, so each crate's wrapper compiles to the body it would
//! have spelled by hand over its own slot.
//!
//! Two policies park over one slot: the bindings park whatever the call grew, uncapped
//! ([`park`]); the free functions park back only stores whose capacity is at most
//! [`ONE_SHOT_PARK_CAP_BYTES`], dropping larger ones ([`park_capped`]).

use std::cell::Cell;
use std::thread::LocalKey;

use super::ONE_SHOT_PARK_CAP_BYTES;

/// A thread's slot for one store type: what a crate's `thread_local!` declares, boxed.
pub type Slot<T> = LocalKey<Cell<Option<Box<T>>>>;

/// Stores a [`Slot`] or a [`Warm`] can park: fresh ones by `Default`, rewound in place by
/// [`Parkable::reset`], and weighed against the free functions' cap by their capacity.
pub trait Parkable: Default {
    /// Rewinds in place, keeping capacity; every handle minted so far is invalid from here on.
    fn reset(&mut self);

    /// The bytes their allocations hold — what parking them retains.
    fn capacity_bytes(&self) -> usize;
}

/// The parked stores, leaving `slot` empty; `None` when it is empty or, for a call from a
/// thread-local destructor, already gone.
#[inline]
fn take_parked<T>(slot: &'static Slot<T>) -> Option<Box<T>> {
    slot.try_with(Cell::take).ok().flatten()
}

/// Puts `stores` in `slot` as they are, dropping whatever it holds; drops `stores` instead
/// when the slot is gone (a call from a thread-local destructor).
#[inline]
fn put_parked<T>(slot: &'static Slot<T>, stores: Box<T>) {
    // a slot already destroyed drops the closure, and `stores` with it
    let _ = slot.try_with(|cell| cell.set(Some(stores)));
}

/// Takes the parked stores out of `slot`, or fresh ones (`Default`) when there are none. The
/// slot is empty until [`park`] hands them back, so a second take meanwhile, and a thread that
/// never parks them (a panic unwinding past the holder), get fresh stores.
#[expect(
    clippy::inline_always,
    reason = "the crate wrapper must compile to the body it spelled by hand over its slot"
)]
#[inline(always)]
#[must_use]
pub fn take<T: Parkable>(slot: &'static Slot<T>) -> Box<T> {
    take_parked(slot).unwrap_or_default()
}

/// Takes the parked stores out of `slot`, or `fresh()` when there are none — [`take`] for a
/// caller that sizes an empty slot's stores itself.
#[expect(
    clippy::inline_always,
    reason = "the crate wrapper must compile to the body it spelled by hand over its slot"
)]
#[inline(always)]
#[must_use]
pub fn take_or<T>(slot: &'static Slot<T>, fresh: impl FnOnce() -> Box<T>) -> Box<T> {
    take_parked(slot).unwrap_or_else(fresh)
}

/// Parks `stores` in `slot` after a [`Parkable::reset`], keeping capacity, so every handle minted
/// from them is invalid from here on. Stores already in the slot are dropped.
#[expect(
    clippy::inline_always,
    reason = "the crate wrapper must compile to the body it spelled by hand over its slot"
)]
#[inline(always)]
pub fn park<T: Parkable>(slot: &'static Slot<T>, mut stores: Box<T>) {
    stores.reset();
    put_parked(slot, stores);
}

/// Parks `stores` as [`park`] does when their capacity is at most
/// [`ONE_SHOT_PARK_CAP_BYTES`], and drops them otherwise — the park half of a free function's
/// call. A drop leaves the slot as the call found it after its take: empty, unless a nested
/// call parked its own stores meanwhile.
#[expect(
    clippy::inline_always,
    reason = "the crate wrapper must compile to the body it spelled by hand over its slot"
)]
#[inline(always)]
pub fn park_capped<T: Parkable>(slot: &'static Slot<T>, stores: Box<T>) {
    if stores.capacity_bytes() <= ONE_SHOT_PARK_CAP_BYTES {
        park(slot, stores);
    }
}

/// The capacity of the stores parked in `slot` ([`Parkable::capacity_bytes`]), or `None` when it
/// holds none (or, for a call from a thread-local destructor, is gone). The stores stay parked.
#[expect(
    clippy::inline_always,
    reason = "the crate wrapper must compile to the body it spelled by hand over its slot"
)]
#[inline(always)]
#[must_use]
pub fn parked_capacity<T: Parkable>(slot: &'static Slot<T>) -> Option<usize> {
    let stores = take_parked(slot)?;
    let bytes = stores.capacity_bytes();
    put_parked(slot, stores);
    Some(bytes)
}

/// Stores a caller keeps warm across calls in a slot it owns, rewound between them: the
/// take-and-park protocol over the caller's slot instead of the thread's, as `tsv_ts`'s and
/// `tsv_css`'s `Formatter` keep their stores. The slot holds a box, so a take and a park each move a
/// pointer.
///
/// A holder that names it through its crate's `WarmStores` alias names no store type, so its
/// code reads the same whatever the stores hold.
#[derive(Debug)]
pub struct Warm<T> {
    parked: Option<Box<T>>,
}

impl<T> Default for Warm<T> {
    fn default() -> Self {
        Self { parked: None }
    }
}

impl<T: Parkable> Warm<T> {
    /// Takes the parked stores out of the slot, or fresh ones when there are none. The slot is
    /// empty until [`Warm::park`] hands them back.
    #[must_use]
    pub fn take(&mut self) -> Box<T> {
        self.parked.take().unwrap_or_default()
    }

    /// Parks `stores` in the slot after a [`Parkable::reset`], keeping capacity, so every
    /// handle minted from them is invalid from here on.
    pub fn park(&mut self, mut stores: Box<T>) {
        stores.reset();
        self.parked = Some(stores);
    }

    /// Run `f` over the parked stores, or over fresh ones when there are none, and park them
    /// after a [`Parkable::reset`], keeping capacity, so every handle `f` minted is invalid
    /// once it returns. A panic out of `f` leaves the slot empty, and the next call starts
    /// from fresh stores.
    pub fn with<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut stores = self.take();
        let result = f(&mut stores);
        self.park(stores);
        result
    }
}
