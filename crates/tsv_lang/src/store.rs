//! Contiguous stores of parsed rows, the typed handles that index them, and the scratch stack
//! that builds their child lists.
//!
//! A [`Store`] is a `Vec` holding the rows of one family. Slot 0 of a written store holds a row
//! that no id names, so an [`Id`] carries its row index directly: `Option<Id>` takes its niche
//! from `NonZero`, and a read decodes nothing. A store allocates nothing until its first write,
//! which writes its first row twice, into slot 0 and slot 1, so no row type needs a sentinel
//! value and a store a document never writes costs nothing. Rows are immutable once pushed.
//! The API offers no way to edit a row, and no way to walk one: a store also holds rows the
//! parse left orphaned, so the tree is what is reachable from its root, never the store's
//! contents.
//!
//! A [`Run`] names consecutive rows. [`ListBuf`] builds a run: list elements go on a scratch
//! stack, one shared by every open list of the same element type, and a flush copies one
//! finished frame into the store. A frame is a [`ListMark`] held in a local of the code building
//! the list, so nested lists are last-in-first-out by construction and nothing counts depth.
//! A rewind needs only the mark taken before the speculation. A mark is typed by its family, so
//! a mark of one buffer cannot be handed to another's.
//!
//! Ids and runs are parameterized by a family tag, not by the row type, because a row may carry
//! a lifetime that the rows and stores holding the handle cannot name. A family is a
//! `'static` type, stated once per row type through [`Row`]; see there for the choices.
//!
//! [`TextPool`] holds strings a span of the source cannot express. A [`PoolText`] is its handle,
//! and `Option<PoolText>` tells an absent string from a present empty one.
//!
//! Handles are only meaningful to the store that minted them, until it resets. The type system
//! catches a handle of the wrong family and nothing more: a same-family id retained across a
//! [`Store::reset`], or minted by another store, reads some other row, in bounds. The
//! `_internal_store_check` feature catches both, by stamping an instance and a generation on
//! stores, pools and the handles they mint and comparing them on each read and each equality
//! test. It also keeps a shadow stack of live marks on each `ListBuf`, so a mark already flushed,
//! rewound past or reset fails at the flush, truncate or frame read it reaches. It widens every
//! handle and changes layout and codegen, so a build carrying it never supplies a performance
//! number. Off, the stamps do not exist: no field, no comparison.
//!
//! Every failed read ends in one zero-argument `#[cold]` function, never in `[]`, so that an
//! inlined accessor carries no panic setup of its own at each site.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::NonZeroU32;
#[cfg(all(target_pointer_width = "64", not(feature = "_internal_store_check")))]
use std::num::NonZeroU64;

/// Whether this build carries the `_internal_store_check` stamps, for a crate that pins a handle's
/// width without a feature of its own: `assert!(STORE_CHECK || size_of::<X>() == N)`.
pub const STORE_CHECK: bool = cfg!(feature = "_internal_store_check");

/// One past the last usable slot of a store or pool, so that every run's exclusive end index
/// still fits a `u32`.
const SLOT_LIMIT: usize = u32::MAX as usize;

/// The bytes, in whole rows, a store first written by a flush reserves on the cold arm that
/// primes it ([`Store::prime_range`]), so a one-shot document's store takes its first lists
/// without growing one doubling at a time, and the hot path pays nothing for the sizing. A
/// store whose capacity already holds the request, a recycled one usually, keeps it untouched.
/// Neither a smaller nor a larger first allocation measured better on the one-shot path.
const FIRST_ALLOC_BYTES: usize = 1024;

/// [`FIRST_ALLOC_BYTES`] in rows of `T`; a zero-sized row counts as one byte, though a `Vec`
/// of one never allocates.
fn first_alloc_rows<T>() -> usize {
    FIRST_ALLOC_BYTES / size_of::<T>().max(1)
}

/// The sentinel at byte 0 of a [`TextPool`], whose offset no span can name.
const POOL_SENTINEL: char = '\0';

/// A read through a handle its store does not hold. Shared by every accessor, zero-argument
/// so the failure path costs an inlined site one call and no argument setup.
#[cold]
#[inline(never)]
#[expect(
    clippy::panic,
    reason = "a handle that misses is an internal invariant broken"
)]
fn store_miss() -> ! {
    panic!("store miss: a handle read past the rows of its store");
}

/// A mint past the `u32` index space, or a slot-count arithmetic overflow.
#[cold]
#[inline(never)]
#[expect(
    clippy::panic,
    reason = "refusing to wrap the index space is the guard's whole job"
)]
fn store_overflow() -> ! {
    panic!("store overflow: more rows than a u32 index can name");
}

/// The id of a row about to be pushed at slot `len`, or `None` when slot `len` cannot take
/// one: past the last index, or slot 0, which a store's first write fills instead.
///
/// The guard is the mint's only arithmetic, kept beside the store rather than inlined in
/// `push` so a test can reach the top of the index space without allocating it. Both refusals
/// share one answer so a push pays no compare beyond the two the guard needs: the caller's
/// one cold arm tells a store never written from one past the top.
#[inline]
fn mint_index(len: usize) -> Option<NonZeroU32> {
    match NonZeroU32::new(len as u32) {
        Some(index) if len < SLOT_LIMIT => Some(index),
        _ => None,
    }
}

/// The start and length of `count` rows appended behind `len` existing slots, or `None` when
/// the run would not fit the index space or would start at slot 0, which a store's first
/// write fills instead.
///
/// Every run a store holds has an end that fits a `u32`, which is what keeps a later
/// `start + len` from overflowing a wasm32 `usize`.
#[inline]
fn mint_range(len: usize, count: usize) -> Option<(NonZeroU32, u32)> {
    let fits = matches!(len.checked_add(count), Some(end) if end <= SLOT_LIMIT);
    match NonZeroU32::new(len as u32) {
        Some(start) if fits => Some((start, count as u32)),
        _ => None,
    }
}

/// `Vec<T>`'s allocation as a `Vec<U>`, emptied. Safe in-place collection: reuse is std's
/// specialization and not its promise (a unit test pins it), so a toolchain without it costs a
/// reallocation, never correctness. A lifetime change is the only intended use, so the two
/// layouts must agree, and a mismatch fails the build.
fn recycle_vec<T, U>(mut items: Vec<T>) -> Vec<U> {
    const { assert!(size_of::<T>() == size_of::<U>() && align_of::<T>() == align_of::<U>()) };
    items.clear();
    items.into_iter().map(|_| -> U { store_miss() }).collect()
}

/// The instance and generation stamped on stores and their handles.
#[cfg(feature = "_internal_store_check")]
mod stamp {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    static NEXT_INSTANCE: AtomicU32 = AtomicU32::new(1);
    static NEXT_MARK: AtomicU64 = AtomicU64::new(1);

    #[derive(Clone, Copy)]
    pub(super) struct Stamp {
        instance: u32,
        generation: u32,
    }

    impl Stamp {
        /// The stamp of a handle no store minted: the empty run and span, which an empty read
        /// answers before the check.
        pub(super) const NONE: Self = Self {
            instance: 0,
            generation: 0,
        };

        pub(super) fn fresh() -> Self {
            Self {
                instance: NEXT_INSTANCE.fetch_add(1, Ordering::Relaxed),
                generation: 1,
            }
        }

        pub(super) fn next_generation(self) -> Self {
            Self {
                instance: self.instance,
                generation: self.generation.wrapping_add(1),
            }
        }

        #[inline]
        pub(super) fn check(self, handle: Self) {
            if self.instance != handle.instance || self.generation != handle.generation {
                stamp_mismatch();
            }
        }

        /// Asserts two handles compared for equality come from one store and generation, a
        /// handle no store minted comparing with anything.
        #[inline]
        pub(super) fn check_comparable(self, other: Self) {
            let both_minted = self.instance != 0 && other.instance != 0;
            if both_minted
                && (self.instance != other.instance || self.generation != other.generation)
            {
                stamp_mismatch();
            }
        }
    }

    /// The live marks of one `ListBuf`, oldest first. The mutation is interior because taking a
    /// mark reads the buffer, as the parser's own checkpoint does.
    #[derive(Default)]
    pub(super) struct LiveMarks {
        stack: Cell<Vec<u64>>,
    }

