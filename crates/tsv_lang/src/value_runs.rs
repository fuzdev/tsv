//! [`value_runs!`](crate::value_runs!), the declaration of a language crate's value-run stores,
//! and [`FieldShape`], what a test reads of one store, scratch stack or pool, with the
//! assertions the language crates' stores tests make over it.
//!
//! A value run is a list whose elements are built whole and stand nowhere else: a
//! [`Run`](crate::store::Run) over a [`Store`] of the element type, built on that type's
//! scratch stack (a [`ListBuf`]) and flushed into the store when the list closes. A language
//! crate keeps one store and one scratch stack per element type, and each pair takes the same
//! lines: a field of each, their place in the constructor, the reset, the scratch marks and
//! the clears, and the view accessor that reads one list. The macro writes those lines from
//! one entry per element type, so the crates that hold such lists declare them alike and none
//! copies the plumbing.

use crate::store::{ListBuf, Row, Store, TextPool};

/// Declares a language crate's value-run stores: a struct holding one [`Store`] and one
/// scratch stack ([`ListBuf`]) per element type, a struct of the scratch stacks' marks, and
/// the accessors that read a list through the crate's view.
///
/// ```ignore
/// tsv_lang::value_runs! {
///     /// The stores of the lists built by value.
///     pub(crate) struct RunStores;
///     /// The tops of their scratch stacks.
///     pub(crate) struct RunMarks;
///     pub view AstView::runs;
///
///     /// Declarators, the `a = 1` of `let a = 1`.
///     declarators, declarator_scratch: VariableDeclarator => declarators;
///     /// The spans of a selector's parts.
///     part_spans, part_span_scratch: Span as Row => part_spans;
///     /// Tokens no reader asks for.
///     tokens, token_scratch: Token;
/// }
/// ```
///
/// The header names what the caller chooses:
///
/// - the stores struct, with its visibility, which its fields and methods take too. The parser
///   reads the fields by path (`stores.runs.declarator_scratch.push(row)`), so a flush can
///   borrow a scratch stack and its store apart from every other field;
/// - the marks struct, with its visibility. Its fields are private: a mark is taken by the
///   stores' `marks` and spent by its `truncate`;
/// - the view the accessors are methods of, with their visibility, and the view's method that
///   returns the stores struct for the view's lifetime (`AstView::runs`, here
///   `fn runs(self) -> &'a RunStores`). The view is a type of the calling crate, named bare
///   (the accessors are an inherent `impl` of it), `Copy`, with one lifetime parameter and no
///   other;
/// - optionally, after the view, a `mint` clause,
///   `, mint AstStores.runs => sealed::ListStore, sealed::Seal`: the caller's stores type that
///   holds the generated struct and the field holding it, then a trait of the caller's and the
///   type of its seal argument. Per entry the macro writes
///   `impl ListStore for Row` with the trait's one method, `split(stores: &mut AstStores, _:
///   Seal) -> (&mut Store<Row>, &mut ListBuf<Row>)`, the entry's store and scratch stack
///   borrowed apart — so a write surface the caller seals behind that trait reaches every
///   entry, one added later included, without a line of its own. A caller with no such
///   surface leaves the clause out.
///
/// Each entry names a store field, its scratch stack's field and the element type; its doc
/// comment documents the store field. An element type has one entry: a second store of the
/// same type would mint runs the first one's accessor also takes. An attribute on an entry
/// goes on the store field alone, so an entry cannot be `cfg`-gated. Two parts of an entry
/// are optional:
///
/// - `as Row` after the type says the type states its family itself, so the macro writes no
///   [`Row`] impl for it: a type another crate owns (a span, an id, a pool handle), or one
///   that is a row of another store too. Without it the macro makes the type its own family
///   (`impl Row for T { type Family = Self; }`), so its lists are `Run<T>`;
/// - `=> name` declares the view accessor `name`, which reads one list as a slice. Without it
///   no reader reaches the store through the view.
///
/// The stores struct gets `new` (allocating nothing), `reset` (every store and scratch stack
/// rewound in place, capacity kept), `capacity_bytes` (the bytes every store's and scratch
/// stack's allocation holds — what parking the stores retains), `marks` and `truncate` (the scratch stacks' tops, taken
/// into a caller's `MarkGroup` and rewound to), `clear_scratch`, `holds_address` (whether an
/// address lies in the rows of one of the stores, the store-range guard's read), and, in a
/// build with debug assertions, `scratch_is_empty`. A test build of the caller also gets `shapes`, every
/// field's [`FieldShape`] in declaration order, and `FIELDS`, the store fields' names. The
/// macro places no bound on an element type beyond [`Row`]: which flush a list takes
/// (`ListBuf::flush_copy` or `flush_copy_short`) is the choice of the code that builds it.
///
/// A scratch stack with no store (a buffer framed and truncated, never flushed) is not a value
/// run and has no entry: it stays a field of the caller's own store struct, beside the
/// generated one, with its line in that struct's reset, marks and clears.
///
/// Every path the expansion names goes through `$crate`, so a caller imports nothing for it.
/// The compiler reports no unused-item lint on what another crate's macro expands to: a
/// generated method the caller no longer calls (the `truncate` its own rewind forwards to,
/// say), an entry nothing reads, or tests that leave `shapes` or `FIELDS` unread are not
/// flagged. The caller's tests stand in for the lint: a fill that leaves every field of
/// `shapes` written, checked against `FIELDS`, and a test through each of the caller's own
/// wrappers (its reset, its marks and rewind, its clears) that reads every field after it.
#[doc(hidden)]
#[macro_export]
macro_rules! value_runs {
    (
        $(#[$stores_meta:meta])*
        $vis:vis struct $Stores:ident;
        $(#[$marks_meta:meta])*
        $marks_vis:vis struct $Marks:ident;
        $acc_vis:vis view $View:ident :: $reach:ident
            $(, mint $Outer:ident . $outer_field:ident => $Mint:path, $Seal:ty)?;
        $(
            $(#[$m:meta])*
            $field:ident, $scratch:ident : $Row:ty $(as $stated:ident)? $(=> $acc:ident)?;
        )*
    ) => {
        $(
            $crate::__value_runs_family!($Row $(, $stated)?);
        )*

        $crate::__value_runs_mint! {
            $($Outer . $outer_field => $Mint, $Seal)?;
            $($field, $scratch : $Row;)*
        }

        $(#[$stores_meta])*
        $vis struct $Stores {
            $(
                $(#[$m])*
                $vis $field: $crate::store::Store<$Row>,
                #[doc = concat!("The scratch stack that builds the lists of `", stringify!($field), "`.")]
                $vis $scratch: $crate::store::ListBuf<$Row>,
            )*
        }

        $(#[$marks_meta])*
        #[derive(Clone, Copy)]
        $marks_vis struct $Marks {
            $($field: $crate::store::ListMark<<$Row as $crate::store::Row>::Family>,)*
        }

        impl $Stores {
            /// Empty stores, allocating nothing.
            $vis fn new() -> Self {
                Self {
                    $(
                        $field: $crate::store::Store::new(),
                        $scratch: $crate::store::ListBuf::new(),
                    )*
                }
            }

            /// The bytes every store's and scratch stack's allocation holds: what parking the
            /// stores retains.
            #[inline]
            $vis fn capacity_bytes(&self) -> usize {
                0 $(+ self.$field.capacity_bytes() + self.$scratch.capacity_bytes())*
            }

            /// Rewinds the stores and scratch stacks in place, keeping each written store's
            /// slot 0 and every allocation's capacity.
            #[inline]
            $vis fn reset(&mut self) {
                $(
                    self.$field.reset();
                    self.$scratch.reset();
                )*
            }

            /// Every store's and scratch stack's shape, for the tests that read what a reset
            /// leaves.
            #[cfg(test)]
            $vis fn shapes(&self, out: &mut ::std::vec::Vec<$crate::value_runs::FieldShape>) {
                $(
                    out.push($crate::value_runs::FieldShape::store(stringify!($field), &self.$field));
                    out.push($crate::value_runs::FieldShape::scratch(
                        stringify!($scratch),
                        &self.$scratch,
                    ));
                )*
            }

            /// The current tops of the scratch stacks, their heights left for `group` to check.
            #[inline]
            $vis fn marks(&self, group: &mut $crate::store::MarkGroup) -> $Marks {
                $Marks {
                    $($field: self.$scratch.mark_in(group),)*
                }
            }

            /// Truncates the scratch stacks to `marks`, discarding the frames opened since.
            #[inline]
            $vis fn truncate(&mut self, marks: &$Marks) {
                $(self.$scratch.truncate(marks.$field);)*
            }

            /// Empties the scratch stacks, discarding every open frame. The stores keep their
            /// rows.
            #[inline]
            $vis fn clear_scratch(&mut self) {
                $(self.$scratch.reset();)*
            }

            /// Whether `address` lies inside the rows of one of the stores, their scratch
            /// stacks excluded: the store-range guard's read
            /// ([`Store::holds_address`](crate::store::Store::holds_address)).
            #[inline]
            $vis fn holds_address(&self, address: usize) -> bool {
                false $(|| self.$field.holds_address(address))*
            }

            /// Whether every scratch stack is empty.
            #[cfg(debug_assertions)]
            $vis fn scratch_is_empty(&self) -> bool {
                true $(&& self.$scratch.is_empty())*
            }

            /// Every store's field name, for the test that walks them all.
            #[cfg(test)]
            $vis const FIELDS: &'static [&'static str] = &[$(stringify!($field)),*];
        }

        impl<'a> $View<'a> {
            $($(
                #[doc = concat!(
                    "The [`", stringify!($Row), "`] rows of `run`, one range check for the whole ",
                    "run. A miss ends in the store's shared cold path."
                )]
                #[inline]
                $acc_vis fn $acc(
                    self,
                    run: $crate::store::Run<<$Row as $crate::store::Row>::Family>,
                ) -> &'a [$Row] {
                    self.$reach().$field.slice(run)
                }
            )?)*
        }
    };
}

/// [`value_runs!`](crate::value_runs!)'s `mint` clause: per entry, an impl of the caller's
/// sealed list trait handing out the entry's store and scratch stack apart, reached through
/// the field of the caller's stores that holds the generated struct. Nothing without the clause.
#[doc(hidden)]
#[macro_export]
macro_rules! __value_runs_mint {
    (; $($field:ident, $scratch:ident : $Row:ty;)*) => {};
    (
        $Outer:ident . $outer_field:ident => $Mint:path, $Seal:ty;
        $($field:ident, $scratch:ident : $Row:ty;)*
    ) => {
        $(
            impl $Mint for $Row {
                #[inline]
                fn split(
                    stores: &mut $Outer,
                    _: $Seal,
                ) -> (&mut $crate::store::Store<$Row>, &mut $crate::store::ListBuf<$Row>) {
                    (&mut stores.$outer_field.$field, &mut stores.$outer_field.$scratch)
                }
            }
        )*
    };
}

/// [`value_runs!`](crate::value_runs!)'s statement of one element type's family: the type as
/// its own family, or nothing for a type an entry marks `as Row`, which states it elsewhere.
#[doc(hidden)]
#[macro_export]
macro_rules! __value_runs_family {
    ($Row:ty) => {
        impl $crate::store::Row for $Row {
            type Family = Self;
        }
    };
    ($Row:ty, Row) => {};
}

/// What a test reads of one store, scratch stack or pool: its length and its capacity.
///
/// A store set lists one per field (the `shapes` a [`value_runs!`](crate::value_runs!) struct
/// generates, and the caller's own lines for the fields it declares by hand), and its tests
/// compare the list across a reset, a clear or a park.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldShape {
    /// The field's name.
    pub name: &'static str,
    /// Which kind of buffer the field is.
    pub kind: FieldKind,
    /// The slots, elements or bytes the buffer holds.
    pub len: usize,
    /// The slots, elements or bytes the buffer's allocation holds.
    pub capacity: usize,
}

/// Which kind of buffer a [`FieldShape`] reads, which decides its rewound length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// A [`Store`].
    Store,
    /// A scratch stack, a [`ListBuf`].
    Scratch,
    /// A [`TextPool`].
    Pool,
}

