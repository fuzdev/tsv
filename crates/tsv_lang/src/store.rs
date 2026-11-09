//! Contiguous stores of parsed rows, the typed handles that index them, and the scratch stack
//! that builds their child lists.
//!
//! A [`Store`] is a `Vec` holding the rows of one family. Slot 0 of every store is a sentinel
//! row that no id names, so an [`Id`] carries its row index directly: `Option<Id>` takes its
//! niche from `NonZero`, and a read decodes nothing. Rows are immutable once pushed. The API
//! offers no way to edit a row, and no way to walk one: a store also holds rows the parse left
//! orphaned, so the tree is what is reachable from its root, never the store's contents.
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
//! rewound past or reset fails at the flush or truncate it reaches. It widens every handle and
//! changes layout and codegen, so a build carrying it never supplies a performance number. Off,
//! the stamps do not exist: no field, no comparison.
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

/// The id of a row about to be pushed at slot `len`.
///
/// The guard is the mint's only arithmetic, kept beside the store rather than inlined in
/// `push` so a test can reach the top of the index space without allocating it.
#[inline]
fn mint_index(len: usize) -> NonZeroU32 {
    match NonZeroU32::new(len as u32) {
        Some(index) if len < SLOT_LIMIT => index,
        _ => store_overflow(),
    }
}