    impl LiveMarks {
        /// Opens a mark and returns its sequence number, unique across every buffer.
        pub(super) fn open(&self) -> u64 {
            let seq = NEXT_MARK.fetch_add(1, Ordering::Relaxed);
            let mut live = self.stack.take();
            live.push(seq);
            self.stack.set(live);
            seq
        }

        /// A flush: requires the mark, and pops it and every abandoned mark above it.
        pub(super) fn pop_through(&self, seq: u64) {
            let mut live = self.stack.take();
            while let Some(top) = live.pop() {
                if top == seq {
                    self.stack.set(live);
                    return;
                }
            }
            stale_mark();
        }

        /// A read of an open frame: requires the mark and pops nothing.
        pub(super) fn require(&self, seq: u64) {
            let live = self.stack.take();
            let found = live.contains(&seq);
            self.stack.set(live);
            if !found {
                stale_mark();
            }
        }

        /// A rewind: requires the mark and pops the marks above it, leaving it live, since a
        /// checkpoint can be rewound to more than once.
        pub(super) fn pop_above(&self, seq: u64) {
            let mut live = self.stack.take();
            let Some(at) = live.iter().rposition(|&mark| mark == seq) else {
                stale_mark();
            };
            live.truncate(at + 1);
            self.stack.set(live);
        }

        pub(super) fn clear(&self) {
            self.stack.take();
        }
    }

    #[cold]
    #[inline(never)]
    #[expect(
        clippy::panic,
        reason = "the audit build's whole job is to fail loudly here"
    )]
    fn stale_mark() -> ! {
        panic!("stale list mark: its frame was already flushed, rewound past or reset");
    }

    #[cold]
    #[inline(never)]
    #[expect(
        clippy::panic,
        reason = "the audit build's whole job is to fail loudly here"
    )]
    fn stamp_mismatch() -> ! {
        panic!("store stamp mismatch: a handle from another instance or generation");
    }
}

/// The family of a row type: the `'static` tag its [`Id`]s and [`Run`]s are parameterized by.
///
/// A row that carries a lifetime names a family tag of its own, an uninhabited `enum`:
///
/// ```ignore
/// pub enum Expr {}
/// impl Row for Expression<'_> { type Family = Expr; }
/// pub type ExprId = Id<Expr>;
/// ```
///
/// A lifetime-free row needs no tag and is its own family, so a list of ids stores in
/// `Store<ExprId>` and is read through a `Run<ExprId>`, and a list with holes through
/// `Run<Option<ExprId>>`, with no parallel API.
///
/// One family belongs to exactly one row type, lifetimes aside. The type system cannot stop two
/// row types claiming the same tag, and an id of one would then read the other's store.
pub trait Row {
    /// The tag; `'static`, so no handle can name a row's lifetime.
    type Family: 'static;
}

impl<F: 'static> Row for Id<F> {
    type Family = Self;
}

impl<T: Row> Row for Option<T> {
    type Family = Option<T::Family>;
}

impl Row for PoolText {
    type Family = Self;
}

/// A typed handle to one row of a [`Store`], minted by [`Store::push`]: the row's index,
/// `Option<Id<F>>` keeping the width of an `Id<F>`.
pub struct Id<F> {
    index: NonZeroU32,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
    family: PhantomData<fn() -> F>,
}

impl<F> Id<F> {
    /// The row index this id carries; a store's first row is `1`.
    #[inline]
    pub fn index(self) -> usize {
        self.index.get() as usize
    }
}

// Not derived: a derive would bound each impl on `F`, which is an uninhabited tag.
impl<F> Clone for Id<F> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Id<F> {}

impl<F> PartialEq for Id<F> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check_comparable(other.stamp);
        self.index == other.index
    }
}

impl<F> Eq for Id<F> {}

impl<F> Hash for Id<F> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<F> fmt::Debug for Id<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.index)
    }
}

/// An [`Id`] carried as a function's success payload, widened on 64-bit so that a
/// `Result<IdRet<F>, Error>` returns in two registers where a `Result<Id<F>, Error>`, with
/// mismatched payload widths, returns through memory. Elsewhere, and in a checked build, an
/// `Id<F>` as it stands.
#[repr(transparent)]
pub struct IdRet<F> {
    #[cfg(all(target_pointer_width = "64", not(feature = "_internal_store_check")))]
    word: NonZeroU64,
    #[cfg(not(all(target_pointer_width = "64", not(feature = "_internal_store_check"))))]
    id: Id<F>,
    family: PhantomData<fn() -> F>,
}

#[cfg(all(target_pointer_width = "64", not(feature = "_internal_store_check")))]
impl<F> IdRet<F> {
    /// Widens `id` to the return word.
    #[inline]
    pub fn wrap(id: Id<F>) -> Self {
        Self {
            word: id.index.into(),
            family: PhantomData,
        }
    }

    /// The id this carries. `wrap` is the only source of the word, so the fallible arm is
    /// never taken.
    #[inline]
    pub fn id(self) -> Id<F> {
        match NonZeroU32::try_from(self.word) {
            Ok(index) => Id {
                index,
                family: PhantomData,
            },
            Err(_) => store_miss(),
        }
    }
}

#[cfg(not(all(target_pointer_width = "64", not(feature = "_internal_store_check"))))]
impl<F> IdRet<F> {
    /// Wraps `id` unchanged.
    #[inline]
    pub fn wrap(id: Id<F>) -> Self {
        Self {
            id,
            family: PhantomData,
        }
    }

    /// The id this carries.
    #[inline]
    pub fn id(self) -> Id<F> {
        self.id
    }
}

impl<F> Clone for IdRet<F> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for IdRet<F> {}

/// Consecutive rows of a [`Store`], minted by a [`ListBuf`] flush and read as a plain slice
/// by [`Store::slice`]. `Option<Run<F>>` keeps the width of a `Run<F>`.
pub struct Run<F> {
    start: NonZeroU32,
    len: u32,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
    family: PhantomData<fn() -> F>,
}

impl<F> Run<F> {
    /// The empty run. A read of it answers empty before it reaches the rows, so it reads in
    /// every store, one never written included.
    pub const EMPTY: Self = Self {
        start: NonZeroU32::MIN,
        len: 0,
        #[cfg(feature = "_internal_store_check")]
        stamp: stamp::Stamp::NONE,
        family: PhantomData,
    };

    /// The number of rows in the run.
    #[inline]
    pub fn len(self) -> usize {
        self.len as usize
    }

    /// Whether the run holds no rows.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The run's slot range. A run's end fits a `u32`, per [`mint_range`], so the addition
    /// cannot overflow a wasm32 `usize`.
    #[inline]
    fn range(self) -> std::ops::Range<usize> {
        let start = self.start.get() as usize;
        start..start + self.len as usize
    }
}

impl<F> Clone for Run<F> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for Run<F> {}

impl<F> PartialEq for Run<F> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check_comparable(other.stamp);
        self.start == other.start && self.len == other.len
    }
}

impl<F> Eq for Run<F> {}

impl<F> fmt::Debug for Run<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Run({}, {})", self.start, self.len)
    }
}

/// A contiguous store of rows, keeping its capacity across [`Store::reset`].
///
/// Slot 0 holds a row no id names. A store holds no rows until its first write, which writes
/// its first row twice, into slot 0 and slot 1; ids are 1-based either way.
pub struct Store<T> {
    rows: Vec<T>,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
}

impl<T: Row> Default for Store<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Row> Store<T> {
    /// An empty store. Allocates nothing until its first write.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// An empty store pre-sized for `rows` slots, slot 0 among them.
    pub fn with_capacity(rows: usize) -> Self {
        Self {
            rows: Vec::with_capacity(rows),
            #[cfg(feature = "_internal_store_check")]
            stamp: stamp::Stamp::fresh(),
        }
    }

    /// Appends `row` and mints its id. On a store's first write `row` lands in slot 0 and its
    /// clone in slot 1, the slot the id names.
    #[inline]
    pub fn push(&mut self, row: T) -> Id<T::Family>
    where
        T: Clone,
    {
        let index = mint_index(self.rows.len());
        self.rows.push(row);
        let index = match index {
            Some(index) => index,
            None => self.prime_after_first_push(),
        };
        Id {
            index,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp,
            family: PhantomData,
        }
    }