impl FieldShape {
    /// The shape of `store`, the field `name`.
    pub fn store<T: Row>(name: &'static str, store: &Store<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Store,
            len: store.len(),
            capacity: store.capacity(),
        }
    }

    /// The shape of `scratch`, the field `name`.
    pub fn scratch<T>(name: &'static str, scratch: &ListBuf<T>) -> Self {
        Self {
            name,
            kind: FieldKind::Scratch,
            len: scratch.len(),
            capacity: scratch.capacity(),
        }
    }

    /// The shape of `pool`, the field `name`.
    pub fn pool<F>(name: &'static str, pool: &TextPool<F>) -> Self {
        Self {
            name,
            kind: FieldKind::Pool,
            len: pool.len(),
            capacity: pool.capacity(),
        }
    }

    /// The length a reset leaves: slot 0 of a store and the sentinel of a pool once written,
    /// nothing of a scratch stack.
    pub fn rewound_len(self) -> usize {
        match self.kind {
            FieldKind::Store | FieldKind::Pool => self.len.min(1),
            FieldKind::Scratch => 0,
        }
    }
}

/// Asserts `filled` wrote every field, so a field nothing fills cannot pass a rewind test by
/// standing empty: a store past slot 0, a scratch stack holding an element, the pool past its
/// sentinel — and that every name in `fields` (a [`value_runs!`](crate::value_runs!) struct's
/// `FIELDS`) has a shape among them.
///
/// A helper of the language crates' stores tests, here so each crate's reads the same.
/// `#[inline]`, so only a test build that calls it codegens it: no shipped build lays out its
/// code, or the formatting instances it reaches.
#[doc(hidden)]
#[inline]
pub fn assert_every_field_written(filled: &[FieldShape], fields: &[&str]) {
    let unwritten: Vec<_> = filled
        .iter()
        .filter(|shape| match shape.kind {
            FieldKind::Store | FieldKind::Pool => shape.len <= 1,
            FieldKind::Scratch => shape.len == 0,
        })
        .map(|shape| shape.name)
        .collect();
    assert!(
        unwritten.is_empty(),
        "the fill leaves these unwritten: {unwritten:?}"
    );
    for field in fields {
        assert!(
            filled.iter().any(|shape| shape.name == *field),
            "{field} has no shape"
        );
    }
}