/// The start and length of `count` rows appended behind `len` existing slots.
///
/// Every run a store holds has an end that fits a `u32`, which is what keeps a later
/// `start + len` from overflowing a wasm32 `usize`.
#[inline]
fn mint_range(len: usize, count: usize) -> (NonZeroU32, u32) {
    let fits = matches!(len.checked_add(count), Some(end) if end <= SLOT_LIMIT);
    match NonZeroU32::new(len as u32) {
        Some(start) if fits => (start, count as u32),
        _ => store_overflow(),
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
        /// answers before the check, and an id list's sentinel, which no read reaches.
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
    /// The value an id list keeps in its own slot 0. No mint reaches this index, so it equals
    /// no minted id and a read through it misses: only ever a sentinel, never an edge.
    pub const SENTINEL: Self = Self {
        index: NonZeroU32::MAX,
        #[cfg(feature = "_internal_store_check")]
        stamp: stamp::Stamp::NONE,
        family: PhantomData,
    };

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
    /// The empty run. Its range is readable in every store, because slot 0 always exists.
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

/// A contiguous store of rows, slot 0 a sentinel, keeping its capacity across [`Store::reset`].
pub struct Store<T> {
    rows: Vec<T>,
    #[cfg(feature = "_internal_store_check")]
    stamp: stamp::Stamp,
}

impl<T: Row> Store<T> {
    /// An empty store holding only `sentinel`.
    pub fn new(sentinel: T) -> Self {
        Self::with_capacity(sentinel, 0)
    }

    /// An empty store pre-sized for `rows` rows beyond its sentinel.
    pub fn with_capacity(sentinel: T, rows: usize) -> Self {
        let mut slots = Vec::with_capacity(rows + 1);
        slots.push(sentinel);
        Self {
            rows: slots,
            #[cfg(feature = "_internal_store_check")]
            stamp: stamp::Stamp::fresh(),
        }
    }

    /// Appends `row` and mints its id.
    #[inline]
    pub fn push(&mut self, row: T) -> Id<T::Family> {
        let index = mint_index(self.rows.len());
        self.rows.push(row);
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

    /// A read-only view of the rows for a walk: the bound slice, so a read after this costs
    /// one load fewer than through the store.
    #[inline]
    pub fn view(&self) -> StoreView<'_, T> {
        StoreView {
            rows: &self.rows,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp,
        }
    }

    /// Discards every row but the sentinel, keeping capacity. Every id and run minted so far is
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
    /// Takes a new sentinel because the old one is of the wrong lifetime. Every id and run
    /// minted so far is invalid from here on, as across a reset.
    pub fn recycle<U>(self, sentinel: U) -> Store<U>
    where
        U: Row<Family = T::Family>,
    {
        let mut rows: Vec<U> = recycle_vec(self.rows);
        rows.push(sentinel);
        Store {
            rows,
            #[cfg(feature = "_internal_store_check")]
            stamp: self.stamp.next_generation(),
        }
    }

    /// Mints the run that `count` rows about to be appended will occupy.
    #[inline]
    fn mint_run(&self, count: usize) -> Run<T::Family> {
        let (start, len) = mint_range(self.rows.len(), count);
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

/// The scratch stack that builds child lists: one buffer shared by every open frame of the same
/// element type, each frame flushing as one contiguous run.
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
    }

    /// Empties the stack between parses, keeping capacity.
    #[inline]
    pub fn reset(&mut self) {
        self.items.clear();
        #[cfg(feature = "_internal_store_check")]
        self.live.clear();
    }

    /// Reuses this buffer's capacity under another element type, as [`Store::recycle`] does.
    pub fn recycle<U>(self) -> ListBuf<U> {
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
        match u32::try_from(self.items.len()) {
            Ok(top) => ListMark {
                top,
                #[cfg(feature = "_internal_store_check")]
                seq: self.live.open(),
                family: PhantomData,
            },
            Err(_) => store_overflow(),
        }
    }

    /// The number of elements above `mark`. A mark above the top of the stack misses. A stale
    /// mark at or below the top is not detected here; the check build catches it at the
    /// flush or truncate it reaches.
    #[inline]
    pub fn len_since(&self, mark: ListMark<T::Family>) -> usize {
        match self.items.len().checked_sub(mark.index()) {
            Some(count) => count,
            None => store_miss(),
        }
    }

    /// Rewinds the stack to `mark`, discarding every element above it. `mark` stays usable, so
    /// a checkpoint can be rewound to more than once.
    #[inline]
    pub fn truncate(&mut self, mark: ListMark<T::Family>) {
        #[cfg(feature = "_internal_store_check")]
        self.live.pop_above(mark.seq);
        let _ = self.len_since(mark);
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
        let run = into.mint_run(count);
        match self.items.get(mark.index()..) {
            Some(frame) => into.rows.extend_from_slice(frame),
            None => store_miss(),
        }
        self.items.truncate(mark.index());
        run
    }

    /// Moves the frame above `mark` into `into` and truncates the stack back to `mark`. An
    /// empty frame returns [`Run::EMPTY`] and does not touch `into`.
    #[inline]
    pub fn flush_move(&mut self, mark: ListMark<T::Family>, into: &mut Store<T>) -> Run<T::Family> {
        #[cfg(feature = "_internal_store_check")]
        self.live.pop_through(mark.seq);
        let count = self.len_since(mark);
        if count == 0 {
            return Run::EMPTY;
        }
        let run = into.mint_run(count);
        into.rows.extend(self.items.drain(mark.index()..));
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
        let (start, len) = mint_range(self.text.len(), text.len());
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

    const SENTINEL: Node<'static> = Node("sentinel");

    fn store() -> Store<Node<'static>> {
        Store::new(SENTINEL)
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

    #[test]
    fn push_mints_row_indices_past_the_sentinel() {
        let mut nodes = store();
        let a = nodes.push(Node("a"));
        let b = nodes.push(Node("b"));
        assert_eq!((a.index(), b.index()), (1, 2));
        assert_eq!(*nodes.get(a), Node("a"));
        assert_eq!(*nodes.get(b), Node("b"));
    }

    #[test]
    fn the_empty_run_is_readable_because_slot_zero_exists() {
        let nodes = store();
        assert_eq!(Run::<Alpha>::EMPTY.start.get(), 1);
        assert_eq!(nodes.rows.len(), 1);
        assert_eq!(nodes.rows.get(Run::<Alpha>::EMPTY.range()), Some(&[][..]));
        assert_eq!(nodes.slice(Run::EMPTY), &[]);
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
        let mut ids: Store<Id<Alpha>> = Store::new(Id::SENTINEL);
        let mut scratch = ListBuf::new();
        let mark = scratch.mark();
        scratch.push(a);
        scratch.push(b);
        let run: Run<Id<Alpha>> = scratch.flush_copy(mark, &mut ids);
        assert_eq!(ids.slice(run), &[a, b]);

        // A list with holes is the same store shape over an `Option` of the id.
        let mut holes: Store<Option<Id<Alpha>>> = Store::new(None);
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
    fn reset_keeps_the_sentinel_and_capacity() {
        let mut nodes = store();
        let mut scratch = ListBuf::with_capacity(8);
        let mark = scratch.mark();
        for _ in 0..8 {
            scratch.push(Node("x"));
        }
        scratch.flush_copy(mark, &mut nodes);
        let capacity = nodes.rows.capacity();

        nodes.reset();
        assert_eq!(nodes.rows, [SENTINEL]);
        assert_eq!(nodes.rows.capacity(), capacity);
        assert_eq!(nodes.push(Node("y")).index(), 1);

        let scratch_capacity = scratch.items.capacity();
        scratch.reset();
        assert!(scratch.items.is_empty());
        assert_eq!(scratch.items.capacity(), scratch_capacity);
    }

    #[test]
    fn recycle_keeps_capacity_across_a_lifetime_change() {
        fn into_shorter<'a>(store: Store<Node<'static>>, text: &'a str) -> Store<Node<'a>> {
            store.recycle(Node(text))
        }

        let mut nodes = Store::with_capacity(SENTINEL, 32);
        nodes.push(Node("a"));
        let capacity = nodes.rows.capacity();
        let owned = String::from("short lived");
        let recycled = into_shorter(nodes, &owned);
        assert_eq!(recycled.rows.capacity(), capacity);
        assert_eq!(recycled.rows, [Node("short lived")]);

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
        assert_eq!(mint_index(1).get(), 1);
        assert_eq!(mint_index(SLOT_LIMIT - 1).get(), u32::MAX - 1);
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn minting_past_the_last_index_overflows() {
        mint_index(SLOT_LIMIT);
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_wrapped_length_overflows_instead_of_wrapping_the_id() {
        mint_index(u32::MAX as usize + 2);
    }

    #[test]
    fn a_run_may_end_at_the_top_of_the_index_space() {
        let (start, len) = mint_range(SLOT_LIMIT - 3, 3);
        assert_eq!((start.get(), len), (u32::MAX - 3, 3));
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_run_past_the_top_of_the_index_space_overflows() {
        mint_range(SLOT_LIMIT - 3, 4);
    }

    #[test]
    #[should_panic(expected = "store overflow")]
    fn a_run_whose_end_overflows_usize_is_refused() {
        mint_range(usize::MAX, 2);
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
    fn the_id_sentinel_is_past_every_mintable_index() {
        assert_eq!(Id::<Alpha>::SENTINEL.index(), u32::MAX as usize);
        assert_ne!(
            Id::<Alpha>::SENTINEL.index(),
            mint_index(SLOT_LIMIT - 1).get() as usize
        );
        let mut nodes = store();
        assert_ne!(nodes.push(Node("a")), Id::SENTINEL);
    }

    #[cfg(not(feature = "_internal_store_check"))]
    #[test]
    #[should_panic(expected = "store miss")]
    fn the_id_sentinel_reads_as_a_miss() {
        store().get(Id::SENTINEL);
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
            let mut recycled: Store<Node<'_>> = nodes.recycle(SENTINEL);
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
            let (nodes, id) = two_row_store();
            assert_ne!(id, Id::SENTINEL);
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