    /// The row `id` names. A miss ends in the shared cold path.
    #[inline]
    pub fn get(&self, id: Id<T::Family>) -> &T {
        self.view().get(id)
    }

    /// The rows of `run` as a plain slice, with one range check for the whole run.
    #[inline]
    pub fn slice(&self, run: Run<T::Family>) -> &[T] {
        self.view().slice(run)
    }

    /// A read-only view of the rows: the bound slice every read of this store goes through.
    #[inline]
    pub fn view(&self) -> StoreView<'_, T> {
        StoreView {
            rows: &self.rows,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp,
        }
    }

    /// Discards every row but slot 0, keeping capacity. Every id and run minted so far is
    /// invalid from here on.
    #[inline]
    pub fn reset(&mut self) {
        self.rows.truncate(1);
        #[cfg(feature = "_internal_store_check")]
        {
            self.stamp = self.stamp.next_generation();
        }
    }

    /// Reuses this store's capacity for a store of the same family under another lifetime,
    /// which lets a store be parked in a `'static` slot between parses and taken back.
    ///
    /// The recycled store holds no rows, slot 0 included, since its old row is of the wrong
    /// lifetime: its next write fills slot 0 again, as a new store's first does. Every id and
    /// run minted so far is invalid from here on, as across a reset.
    pub fn recycle<U>(self) -> Store<U>
    where
        U: Row<Family = T::Family>,
    {
        Store {
            rows: recycle_vec(self.rows),
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp.next_generation(),
        }
    }

    /// The refused arm of a push's mint, taken after the row is appended so the row itself
    /// never has its address taken: a store whose only row is the one just pushed (it was not
    /// yet written) appends a clone of it, so slot 0 holds the original and index 1 its clone;
    /// a store already written is past the last index. Out of line and cold, reading only the
    /// store, so a push's inlined tail keeps the base shape: the row is stored straight into
    /// its slot, and nothing is held across a call.
    #[cold]
    #[inline(never)]
    fn prime_after_first_push(&mut self) -> NonZeroU32
    where
        T: Clone,
    {
        let first = match self.rows.as_slice() {
            [only] => only.clone(),
            _ => store_overflow(),
        };
        self.rows.push(first);
        NonZeroU32::MIN
    }

    /// The refused arm of a flush's mint, as [`Store::prime_after_first_push`] is a push's: a
    /// store not yet written fills slot 0 with a clone of `first`, the frame's first row, and
    /// mints the run behind it; a store already written, or a run that cannot fit, overflows.
    /// The store's first allocation is sized here, to [`FIRST_ALLOC_BYTES`] of rows or the
    /// run and its slot 0, whichever is more; capacity that already holds that is kept.
    #[cold]
    #[inline(never)]
    fn prime_range(&mut self, first: Option<&T>, count: usize) -> (NonZeroU32, u32)
    where
        T: Clone,
    {
        let Some(first) = first.filter(|_| self.rows.is_empty()) else {
            store_overflow();
        };
        // the run starts behind slot 0; minted before the reserve, so a run past the index
        // space never asks the allocator for its rows, and `count + 1` fits a `u32`
        let Some(range) = mint_range(1, count) else {
            store_overflow();
        };
        self.rows.reserve(first_alloc_rows::<T>().max(count + 1));
        self.rows.push(first.clone());
        range
    }

    /// Mints the run that `count` rows about to be appended will occupy, priming slot 0 first
    /// from the row `first` names when the store is not yet written. `first` is asked only
    /// then.
    #[inline]
    fn mint_run<'r>(
        &mut self,
        count: usize,
        first: impl FnOnce() -> Option<&'r T>,
    ) -> Run<T::Family>
    where
        T: Clone + 'r,
    {
        let (start, len) = match mint_range(self.rows.len(), count) {
            Some(range) => range,
            None => self.prime_range(first(), count),
        };
        Run {
            start,
            len,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp,
            family: PhantomData,
        }
    }
}

/// A read-only view of a [`Store`]'s rows, minted by [`Store::view`].
pub struct StoreView<'a, T> {
    rows: &'a [T],
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
}

impl<T> Clone for StoreView<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for StoreView<'_, T> {}

impl<'a, T: Row> StoreView<'a, T> {
    /// The row `id` names. A miss ends in the shared cold path.
    #[inline]
    pub fn get(self, id: Id<T::Family>) -> &'a T {
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check(id.stamp);
        match self.rows.get(id.index.get() as usize) {
            Some(row) => row,
            None => store_miss(),
        }
    }

    /// The rows of `run` as a plain slice, with one range check for the whole run.
    #[inline]
    pub fn slice(self, run: Run<T::Family>) -> &'a [T] {
        if run.len == 0 {
            return &[];
        }
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check(run.stamp);
        match self.rows.get(run.range()) {
            Some(rows) => rows,
            None => store_miss(),
        }
    }
}

/// A position on a [`ListBuf`]: the start of one open list frame, or the point a rewind returns
/// to. Held in a local of the code building the list. Parameterized by the family so a mark of
/// one list buffer cannot be handed to another's.
#[must_use]
pub struct ListMark<F> {
    top: u32,
    #[cfg(feature = "_internal_store_check")]
    seq: u64,
    family: PhantomData<fn() -> F>,
}

impl<F> ListMark<F> {
    #[inline]
    fn index(self) -> usize {
        self.top as usize
    }
}

impl<F> Clone for ListMark<F> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<F> Copy for ListMark<F> {}

impl<F> fmt::Debug for ListMark<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ListMark({})", self.top)
    }
}

/// One overflow check for the marks of a group of [`ListBuf`]s, rather than one per buffer:
/// [`ListBuf::mark_in`] marks a buffer and folds its height into the group, and
/// [`MarkGroup::check`] then checks every folded height at once.
///
/// A mark holds its buffer's height as a `u32`, so a height past `u32::MAX` must end in the
/// overflow path rather than truncate. The group keeps the bitwise OR of its heights, which is
/// past `u32::MAX` exactly when one of the heights is: a height fits a `u32` exactly when its
/// bits above the 32nd are all clear, and the OR clears those bits only when every height
/// does. On a 32-bit target every height fits and the check folds away.
///
/// A mark taken into a group is unchecked until the group is: the caller hands no mark on
/// before [`MarkGroup::check`] returns, and a debug build also checks each mark as it is taken.
/// Checking after the marks rather than before lets each height be read, folded and stored in
/// one pass, rather than held across the check.
#[must_use]
pub struct MarkGroup {
    heights: usize,
}

impl MarkGroup {
    /// An empty group.
    #[inline]
    pub const fn new() -> Self {
        Self { heights: 0 }
    }

    /// Ends in the store's overflow path when a height folded into the group is past
    /// `u32::MAX`, which makes every mark taken into it a faithful one.
    #[inline]
    pub fn check(self) {
        if u32::try_from(self.heights).is_err() {
            store_overflow();
        }
    }
}

impl Default for MarkGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// The scratch stack that builds child lists: one buffer shared by every open frame of the same
/// element type, each frame flushing as one contiguous run.
///
/// Under the `_internal_ast_census` feature, every change to the stack's length moves the
/// census's open-list total of `T` (`ast_census_probe::list_delta`): a push adds one, and a
/// flush, a truncate, a reset or a recycle takes back what it removes.
pub struct ListBuf<T> {
    items: Vec<T>,
    #[cfg(feature = "_internal_store_check")]
    live: stamp::LiveMarks,
}

impl<T> Default for ListBuf<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ListBuf<T> {
    /// An empty scratch stack.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// An empty scratch stack pre-sized for `items` elements.
    pub fn with_capacity(items: usize) -> Self {
        Self {
            items: Vec::with_capacity(items),
            #[cfg(feature = "_internal_store_check")]
            live: stamp::LiveMarks::default(),
        }
    }

    /// Pushes an element onto the frame in progress.
    #[inline]
    pub fn push(&mut self, item: T) {
        self.items.push(item);
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(1);
    }

    /// Empties the stack between parses, keeping capacity.
    #[inline]
    pub fn reset(&mut self) {
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(-(self.items.len() as isize));
        self.items.clear();
        #[cfg(feature = "_internal_store_check")]
        self.live.clear();
    }