/// Asserts `rewound` is `filled` after a reset, field by field: every field back at its
/// rewound length ([`FieldShape::rewound_len`]), slot 0 and the sentinel kept, and no capacity
/// shrunk or grown.
///
/// A helper of the language crates' stores tests, here so each crate's reads the same.
/// `#[inline]`, so only a test build that calls it codegens it: no shipped build lays out its
/// code, or the formatting instances it reaches.
#[doc(hidden)]
#[inline]
pub fn assert_rewound(filled: &[FieldShape], rewound: &[FieldShape]) {
    assert_eq!(filled.len(), rewound.len(), "the same fields");
    for (&before, &after) in filled.iter().zip(rewound) {
        assert_eq!(
            after,
            FieldShape {
                len: before.rewound_len(),
                ..before
            },
            "{} rewinds and keeps its capacity",
            before.name
        );
    }
}

/// Asserts `read` is refused as the store-check build refuses a handle from another
/// generation (a panic naming the stamp mismatch), and that an unstamped build reads it in
/// bounds instead.
///
/// A helper of the language crates' stores tests, here so each crate's reads the same.
#[doc(hidden)]
#[expect(
    clippy::expect_used,
    reason = "a test helper: a failed expectation is the assertion"
)]
pub fn assert_rejected_under_the_store_check(read: impl FnOnce()) {
    let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read));
    if crate::store::STORE_CHECK {
        let message = read.expect_err("a retained handle is rejected");
        let message = message
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| message.downcast_ref::<&str>().copied())
            .expect("a panic message");
        assert!(message.contains("store stamp mismatch"), "{message}");
    } else {
        assert!(read.is_ok(), "unstamped, a retained handle reads in bounds");
    }
}