    /// Reuses this buffer's capacity under another element type, as [`Store::recycle`] does.
    pub fn recycle<U>(self) -> ListBuf<U> {
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(-(self.items.len() as isize));
        ListBuf {
            items: recycle_vec(self.items),
            #[cfg(feature = "_internal_store_check")]
            live: stamp::LiveMarks::default(),
        }
    }
}

impl<T: Row> ListBuf<T> {
    /// Opens a frame at the current top of the stack.
    #[inline]
    pub fn mark(&self) -> ListMark<T::Family> {
        let mut group = MarkGroup::new();
        let mark = self.mark_in(&mut group);
        group.check();
        mark
    }

    /// Opens a frame at the current top of the stack, its height left for `group` to check (see
    /// [`MarkGroup`]): the mark is faithful only once [`MarkGroup::check`] has returned, so hand
    /// it on after that, never before.
    #[inline]
    pub fn mark_in(&self, group: &mut MarkGroup) -> ListMark<T::Family> {
        let height = self.items.len();
        debug_assert!(
            u32::try_from(height).is_ok(),
            "store overflow: more rows than a u32 index can name"
        );
        group.heights |= height;
        ListMark {
            top: height as u32,
            #[cfg(feature = "_internal_store_check")]
            seq: self.live.open(),
            family: PhantomData,
        }
    }

    /// The number of elements above `mark`. A mark above the top of the stack misses. A stale
    /// mark at or below the top is not detected here; the check build catches it at the
    /// flush, truncate or frame read it reaches.
    #[inline]
    pub fn len_since(&self, mark: ListMark<T::Family>) -> usize {
        match self.items.len().checked_sub(mark.index()) {
            Some(count) => count,
            None => store_miss(),
        }
    }

    /// The elements above `mark`: the open frame it starts, read while the frame is still being
    /// built, as [`ListBuf::len_since`] counts it. A mark above the top of the stack misses; the
    /// check build also refuses a stale one, as [`ListBuf::truncate`] does.
    #[inline]
    pub fn frame(&self, mark: ListMark<T::Family>) -> &[T] {
        #[cfg(feature = "_internal_store_check")]
        self.live.require(mark.seq);
        match self.items.get(mark.index()..) {
            Some(frame) => frame,
            None => store_miss(),
        }
    }

    /// Rewinds the stack to `mark`, discarding every element above it. `mark` stays usable, so
    /// a checkpoint can be rewound to more than once.
    #[inline]
    pub fn truncate(&mut self, mark: ListMark<T::Family>) {
        #[cfg(feature = "_internal_store_check")]
        self.live.pop_above(mark.seq);
        #[cfg(not(feature = "_internal_ast_census"))]
        let _ = self.len_since(mark);
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(-(self.len_since(mark) as isize));
        self.items.truncate(mark.index());
    }

    /// Copies the frame above `mark` into `into` and truncates the stack back to `mark`. An
    /// empty frame returns [`Run::EMPTY`] and does not touch `into`.
    #[inline]
    pub fn flush_copy(&mut self, mark: ListMark<T::Family>, into: &mut Store<T>) -> Run<T::Family>
    where
        T: Copy,
    {
        #[cfg(feature = "_internal_store_check")]
        self.live.pop_through(mark.seq);
        let count = self.len_since(mark);
        if count == 0 {
            return Run::EMPTY;
        }
        let Some(frame) = self.items.get(mark.index()..) else {
            store_miss();
        };
        let run = into.mint_run(count, || frame.first());
        into.rows.extend_from_slice(frame);
        self.items.truncate(mark.index());
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(-(count as isize));
        run
    }

    /// Moves the frame above `mark` into `into` and truncates the stack back to `mark`. An
    /// empty frame returns [`Run::EMPTY`] and does not touch `into`.
    #[inline]
    pub fn flush_move(&mut self, mark: ListMark<T::Family>, into: &mut Store<T>) -> Run<T::Family>
    where
        T: Clone,
    {
        #[cfg(feature = "_internal_store_check")]
        self.live.pop_through(mark.seq);
        let count = self.len_since(mark);
        if count == 0 {
            return Run::EMPTY;
        }
        let run = into.mint_run(count, || self.items.get(mark.index()));
        into.rows.extend(self.items.drain(mark.index()..));
        #[cfg(feature = "_internal_ast_census")]
        crate::ast_census_probe::list_delta::<T>(-(count as isize));
        run
    }
}

/// A handle to one string of a [`TextPool`]. `Option<PoolText>` keeps the width of a
/// `PoolText`, and `None` (absent) is distinct from [`PoolText::EMPTY`] (present, empty).
///
/// A span of the source is a `Span`; this is decoded text, which has no source span.
#[derive(Clone, Copy)]
pub struct PoolText {
    start: NonZeroU32,
    len: u32,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
}

impl PoolText {
    /// The present, empty string. Every pool resolves it to `""`, untouched.
    pub const EMPTY: Self = Self {
        start: NonZeroU32::MIN,
        len: 0,
        #[cfg(feature = "_internal_store_check")]
        stamp: stamp::Stamp::NONE,
    };
}

impl PartialEq for PoolText {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check_comparable(other.stamp);
        self.start == other.start && self.len == other.len
    }
}

impl Eq for PoolText {}

impl fmt::Debug for PoolText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PoolText({}, {})", self.start, self.len)
    }
}

/// An append-only pool of UTF-8 strings, byte 0 a sentinel, keeping its capacity across
/// [`TextPool::reset`].
pub struct TextPool {
    text: String,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
}

impl Default for TextPool {
    fn default() -> Self {
        Self::new()
    }
}

impl TextPool {
    /// An empty pool.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// An empty pool pre-sized for `bytes` bytes of text.
    pub fn with_capacity(bytes: usize) -> Self {
        let mut text = String::with_capacity(bytes + POOL_SENTINEL.len_utf8());
        text.push(POOL_SENTINEL);
        Self {
            text,
            #[cfg(feature = "_internal_store_check")]
            stamp: stamp::Stamp::fresh(),
        }
    }

    /// Appends `text` and mints its span. An empty string is [`PoolText::EMPTY`].
    #[inline]
    pub fn push(&mut self, text: &str) -> PoolText {
        if text.is_empty() {
            return PoolText::EMPTY;
        }
        let Some((start, len)) = mint_range(self.text.len(), text.len()) else {
            store_overflow();
        };
        self.text.push_str(text);
        PoolText {
            start,
            len,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp,
        }
    }

    /// The string `span` names. A span from this pool always lands on the character boundaries
    /// it was minted at, and a miss ends in the shared cold path rather than a slice panic.
    #[inline]
    pub fn get(&self, span: PoolText) -> &str {
        if span.len == 0 {
            return "";
        }
        #[cfg(feature = "_internal_store_check")]
        self.stamp.check(span.stamp);
        let start = span.start.get() as usize;
        match self.text.get(start..start + span.len as usize) {
            Some(text) => text,
            None => store_miss(),
        }
    }

    /// Discards every string, keeping capacity. Every span minted so far is invalid from here on.
    #[inline]
    pub fn reset(&mut self) {
        self.text.truncate(POOL_SENTINEL.len_utf8());
        #[cfg(feature = "_internal_store_check")]
        {
            self.stamp = self.stamp.next_generation();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    enum Alpha {}

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Node<'a>(&'a str);

    impl Row for Node<'_> {
        type Family = Alpha;
    }

    fn store() -> Store<Node<'static>> {
        Store::new()
    }

    /// An id with an arbitrary index: only the mint and the round trip read one this way.
    fn id_at(index: u32) -> Id<Alpha> {
        Id {
            index: NonZeroU32::new(index).expect("a test index"),
            #[cfg(feature = "_internal_store_check")]
            stamp: stamp::Stamp::NONE,
            family: PhantomData,
        }
    }

    fn texts<'a>(store: &'a Store<Node<'a>>, run: Run<Alpha>) -> Vec<&'a str> {
        store.slice(run).iter().map(|node| node.0).collect()
    }

    // `const` so a wrong width fails the build. Stamped handles are wider on purpose, and the
    // niches hold either way.
    const _: () = {
        assert!(STORE_CHECK || size_of::<Id<Alpha>>() == 4);
        assert!(STORE_CHECK || size_of::<Run<Alpha>>() == 8);
        assert!(STORE_CHECK || size_of::<PoolText>() == 8);
        assert!(STORE_CHECK || size_of::<ListMark<Alpha>>() == 4);
        #[cfg(target_pointer_width = "64")]
        {
            assert!(STORE_CHECK || size_of::<IdRet<Alpha>>() == 8);
            assert!(STORE_CHECK || size_of::<Result<IdRet<Alpha>, crate::Error>>() == 16);
            assert!(STORE_CHECK || size_of::<StoreView<'static, Id<Alpha>>>() == 16);
            assert!(STORE_CHECK || size_of::<Store<Id<Alpha>>>() == 24);
            assert!(STORE_CHECK || size_of::<ListBuf<Id<Alpha>>>() == 24);
            assert!(STORE_CHECK || size_of::<TextPool>() == 24);
        }
    };

    const _: () = {
        assert!(size_of::<Option<Id<Alpha>>>() == size_of::<Id<Alpha>>());
        assert!(size_of::<Option<Run<Alpha>>>() == size_of::<Run<Alpha>>());
        assert!(size_of::<Option<PoolText>>() == size_of::<PoolText>());
    };

    /// Under the census, the open-list total follows the stack's length: a push adds one, and a
    /// flush, a truncate, a reset and a recycle each take back what they remove, so a nested
    /// frame's peak sits on top of the outer frame's live prefix. A shrink that leaked would lift
    /// every later peak; none mints, the flush's caller does.
    #[cfg(feature = "_internal_ast_census")]
    #[test]
    fn the_census_follows_the_open_prefixes_of_a_scratch_stack() {
        use crate::ast_census_probe;

        ast_census_probe::arm();
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));
        scratch.push(Node("b"));
        let inner = scratch.mark();
        for _ in 0..3 {
            scratch.push(Node("c"));
        }
        // the peak so far: the outer two under the inner three
        scratch.flush_move(inner, &mut nodes);
        scratch.push(Node("d"));
        let speculation = scratch.mark();
        scratch.push(Node("e"));
        scratch.truncate(speculation);
        scratch.flush_copy(outer, &mut nodes);
        for _ in 0..6 {
            scratch.push(Node("f"));
        }
        scratch.reset();
        for _ in 0..7 {
            scratch.push(Node("g"));
        }
        let mut scratch: ListBuf<Node<'static>> = scratch.recycle();
        for _ in 0..8 {
            scratch.push(Node("h"));
        }
        scratch.reset();

        // a recycle into another element type takes the open prefix back from the old one
        scratch.push(Node("i"));
        let mut other: ListBuf<[usize; 2]> = scratch.recycle();
        other.push([0, 0]);
        other.reset();
        let tally = ast_census_probe::disarm();
        let (_, row) = tally
            .rows
            .iter()
            .find(|(name, _)| name.contains("::Node"))
            .expect("a row for the element type");
        assert_eq!(row.scratch_peak, 8, "each shrink took back what it removed");
        assert_eq!(row.minted, 0, "a flush mints nothing of its own");
    }

    #[test]
    fn a_new_store_is_empty_and_allocates_nothing() {
        let nodes = store();
        assert!(nodes.rows.is_empty());
        assert_eq!(nodes.rows.capacity(), 0);
        let nodes: Store<Node<'static>> = Store::default();
        assert_eq!(nodes.rows.capacity(), 0);
        let sized: Store<Node<'static>> = Store::with_capacity(8);
        assert!(sized.rows.is_empty());
        assert_eq!(sized.rows.capacity(), 8);
    }

    #[test]
    fn the_first_push_primes_slot_zero_and_mints_index_one() {
        let mut nodes = store();
        let a = nodes.push(Node("a"));
        assert_eq!(a.index(), 1);
        assert_eq!(
            nodes.rows,
            [Node("a"), Node("a")],
            "the first row is written twice, into slot 0 and slot 1"
        );
        let b = nodes.push(Node("b"));
        assert_eq!(b.index(), 2);
        assert_eq!(*nodes.get(a), Node("a"));
        assert_eq!(*nodes.get(b), Node("b"));
    }

    #[test]
    fn the_first_flush_primes_slot_zero_then_places_its_run_at_one() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        scratch.push(Node("b"));
        let run = scratch.flush_move(mark, &mut nodes);
        assert_eq!((run.start.get(), run.len()), (1, 2));
        assert_eq!(nodes.rows, [Node("a"), Node("a"), Node("b")]);
        assert_eq!(texts(&nodes, run), ["a", "b"]);

        let mut copied = store();
        let mark = scratch.mark();
        scratch.push(Node("c"));
        let run = scratch.flush_copy(mark, &mut copied);
        assert_eq!((run.start.get(), run.len()), (1, 1));
        assert_eq!(copied.rows, [Node("c"), Node("c")]);
        assert_eq!(copied.push(Node("d")).index(), 2);
    }

    #[test]
    fn the_first_flush_reserves_the_first_allocation() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        scratch.push(Node("b"));
        scratch.flush_move(mark, &mut nodes);
        assert!(first_alloc_rows::<Node<'static>>() > 4);
        assert!(nodes.rows.capacity() >= first_alloc_rows::<Node<'static>>());

        let mut ids: Store<Id<Alpha>> = Store::new();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(id_at(1));
        scratch.flush_copy(mark, &mut ids);
        assert!(ids.rows.capacity() >= first_alloc_rows::<Id<Alpha>>());
        assert!(STORE_CHECK || first_alloc_rows::<Id<Alpha>>() == FIRST_ALLOC_BYTES / 4);
    }

    #[test]
    fn a_first_flush_into_enough_capacity_keeps_the_allocation() {
        let wanted = first_alloc_rows::<Node<'static>>() + 8;
        let mut sized: Store<Node<'static>> = Store::with_capacity(wanted);
        let (before, capacity) = (sized.rows.as_ptr(), sized.rows.capacity());
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        scratch.flush_copy(mark, &mut sized);
        assert_eq!(
            (sized.rows.as_ptr(), sized.rows.capacity()),
            (before, capacity)
        );

        // a recycled store is empty, slot 0 included, so its next flush primes again
        let mut recycled: Store<Node<'_>> = sized.recycle();
        let (before, capacity) = (recycled.rows.as_ptr(), recycled.rows.capacity());
        let mark = scratch.mark();
        scratch.push(Node("b"));
        scratch.push(Node("c"));
        let run = scratch.flush_move(mark, &mut recycled);
        assert_eq!(
            (recycled.rows.as_ptr(), recycled.rows.capacity()),
            (before, capacity)
        );
        assert_eq!(texts(&recycled, run), ["b", "c"]);
    }

    #[test]
    fn a_first_flush_into_too_little_capacity_grows_to_the_first_allocation() {
        let mut sized: Store<Node<'static>> = Store::with_capacity(2);
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        scratch.flush_copy(mark, &mut sized);
        assert!(sized.rows.capacity() >= first_alloc_rows::<Node<'static>>());
    }

    // `count + 1` against `count` in the reserve is capacity only: a short reserve grows once
    // more in the extend, and no test can tell that from `reserve`'s own latitude
    #[test]
    fn a_first_flush_past_the_first_allocation_lands_intact() {
        const WORDS: [&str; 5] = ["p", "q", "r", "s", "t"];
        let count = first_alloc_rows::<Node<'static>>() + 5;
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        for index in 0..count {
            scratch.push(Node(WORDS[index % WORDS.len()]));
        }
        let run = scratch.flush_move(mark, &mut nodes);
        assert_eq!((run.start.get(), run.len()), (1, count));
        assert_eq!(nodes.rows.len(), count + 1);
        assert_eq!(
            nodes.rows[0],
            Node("p"),
            "slot 0 holds the frame's first row"
        );
        let read = texts(&nodes, run);
        assert!(
            read.iter()
                .enumerate()
                .all(|(index, text)| *text == WORDS[index % 5])
        );
        assert_eq!(nodes.push(Node("u")).index(), count + 1);
    }

    #[cfg(not(feature = "_internal_store_check"))]
    #[test]
    #[should_panic(expected = "store miss")]
    fn a_read_of_a_store_never_written_misses() {
        store().get(id_at(1));
    }

    #[test]
    fn the_empty_run_reads_empty_in_a_store_never_written() {
        let nodes = store();
        assert_eq!(Run::<Alpha>::EMPTY.start.get(), 1);
        assert!(nodes.rows.is_empty());
        assert_eq!(nodes.slice(Run::EMPTY), &[]);
        assert_eq!(nodes.view().slice(Run::EMPTY), &[]);
        assert!(Run::<Alpha>::EMPTY.is_empty());
        assert_eq!(Run::<Alpha>::EMPTY.len(), 0);
    }

    #[test]
    fn an_empty_frame_returns_the_empty_run_and_leaves_the_store_alone() {
        let mut nodes = store();
        nodes.push(Node("a"));
        let (rows, capacity) = (nodes.rows.len(), nodes.rows.capacity());
        let mut scratch = ListBuf::new();
        let moved = scratch.mark();
        let copied = scratch.mark();
        assert_eq!(scratch.flush_copy(copied, &mut nodes), Run::EMPTY);
        assert_eq!(scratch.flush_move(moved, &mut nodes), Run::EMPTY);
        assert_eq!((nodes.rows.len(), nodes.rows.capacity()), (rows, capacity));
    }

    #[test]
    fn an_empty_frame_leaves_a_store_never_written_unprimed() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let moved = scratch.mark();
        let copied = scratch.mark();
        assert_eq!(scratch.flush_copy(copied, &mut nodes), Run::EMPTY);
        assert_eq!(scratch.flush_move(moved, &mut nodes), Run::EMPTY);
        assert!(nodes.rows.is_empty());
        assert_eq!(nodes.rows.capacity(), 0);
    }

    #[test]
    fn a_run_of_one_reads_back() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("only"));
        let run = scratch.flush_copy(mark, &mut nodes);
        assert_eq!(run.len(), 1);
        assert_eq!(texts(&nodes, run), ["only"]);
        assert_eq!(scratch.len_since(mark), 0);
    }

    #[test]
    fn nested_frames_of_one_type_interleave_and_flush_as_contiguous_runs() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));
        let inner = scratch.mark();
        scratch.push(Node("b"));
        scratch.push(Node("c"));
        assert_eq!(scratch.len_since(inner), 2);
        let inner_run = scratch.flush_move(inner, &mut nodes);
        scratch.push(Node("d"));
        let second_inner = scratch.mark();
        scratch.push(Node("e"));
        let second_run = scratch.flush_copy(second_inner, &mut nodes);
        scratch.push(Node("f"));
        assert_eq!(scratch.len_since(outer), 3);
        let outer_run = scratch.flush_move(outer, &mut nodes);

        assert_eq!(texts(&nodes, inner_run), ["b", "c"]);
        assert_eq!(texts(&nodes, second_run), ["e"]);
        assert_eq!(texts(&nodes, outer_run), ["a", "d", "f"]);
        assert_eq!(scratch.len_since(outer), 0);
    }

    #[test]
    fn a_flush_truncates_the_scratch_to_its_mark() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        scratch.push(Node("below"));
        let mark = scratch.mark();
        scratch.push(Node("x"));
        scratch.flush_copy(mark, &mut nodes);
        assert_eq!(scratch.items, [Node("below")]);

        let mark = scratch.mark();
        scratch.push(Node("y"));
        scratch.flush_move(mark, &mut nodes);
        assert_eq!(scratch.items, [Node("below")]);
    }

    #[test]
    fn an_id_list_uses_the_same_primitives() {
        let mut nodes = store();
        let a = nodes.push(Node("a"));
        let b = nodes.push(Node("b"));
        let mut ids: Store<Id<Alpha>> = Store::new();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(a);
        scratch.push(b);
        let run: Run<Id<Alpha>> = scratch.flush_copy(mark, &mut ids);
        assert_eq!(ids.slice(run), &[a, b]);

        // A list with holes is the same store shape over an `Option` of the id.
        let mut holes: Store<Option<Id<Alpha>>> = Store::new();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Some(a));
        scratch.push(None);
        let run: Run<Option<Id<Alpha>>> = scratch.flush_copy(mark, &mut holes);
        assert_eq!(holes.slice(run), &[Some(a), None]);
    }

    #[test]
    fn a_rewind_truncates_what_the_speculation_pushed() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));

        let rewind = scratch.mark();
        scratch.push(Node("spec1"));
        let _speculated = scratch.mark();
        scratch.push(Node("spec2"));
        scratch.truncate(rewind);
        assert_eq!(scratch.len_since(outer), 1);
        // The frame opened during the speculation is gone with it; a discarded success rewinds
        // the same way, and rewinding at the top is a no-op.
        assert_eq!(scratch.mark().index(), rewind.index());
        scratch.truncate(rewind);
        assert_eq!(scratch.len_since(outer), 1);

        scratch.push(Node("b"));
        let run = scratch.flush_move(outer, &mut nodes);
        assert_eq!(texts(&nodes, run), ["a", "b"]);
    }

    #[test]
    fn reset_keeps_slot_zero_and_capacity() {
        let mut nodes = store();
        let mut scratch = ListBuf::with_capacity(8);
        let mark = scratch.mark();
        for _ in 0..8 {
            scratch.push(Node("x"));
        }
        scratch.flush_copy(mark, &mut nodes);
        let capacity = nodes.rows.capacity();

        nodes.reset();
        assert_eq!(
            nodes.rows,
            [Node("x")],
            "slot 0 stays, primed by the first write"
        );
        assert_eq!(nodes.rows.capacity(), capacity);
        assert_eq!(nodes.push(Node("y")).index(), 1);
        assert_eq!(
            nodes.rows,
            [Node("x"), Node("y")],
            "a reset store does not prime again"
        );

        let mut unwritten = store();
        unwritten.reset();
        assert!(
            unwritten.rows.is_empty(),
            "a reset of a store never written is a no-op"
        );
        assert_eq!(unwritten.rows.capacity(), 0);

        let scratch_capacity = scratch.items.capacity();
        scratch.reset();
        assert!(scratch.items.is_empty());
        assert_eq!(scratch.items.capacity(), scratch_capacity);
    }

    #[test]
    fn recycle_keeps_capacity_across_a_lifetime_change() {
        fn into_shorter<'a>(store: Store<Node<'static>>, _text: &'a str) -> Store<Node<'a>> {
            store.recycle()
        }

        let mut nodes = Store::with_capacity(32);
        nodes.push(Node("a"));
        let capacity = nodes.rows.capacity();
        let owned = String::from("short lived");
        let mut recycled = into_shorter(nodes, &owned);
        assert_eq!(recycled.rows.capacity(), capacity);
        assert!(
            recycled.rows.is_empty(),
            "a recycled store is empty, slot 0 included"
        );
        assert_eq!(
            recycled.push(Node(&owned)).index(),
            1,
            "its next write primes again"
        );
        assert_eq!(recycled.rows, [Node("short lived"), Node("short lived")]);
        assert_eq!(recycled.rows.capacity(), capacity);

        let mut scratch: ListBuf<Node<'static>> = ListBuf::with_capacity(32);
        let mark = scratch.mark();
        scratch.push(Node("x"));
        scratch.truncate(mark);
        let capacity = scratch.items.capacity();
        let recycled: ListBuf<Node<'_>> = scratch.recycle();
        assert_eq!(recycled.items.capacity(), capacity);
        assert!(recycled.items.is_empty());
    }

    #[test]
    fn minting_reaches_the_last_index() {
        assert_eq!(mint_index(1).map(NonZeroU32::get), Some(1));
        assert_eq!(
            mint_index(SLOT_LIMIT - 1).map(NonZeroU32::get),
            Some(u32::MAX - 1)
        );
    }

    #[test]
    fn minting_past_the_last_index_is_refused() {
        assert_eq!(mint_index(SLOT_LIMIT), None);
    }

    #[test]
    fn a_wrapped_length_is_refused_instead_of_wrapping_the_id() {
        assert_eq!(mint_index(u32::MAX as usize + 1), None);
        assert_eq!(mint_index(u32::MAX as usize + 2), None);
    }

    #[test]
    fn minting_at_slot_zero_is_refused_for_the_priming_arm() {
        assert_eq!(mint_index(0), None);
        assert_eq!(mint_range(0, 2), None);
    }

    #[test]
    fn a_run_may_end_at_the_top_of_the_index_space() {
        let range = mint_range(SLOT_LIMIT - 3, 3).map(|(start, len)| (start.get(), len));
        assert_eq!(range, Some((u32::MAX - 3, 3)));
    }

    #[test]
    fn a_run_past_the_top_of_the_index_space_is_refused() {
        assert_eq!(mint_range(SLOT_LIMIT - 3, 4), None);
    }

    #[test]
    fn a_run_whose_end_overflows_usize_is_refused() {
        assert_eq!(mint_range(usize::MAX, 2), None);
    }

    // The refused arms on a written store are the overflow: a mint refuses a written store only
    // past the last index, which a test cannot allocate, so these call the arms directly.
    #[test]
    fn a_mark_group_admits_heights_up_to_the_last_u32() {
        MarkGroup { heights: 0 }.check();
        MarkGroup {
            heights: u32::MAX as usize,
        }
        .check();
    }

    // one height past the last `u32` among small ones, which a test cannot allocate, so the
    // group is built with the OR its marks would fold
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_mark_group_refuses_one_height_past_the_last_u32() {
        MarkGroup {
            heights: 5 | 1 << 32 | 2,
        }
        .check();
    }

    #[test]
    fn a_group_mark_is_the_checked_mark() {
        let mut scratch: ListBuf<Node<'_>> = ListBuf::new();
        scratch.push(Node("a"));
        let mut deeper: ListBuf<Node<'_>> = ListBuf::new();
        deeper.push(Node("b"));
        deeper.push(Node("c"));
        let mut group = MarkGroup::new();
        let (shallow, deep) = (scratch.mark_in(&mut group), deeper.mark_in(&mut group));
        assert_eq!(group.heights, 1 | 2);
        group.check();
        assert_eq!(shallow.index(), scratch.mark().index());
        assert_eq!(deep.index(), 2);
        assert_eq!(deeper.len_since(deep), 0);
    }

    #[test]
    fn a_group_mark_keeps_a_height_past_sixteen_bits() {
        let mut scratch: ListBuf<Node<'_>> = ListBuf::new();
        for _ in 0..=u16::MAX {
            scratch.push(Node("a"));
        }
        let mut group = MarkGroup::new();
        let mark = scratch.mark_in(&mut group);
        group.check();
        assert_eq!(mark.index(), 1 << 16);
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_refused_push_on_a_written_store_overflows() {
        let mut nodes = store();
        nodes.push(Node("a"));
        nodes.prime_after_first_push();
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_refused_flush_on_a_written_store_overflows() {
        let mut nodes = store();
        nodes.push(Node("a"));
        nodes.prime_range(Some(&Node("b")), 1);
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_refused_flush_that_cannot_fit_after_priming_overflows() {
        store().prime_range(Some(&Node("a")), SLOT_LIMIT);
    }

    #[test]
    #[cfg_attr(
        not(feature = "_internal_store_check"),
        should_panic(expected = "store miss")
    )]
    #[cfg_attr(
        feature = "_internal_store_check",
        should_panic(expected = "stale list mark")
    )]
    fn a_mark_above_the_top_after_an_outer_flush_is_refused() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));
        let inner = scratch.mark();
        scratch.push(Node("b"));
        scratch.flush_move(outer, &mut nodes);
        scratch.flush_move(inner, &mut nodes);
    }

    #[test]
    #[cfg_attr(
        not(feature = "_internal_store_check"),
        should_panic(expected = "store miss")
    )]
    #[cfg_attr(
        feature = "_internal_store_check",
        should_panic(expected = "stale list mark")
    )]
    fn a_truncate_to_a_mark_above_the_top_is_refused() {
        let mut scratch = ListBuf::new();
        let low = scratch.mark();
        scratch.push(Node("a"));
        let high = scratch.mark();
        scratch.truncate(low);
        scratch.truncate(high);
    }

    #[test]
    fn an_empty_frame_reads_as_an_empty_slice() {
        let mut scratch = ListBuf::new();
        let first = scratch.mark();
        assert_eq!(scratch.frame(first), &[]);
        scratch.push(Node("below"));
        let above = scratch.mark();
        assert_eq!(scratch.frame(above), &[]);
        assert_eq!(scratch.frame(first), [Node("below")]);
    }

    #[test]
    fn a_frame_reads_its_own_elements_and_every_inner_frame_open_above_it() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));
        let inner = scratch.mark();
        scratch.push(Node("b"));
        scratch.push(Node("c"));
        assert_eq!(scratch.frame(inner), [Node("b"), Node("c")]);
        assert_eq!(scratch.frame(outer), [Node("a"), Node("b"), Node("c")]);

        let inner_run = scratch.flush_copy(inner, &mut nodes);
        scratch.push(Node("d"));
        assert_eq!(scratch.frame(outer), [Node("a"), Node("d")]);
        assert_eq!(texts(&nodes, inner_run), ["b", "c"]);
    }

    #[test]
    fn a_frame_after_a_truncate_reads_what_the_rewind_kept() {
        let mut scratch = ListBuf::new();
        let frame = scratch.mark();
        scratch.push(Node("a"));
        let checkpoint = scratch.mark();
        scratch.push(Node("spec"));
        assert_eq!(scratch.frame(frame), [Node("a"), Node("spec")]);
        scratch.truncate(checkpoint);
        assert_eq!(scratch.frame(frame), [Node("a")]);
        // The checkpoint stays live, so its own frame reads too.
        assert_eq!(scratch.frame(checkpoint), &[]);
        scratch.push(Node("kept"));
        assert_eq!(scratch.frame(frame), [Node("a"), Node("kept")]);
    }

    #[test]
    #[cfg_attr(
        not(feature = "_internal_store_check"),
        should_panic(expected = "store miss")
    )]
    #[cfg_attr(
        feature = "_internal_store_check",
        should_panic(expected = "stale list mark")
    )]
    fn a_frame_read_at_a_mark_above_the_top_is_refused() {
        let mut scratch = ListBuf::new();
        let low = scratch.mark();
        scratch.push(Node("a"));
        let high = scratch.mark();
        scratch.truncate(low);
        let _ = scratch.frame(high);
    }

    #[test]
    fn a_checkpoint_can_be_rewound_to_more_than_once() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let frame = scratch.mark();
        scratch.push(Node("a"));
        let checkpoint = scratch.mark();
        scratch.push(Node("spec"));
        scratch.truncate(checkpoint);
        scratch.push(Node("spec again"));
        scratch.truncate(checkpoint);
        scratch.push(Node("kept"));
        let run = scratch.flush_move(frame, &mut nodes);
        assert_eq!(texts(&nodes, run), ["a", "kept"]);
    }

    #[test]
    fn an_abandoned_checkpoint_is_popped_by_the_next_outer_flush() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let outer = scratch.mark();
        scratch.push(Node("a"));
        // A speculation that succeeded: its checkpoint is never rewound to or flushed.
        let _abandoned = scratch.mark();
        scratch.push(Node("b"));
        let inner = scratch.mark();
        scratch.push(Node("c"));
        let inner_run = scratch.flush_copy(inner, &mut nodes);
        let outer_run = scratch.flush_move(outer, &mut nodes);
        assert_eq!(texts(&nodes, inner_run), ["c"]);
        assert_eq!(texts(&nodes, outer_run), ["a", "b"]);

        // The stack beneath stays usable.
        let next = scratch.mark();
        scratch.push(Node("d"));
        let next_run = scratch.flush_move(next, &mut nodes);
        assert_eq!(texts(&nodes, next_run), ["d"]);
    }

    #[test]
    fn an_id_survives_the_return_word() {
        for index in [1, 2, 0x8000_0000, u32::MAX - 1] {
            let id = id_at(index);
            let word = IdRet::wrap(id);
            let copied = word;
            assert_eq!(word.id().index(), id.index());
            assert_eq!(copied.id().index(), index as usize);
        }
    }

    #[test]
    fn a_view_reads_what_the_store_reads() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        scratch.push(Node("b"));
        let run = scratch.flush_copy(mark, &mut nodes);
        let id = nodes.push(Node("c"));

        let view = nodes.view();
        let same = view;
        assert_eq!(*same.get(id), Node("c"));
        assert_eq!(view.slice(run), [Node("a"), Node("b")]);
        assert_eq!(view.slice(Run::EMPTY), &[]);
    }

    #[test]
    fn an_absent_string_is_distinct_from_an_empty_one() {
        let mut pool = TextPool::new();
        let empty = pool.push("");
        let absent: Option<PoolText> = None;
        assert_ne!(Some(empty), absent);
        assert_eq!(pool.get(empty), "");
        assert_eq!(pool.text.len(), POOL_SENTINEL.len_utf8());
    }

    #[test]
    fn pool_text_preserves_multibyte_strings_at_their_boundaries() {
        let mut pool = TextPool::with_capacity(32);
        let accented = pool.push("é");
        let ascii = pool.push("abc");
        let cjk = pool.push("日本語");
        let emoji = pool.push("👍🏽");
        assert_eq!(pool.get(accented), "é");
        assert_eq!(pool.get(ascii), "abc");
        assert_eq!(pool.get(cjk), "日本語");
        assert_eq!(pool.get(emoji), "👍🏽");
    }

    #[test]
    #[should_panic(expected = "store miss")]
    fn a_span_off_a_character_boundary_misses_rather_than_slice_panics() {
        let mut pool = TextPool::new();
        let span = pool.push("é");
        let torn = PoolText { len: 1, ..span };
        pool.get(torn);
    }

    #[test]
    fn pool_reset_keeps_capacity_and_the_sentinel() {
        let mut pool = TextPool::with_capacity(16);
        pool.push("some text");
        let capacity = pool.text.capacity();
        pool.reset();
        assert_eq!(pool.text, "\0");
        assert_eq!(pool.text.capacity(), capacity);
        let span = pool.push("again");
        assert_eq!(span.start.get(), 1);
        assert_eq!(pool.get(span), "again");
    }

    #[cfg(not(feature = "_internal_store_check"))]
    #[test]
    #[should_panic(expected = "store miss")]
    fn an_id_past_the_rows_reaches_the_shared_miss() {
        let mut nodes = store();
        let id = nodes.push(Node("a"));
        nodes.reset();
        nodes.get(id);
    }

    #[cfg(not(feature = "_internal_store_check"))]
    #[test]
    #[should_panic(expected = "store miss")]
    fn a_run_past_the_rows_reaches_the_shared_miss() {
        let mut nodes = store();
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(Node("a"));
        let run = scratch.flush_copy(mark, &mut nodes);
        nodes.reset();
        nodes.slice(run);
    }

    #[cfg(feature = "_internal_store_check")]
    mod stamped {
        use super::*;

        fn two_row_store() -> (Store<Node<'static>>, Id<Alpha>) {
            let mut nodes = store();
            nodes.push(Node("a"));
            let id = nodes.push(Node("b"));
            (nodes, id)
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn an_id_retained_across_reset_is_rejected_in_bounds() {
            let (mut nodes, id) = two_row_store();
            nodes.reset();
            nodes.push(Node("c"));
            nodes.push(Node("d"));
            nodes.get(id);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn an_id_from_another_instance_is_rejected_in_bounds() {
            let (_nodes, id) = two_row_store();
            let (other, _) = two_row_store();
            other.get(id);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn an_id_retained_across_recycle_is_rejected() {
            let (nodes, id) = two_row_store();
            let mut recycled: Store<Node<'_>> = nodes.recycle();
            recycled.push(Node("c"));
            recycled.push(Node("d"));
            recycled.get(id);
        }

        fn run_of_two() -> (Store<Node<'static>>, Run<Alpha>) {
            let mut nodes = store();
            let mut scratch = ListBuf::new();
            let mark = scratch.mark();
            scratch.push(Node("a"));
            scratch.push(Node("b"));
            let run = scratch.flush_copy(mark, &mut nodes);
            (nodes, run)
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_run_retained_across_reset_is_rejected_in_bounds() {
            let (mut nodes, run) = run_of_two();
            nodes.reset();
            nodes.push(Node("c"));
            nodes.push(Node("d"));
            nodes.slice(run);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_run_from_another_instance_is_rejected_in_bounds() {
            let (_nodes, run) = run_of_two();
            let (other, _) = run_of_two();
            other.slice(run);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_view_rejects_an_id_retained_across_reset() {
            let (mut nodes, id) = two_row_store();
            nodes.reset();
            nodes.push(Node("c"));
            nodes.push(Node("d"));
            nodes.view().get(id);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_view_rejects_a_run_retained_across_reset() {
            let (mut nodes, run) = run_of_two();
            nodes.reset();
            nodes.push(Node("c"));
            nodes.push(Node("d"));
            nodes.view().slice(run);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn equality_across_a_reset_is_rejected() {
            let (mut nodes, stale) = two_row_store();
            nodes.reset();
            let fresh = nodes.push(Node("c"));
            let _ = stale == fresh;
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn equality_across_instances_is_rejected() {
            let (_nodes, a) = two_row_store();
            let (_other, b) = two_row_store();
            let _ = a == b;
        }

        #[test]
        fn equality_with_an_unminted_handle_is_no_error() {
            let (nodes, _) = two_row_store();
            let (_nodes, run) = run_of_two();
            assert_ne!(run, Run::EMPTY);
            assert_eq!(nodes.slice(Run::EMPTY), &[]);
            let _ = PoolText::EMPTY == PoolText::EMPTY;
        }

        #[test]
        #[should_panic(expected = "stale list mark")]
        fn a_double_flush_is_rejected_after_regrowth() {
            let mut nodes = store();
            let mut scratch = ListBuf::new();
            let inner = scratch.mark();
            scratch.push(Node("b"));
            scratch.flush_move(inner, &mut nodes);
            scratch.push(Node("c"));
            scratch.flush_move(inner, &mut nodes);
        }

        #[test]
        #[should_panic(expected = "stale list mark")]
        fn an_outer_flush_before_its_inner_is_rejected_after_regrowth() {
            let mut nodes = store();
            let mut scratch = ListBuf::new();
            let outer = scratch.mark();
            scratch.push(Node("a"));
            let inner = scratch.mark();
            scratch.push(Node("b"));
            scratch.flush_move(outer, &mut nodes);
            scratch.push(Node("p"));
            scratch.push(Node("q"));
            scratch.flush_move(inner, &mut nodes);
        }

        #[test]
        #[should_panic(expected = "stale list mark")]
        fn a_frame_read_at_a_flushed_mark_is_rejected_after_regrowth() {
            let mut nodes = store();
            let mut scratch = ListBuf::new();
            let inner = scratch.mark();
            scratch.push(Node("b"));
            scratch.flush_move(inner, &mut nodes);
            scratch.push(Node("c"));
            let _ = scratch.frame(inner);
        }

        #[test]
        #[should_panic(expected = "stale list mark")]
        fn a_mark_is_stale_after_its_buffer_resets() {
            let mut nodes = store();
            let mut scratch = ListBuf::new();
            scratch.push(Node("old"));
            let mark = scratch.mark();
            scratch.reset();
            scratch.push(Node("new1"));
            scratch.push(Node("new2"));
            scratch.flush_move(mark, &mut nodes);
        }

        #[test]
        fn an_empty_run_reads_empty_in_any_store() {
            let (other, _) = run_of_two();
            assert_eq!(other.slice(Run::EMPTY), &[]);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_span_retained_across_reset_is_rejected_in_bounds() {
            let mut pool = TextPool::new();
            let span = pool.push("abc");
            pool.reset();
            pool.push("xyz");
            pool.get(span);
        }

        #[test]
        #[should_panic(expected = "store stamp mismatch")]
        fn a_span_from_another_pool_is_rejected_in_bounds() {
            let mut pool = TextPool::new();
            let mut other = TextPool::new();
            let span = pool.push("abc");
            other.push("xyz");
            other.get(span);
        }

        #[test]
        fn a_present_empty_span_reads_empty_in_any_pool() {
            let mut pool = TextPool::new();
            let other = TextPool::new();
            let span = pool.push("");
            assert_eq!(other.get(span), "");
        }
    }
}