#[cfg(test)]
mod tests {
    use super::{FieldKind, FieldShape, assert_every_field_written, assert_rewound};
    use crate::Span;
    use crate::store::{ListBuf, MarkGroup, PoolText, Row, Run, Store, TextPool};

    /// The family of the toy pool.
    enum Words {}

    /// A `Copy` element, flushed as a block.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Leaf(u32);

    /// An element holding a list of its own type and a string of the pool.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Branch {
        name: PoolText<Words>,
        children: Run<Branch>,
        leaves: Run<Leaf>,
    }

    /// An element no reader reaches through the view.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Token(u8);

    /// A toy document's stores: the generated struct beside a field declared by hand.
    struct ToyStores {
        lists: ToyLists,
        text: TextPool<Words>,
    }

    #[derive(Clone, Copy)]
    struct ToyView<'a> {
        stores: &'a ToyStores,
    }

    impl<'a> ToyView<'a> {
        fn lists(self) -> &'a ToyLists {
            &self.stores.lists
        }

        fn text(self, text: PoolText<Words>) -> &'a str {
            self.stores.text.get(text)
        }
    }

    crate::value_runs! {
        /// The toy stores: private, where a language crate's are crate-visible.
        struct ToyLists;
        /// The tops of the toy scratch stacks.
        struct ToyMarks;
        view ToyView::lists, mint ToyStores.lists => ToySplit, ToySeal;

        /// `Copy` rows, read through the view.
        leaves, leaf_scratch: Leaf => leaves;
        /// Rows each holding a list of its own type.
        branches, branch_scratch: Branch => branches;
        /// Rows with no accessor.
        tokens, token_scratch: Token;
        /// Rows whose type states its family itself.
        spans, span_scratch: Span as Row => spans;
        /// Rows that are handles of the pool, a generic type whose family is stated beside it.
        names, name_scratch: PoolText<Words> as Row => names;
    }

    /// The seal argument of [`ToySplit`], standing in for a language crate's unconstructible
    /// one.
    struct ToySeal;

    /// The toy's list trait, which the `mint` clause implements for every entry.
    trait ToySplit: Sized {
        fn split(stores: &mut ToyStores, seal: ToySeal) -> (&mut Store<Self>, &mut ListBuf<Self>);
    }

    /// `items` as one run of `T`'s store, through the store and scratch stack the `mint` clause
    /// hands out.
    fn push_list<T: ToySplit + Row + Copy>(stores: &mut ToyStores, items: &[T]) -> Run<T::Family> {
        let (store, scratch) = T::split(stores, ToySeal);
        let mark = scratch.mark();
        scratch.extend_from_slice(items);
        scratch.flush_copy(mark, store)
    }

    impl ToyStores {
        fn new() -> Self {
            Self {
                lists: ToyLists::new(),
                text: TextPool::new(),
            }
        }

        fn view(&self) -> ToyView<'_> {
            ToyView { stores: self }
        }

        fn shapes(&self) -> Vec<FieldShape> {
            let mut out = vec![FieldShape::pool("text", &self.text)];
            self.lists.shapes(&mut out);
            out
        }
    }

    fn span(start: u32) -> Span {
        Span {
            start,
            end: start + 1,
        }
    }

    fn branch(stores: &mut ToyStores, name: &str, children: Run<Branch>) -> Branch {
        Branch {
            name: stores.text.push(name),
            children,
            leaves: Run::EMPTY,
        }
    }

    fn names(stores: &ToyStores, run: Run<Branch>) -> Vec<&str> {
        let view = stores.view();
        view.branches(run)
            .iter()
            .map(|branch| view.text(branch.name))
            .collect()
    }

    /// Writes every store, leaves one element on every scratch stack, and writes the pool.
    fn fill(stores: &mut ToyStores) {
        let lists = &mut stores.lists;
        for round in 0..40 {
            let mark = lists.leaf_scratch.mark();
            lists.leaf_scratch.push(Leaf(round));
            lists.leaf_scratch.push(Leaf(round + 1));
            lists.leaf_scratch.flush_copy(mark, &mut lists.leaves);
            let mark = lists.token_scratch.mark();
            lists.token_scratch.push(Token(1));
            lists
                .token_scratch
                .flush_copy_short(mark, &mut lists.tokens);
            let mark = lists.span_scratch.mark();
            lists.span_scratch.push(span(round));
            lists.span_scratch.flush_copy_short(mark, &mut lists.spans);
        }
        let name = stores.text.push("a name");
        let mark = stores.lists.name_scratch.mark();
        stores.lists.name_scratch.push(name);
        stores
            .lists
            .name_scratch
            .flush_copy_short(mark, &mut stores.lists.names);
        stores.lists.name_scratch.push(name);
        let mark = stores.lists.branch_scratch.mark();
        let row = branch(stores, "filled", Run::EMPTY);
        stores.lists.branch_scratch.push(row);
        stores
            .lists
            .branch_scratch
            .flush_copy(mark, &mut stores.lists.branches);
        stores.lists.leaf_scratch.push(Leaf(0));
        stores.lists.branch_scratch.push(row);
        stores.lists.token_scratch.push(Token(0));
        stores.lists.span_scratch.push(span(0));
    }

    #[test]
    fn the_fields_are_declared_in_entry_order() {
        assert_eq!(
            ToyLists::FIELDS,
            ["leaves", "branches", "tokens", "spans", "names"]
        );
        let stores = ToyStores::new();
        let shapes = stores.shapes();
        let named: Vec<_> = shapes
            .iter()
            .map(|shape| (shape.name, shape.kind))
            .collect();
        assert_eq!(
            named,
            [
                ("text", FieldKind::Pool),
                ("leaves", FieldKind::Store),
                ("leaf_scratch", FieldKind::Scratch),
                ("branches", FieldKind::Store),
                ("branch_scratch", FieldKind::Scratch),
                ("tokens", FieldKind::Store),
                ("token_scratch", FieldKind::Scratch),
                ("spans", FieldKind::Store),
                ("span_scratch", FieldKind::Scratch),
                ("names", FieldKind::Store),
                ("name_scratch", FieldKind::Scratch),
            ]
        );
    }

    #[test]
    fn new_stores_allocate_nothing() {
        let stores = ToyStores::new();
        for shape in stores.shapes() {
            assert_eq!((shape.len, shape.capacity), (0, 0), "{}", shape.name);
        }
    }

    /// Lists of one element type nest on one scratch stack: an inner list opened while the
    /// outer frame holds elements flushes as its own contiguous run, and the outer frame
    /// flushes around it.
    #[test]
    fn nested_lists_of_one_type_interleave_and_flush_as_contiguous_runs() {
        let mut stores = ToyStores::new();
        let outer = stores.lists.branch_scratch.mark();
        let a = branch(&mut stores, "a", Run::EMPTY);
        stores.lists.branch_scratch.push(a);

        let inner = stores.lists.branch_scratch.mark();
        let b1 = branch(&mut stores, "b1", Run::EMPTY);
        stores.lists.branch_scratch.push(b1);
        let innermost = stores.lists.branch_scratch.mark();
        let c1 = branch(&mut stores, "c1", Run::EMPTY);
        stores.lists.branch_scratch.push(c1);
        let c2 = branch(&mut stores, "c2", Run::EMPTY);
        stores.lists.branch_scratch.push(c2);
        let grandchildren = stores
            .lists
            .branch_scratch
            .flush_copy(innermost, &mut stores.lists.branches);
        let b2 = branch(&mut stores, "b2", grandchildren);
        stores.lists.branch_scratch.push(b2);
        let children = stores
            .lists
            .branch_scratch
            .flush_copy(inner, &mut stores.lists.branches);

        let b = branch(&mut stores, "b", children);
        stores.lists.branch_scratch.push(b);
        let top = stores
            .lists
            .branch_scratch
            .flush_copy(outer, &mut stores.lists.branches);

        assert_eq!(names(&stores, top), ["a", "b"]);
        assert_eq!(names(&stores, children), ["b1", "b2"]);
        assert_eq!(names(&stores, grandchildren), ["c1", "c2"]);
        let view = stores.view();
        assert!(view.branches(view.branches(top)[0].children).is_empty());
        assert_eq!(format!("{grandchildren:?}"), "Run(1, 2)");
        assert_eq!(format!("{children:?}"), "Run(3, 2)");
        assert_eq!(format!("{top:?}"), "Run(5, 2)");
        assert_eq!(stores.lists.branch_scratch.len(), 0);
    }

    #[test]
    fn an_empty_list_leaves_its_store_unwritten() {
        let mut stores = ToyStores::new();
        let mark = stores.lists.leaf_scratch.mark();
        let run = stores
            .lists
            .leaf_scratch
            .flush_copy_short(mark, &mut stores.lists.leaves);
        assert_eq!(run, Run::EMPTY);
        assert!(stores.view().leaves(run).is_empty());
        assert_eq!(stores.lists.leaves.len(), 0, "nothing primed");
        assert_eq!(stores.lists.leaves.capacity(), 0, "nothing allocated");
    }

    #[test]
    fn a_run_of_one_reads_back_through_its_accessor() {
        let mut stores = ToyStores::new();
        let lists = &mut stores.lists;
        let mark = lists.leaf_scratch.mark();
        lists.leaf_scratch.push(Leaf(7));
        let leaf = lists.leaf_scratch.flush_copy_short(mark, &mut lists.leaves);
        let mark = lists.span_scratch.mark();
        lists.span_scratch.push(span(3));
        let spans = lists.span_scratch.flush_copy(mark, &mut lists.spans);
        let name = stores.text.push("one");
        let lists = &mut stores.lists;
        let mark = lists.name_scratch.mark();
        lists.name_scratch.push(name);
        let names = lists.name_scratch.flush_copy_short(mark, &mut lists.names);
        assert_eq!(leaf.len(), 1);
        let view = stores.view();
        assert_eq!(view.leaves(leaf), [Leaf(7)]);
        assert_eq!(view.spans(spans), [span(3)]);
        assert_eq!(view.text(view.names(names)[0]), "one");
        assert_eq!(stores.lists.leaves.len(), 2, "slot 0 and the one row");
    }

    /// The marks rewind every scratch stack to where it stood, more than once, and leave the
    /// frames opened below them whole.
    #[test]
    fn marks_and_truncate_rewind_every_scratch_stack() {
        let mut stores = ToyStores::new();
        let leaf_frame = stores.lists.leaf_scratch.mark();
        stores.lists.leaf_scratch.push(Leaf(1));
        let branch_frame = stores.lists.branch_scratch.mark();
        let kept = branch(&mut stores, "kept", Run::EMPTY);
        stores.lists.branch_scratch.push(kept);
        stores.lists.token_scratch.push(Token(1));
        stores.lists.span_scratch.push(span(1));
        stores.lists.name_scratch.push(PoolText::EMPTY);

        let mut group = MarkGroup::new();
        let marks: ToyMarks = stores.lists.marks(&mut group);
        group.check();
        for _ in 0..2 {
            stores.lists.leaf_scratch.push(Leaf(2));
            let dropped = branch(&mut stores, "dropped", Run::EMPTY);
            stores.lists.branch_scratch.push(dropped);
            stores.lists.token_scratch.push(Token(2));
            stores.lists.token_scratch.push(Token(3));
            stores.lists.span_scratch.push(span(2));
            stores.lists.name_scratch.push(PoolText::EMPTY);
            let copied = marks;
            stores.lists.truncate(&copied);
            let heights: Vec<_> = stores
                .shapes()
                .into_iter()
                .filter(|shape| shape.kind == FieldKind::Scratch)
                .map(|shape| shape.len)
                .collect();
            assert_eq!(heights, [1, 1, 1, 1, 1]);
        }

        let lists = &mut stores.lists;
        let leaves = lists.leaf_scratch.flush_copy(leaf_frame, &mut lists.leaves);
        let branches = lists
            .branch_scratch
            .flush_copy(branch_frame, &mut lists.branches);
        assert_eq!(stores.view().leaves(leaves), [Leaf(1)]);
        assert_eq!(names(&stores, branches), ["kept"]);
    }

    #[test]
    fn clear_scratch_empties_every_scratch_stack_and_nothing_else() {
        let mut stores = ToyStores::new();
        fill(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled, ToyLists::FIELDS);
        #[cfg(debug_assertions)]
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        #[cfg(debug_assertions)]
        assert!(stores.lists.scratch_is_empty());
        let expected: Vec<FieldShape> = filled
            .iter()
            .map(|&shape| match shape.kind {
                FieldKind::Scratch => FieldShape { len: 0, ..shape },
                FieldKind::Store | FieldKind::Pool => shape,
            })
            .collect();
        assert_eq!(stores.shapes(), expected);
    }

    /// One scratch stack left open is enough for the check to read the stores as not empty.
    #[test]
    #[cfg(debug_assertions)]
    fn scratch_is_empty_reads_every_scratch_stack() {
        let mut stores = ToyStores::new();
        assert!(stores.lists.scratch_is_empty());
        stores.lists.leaf_scratch.push(Leaf(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.span_scratch.push(span(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.token_scratch.push(Token(1));
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        stores.lists.name_scratch.push(PoolText::EMPTY);
        assert!(!stores.lists.scratch_is_empty());
        stores.lists.clear_scratch();
        let open = branch(&mut stores, "open", Run::EMPTY);
        stores.lists.branch_scratch.push(open);
        assert!(!stores.lists.scratch_is_empty());
    }

    #[test]
    fn reset_rewinds_every_field_and_keeps_capacity() {
        let mut stores = ToyStores::new();
        fill(&mut stores);
        let filled = stores.shapes();
        assert_every_field_written(&filled, ToyLists::FIELDS);
        stores.lists.reset();
        stores.text.reset();
        assert_rewound(&filled, &stores.shapes());

        // slot 0 survives, so the next list mints straight behind it
        let lists = &mut stores.lists;
        let mark = lists.leaf_scratch.mark();
        lists.leaf_scratch.push(Leaf(1));
        lists.leaf_scratch.push(Leaf(2));
        let run = lists.leaf_scratch.flush_copy(mark, &mut lists.leaves);
        assert_eq!(format!("{run:?}"), "Run(1, 2)");
        assert_eq!(stores.view().leaves(run), [Leaf(1), Leaf(2)]);
        assert_eq!(stores.shapes()[1].capacity, filled[1].capacity);
    }

    /// `capacity_bytes` reads every field's allocation: nothing for new stores, what a fill
    /// grew after it, and the same across a reset, which keeps capacity.
    #[test]
    fn capacity_bytes_reads_what_a_reset_keeps() {
        let mut stores = ToyStores::new();
        assert_eq!(stores.lists.capacity_bytes(), 0);
        fill(&mut stores);
        let filled = stores.lists.capacity_bytes();
        assert!(filled > 0);
        stores.lists.reset();
        assert_eq!(stores.lists.capacity_bytes(), filled);
    }

    /// The `mint` clause hands out each entry's own store and scratch stack: a list built
    /// through it reads back through the entry's accessor, and leaves the scratch empty.
    #[test]
    fn the_mint_clause_splits_each_entry_to_its_own_store() {
        let mut stores = ToyStores::new();
        let leaves = push_list(&mut stores, &[Leaf(7), Leaf(8)]);
        let spans = push_list(&mut stores, &[span(3)]);
        let tokens = push_list(&mut stores, &[Token(9), Token(10)]);
        let view = stores.view();
        assert_eq!(view.leaves(leaves), [Leaf(7), Leaf(8)]);
        assert_eq!(view.spans(spans), [span(3)]);
        assert_eq!(stores.lists.tokens.slice(tokens), [Token(9), Token(10)]);
        assert!(stores.lists.scratch_is_empty());
    }

    /// `holds_address` answers for the rows of every store: an element read in place lies in
    /// one, a copy of it on the stack does not, and nothing does before a store is written.
    #[test]
    fn holds_address_reads_every_store_and_no_copy() {
        let mut stores = ToyStores::new();
        let probe = Leaf(1);
        assert!(!stores.lists.holds_address((&raw const probe).addr()));
        let leaves = push_list(&mut stores, &[Leaf(1), Leaf(2)]);
        let tokens = push_list(&mut stores, &[Token(3)]);
        let view = stores.view();
        let leaf = &view.leaves(leaves)[1];
        assert!(stores.lists.holds_address((&raw const *leaf).addr()));
        let token = &stores.lists.tokens.slice(tokens)[0];
        assert!(stores.lists.holds_address((&raw const *token).addr()));
        let copy = *leaf;
        assert!(!stores.lists.holds_address((&raw const copy).addr()));
    }
}
